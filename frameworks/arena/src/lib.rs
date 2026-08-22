//! Pools of values reached by index instead of by pointer.
//!
//! An arena owns a block of memory and hands out an [`Index`] for every
//! value put in it. The index is an id plus a generation, so an index to a
//! removed value reports as missing rather than naming whatever took its
//! place. That makes indices safe to store inside the values themselves,
//! which plain references cannot be.
//!
//! Two flavours:
//!
//! - [`Arena`] holds one type. It knows that type only as a [`TypeHash`], a
//!   [`Layout`] and a finalizer, so it can be built for a type that has no
//!   Rust name at the point of use.
//! - [`AnyArena`] holds one [`Arena`] per type and reaches values by
//!   [`AnyIndex`], which is an [`Index`] plus the type it belongs to.
//!
//! Reads and writes take `&self` and are checked at runtime by the
//! [`Lifetime`] of each item, so many readers or one writer can be handed
//! out of a shared arena. Everything that can move values around takes
//! `&mut self` instead.
//!
//! ```
//! use intuicio_framework_arena::Arena;
//!
//! let mut arena = Arena::new::<String>(8);
//! let index = arena.insert("Hello".to_owned()).unwrap();
//! assert_eq!(arena.read::<String>(index).unwrap().as_str(), "Hello");
//!
//! *arena.write::<String>(index).unwrap() = "World".to_owned();
//! assert_eq!(arena.read::<String>(index).unwrap().as_str(), "World");
//!
//! arena.remove(index).unwrap();
//! // the index went stale, it did not dangle
//! assert!(arena.read::<String>(index).is_err());
//! ```
use intuicio_data::{
    Finalize,
    lifetime::{Lifetime, ReadLock, ValueReadAccess, ValueWriteAccess},
    non_zero_alloc, non_zero_dealloc,
    type_hash::TypeHash,
};
use serde::{Deserialize, Serialize};
use std::{alloc::Layout, error::Error, marker::PhantomData};

/// What went wrong reaching a value.
#[derive(Debug, PartialEq, Eq)]
pub enum ArenaError {
    /// The type asked for is not the type the arena holds.
    InvalidAreaType { type_hash: TypeHash },
    /// No live value in that arena has that index.
    IndexNotFound { type_hash: TypeHash, index: Index },
    /// Something is writing to that item right now.
    CannotReadItem { type_hash: TypeHash, index: Index },
    /// Something is reading or writing that item right now.
    CannotWriteItem { type_hash: TypeHash, index: Index },
    /// [`AnyArena`] holds no arena for that type.
    ArenaNotFound { type_hash: TypeHash },
}

impl Error for ArenaError {}

impl std::fmt::Display for ArenaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAreaType { type_hash } => {
                write!(f, "Invalid area type: {type_hash:?}")
            }
            Self::IndexNotFound { type_hash, index } => {
                write!(f, "Index: {index} not found in arena: {type_hash:?}")
            }
            Self::CannotReadItem { type_hash, index } => {
                write!(
                    f,
                    "Cannot read item at index: {index} in arena: {type_hash:?}"
                )
            }
            Self::CannotWriteItem { type_hash, index } => {
                write!(
                    f,
                    "Cannot write item at index: {index} in arena: {type_hash:?}"
                )
            }
            Self::ArenaNotFound { type_hash } => {
                write!(f, "Arena not found: {type_hash:?}")
            }
        }
    }
}

/// A slot in an [`Arena`]: an id plus a generation.
///
/// The generation goes up every time a slot is reused, and both halves are
/// compared on lookup, so an index to a removed value never reaches the
/// value that replaced it. [`Index::to_u64`] packs the pair into one
/// integer for code that can only carry a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Index {
    id: u32,
    generation: u32,
}

impl Default for Index {
    fn default() -> Self {
        Self::INVALID
    }
}

impl Index {
    /// The index that names nothing. Also what [`Default`] gives.
    pub const INVALID: Self = unsafe { Self::new_unchecked(u32::MAX, 0) };

    /// An index for `id` and `generation`, or [`None`] when `id` is `u32::MAX`.
    pub const fn new(id: u32, generation: u32) -> Option<Self> {
        if id < u32::MAX {
            Some(Self { id, generation })
        } else {
            None
        }
    }

    /// Builds an index without checking `id`.
    ///
    /// # Safety
    ///
    /// `id` must be below `u32::MAX`. An `id` of `u32::MAX` makes the index
    /// equal to [`Index::INVALID`], which is how that constant is built.
    pub const unsafe fn new_unchecked(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }

    /// Whether this is anything but [`Index::INVALID`].
    pub const fn is_valid(self) -> bool {
        self.id < u32::MAX
    }

    /// The slot this index names.
    pub const fn id(self) -> u32 {
        self.id
    }

    /// How many times that slot has been reused.
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// Packs both halves into one integer, generation in the high bits.
    pub const fn to_u64(self) -> u64 {
        ((self.generation as u64) << 32) | self.id as u64
    }

    /// Unpacks what [`Index::to_u64`] made. Does not check that the result is
    /// valid.
    pub const fn from_u64(value: u64) -> Self {
        Self {
            generation: (value >> 32) as u32,
            id: value as u32,
        }
    }

    /// The same slot, one generation on. Wraps around past `u32::MAX`.
    pub const fn bump_generation(mut self) -> Self {
        self.generation = self.generation.wrapping_add(1);
        self
    }
}

impl std::fmt::Display for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid() {
            write!(f, "@{}:#{}", self.id, self.generation)
        } else {
            write!(f, "@none:#{}", self.generation)
        }
    }
}

/// An [`Index`] together with the type of the value it names, for use with
/// [`AnyArena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnyIndex {
    index: Index,
    type_hash: TypeHash,
}

impl Default for AnyIndex {
    fn default() -> Self {
        Self::INVALID
    }
}

impl AnyIndex {
    /// The index that names nothing. Also what [`Default`] gives.
    pub const INVALID: Self = Self::new(Index::INVALID, TypeHash::INVALID);

    /// Pairs `index` with `type_hash`.
    pub const fn new(index: Index, type_hash: TypeHash) -> Self {
        Self { index, type_hash }
    }

    /// Whether this index was made for values of type `T`.
    pub fn is<T>(self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    /// Whether the slot half is valid. Says nothing about the type.
    pub const fn is_valid(self) -> bool {
        self.index.is_valid()
    }

    /// The slot half.
    pub const fn index(self) -> Index {
        self.index
    }

    /// The type half.
    pub const fn type_hash(self) -> TypeHash {
        self.type_hash
    }
}

impl std::fmt::Display for AnyIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:^{}", self.index, self.type_hash)
    }
}

/// A pool of values of one type, reached by [`Index`].
///
/// Values are packed with no gaps: removing one moves the last one into the
/// hole, so an item's address changes but its index does not. Running out of
/// room doubles the capacity, which moves every item, so an arena hands out
/// indices rather than pointers.
///
/// The type is known only as a [`TypeHash`], a [`Layout`] and a finalizer,
/// which is what lets [`Arena::new_raw`] build one for a type that has no
/// Rust name here.
///
/// [`Arena::read`] and [`Arena::write`] take `&self` and are checked per item
/// at runtime, so many readers or one writer can come out of a shared arena.
/// Everything that moves values takes `&mut self`.
///
/// Looking an index up is a linear scan over the live ones, so this suits
/// pools of hundreds of values rather than millions.
pub struct Arena {
    type_hash: TypeHash,
    item_layout: Layout,
    finalizer: unsafe fn(*mut ()),
    memory: *mut u8,
    capacity: usize,
    layout: Layout,
    lifetime: Lifetime,
    indices_lifetimes: Vec<(Index, Lifetime)>,
    indices_to_reuse: Vec<Index>,
    index_generator: u32,
}

unsafe impl Send for Arena {}
unsafe impl Sync for Arena {}

impl Drop for Arena {
    fn drop(&mut self) {
        self.clear();
        self.lifetime.write_lock().using(|| unsafe {
            if self.memory.is_null() {
                return;
            }
            non_zero_dealloc(self.memory, self.layout);
            self.memory = std::ptr::null_mut();
        });
    }
}

impl Arena {
    /// An empty arena for values of type `T`.
    ///
    /// A `capacity` of `0` is raised to `1`.
    pub fn new<T: Finalize>(capacity: usize) -> Self {
        unsafe {
            Self::new_raw(
                TypeHash::of::<T>(),
                Layout::new::<T>(),
                T::finalize_raw,
                capacity,
            )
        }
    }

    /// An empty arena for a type described by hand.
    ///
    /// # Safety
    ///
    /// `item_layout` and `finalizer` must both describe the type named by
    /// `type_hash`. Every item is stored in `item_layout` bytes and dropped by
    /// calling `finalizer` on it, so a mismatch drops the wrong type or runs
    /// off the end of an item.
    pub unsafe fn new_raw(
        type_hash: TypeHash,
        mut item_layout: Layout,
        finalizer: unsafe fn(*mut ()),
        mut capacity: usize,
    ) -> Self {
        item_layout = item_layout.pad_to_align();
        capacity = capacity.max(1);
        let (memory, layout) = unsafe { Self::allocate_memory_unlocked(item_layout, capacity) };
        Self {
            type_hash,
            item_layout,
            finalizer,
            memory,
            capacity,
            layout,
            lifetime: Default::default(),
            indices_lifetimes: Vec::with_capacity(capacity),
            indices_to_reuse: Default::default(),
            index_generator: 0,
        }
    }

    /// The type this arena holds.
    pub fn type_hash(&self) -> TypeHash {
        self.type_hash
    }

    /// Layout of one item, padded to its own alignment.
    pub fn item_layout(&self) -> &Layout {
        &self.item_layout
    }

    /// The function that drops an item in place.
    pub fn finalizer(&self) -> unsafe fn(*mut ()) {
        self.finalizer
    }

    /// How many items fit before the next reallocation. Never zero.
    pub fn capacity(&self) -> usize {
        self.lifetime.read_lock().using(|| self.capacity)
    }

    /// How many live values the arena holds.
    pub fn len(&self) -> usize {
        self.lifetime
            .read_lock()
            .using(|| self.indices_lifetimes.len())
    }

    /// Whether the arena holds no values.
    pub fn is_empty(&self) -> bool {
        self.lifetime
            .read_lock()
            .using(|| self.indices_lifetimes.is_empty())
    }

    /// Whether `index` names a live value, generation included.
    pub fn contains(&self, index: Index) -> bool {
        self.lifetime
            .read_lock()
            .using(|| self.indices_lifetimes.iter().any(|(idx, _)| *idx == index))
    }

    /// Drops every value, keeping the memory block and its capacity.
    ///
    /// The indices freed this way are not queued for reuse, so later inserts get
    /// fresh ids instead.
    pub fn clear(&mut self) {
        self.lifetime.write_lock().using(|| {
            for index in 0..self.indices_lifetimes.len() {
                unsafe {
                    let target = self.memory.add(index * self.item_layout.size());
                    (self.finalizer)(target.cast());
                }
            }
            self.indices_lifetimes.clear();
        });
    }

    /// Adds `value` and returns its index.
    ///
    /// Fails with [`ArenaError::InvalidAreaType`] when `T` is not this arena's
    /// type. Doubles the capacity when the arena is full, which moves every
    /// item in memory.
    pub fn insert<T>(&mut self, value: T) -> Result<Index, ArenaError> {
        self.lifetime.write_lock().using(move || unsafe {
            let type_hash = TypeHash::of::<T>();
            if self.type_hash == type_hash {
                let (index, target) = self.allocate_unlocked();
                target.cast::<T>().write(value);
                Ok(index)
            } else {
                Err(ArenaError::InvalidAreaType { type_hash })
            }
        })
    }

    /// Reserves a slot and returns it with a pointer to its uninitialized
    /// storage.
    ///
    /// # Safety
    ///
    /// The caller has to write a valid value of the arena's type to that
    /// pointer before anyone reads the index, since reads and the finalizer
    /// will treat whatever is there as one. The pointer only stays good until
    /// the next call that inserts or removes.
    pub unsafe fn allocate(&mut self) -> (Index, *mut u8) {
        self.lifetime
            .write_lock()
            .using(|| unsafe { self.allocate_unlocked() })
    }

    /// Drops the value at `index` and frees its slot.
    ///
    /// The last item moves into the hole, and `index` is queued for reuse with
    /// its generation bumped.
    pub fn remove(&mut self, index: Index) -> Result<(), ArenaError> {
        self.lifetime.write_lock().using(|| {
            if self.indices_lifetimes.is_empty() {
                return Err(ArenaError::IndexNotFound {
                    type_hash: self.type_hash,
                    index,
                });
            }
            let Some(idx) = self
                .indices_lifetimes
                .iter()
                .position(|(idx, _)| *idx == index)
            else {
                return Err(ArenaError::IndexNotFound {
                    type_hash: self.type_hash,
                    index,
                });
            };
            self.indices_to_reuse.push(index);
            unsafe {
                let target = self.memory.add(idx * self.item_layout.size());
                (self.finalizer)(target.cast());
                self.indices_lifetimes.swap_remove(idx);
                if self.indices_lifetimes.len() != idx {
                    let source = self
                        .memory
                        .add(self.indices_lifetimes.len() * self.item_layout.size());
                    source.copy_to(target, self.item_layout.size());
                    self.indices_lifetimes[idx].1 = Default::default();
                }
            }
            Ok(())
        })
    }

    /// Borrows the value at `index` for reading.
    ///
    /// Fails when `T` is not the arena's type, when `index` names nothing, or
    /// when something is writing to that item.
    pub fn read<T>(&'_ self, index: Index) -> Result<ValueReadAccess<'_, T>, ArenaError> {
        self.lifetime.read_lock().using(|| unsafe {
            let type_hash = TypeHash::of::<T>();
            if self.type_hash != type_hash {
                return Err(ArenaError::InvalidAreaType { type_hash });
            }
            if let Some(idx) = self
                .indices_lifetimes
                .iter()
                .position(|(idx, _)| *idx == index)
            {
                let address = self
                    .memory
                    .cast_const()
                    .add(idx * self.item_layout.size())
                    .cast::<T>();
                self.indices_lifetimes[idx]
                    .1
                    .read_ptr(address)
                    .ok_or(ArenaError::CannotReadItem {
                        type_hash: self.type_hash,
                        index,
                    })
            } else {
                Err(ArenaError::IndexNotFound {
                    type_hash: self.type_hash,
                    index,
                })
            }
        })
    }

    /// Borrows the value at `index` for writing.
    ///
    /// Fails when `T` is not the arena's type, when `index` names nothing, or
    /// when anything else is reading or writing that item.
    pub fn write<T>(&'_ self, index: Index) -> Result<ValueWriteAccess<'_, T>, ArenaError> {
        self.lifetime.read_lock().using(|| unsafe {
            let type_hash = TypeHash::of::<T>();
            if self.type_hash != type_hash {
                return Err(ArenaError::InvalidAreaType { type_hash });
            }
            if let Some(idx) = self
                .indices_lifetimes
                .iter()
                .position(|(idx, _)| *idx == index)
            {
                let address = self.memory.add(idx * self.item_layout.size()).cast::<T>();
                self.indices_lifetimes[idx].1.write_ptr(address).ok_or(
                    ArenaError::CannotWriteItem {
                        type_hash: self.type_hash,
                        index,
                    },
                )
            } else {
                Err(ArenaError::IndexNotFound {
                    type_hash: self.type_hash,
                    index,
                })
            }
        })
    }

    /// The address of the value at `index`.
    ///
    /// # Safety
    ///
    /// Skips the per-item borrow check, so the caller has to make sure nothing
    /// writes to that item while the pointer is in use, and must not read past
    /// [`Arena::item_layout`]. The pointer dies at the next insert or remove.
    pub unsafe fn read_ptr(&self, index: Index) -> Result<*const u8, ArenaError> {
        self.lifetime.read_lock().using(|| unsafe {
            if let Some(idx) = self
                .indices_lifetimes
                .iter()
                .position(|(idx, _)| *idx == index)
            {
                Ok(self.memory.cast_const().add(idx * self.item_layout.size()))
            } else {
                Err(ArenaError::IndexNotFound {
                    type_hash: self.type_hash,
                    index,
                })
            }
        })
    }

    /// The address of the value at `index`, for writing.
    ///
    /// # Safety
    ///
    /// Skips the per-item borrow check, so the caller has to make sure nothing
    /// else touches that item while the pointer is in use, and must not write
    /// past [`Arena::item_layout`]. The pointer dies at the next insert or
    /// remove.
    pub unsafe fn write_ptr(&self, index: Index) -> Result<*mut u8, ArenaError> {
        self.lifetime.read_lock().using(|| unsafe {
            if let Some(idx) = self
                .indices_lifetimes
                .iter()
                .position(|(idx, _)| *idx == index)
            {
                Ok(self.memory.add(idx * self.item_layout.size()))
            } else {
                Err(ArenaError::IndexNotFound {
                    type_hash: self.type_hash,
                    index,
                })
            }
        })
    }

    /// Whether `index` names a live value of type `T`.
    ///
    /// Fails with [`ArenaError::InvalidAreaType`] when `T` is not this arena's
    /// type, rather than answering `false`.
    pub fn is<T>(&self, index: Index) -> Result<bool, ArenaError> {
        self.is_raw(index, TypeHash::of::<T>())
    }

    /// [`Arena::is`] with the type given as a hash.
    pub fn is_raw(&self, index: Index, type_hash: TypeHash) -> Result<bool, ArenaError> {
        self.lifetime.read_lock().using(|| {
            if self.type_hash == type_hash {
                Ok(self.indices_lifetimes.iter().any(|(idx, _)| *idx == index))
            } else {
                Err(ArenaError::InvalidAreaType { type_hash })
            }
        })
    }

    /// Every live index, in storage order.
    pub fn indices(&self) -> impl Iterator<Item = Index> + '_ {
        let _lock = self.lifetime.read_lock();
        ArenaLockedIter {
            inner: self.indices_lifetimes.iter().map(|(index, _)| *index),
            _lock,
        }
    }

    /// Reads every value, in storage order.
    ///
    /// Yields nothing at all when `T` is not this arena's type, and skips any
    /// item that something is writing to right now.
    pub fn iter<'a, T: 'a>(&'a self) -> impl Iterator<Item = ValueReadAccess<'a, T>> {
        let type_hash = TypeHash::of::<T>();
        (self.type_hash == type_hash)
            .then_some(())
            .into_iter()
            .flat_map(|_| {
                let _lock = self.lifetime.read_lock();
                ArenaLockedIter {
                    inner: ArenaIter {
                        arena: self,
                        index: 0,
                        _phantom: PhantomData,
                    },
                    _lock,
                }
            })
    }

    /// Writes to every value, in storage order.
    ///
    /// Yields nothing at all when `T` is not this arena's type, and skips any
    /// item that anything else is reading or writing right now.
    pub fn iter_mut<'a, T: 'a>(&'a self) -> impl Iterator<Item = ValueWriteAccess<'a, T>> {
        let type_hash = TypeHash::of::<T>();
        (self.type_hash == type_hash)
            .then_some(())
            .into_iter()
            .flat_map(|_| {
                let _lock = self.lifetime.read_lock();
                ArenaLockedIter {
                    inner: ArenaIterMut {
                        arena: self,
                        index: 0,
                        _phantom: PhantomData,
                    },
                    _lock,
                }
            })
    }

    /// Reserves a slot, growing the arena when it is full.
    ///
    /// # Safety
    ///
    /// The write lock of the arena has to be held already. Every pointer handed
    /// out earlier goes stale when this grows the arena.
    unsafe fn allocate_unlocked(&mut self) -> (Index, *mut u8) {
        if self.indices_lifetimes.len() == self.capacity {
            self.capacity *= 2;
            unsafe { self.reallocate_unlocked(self.indices_lifetimes.len(), self.capacity) };
        }
        let index = match self.indices_to_reuse.pop() {
            Some(index) => index.bump_generation(),
            None => {
                let id = self.index_generator;
                self.index_generator = self.index_generator.wrapping_add(1);
                unsafe { Index::new_unchecked(id, 0) }
            }
        };
        let idx = self.indices_lifetimes.len();
        self.indices_lifetimes.push((index, Default::default()));
        (index, unsafe {
            self.memory.add(idx * self.item_layout.size())
        })
    }

    /// Moves the first `size` items into a fresh block of `capacity` items.
    ///
    /// # Safety
    ///
    /// The write lock of the arena has to be held already, and `capacity` must
    /// be at least `size`. Every item lifetime is reset, because every item
    /// moves.
    unsafe fn reallocate_unlocked(&mut self, size: usize, capacity: usize) {
        let (memory, layout) =
            unsafe { Self::allocate_memory_unlocked(self.item_layout, capacity) };
        unsafe { self.memory.copy_to(memory, self.item_layout.size() * size) };
        unsafe { non_zero_dealloc(self.memory, self.layout) };
        self.memory = memory;
        self.layout = layout;
        for (_, lifetime) in &mut self.indices_lifetimes {
            *lifetime = Default::default();
        }
    }

    /// Allocates room for `capacity` items.
    ///
    /// A zero-sized item still gets one byte, so every value of such a type
    /// shares one address.
    ///
    /// # Safety
    ///
    /// The size of `item_layout` times `capacity` must not overflow. Free the
    /// result with `non_zero_dealloc` and the layout it returns.
    unsafe fn allocate_memory_unlocked(
        mut item_layout: Layout,
        capacity: usize,
    ) -> (*mut u8, Layout) {
        item_layout = item_layout.pad_to_align();
        let layout = if item_layout.size() == 0 {
            unsafe { Layout::from_size_align_unchecked(1, 1) }
        } else {
            unsafe {
                Layout::from_size_align_unchecked(
                    item_layout.size() * capacity,
                    item_layout.align(),
                )
            }
        };
        let memory = unsafe { non_zero_alloc(layout) };
        (memory, layout)
    }
}

/// One [`Arena`] per type, reached by [`AnyIndex`].
///
/// Arenas appear on demand and then stay, empty or not, so that generation
/// counters keep counting and stale indices keep reporting as missing.
/// [`AnyArena::remove_empty_arenas`] gives that memory back when the old
/// indices are gone too.
#[derive(Default)]
pub struct AnyArena {
    /// Capacity handed to each arena this one creates. `0` means one item.
    pub new_arena_capacity: usize,
    arenas: Vec<Arena>,
}

impl AnyArena {
    /// [`AnyArena::new_arena_capacity`], builder style.
    pub fn with_new_arena_capacity(mut self, capacity: usize) -> Self {
        self.new_arena_capacity = capacity;
        self
    }

    /// How many values there are across every arena.
    pub fn len(&self) -> usize {
        self.arenas.iter().map(|arena| arena.len()).sum()
    }

    /// Whether every arena is empty.
    pub fn is_empty(&self) -> bool {
        self.arenas.iter().all(|arena| arena.is_empty())
    }

    /// Whether `index` names a live value, type included.
    pub fn contains(&self, index: AnyIndex) -> bool {
        self.arenas
            .iter()
            .find(|arena| arena.type_hash == index.type_hash)
            .map(|arena| arena.contains(index.index))
            .unwrap_or_default()
    }

    /// Every arena, one per type.
    pub fn arenas(&self) -> &[Arena] {
        &self.arenas
    }

    /// [`AnyArena::arenas`], mutably.
    pub fn arenas_mut(&mut self) -> &mut [Arena] {
        &mut self.arenas
    }

    /// The arena holding values of type `T`, if there is one.
    pub fn arena<T>(&self) -> Option<&Arena> {
        unsafe { self.arena_raw(TypeHash::of::<T>()) }
    }

    /// [`AnyArena::arena`] with the type given as a hash.
    ///
    /// # Safety
    ///
    /// This call itself is safe. `type_hash` only picks an arena out of the
    /// list. The `unsafe` marker is here because the reason to use it is the
    /// raw methods of [`Arena`], which are not safe.
    pub unsafe fn arena_raw(&self, type_hash: TypeHash) -> Option<&Arena> {
        self.arenas
            .iter()
            .find(|arena| arena.type_hash == type_hash)
    }

    /// The arena for `T`, creating an empty one if there is none.
    pub fn ensure_arena<T: Finalize>(&mut self) -> &mut Arena {
        unsafe {
            self.ensure_arena_raw(
                TypeHash::of::<T>(),
                Layout::new::<T>().pad_to_align(),
                T::finalize_raw,
            )
        }
    }

    /// [`AnyArena::ensure_arena`] with the type described by hand.
    ///
    /// # Safety
    ///
    /// `item_layout` and `finalizer` must describe the type named by
    /// `type_hash`, on the same terms as [`Arena::new_raw`]. They are only used
    /// when the arena does not exist yet, so an arena already there keeps the
    /// layout and finalizer it was built with.
    pub unsafe fn ensure_arena_raw(
        &mut self,
        type_hash: TypeHash,
        item_layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> &mut Arena {
        let index = match self
            .arenas
            .iter()
            .position(|arena| arena.type_hash == type_hash)
        {
            Some(index) => index,
            None => {
                self.arenas.push(unsafe {
                    Arena::new_raw(type_hash, item_layout, finalizer, self.new_arena_capacity)
                });
                self.arenas.len() - 1
            }
        };
        &mut self.arenas[index]
    }

    /// Drops every value and every arena, generation counters included, so
    /// every [`AnyIndex`] handed out so far can start naming a later value.
    pub fn clear(&mut self) {
        for arena in &mut self.arenas {
            arena.clear();
        }
        self.arenas.clear();
    }

    /// Adds `value`, creating an arena for its type if there is none.
    pub fn insert<T: Finalize>(&mut self, value: T) -> AnyIndex {
        let type_hash = TypeHash::of::<T>();
        if let Some(arena) = self
            .arenas
            .iter_mut()
            .find(|arena| arena.type_hash == type_hash)
        {
            AnyIndex::new(arena.insert(value).unwrap(), type_hash)
        } else {
            let mut arena = Arena::new::<T>(self.new_arena_capacity);
            let index = arena.insert(value).unwrap();
            self.arenas.push(arena);
            AnyIndex::new(index, type_hash)
        }
    }

    /// Reserves a slot for `type_hash` and returns it with a pointer to its
    /// uninitialized storage, creating the arena if there is none.
    ///
    /// # Safety
    ///
    /// `item_layout` and `finalizer` must describe the type named by
    /// `type_hash`, and the caller has to write a valid value to the pointer
    /// before anyone reads the index, on the same terms as [`Arena::allocate`].
    pub unsafe fn allocate(
        &mut self,
        type_hash: TypeHash,
        item_layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> (AnyIndex, *mut u8) {
        if let Some(arena) = self
            .arenas
            .iter_mut()
            .find(|arena| arena.type_hash == type_hash)
        {
            let (index, address) = unsafe { arena.allocate() };
            (AnyIndex::new(index, type_hash), address)
        } else {
            let mut arena = unsafe {
                Arena::new_raw(type_hash, item_layout, finalizer, self.new_arena_capacity)
            };
            let (index, address) = unsafe { arena.allocate() };
            self.arenas.push(arena);
            (AnyIndex::new(index, type_hash), address)
        }
    }

    /// Drops the value at `index` and frees its slot.
    ///
    /// The arena stays even once it is empty, so its generation counter keeps
    /// counting and old indices keep reporting as missing. Use
    /// [`AnyArena::remove_empty_arenas`] to give that memory back.
    pub fn remove(&mut self, index: AnyIndex) -> Result<(), ArenaError> {
        if let Some(arena) = self
            .arenas
            .iter_mut()
            .find(|arena| arena.type_hash == index.type_hash)
        {
            arena.remove(index.index)
        } else {
            Err(ArenaError::ArenaNotFound {
                type_hash: index.type_hash,
            })
        }
    }

    /// Drops every arena that has no values left, freeing the memory they
    /// still hold on to.
    ///
    /// This throws away those types' generation counters, so an [`AnyIndex`]
    /// kept from before can start naming a later value. Only call it once the
    /// old indices are gone too.
    pub fn remove_empty_arenas(&mut self) {
        self.arenas.retain(|arena| !arena.is_empty());
    }

    /// Borrows the value at `index` for reading.
    ///
    /// Fails with [`ArenaError::ArenaNotFound`] when no value of that type was
    /// ever inserted.
    pub fn read<T>(&'_ self, index: AnyIndex) -> Result<ValueReadAccess<'_, T>, ArenaError> {
        if let Some(arena) = self
            .arenas
            .iter()
            .find(|arena| arena.type_hash == index.type_hash)
        {
            arena.read(index.index)
        } else {
            Err(ArenaError::ArenaNotFound {
                type_hash: index.type_hash,
            })
        }
    }

    /// Borrows the value at `index` for writing.
    ///
    /// Fails with [`ArenaError::ArenaNotFound`] when no value of that type was
    /// ever inserted.
    pub fn write<T>(&'_ self, index: AnyIndex) -> Result<ValueWriteAccess<'_, T>, ArenaError> {
        if let Some(arena) = self
            .arenas
            .iter()
            .find(|arena| arena.type_hash == index.type_hash)
        {
            arena.write(index.index)
        } else {
            Err(ArenaError::ArenaNotFound {
                type_hash: index.type_hash,
            })
        }
    }

    /// The address of the value at `index`.
    ///
    /// # Safety
    ///
    /// Same terms as [`Arena::read_ptr`].
    pub unsafe fn read_ptr(&self, index: AnyIndex) -> Result<*const u8, ArenaError> {
        if let Some(arena) = self
            .arenas
            .iter()
            .find(|arena| arena.type_hash == index.type_hash)
        {
            unsafe { arena.read_ptr(index.index) }
        } else {
            Err(ArenaError::ArenaNotFound {
                type_hash: index.type_hash,
            })
        }
    }

    /// The address of the value at `index`, for writing.
    ///
    /// # Safety
    ///
    /// Same terms as [`Arena::write_ptr`].
    pub unsafe fn write_ptr(&self, index: AnyIndex) -> Result<*mut u8, ArenaError> {
        if let Some(arena) = self
            .arenas
            .iter()
            .find(|arena| arena.type_hash == index.type_hash)
        {
            unsafe { arena.write_ptr(index.index) }
        } else {
            Err(ArenaError::ArenaNotFound {
                type_hash: index.type_hash,
            })
        }
    }

    /// Whether `index` names a live value of type `T`.
    pub fn is<T>(&self, index: AnyIndex) -> Result<bool, ArenaError> {
        self.is_raw(index, TypeHash::of::<T>())
    }

    /// [`AnyArena::is`] with the type given as a hash.
    ///
    /// An index made for another type answers `false`, since slot ids start
    /// again in every arena and would otherwise collide.
    pub fn is_raw(&self, index: AnyIndex, type_hash: TypeHash) -> Result<bool, ArenaError> {
        if index.type_hash != type_hash {
            return Ok(false);
        }
        for arena in &self.arenas {
            if arena.type_hash == type_hash {
                return Ok(arena.contains(index.index));
            }
        }
        Err(ArenaError::ArenaNotFound { type_hash })
    }

    /// Every live index, across every arena.
    pub fn indices(&self) -> impl Iterator<Item = AnyIndex> + '_ {
        self.arenas.iter().flat_map(|arena| {
            arena
                .indices()
                .map(move |index| AnyIndex::new(index, arena.type_hash))
        })
    }

    /// Reads every value of type `T`. Empty when no arena holds `T`.
    ///
    /// Skips any item that something is writing to right now.
    pub fn iter<'a, T: 'a>(&'a self) -> impl Iterator<Item = ValueReadAccess<'a, T>> {
        self.arenas.iter().flat_map(|arena| arena.iter::<T>())
    }

    /// Writes to every value of type `T`. Empty when no arena holds `T`.
    ///
    /// Skips any item that anything else is reading or writing right now.
    pub fn iter_mut<'a, T: 'a>(&'a self) -> impl Iterator<Item = ValueWriteAccess<'a, T>> {
        self.arenas.iter().flat_map(|arena| arena.iter_mut::<T>())
    }
}

/// An iterator that keeps the arena marked as read-borrowed while it runs.
pub struct ArenaLockedIter<T, I: Iterator<Item = T>> {
    inner: I,
    _lock: ReadLock,
}

impl<T, I: Iterator<Item = T>> Iterator for ArenaLockedIter<T, I> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Reads items in storage order, which stops being insertion order after a
/// removal.
///
/// Items that cannot be read right now are skipped, so `size_hint` can only
/// promise an upper bound.
pub struct ArenaIter<'a, T> {
    index: usize,
    arena: &'a Arena,
    _phantom: PhantomData<fn() -> T>,
}

impl<'a, T: 'a> Iterator for ArenaIter<'a, T> {
    type Item = ValueReadAccess<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.arena.indices_lifetimes.len() {
            let result = unsafe {
                let address = self.arena.memory.cast::<T>().add(self.index);
                self.arena.indices_lifetimes[self.index]
                    .1
                    .read_ptr::<T>(address)
            };
            self.index += 1;
            if result.is_some() {
                return result;
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.arena.indices_lifetimes.len() - self.index))
    }
}

/// Writes to items in storage order, which stops being insertion order
/// after a removal.
///
/// Items that cannot be written right now are skipped, so `size_hint` can only
/// promise an upper bound.
pub struct ArenaIterMut<'a, T> {
    index: usize,
    arena: &'a Arena,
    _phantom: PhantomData<fn() -> T>,
}

impl<'a, T: 'a> Iterator for ArenaIterMut<'a, T> {
    type Item = ValueWriteAccess<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.arena.indices_lifetimes.len() {
            let result = unsafe {
                let address = self.arena.memory.cast::<T>().add(self.index);
                self.arena.indices_lifetimes[self.index]
                    .1
                    .write_ptr::<T>(address)
            };
            self.index += 1;
            if result.is_some() {
                return result;
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.arena.indices_lifetimes.len() - self.index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async() {
        fn is_async<T: Send + Sync>() {}

        is_async::<Arena>();
        is_async::<AnyArena>();
    }

    #[test]
    fn test_arena() {
        let mut arena = Arena::new::<String>(0);
        assert_eq!(arena.type_hash(), TypeHash::of::<String>());
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);
        assert_eq!(arena.capacity(), 1);

        let hello = arena.insert("Hello".to_owned()).unwrap();
        assert!(!arena.is_empty());
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.capacity(), 1);
        assert!(arena.contains(hello));

        let world = arena.insert("World!".to_owned()).unwrap();
        assert!(!arena.is_empty());
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.capacity(), 2);
        assert!(arena.contains(world));

        assert_eq!(arena.read::<String>(hello).unwrap().as_str(), "Hello");
        assert_eq!(arena.read::<String>(world).unwrap().as_str(), "World!");

        *arena.write(world).unwrap() = "world".to_owned();
        assert_eq!(arena.read::<String>(world).unwrap().as_str(), "world");

        assert_eq!(
            arena
                .iter::<String>()
                .map(|item| item.to_owned())
                .collect::<Vec<_>>(),
            vec!["Hello".to_owned(), "world".to_owned()]
        );

        arena.remove(hello).unwrap();
        assert!(!arena.is_empty());
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.capacity(), 2);
        assert!(!arena.contains(hello));

        assert!(arena.read::<String>(hello).is_err());
        assert_eq!(arena.read::<String>(world).unwrap().as_str(), "world");

        arena.clear();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);
        assert_eq!(arena.capacity(), 2);
    }

    #[test]
    fn test_typed_arena() {
        let mut arena = AnyArena::default();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);

        let number = arena.insert(42usize);
        assert!(!arena.is_empty());
        assert_eq!(arena.len(), 1);
        assert!(arena.contains(number));

        let boolean = arena.insert(true);
        assert!(!arena.is_empty());
        assert_eq!(arena.len(), 2);
        assert!(arena.contains(boolean));

        assert_eq!(*arena.read::<usize>(number).unwrap(), 42);
        assert!(*arena.read::<bool>(boolean).unwrap());

        *arena.write(boolean).unwrap() = false;
        assert!(!*arena.read::<bool>(boolean).unwrap());

        arena.remove(number).unwrap();
        assert!(!arena.is_empty());
        assert_eq!(arena.len(), 1);
        assert!(!arena.contains(number));

        assert!(arena.read::<usize>(number).is_err());
        assert!(!*arena.read::<bool>(boolean).unwrap());

        arena.clear();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);
    }

    #[test]
    fn test_any_arena_is_respects_index_type() {
        let mut arena = AnyArena::default();
        let number = arena.insert(42usize);
        let boolean = arena.insert(true);
        // both arenas start their slot ids at 0, so only the type tells them
        // apart
        assert_eq!(number.index(), boolean.index());

        assert!(arena.is::<usize>(number).unwrap());
        assert!(arena.is::<bool>(boolean).unwrap());
        assert!(!arena.is::<bool>(number).unwrap());
        assert!(!arena.is::<usize>(boolean).unwrap());
    }

    #[test]
    fn test_any_arena_keeps_generation_after_last_remove() {
        let mut arena = AnyArena::default();
        let old = arena.insert(42usize);
        arena.remove(old).unwrap();
        assert!(arena.is_empty());
        assert!(!arena.contains(old));

        let new = arena.insert(7usize);
        assert_ne!(old, new);
        assert!(!arena.contains(old));
        assert!(arena.read::<usize>(old).is_err());
        assert_eq!(*arena.read::<usize>(new).unwrap(), 7);

        arena.remove(new).unwrap();
        assert_eq!(arena.arenas().len(), 1);
        arena.remove_empty_arenas();
        assert!(arena.arenas().is_empty());
    }

    #[test]
    fn test_arena_iter_skips_borrowed_items() {
        let mut arena = Arena::new::<String>(8);
        let a = arena.insert("a".to_owned()).unwrap();
        arena.insert("b".to_owned()).unwrap();
        let c = arena.insert("c".to_owned()).unwrap();

        // a writer on the first item hides that item, not the ones after it
        let held = arena.write::<String>(a).unwrap();
        assert_eq!(
            arena
                .iter::<String>()
                .map(|item| item.to_owned())
                .collect::<Vec<_>>(),
            vec!["b".to_owned(), "c".to_owned()]
        );
        assert_eq!(arena.iter_mut::<String>().count(), 2);
        drop(held);

        // a reader hides its item from writers, but not from other readers
        let held = arena.read::<String>(c).unwrap();
        assert_eq!(arena.iter::<String>().count(), 3);
        assert_eq!(arena.iter_mut::<String>().count(), 2);
        drop(held);

        assert_eq!(arena.iter::<String>().count(), 3);
        assert_eq!(arena.iter_mut::<String>().count(), 3);
    }
}

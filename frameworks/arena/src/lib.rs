use intuicio_data::{
    Finalize,
    lifetime::{Lifetime, ReadLock, ValueReadAccess, ValueWriteAccess},
    type_hash::TypeHash,
};
use serde::{Deserialize, Serialize};
use std::{
    alloc::{Layout, alloc, dealloc},
    error::Error,
    marker::PhantomData,
};

#[derive(Debug, PartialEq, Eq)]
pub enum ArenaError {
    InvalidAreaType { type_hash: TypeHash },
    IndexNotFound { type_hash: TypeHash, index: Index },
    CannotReadItem { type_hash: TypeHash, index: Index },
    CannotWriteItem { type_hash: TypeHash, index: Index },
    ArenaNotFound { type_hash: TypeHash },
}

impl Error for ArenaError {}

impl std::fmt::Display for ArenaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAreaType { type_hash } => {
                write!(f, "Invalid area type: {:?}", type_hash)
            }
            Self::IndexNotFound { type_hash, index } => {
                write!(f, "Index: {} not found in arena: {:?}", index, type_hash)
            }
            Self::CannotReadItem { type_hash, index } => {
                write!(
                    f,
                    "Cannot read item at index: {} in arena: {:?}",
                    index, type_hash
                )
            }
            Self::CannotWriteItem { type_hash, index } => {
                write!(
                    f,
                    "Cannot write item at index: {} in arena: {:?}",
                    index, type_hash
                )
            }
            Self::ArenaNotFound { type_hash } => {
                write!(f, "Arena not found: {:?}", type_hash)
            }
        }
    }
}

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
    pub const INVALID: Self = unsafe { Self::new_unchecked(u32::MAX, 0) };

    pub const fn new(id: u32, generation: u32) -> Option<Self> {
        if id < u32::MAX {
            Some(Self { id, generation })
        } else {
            None
        }
    }

    /// # Safety
    pub const unsafe fn new_unchecked(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }

    pub const fn is_valid(self) -> bool {
        self.id < u32::MAX
    }

    pub const fn id(self) -> u32 {
        self.id
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub const fn to_u64(self) -> u64 {
        ((self.generation as u64) << 32) | self.id as u64
    }

    pub const fn from_u64(value: u64) -> Self {
        Self {
            generation: (value >> 32) as u32,
            id: value as u32,
        }
    }

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
    pub const INVALID: Self = Self::new(Index::INVALID, TypeHash::INVALID);

    pub const fn new(index: Index, type_hash: TypeHash) -> Self {
        Self { index, type_hash }
    }

    pub fn is<T>(self) -> bool {
        self.type_hash == TypeHash::of::<T>()
    }

    pub const fn is_valid(self) -> bool {
        self.index.is_valid()
    }

    pub const fn index(self) -> Index {
        self.index
    }

    pub const fn type_hash(self) -> TypeHash {
        self.type_hash
    }
}

impl std::fmt::Display for AnyIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:^{}", self.index, self.type_hash)
    }
}

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
            dealloc(self.memory, self.layout);
        });
    }
}

impl Arena {
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

    /// # Safety
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

    pub fn type_hash(&self) -> TypeHash {
        self.type_hash
    }

    pub fn item_layout(&self) -> &Layout {
        &self.item_layout
    }

    pub fn finalizer(&self) -> unsafe fn(*mut ()) {
        self.finalizer
    }

    pub fn capacity(&self) -> usize {
        self.lifetime.read_lock().using(|| self.capacity)
    }

    pub fn len(&self) -> usize {
        self.lifetime
            .read_lock()
            .using(|| self.indices_lifetimes.len())
    }

    pub fn is_empty(&self) -> bool {
        self.lifetime
            .read_lock()
            .using(|| self.indices_lifetimes.is_empty())
    }

    pub fn contains(&self, index: Index) -> bool {
        self.lifetime
            .read_lock()
            .using(|| self.indices_lifetimes.iter().any(|(idx, _)| *idx == index))
    }

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

    /// # Safety
    pub unsafe fn allocate(&mut self) -> (Index, *mut u8) {
        self.lifetime
            .write_lock()
            .using(|| unsafe { self.allocate_unlocked() })
    }

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

    pub fn read<T>(&self, index: Index) -> Result<ValueReadAccess<T>, ArenaError> {
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

    pub fn write<T>(&self, index: Index) -> Result<ValueWriteAccess<T>, ArenaError> {
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

    /// # Safety
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

    /// # Safety
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

    pub fn is<T>(&self, index: Index) -> Result<bool, ArenaError> {
        self.is_raw(index, TypeHash::of::<T>())
    }

    pub fn is_raw(&self, index: Index, type_hash: TypeHash) -> Result<bool, ArenaError> {
        self.lifetime.read_lock().using(|| {
            if self.type_hash == type_hash {
                Ok(self.indices_lifetimes.iter().any(|(idx, _)| *idx == index))
            } else {
                Err(ArenaError::InvalidAreaType { type_hash })
            }
        })
    }

    pub fn indices(&self) -> impl Iterator<Item = Index> + '_ {
        let _lock = self.lifetime.read_lock();
        ArenaLockedIter {
            inner: self.indices_lifetimes.iter().map(|(index, _)| *index),
            _lock,
        }
    }

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

    /// # Safety
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

    unsafe fn reallocate_unlocked(&mut self, size: usize, capacity: usize) {
        let (memory, layout) =
            unsafe { Self::allocate_memory_unlocked(self.item_layout, capacity) };
        unsafe { self.memory.copy_to(memory, self.item_layout.size() * size) };
        unsafe { dealloc(self.memory, self.layout) };
        self.memory = memory;
        self.layout = layout;
        for (_, lifetime) in &mut self.indices_lifetimes {
            *lifetime = Default::default();
        }
    }

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
        let memory = unsafe { alloc(layout) };
        (memory, layout)
    }
}

#[derive(Default)]
pub struct AnyArena {
    pub new_arena_capacity: usize,
    arenas: Vec<Arena>,
}

impl AnyArena {
    pub fn with_new_arena_capacity(mut self, capacity: usize) -> Self {
        self.new_arena_capacity = capacity;
        self
    }

    pub fn len(&self) -> usize {
        self.arenas.iter().map(|arena| arena.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.arenas.iter().all(|arena| arena.is_empty())
    }

    pub fn contains(&self, index: AnyIndex) -> bool {
        self.arenas
            .iter()
            .find(|arena| arena.type_hash == index.type_hash)
            .map(|arena| arena.contains(index.index))
            .unwrap_or_default()
    }

    pub fn arenas(&self) -> &[Arena] {
        &self.arenas
    }

    pub fn arenas_mut(&mut self) -> &mut [Arena] {
        &mut self.arenas
    }

    pub fn arena<T>(&self) -> Option<&Arena> {
        unsafe { self.arena_raw(TypeHash::of::<T>()) }
    }

    /// # Safety
    pub unsafe fn arena_raw(&self, type_hash: TypeHash) -> Option<&Arena> {
        self.arenas
            .iter()
            .find(|arena| arena.type_hash == type_hash)
    }

    pub fn ensure_arena<T: Finalize>(&mut self) -> &mut Arena {
        unsafe {
            self.ensure_arena_raw(
                TypeHash::of::<T>(),
                Layout::new::<T>().pad_to_align(),
                T::finalize_raw,
            )
        }
    }

    /// # Safety
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

    pub fn clear(&mut self) {
        for arena in &mut self.arenas {
            arena.clear();
        }
        self.arenas.clear();
    }

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

    /// # Safety
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

    pub fn remove(&mut self, index: AnyIndex) -> Result<(), ArenaError> {
        if let Some(idx) = self
            .arenas
            .iter_mut()
            .position(|arena| arena.type_hash == index.type_hash)
        {
            let result = self.arenas[idx].remove(index.index);
            if self.arenas[idx].is_empty() {
                self.arenas.swap_remove(idx);
            }
            result
        } else {
            Err(ArenaError::ArenaNotFound {
                type_hash: index.type_hash,
            })
        }
    }

    pub fn read<T>(&self, index: AnyIndex) -> Result<ValueReadAccess<T>, ArenaError> {
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

    pub fn write<T>(&self, index: AnyIndex) -> Result<ValueWriteAccess<T>, ArenaError> {
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

    /// # Safety
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

    /// # Safety
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

    pub fn is<T>(&self, index: AnyIndex) -> Result<bool, ArenaError> {
        self.is_raw(index, TypeHash::of::<T>())
    }

    pub fn is_raw(&self, index: AnyIndex, type_hash: TypeHash) -> Result<bool, ArenaError> {
        for arena in &self.arenas {
            if arena.type_hash == type_hash {
                return Ok(arena.contains(index.index));
            }
        }
        Err(ArenaError::ArenaNotFound {
            type_hash: index.type_hash,
        })
    }

    pub fn indices(&self) -> impl Iterator<Item = AnyIndex> + '_ {
        self.arenas.iter().flat_map(|arena| {
            arena
                .indices()
                .map(move |index| AnyIndex::new(index, arena.type_hash))
        })
    }

    pub fn iter<'a, T: 'a>(&'a self) -> impl Iterator<Item = ValueReadAccess<'a, T>> {
        self.arenas.iter().flat_map(|arena| arena.iter::<T>())
    }

    pub fn iter_mut<'a, T: 'a>(&'a self) -> impl Iterator<Item = ValueWriteAccess<'a, T>> {
        self.arenas.iter().flat_map(|arena| arena.iter_mut::<T>())
    }
}

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

pub struct ArenaIter<'a, T> {
    index: usize,
    arena: &'a Arena,
    _phantom: PhantomData<fn() -> T>,
}

impl<'a, T: 'a> Iterator for ArenaIter<'a, T> {
    type Item = ValueReadAccess<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.arena.indices_lifetimes.len() {
            unsafe {
                let address = self.arena.memory.cast::<T>().add(self.index);
                let result = self.arena.indices_lifetimes[self.index]
                    .1
                    .read_ptr::<T>(address);
                self.index += 1;
                result
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.arena.indices_lifetimes.len() - self.index;
        (size, Some(size))
    }
}

pub struct ArenaIterMut<'a, T> {
    index: usize,
    arena: &'a Arena,
    _phantom: PhantomData<fn() -> T>,
}

impl<'a, T: 'a> Iterator for ArenaIterMut<'a, T> {
    type Item = ValueWriteAccess<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.arena.indices_lifetimes.len() {
            unsafe {
                let address = self.arena.memory.cast::<T>().add(self.index);
                let result = self.arena.indices_lifetimes[self.index]
                    .1
                    .write_ptr::<T>(address);
                self.index += 1;
                result
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.arena.indices_lifetimes.len() - self.index;
        (size, Some(size))
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
}

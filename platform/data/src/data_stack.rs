//! Type-erased stack that carries values between function calls.
//!
//! [`DataStack`] is the single place where Intuicio moves data. Native and
//! script functions both take their arguments off it and put their results
//! back on it, so neither side can tell the other apart.
//!
//! # Layout
//!
//! The stack is one flat byte buffer that grows upwards. Each pushed value is
//! stored as its bytes followed by its [`TypeHash`], so a pop can check the
//! type before reading anything back. The stack also remembers a drop function
//! per type it has seen, so it can destroy values it no longer knows the Rust
//! type of.
//!
//! Registers, the analogue of local variables, live on the same buffer. A
//! register is a slot with a fixed type that can be empty or full, and it is
//! addressed by index rather than by position. See [`DataStackRegisterAccess`].
//!
//! # Rules
//!
//! Data is only ever **moved**, never copied or cloned. A pop takes the value
//! off the stack. A move into a register empties the place the value came
//! from. A type that wants copies has to provide a function that pushes a
//! duplicate itself.
//!
//! ```
//! # use intuicio_data::data_stack::{DataStack, DataStackMode};
//! let mut stack = DataStack::new(1024, DataStackMode::Mixed);
//! stack.push(42_i32);
//! assert_eq!(stack.pop::<i32>().unwrap(), 42);
//! ```
use crate::{Finalize, pointer_alignment_padding, type_hash::TypeHash};
use smallvec::SmallVec;
use std::{
    alloc::Layout,
    collections::{HashMap, hash_map::Entry},
    ops::Range,
};

/// How to destroy a value of one type, remembered per type the stack saw.
#[derive(Debug, Copy, Clone)]
struct DataStackFinalizer {
    callback: unsafe fn(*mut ()),
    layout: Layout,
}

/// Header stored above a register slot.
///
/// `finalizer` doubles as the empty or full flag: [`None`] means the slot
/// holds no value. `padding` records how many bytes were skipped below the
/// slot to align it, so unwinding can give them back.
#[derive(Debug, Copy, Clone)]
struct DataStackRegisterTag {
    type_hash: TypeHash,
    layout: Layout,
    finalizer: Option<unsafe fn(*mut ())>,
    padding: u8,
}

/// Marker of a stack position, taken with [`DataStack::store`].
///
/// Passing it to [`DataStack::restore`] unwinds everything pushed after it,
/// running the drop function of every value on the way. Passing it to
/// [`DataStack::reverse`] flips the order of those items instead.
pub struct DataStackToken(usize);

impl DataStackToken {
    /// Builds a token pointing at an arbitrary position.
    ///
    /// # Safety
    ///
    /// `position` must be a real item boundary of the stack it will be used
    /// with. Restoring to a position inside a value leaves the stack reading
    /// garbage as type tags.
    pub unsafe fn new(position: usize) -> Self {
        Self(position)
    }
}

/// Handle to one register slot, taken with [`DataStack::access_register`].
///
/// A register has a fixed type chosen when it was pushed, and is either
/// empty or full. Reads and takes check the requested type against the
/// register type and return [`None`] on a mismatch.
pub struct DataStackRegisterAccess<'a> {
    stack: &'a mut DataStack,
    position: usize,
}

impl<'a> DataStackRegisterAccess<'a> {
    /// Returns the type this register slot was declared with.
    pub fn type_hash(&self) -> TypeHash {
        unsafe {
            self.stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned()
                .type_hash
        }
    }

    /// Returns the memory layout of the register slot.
    pub fn layout(&self) -> Layout {
        unsafe {
            self.stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned()
                .layout
        }
    }

    /// Returns type and layout together, reading the header only once.
    pub fn type_hash_layout(&self) -> (TypeHash, Layout) {
        unsafe {
            let tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned();
            (tag.type_hash, tag.layout)
        }
    }

    /// Returns `true` when the register currently holds a value.
    pub fn has_value(&self) -> bool {
        unsafe {
            self.stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned()
                .finalizer
                .is_some()
        }
    }

    /// Borrows the stored value, or returns [`None`] when the register is empty
    /// or holds another type.
    pub fn read<T: 'static>(&'a self) -> Option<&'a T> {
        unsafe {
            let tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned();
            if tag.type_hash == TypeHash::of::<T>() && tag.finalizer.is_some() {
                self.stack
                    .memory
                    .as_ptr()
                    .add(self.position - tag.layout.size())
                    .cast::<T>()
                    .as_ref()
            } else {
                None
            }
        }
    }

    /// Borrows the stored value mutably, or returns [`None`] when the register
    /// is empty or holds another type.
    pub fn write<T: 'static>(&'a mut self) -> Option<&'a mut T> {
        unsafe {
            let tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned();
            if tag.type_hash == TypeHash::of::<T>() && tag.finalizer.is_some() {
                self.stack
                    .memory
                    .as_mut_ptr()
                    .add(self.position - tag.layout.size())
                    .cast::<T>()
                    .as_mut()
            } else {
                None
            }
        }
    }

    /// Moves the value out and leaves the register empty.
    ///
    /// Returns [`None`] when the register is empty or holds another type.
    pub fn take<T: 'static>(&mut self) -> Option<T> {
        unsafe {
            let mut tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned();
            if tag.type_hash == TypeHash::of::<T>() && tag.finalizer.is_some() {
                tag.finalizer = None;
                self.stack
                    .memory
                    .as_mut_ptr()
                    .add(self.position)
                    .cast::<DataStackRegisterTag>()
                    .write_unaligned(tag);
                Some(
                    self.stack
                        .memory
                        .as_ptr()
                        .add(self.position - tag.layout.size())
                        .cast::<T>()
                        .read_unaligned(),
                )
            } else {
                None
            }
        }
    }

    /// Drops the stored value in place and leaves the register empty.
    ///
    /// Returns `false` when there was nothing to drop.
    pub fn free(&mut self) -> bool {
        unsafe {
            let mut tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned();
            if let Some(finalizer) = tag.finalizer {
                (finalizer)(
                    self.stack
                        .memory
                        .as_mut_ptr()
                        .add(self.position - tag.layout.size())
                        .cast::<()>(),
                );
                tag.finalizer = None;
                self.stack
                    .memory
                    .as_mut_ptr()
                    .add(self.position)
                    .cast::<DataStackRegisterTag>()
                    .write_unaligned(tag);
                true
            } else {
                false
            }
        }
    }

    /// Moves a value into the register, dropping whatever was there.
    ///
    /// Does nothing when `T` is not the type the register was declared with.
    pub fn set<T: Finalize + 'static>(&mut self, value: T) {
        unsafe {
            let mut tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned();
            if tag.type_hash == TypeHash::of::<T>() {
                if let Some(finalizer) = tag.finalizer {
                    (finalizer)(
                        self.stack
                            .memory
                            .as_mut_ptr()
                            .add(self.position - tag.layout.size())
                            .cast::<()>(),
                    );
                } else {
                    tag.finalizer = Some(T::finalize_raw);
                }
                self.stack
                    .memory
                    .as_mut_ptr()
                    .add(self.position - tag.layout.size())
                    .cast::<T>()
                    .write_unaligned(value);
                self.stack
                    .memory
                    .as_mut_ptr()
                    .add(self.position)
                    .cast::<DataStackRegisterTag>()
                    .write_unaligned(tag);
            }
        }
    }

    /// Moves this register value into `other`, leaving this one empty.
    ///
    /// Does nothing when the two registers differ in type or layout, or when
    /// they are the same slot.
    pub fn move_to(&mut self, other: &mut Self) {
        if self.position == other.position {
            return;
        }
        unsafe {
            let mut tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned();
            let other_tag = other
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned();
            if tag.type_hash == other_tag.type_hash && tag.layout == other_tag.layout {
                if let Some(finalizer) = other_tag.finalizer {
                    (finalizer)(
                        self.stack
                            .memory
                            .as_mut_ptr()
                            .add(other.position - other_tag.layout.size())
                            .cast::<()>(),
                    );
                }
                tag.finalizer = None;
                let source = self
                    .stack
                    .memory
                    .as_ptr()
                    .add(self.position - tag.layout.size());
                let target = self
                    .stack
                    .memory
                    .as_mut_ptr()
                    .add(other.position - other_tag.layout.size());
                target.copy_from(source, tag.layout.size());
                self.stack
                    .memory
                    .as_mut_ptr()
                    .add(self.position)
                    .cast::<DataStackRegisterTag>()
                    .write_unaligned(tag);
            }
        }
    }
}

/// What a [`DataStack`] is allowed to hold.
///
/// A context keeps its call stack and its registers in two separate stacks,
/// so each of them can be restricted to what it actually needs.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum DataStackMode {
    /// Pushed values only, no registers.
    Values,
    /// Registers only, no pushed values.
    Registers,
    #[default]
    /// Both, which is the default.
    Mixed,
}

impl DataStackMode {
    /// Returns `true` when pushing and popping values is allowed.
    pub fn allows_values(self) -> bool {
        matches!(self, Self::Values | Self::Mixed)
    }

    /// Returns `true` when registers are allowed.
    pub fn allows_registers(self) -> bool {
        matches!(self, Self::Registers | Self::Mixed)
    }
}

/// One item seen by [`DataStack::visit`], reported from the top down.
///
/// Mostly useful for debugging and for tools that want to show what is
/// currently on a stack.
pub enum DataStackVisitedItem<'a> {
    /// A pushed value.
    Value {
        type_hash: TypeHash,
        layout: Layout,
        data: &'a [u8],
        range: Range<usize>,
    },
    /// A register slot. `valid` tells whether it currently holds a value.
    Register {
        type_hash: TypeHash,
        layout: Layout,
        data: &'a [u8],
        range: Range<usize>,
        valid: bool,
    },
}

/// Type-erased stack of values and registers.
///
/// Capacity is fixed at construction and rounded up to a power of two. A push
/// that does not fit fails instead of growing the stack, so a running script
/// cannot make the host reallocate under it. Dropping the stack unwinds
/// everything still on it. See the [module docs](self) for the layout.
pub struct DataStack {
    memory: Vec<u8>,
    position: usize,
    mode: DataStackMode,
    finalizers: HashMap<TypeHash, DataStackFinalizer>,
    registers: Vec<usize>,
    drop: bool,
}

impl Drop for DataStack {
    fn drop(&mut self) {
        if self.drop {
            self.restore(DataStackToken(0));
        }
    }
}

impl DataStack {
    /// Allocates a stack of at least `capacity` bytes, rounded up to a power of
    /// two.
    pub fn new(mut capacity: usize, mode: DataStackMode) -> Self {
        capacity = capacity.next_power_of_two();
        Self {
            memory: vec![0; capacity],
            position: 0,
            mode,
            finalizers: Default::default(),
            registers: vec![],
            drop: true,
        }
    }

    /// Returns how many bytes are used.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Returns the total capacity in bytes.
    pub fn size(&self) -> usize {
        self.memory.len()
    }

    /// Returns how many bytes are still free.
    pub fn available(&self) -> usize {
        self.size().saturating_sub(self.position)
    }

    /// Returns the used part of the buffer, tags included.
    pub fn as_bytes(&self) -> &[u8] {
        &self.memory[0..self.position]
    }

    /// Walks the stack from the top down, calling `f` for every item.
    ///
    /// Stops early when `f` returns `false`, or when an item cannot be read.
    pub fn visit(&self, mut f: impl FnMut(DataStackVisitedItem) -> bool) {
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        let tag_layout = Layout::new::<DataStackRegisterTag>().pad_to_align();
        let mut position = self.position;
        while position > 0 {
            if position < type_layout.size() {
                return;
            }
            position -= type_layout.size();
            let type_hash = unsafe {
                self.memory
                    .as_ptr()
                    .add(position)
                    .cast::<TypeHash>()
                    .read_unaligned()
            };
            if type_hash == TypeHash::of::<DataStackRegisterTag>() {
                if position < tag_layout.size() {
                    return;
                }
                position -= tag_layout.size();
                let tag = unsafe {
                    self.memory
                        .as_ptr()
                        .add(position)
                        .cast::<DataStackRegisterTag>()
                        .read_unaligned()
                };
                if position < tag.layout.size() {
                    return;
                }
                position -= tag.layout.size();
                let range = position..(position + tag.layout.size());
                let status = f(DataStackVisitedItem::Register {
                    type_hash: tag.type_hash,
                    layout: tag.layout,
                    data: &self.memory[range.clone()],
                    range,
                    valid: tag.finalizer.is_some(),
                });
                if !status {
                    return;
                }
                position -= tag.padding as usize;
            } else if let Some(finalizer) = self.finalizers.get(&type_hash) {
                if position < finalizer.layout.size() {
                    return;
                }
                position -= finalizer.layout.size();
                let range = position..(position + finalizer.layout.size());
                let status = f(DataStackVisitedItem::Value {
                    type_hash,
                    layout: finalizer.layout,
                    data: &self.memory[range.clone()],
                    range,
                });
                if !status {
                    return;
                }
            }
        }
    }

    /// Moves a value onto the stack.
    ///
    /// Returns `false` without touching anything when the mode forbids values
    /// or the value does not fit.
    pub fn push<T: Finalize + Sized + 'static>(&mut self, value: T) -> bool {
        if !self.mode.allows_values() {
            return false;
        }
        let value_layout = Layout::new::<T>().pad_to_align();
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        if self.position + value_layout.size() + type_layout.size() > self.size() {
            return false;
        }
        let type_hash = TypeHash::of::<T>();
        self.finalizers
            .entry(type_hash)
            .or_insert(DataStackFinalizer {
                callback: T::finalize_raw,
                layout: value_layout,
            });
        unsafe {
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<T>()
                .write_unaligned(value);
            self.position += value_layout.size();
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<TypeHash>()
                .write_unaligned(type_hash);
            self.position += type_layout.size();
        }
        true
    }

    /// [`DataStack::push`] for a value whose type is only known at runtime.
    ///
    /// # Safety
    ///
    /// `data` must be a valid byte image of a value of the type named by
    /// `type_hash`, matching `layout`, and `finalizer` must be the drop
    /// function of that type. The bytes are moved, so the caller must not drop
    /// the source afterwards.
    pub unsafe fn push_raw(
        &mut self,
        layout: Layout,
        type_hash: TypeHash,
        finalizer: unsafe fn(*mut ()),
        data: &[u8],
    ) -> bool {
        if !self.mode.allows_values() {
            return false;
        }
        let value_layout = layout.pad_to_align();
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        if data.len() != value_layout.size()
            && self.position + value_layout.size() + type_layout.size() > self.size()
        {
            return false;
        }
        self.finalizers
            .entry(type_hash)
            .or_insert(DataStackFinalizer {
                callback: finalizer,
                layout: value_layout,
            });
        self.memory[self.position..(self.position + value_layout.size())].copy_from_slice(data);
        self.position += value_layout.size();
        unsafe {
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<TypeHash>()
                .write_unaligned(type_hash)
        };
        self.position += type_layout.size();
        true
    }

    /// Reserves an empty register for values of type `T` and returns its index.
    pub fn push_register<T: Finalize + 'static>(&mut self) -> Option<usize> {
        unsafe { self.push_register_raw(TypeHash::of::<T>(), Layout::new::<T>().pad_to_align()) }
    }

    /// Reserves a register for `T` and moves `value` into it, returning its
    /// index.
    pub fn push_register_value<T: Finalize + 'static>(&mut self, value: T) -> Option<usize> {
        let result = self.push_register::<T>()?;
        let mut access = self.access_register(result)?;
        access.set(value);
        Some(result)
    }

    /// [`DataStack::push_register`] for a type only known at runtime.
    ///
    /// # Safety
    ///
    /// `value_layout` must be the real layout of the type named by
    /// `type_hash`. A wrong layout makes every later access to that register
    /// read or write out of bounds.
    pub unsafe fn push_register_raw(
        &mut self,
        type_hash: TypeHash,
        value_layout: Layout,
    ) -> Option<usize> {
        if !self.mode.allows_registers() {
            return None;
        }
        let tag_layout = Layout::new::<DataStackRegisterTag>().pad_to_align();
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        let padding = unsafe { self.alignment_padding(value_layout.align()) };
        if self.position + padding + value_layout.size() + tag_layout.size() + type_layout.size()
            > self.size()
        {
            return None;
        }
        unsafe {
            self.position += padding + value_layout.size();
            let position = self.position;
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .write_unaligned(DataStackRegisterTag {
                    type_hash,
                    layout: value_layout,
                    finalizer: None,
                    padding: padding as u8,
                });
            self.position += tag_layout.size();
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<TypeHash>()
                .write_unaligned(TypeHash::of::<DataStackRegisterTag>());
            self.position += type_layout.size();
            self.registers.push(position);
            Some(self.registers.len() - 1)
        }
    }

    /// Moves the whole content of `other` on top of this stack.
    ///
    /// Gives `other` back untouched when it does not fit. Used to hand a batch
    /// of arguments prepared elsewhere to a call.
    pub fn push_stack(&mut self, mut other: Self) -> Result<(), Self> {
        if self.available() < other.position {
            return Err(other);
        }
        self.memory[self.position..(self.position + other.position)]
            .copy_from_slice(&other.memory[0..other.position]);
        self.position += other.position;
        self.finalizers
            .extend(other.finalizers.iter().map(|(key, value)| {
                (
                    *key,
                    DataStackFinalizer {
                        callback: value.callback,
                        layout: value.layout,
                    },
                )
            }));
        unsafe { other.prevent_drop() };
        Ok(())
    }

    /// Moves a register value onto the stack, leaving the register empty.
    ///
    /// Returns `false` when the mode forbids values or the value does not fit.
    pub fn push_from_register(&mut self, register: &mut DataStackRegisterAccess) -> bool {
        if !self.mode.allows_values() {
            return false;
        }
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        let mut tag = unsafe {
            register
                .stack
                .memory
                .as_ptr()
                .add(register.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned()
        };
        if self.position + tag.layout.size() + type_layout.size() > self.size() {
            return false;
        }
        if let Entry::Vacant(e) = self.finalizers.entry(tag.type_hash)
            && let Some(finalizer) = tag.finalizer
        {
            e.insert(DataStackFinalizer {
                callback: finalizer,
                layout: tag.layout,
            });
        }
        tag.finalizer = None;
        unsafe {
            let source = register
                .stack
                .memory
                .as_ptr()
                .add(register.position - tag.layout.size());
            let target = self.memory.as_mut_ptr().add(self.position);
            target.copy_from(source, tag.layout.size());
            self.position += tag.layout.size();
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<TypeHash>()
                .write_unaligned(tag.type_hash);
            self.position += type_layout.size();
            register
                .stack
                .memory
                .as_mut_ptr()
                .add(register.position)
                .cast::<DataStackRegisterTag>()
                .write_unaligned(tag);
        }
        true
    }

    /// Moves the top value off the stack.
    ///
    /// Returns [`None`], leaving the stack untouched, when the top value is not
    /// a `T`.
    pub fn pop<T: Sized + 'static>(&mut self) -> Option<T> {
        if !self.mode.allows_values() {
            return None;
        }
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        let value_layout = Layout::new::<T>().pad_to_align();
        if self.position < type_layout.size() + value_layout.size() {
            return None;
        }
        let type_hash = unsafe {
            self.memory
                .as_mut_ptr()
                .add(self.position - type_layout.size())
                .cast::<TypeHash>()
                .read_unaligned()
        };
        if type_hash != TypeHash::of::<T>() || type_hash == TypeHash::of::<DataStackRegisterTag>() {
            return None;
        }
        self.position -= type_layout.size();
        let result = unsafe {
            self.memory
                .as_ptr()
                .add(self.position - value_layout.size())
                .cast::<T>()
                .read_unaligned()
        };
        self.position -= value_layout.size();
        Some(result)
    }

    /// [`DataStack::pop`] without knowing the type, returning the raw bytes
    /// along with the layout, type and drop function.
    ///
    /// # Safety
    ///
    /// The returned bytes are an owned value that nothing drops for the caller.
    /// Losing them leaks, and dropping them twice is undefined. Feed them back
    /// to [`DataStack::push_raw`] or run the returned finalizer once.
    #[allow(clippy::type_complexity)]
    pub unsafe fn pop_raw(&mut self) -> Option<(Layout, TypeHash, unsafe fn(*mut ()), Vec<u8>)> {
        if !self.mode.allows_values() {
            return None;
        }
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        if self.position < type_layout.size() {
            return None;
        }
        let type_hash = unsafe {
            self.memory
                .as_mut_ptr()
                .add(self.position - type_layout.size())
                .cast::<TypeHash>()
                .read_unaligned()
        };
        if type_hash == TypeHash::of::<DataStackRegisterTag>() {
            return None;
        }
        let finalizer = self.finalizers.get(&type_hash)?;
        if self.position < type_layout.size() + finalizer.layout.size() {
            return None;
        }
        self.position -= type_layout.size();
        let data = self.memory[(self.position - finalizer.layout.size())..self.position].to_vec();
        self.position -= finalizer.layout.size();
        Some((finalizer.layout, type_hash, finalizer.callback, data))
    }

    /// Drops the top value in place instead of returning it.
    ///
    /// Returns `false` when the top of the stack is a register.
    pub fn drop(&mut self) -> bool {
        if !self.mode.allows_values() {
            return false;
        }
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        self.position -= type_layout.size();
        let type_hash = unsafe {
            self.memory
                .as_ptr()
                .add(self.position)
                .cast::<TypeHash>()
                .read_unaligned()
        };
        if type_hash == TypeHash::of::<DataStackRegisterTag>() {
            return false;
        }
        if let Some(finalizer) = self.finalizers.get(&type_hash) {
            self.position -= finalizer.layout.size();
            unsafe {
                (finalizer.callback)(self.memory.as_mut_ptr().add(self.position).cast::<()>());
            }
        }
        true
    }

    /// Removes the topmost register, dropping its value if it holds one.
    ///
    /// Returns `false` when the top of the stack is not a register.
    pub fn drop_register(&mut self) -> bool {
        if !self.mode.allows_registers() {
            return false;
        }
        let tag_layout = Layout::new::<DataStackRegisterTag>().pad_to_align();
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        unsafe {
            let type_hash = self
                .memory
                .as_mut_ptr()
                .add(self.position - type_layout.size())
                .cast::<TypeHash>()
                .read_unaligned();
            if type_hash != TypeHash::of::<DataStackRegisterTag>() {
                return false;
            }
            self.position -= type_layout.size();
            self.position -= tag_layout.size();
            let tag = self
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned();
            self.position -= tag.layout.size() - tag.padding as usize;
            if let Some(finalizer) = tag.finalizer {
                (finalizer)(self.memory.as_mut_ptr().add(self.position).cast::<()>());
            }
            self.registers.pop();
        }
        true
    }

    /// Moves the top `data_count` values into a new stack of their own.
    ///
    /// `capacity` sizes the new stack, and is raised when the values need more.
    /// Used to detach arguments for a call that runs elsewhere, for example on
    /// another thread.
    pub fn pop_stack(&mut self, mut data_count: usize, capacity: Option<usize>) -> Self {
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        let mut size = 0;
        let mut position = self.position;
        let mut finalizers = HashMap::new();
        while data_count > 0 && position > 0 {
            data_count -= 1;
            position -= type_layout.size();
            size += type_layout.size();
            let type_hash = unsafe {
                self.memory
                    .as_mut_ptr()
                    .add(position)
                    .cast::<TypeHash>()
                    .read_unaligned()
            };
            if let Some(finalizer) = self.finalizers.get(&type_hash) {
                position -= finalizer.layout.size();
                size += finalizer.layout.size();
                finalizers.insert(
                    type_hash,
                    DataStackFinalizer {
                        callback: finalizer.callback,
                        layout: finalizer.layout,
                    },
                );
            }
        }
        let mut result = Self::new(capacity.unwrap_or(size).max(size), self.mode);
        result.memory[0..size].copy_from_slice(&self.memory[position..self.position]);
        result.finalizers.extend(finalizers);
        self.position = position;
        result.position = size;
        result
    }

    /// Moves the top value into a register, dropping whatever the register
    /// held.
    ///
    /// Returns `false` when the types do not match or the stack is empty.
    pub fn pop_to_register(&mut self, register: &mut DataStackRegisterAccess) -> bool {
        if !self.mode.allows_values() {
            return false;
        }
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        if self.position < type_layout.size() {
            return false;
        }
        let type_hash = unsafe {
            self.memory
                .as_mut_ptr()
                .add(self.position - type_layout.size())
                .cast::<TypeHash>()
                .read_unaligned()
        };
        let mut tag = unsafe {
            register
                .stack
                .memory
                .as_ptr()
                .add(register.position)
                .cast::<DataStackRegisterTag>()
                .read_unaligned()
        };
        if type_hash != tag.type_hash || type_hash == TypeHash::of::<DataStackRegisterTag>() {
            return false;
        }
        if self.position < type_layout.size() + tag.layout.size() {
            return false;
        }
        let finalizer = match self.finalizers.get(&type_hash) {
            Some(finalizer) => finalizer.callback,
            None => return false,
        };
        unsafe {
            if let Some(finalizer) = tag.finalizer {
                (finalizer)(
                    register
                        .stack
                        .memory
                        .as_mut_ptr()
                        .add(register.position - tag.layout.size())
                        .cast::<()>(),
                );
            }
            tag.finalizer = Some(finalizer);
            let source = self
                .memory
                .as_ptr()
                .add(self.position - type_layout.size() - tag.layout.size());
            let target = register
                .stack
                .memory
                .as_mut_ptr()
                .add(register.position - tag.layout.size());
            target.copy_from(source, tag.layout.size());
            register
                .stack
                .memory
                .as_mut_ptr()
                .add(register.position)
                .cast::<DataStackRegisterTag>()
                .write_unaligned(tag);
        }
        self.position -= type_layout.size();
        self.position -= tag.layout.size();
        true
    }

    /// Marks the current position, to unwind or reverse back to later.
    pub fn store(&self) -> DataStackToken {
        DataStackToken(self.position)
    }

    /// Unwinds down to `token`, dropping every value and register above it.
    ///
    /// This is how a scope cleans up after itself.
    pub fn restore(&mut self, token: DataStackToken) {
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        let tag_layout = Layout::new::<DataStackRegisterTag>().pad_to_align();
        let tag_type_hash = TypeHash::of::<DataStackRegisterTag>();
        while self.position > token.0 {
            self.position -= type_layout.size();
            let type_hash = unsafe {
                self.memory
                    .as_ptr()
                    .add(self.position)
                    .cast::<TypeHash>()
                    .read_unaligned()
            };
            if type_hash == tag_type_hash {
                unsafe {
                    let tag = self
                        .memory
                        .as_ptr()
                        .add(self.position - tag_layout.size())
                        .cast::<DataStackRegisterTag>()
                        .read_unaligned();
                    self.position -= tag_layout.size();
                    self.position -= tag.layout.size();
                    if let Some(finalizer) = tag.finalizer {
                        (finalizer)(self.memory.as_mut_ptr().add(self.position).cast::<()>());
                    }
                    self.position -= tag.padding as usize;
                    self.registers.pop();
                }
            } else if let Some(finalizer) = self.finalizers.get(&type_hash) {
                self.position -= finalizer.layout.size();
                unsafe {
                    (finalizer.callback)(self.memory.as_mut_ptr().add(self.position).cast::<()>());
                }
            }
        }
    }

    /// Reverses the order of the items pushed since `token`.
    ///
    /// Callers push arguments in declaration order while callees pop them in
    /// the same order, so the block has to be flipped in between.
    /// See [`DataStackPack::stack_push_reversed`].
    pub fn reverse(&mut self, token: DataStackToken) {
        let size = self.position.saturating_sub(token.0);
        let mut meta_data = SmallVec::<[_; 8]>::with_capacity(8);
        let mut meta_registers = 0;
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        let tag_layout = Layout::new::<DataStackRegisterTag>().pad_to_align();
        let tag_type_hash = TypeHash::of::<DataStackRegisterTag>();
        let mut position = self.position;
        while position > token.0 {
            position -= type_layout.size();
            let type_hash = unsafe {
                self.memory
                    .as_mut_ptr()
                    .add(position)
                    .cast::<TypeHash>()
                    .read_unaligned()
            };
            if type_hash == tag_type_hash {
                unsafe {
                    let tag = self
                        .memory
                        .as_ptr()
                        .add(self.position - tag_layout.size())
                        .cast::<DataStackRegisterTag>()
                        .read_unaligned();
                    position -= tag_layout.size();
                    position -= tag.layout.size();
                    meta_data.push((
                        position - token.0,
                        type_layout.size() + tag_layout.size() + tag.layout.size(),
                    ));
                    meta_registers += 1;
                }
            } else if let Some(finalizer) = self.finalizers.get(&type_hash) {
                position -= finalizer.layout.size();
                meta_data.push((
                    position - token.0,
                    type_layout.size() + finalizer.layout.size(),
                ));
            }
        }
        if meta_data.len() <= 1 {
            return;
        }
        let mut memory = SmallVec::<[_; 256]>::new();
        memory.resize(size, 0);
        memory.copy_from_slice(&self.memory[token.0..self.position]);
        for (source_position, size) in meta_data {
            self.memory[position..(position + size)]
                .copy_from_slice(&memory[source_position..(source_position + size)]);
            position += size;
        }
        let start = self.registers.len() - meta_registers;
        self.registers[start..].reverse();
    }

    /// Returns the type of the top item without moving anything.
    ///
    /// A register reports the type tag of its header, not the type it stores.
    pub fn peek(&self) -> Option<TypeHash> {
        if self.position == 0 {
            return None;
        }
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        Some(unsafe {
            self.memory
                .as_ptr()
                .add(self.position - type_layout.size())
                .cast::<TypeHash>()
                .read_unaligned()
        })
    }

    /// Returns how many registers are alive.
    pub fn registers_count(&self) -> usize {
        self.registers.len()
    }

    /// Takes a handle to one register, or [`None`] when the index is unused.
    pub fn access_register(&'_ mut self, index: usize) -> Option<DataStackRegisterAccess<'_>> {
        let position = *self.registers.get(index)?;
        Some(DataStackRegisterAccess {
            stack: self,
            position,
        })
    }

    /// Takes handles to two different registers at once, for moving a value
    /// between them.
    ///
    /// Returns [`None`] when the indices are equal or unused.
    pub fn access_registers_pair(
        &'_ mut self,
        a: usize,
        b: usize,
    ) -> Option<(DataStackRegisterAccess<'_>, DataStackRegisterAccess<'_>)> {
        if a == b {
            return None;
        }
        let position_a = *self.registers.get(a)?;
        let position_b = *self.registers.get(b)?;
        unsafe {
            Some((
                DataStackRegisterAccess {
                    stack: (self as *mut Self).as_mut()?,
                    position: position_a,
                },
                DataStackRegisterAccess {
                    stack: (self as *mut Self).as_mut()?,
                    position: position_b,
                },
            ))
        }
    }

    /// Stops this stack from unwinding its content when it is dropped.
    ///
    /// # Safety
    ///
    /// Everything still on the stack leaks unless its ownership was already
    /// handed to someone else, which is what [`DataStack::push_stack`] does.
    pub unsafe fn prevent_drop(&mut self) {
        self.drop = false;
    }

    /// Returns the padding needed at the current position to reach
    /// `alignment`.
    ///
    /// # Safety
    ///
    /// Reads the buffer pointer at the current position, which must be inside
    /// the allocation.
    #[inline]
    unsafe fn alignment_padding(&self, alignment: usize) -> usize {
        pointer_alignment_padding(
            unsafe { self.memory.as_ptr().add(self.position) },
            alignment,
        )
    }
}

/// Moves a tuple of values on and off a [`DataStack`] in one step.
///
/// This is the bridge between a Rust call with typed arguments and the untyped
/// stack. Argument tuples and result tuples both go through it.
/// Implemented for tuples of up to sixteen elements, and for `()`.
pub trait DataStackPack: Sized {
    /// Pushes every element in tuple order.
    fn stack_push(self, stack: &mut DataStack);

    /// Pushes every element so that the first one ends up on top.
    ///
    /// This is the order a callee expects, since it pops its arguments from
    /// first to last.
    fn stack_push_reversed(self, stack: &mut DataStack) {
        let token = stack.store();
        self.stack_push(stack);
        stack.reverse(token);
    }

    /// Pops every element in tuple order.
    ///
    /// # Panics
    ///
    /// Panics when the stack does not hold the expected types.
    fn stack_pop(stack: &mut DataStack) -> Self;

    /// Returns the types of the tuple elements, in order.
    fn pack_types() -> Vec<TypeHash>;
}

impl DataStackPack for () {
    fn stack_push(self, _: &mut DataStack) {}

    fn stack_pop(_: &mut DataStack) -> Self {}

    fn pack_types() -> Vec<TypeHash> {
        vec![]
    }
}

/// Implements [`DataStackPack`] for a tuple of the given element types.
macro_rules! impl_data_stack_tuple {
    ($($type:ident),+) => {
        impl<$($type: 'static),+> DataStackPack for ($($type,)+) {
            #[allow(non_snake_case)]
            fn stack_push(self, stack: &mut DataStack) {
                let ($( $type, )+) = self;
                $( stack.push($type); )+
            }

            #[allow(non_snake_case)]
            fn stack_pop(stack: &mut DataStack) -> Self {
                ($(
                    stack.pop::<$type>().unwrap_or_else(
                        || panic!("Could not pop data of type: {}", std::any::type_name::<$type>())
                    ),
                )+)
            }

            #[allow(non_snake_case)]
            fn pack_types() -> Vec<TypeHash> {
                vec![ $( TypeHash::of::<$type>() ),+ ]
            }
        }
    };
}

impl_data_stack_tuple!(A);
impl_data_stack_tuple!(A, B);
impl_data_stack_tuple!(A, B, C);
impl_data_stack_tuple!(A, B, C, D);
impl_data_stack_tuple!(A, B, C, D, E);
impl_data_stack_tuple!(A, B, C, D, E, F);
impl_data_stack_tuple!(A, B, C, D, E, F, G);
impl_data_stack_tuple!(A, B, C, D, E, F, G, H);
impl_data_stack_tuple!(A, B, C, D, E, F, G, H, I);
impl_data_stack_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_data_stack_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_data_stack_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_data_stack_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_data_stack_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_data_stack_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_data_stack_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

#[cfg(test)]
mod tests {
    use crate::{
        data_stack::{DataStack, DataStackMode},
        type_hash::TypeHash,
    };
    use std::{alloc::Layout, cell::RefCell, rc::Rc};

    #[test]
    fn test_data_stack() {
        struct Droppable(Rc<RefCell<bool>>);

        impl Drop for Droppable {
            fn drop(&mut self) {
                *self.0.borrow_mut() = true;
            }
        }

        let dropped = Rc::new(RefCell::new(false));
        let mut stack = DataStack::new(10240, DataStackMode::Values);
        assert_eq!(stack.size(), 16384);
        assert_eq!(stack.position(), 0);
        stack.push(Droppable(dropped.clone()));
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                32
            } else {
                16
            }
        );
        let token = stack.store();
        stack.push(42_usize);
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                64
            } else {
                32
            }
        );
        stack.push(true);
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                89
            } else {
                41
            }
        );
        stack.push(4.2_f32);
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                117
            } else {
                53
            }
        );
        assert!(!*dropped.borrow());
        assert!(stack.pop::<()>().is_none());
        stack.push(());
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                141
            } else {
                61
            }
        );
        stack.reverse(token);
        let mut stack2 = stack.pop_stack(2, None);
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                84
            } else {
                36
            }
        );
        assert_eq!(
            stack2.size(),
            if cfg!(feature = "typehash_debug_name") {
                64
            } else {
                32
            }
        );
        assert_eq!(
            stack2.position(),
            if cfg!(feature = "typehash_debug_name") {
                57
            } else {
                25
            }
        );
        assert_eq!(stack2.pop::<usize>().unwrap(), 42_usize);
        assert_eq!(
            stack2.position(),
            if cfg!(feature = "typehash_debug_name") {
                25
            } else {
                9
            }
        );
        assert!(stack2.pop::<bool>().unwrap());
        assert_eq!(stack2.position(), 0);
        stack2.push(true);
        stack2.push(42_usize);
        stack.push_stack(stack2).ok().unwrap();
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                141
            } else {
                61
            }
        );
        assert_eq!(stack.pop::<usize>().unwrap(), 42_usize);
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                109
            } else {
                45
            }
        );
        assert!(stack.pop::<bool>().unwrap());
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                84
            } else {
                36
            }
        );
        assert_eq!(stack.pop::<f32>().unwrap(), 4.2_f32);
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                56
            } else {
                24
            }
        );
        stack.pop::<()>().unwrap();
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                32
            } else {
                16
            }
        );
        stack.push(42_usize);
        unsafe {
            let (layout, type_hash, finalizer, data) = stack.pop_raw().unwrap();
            assert_eq!(layout, Layout::new::<usize>().pad_to_align());
            assert_eq!(type_hash, TypeHash::of::<usize>());
            assert!(stack.push_raw(layout, type_hash, finalizer, &data));
            assert_eq!(
                stack.position(),
                if cfg!(feature = "typehash_debug_name") {
                    64
                } else {
                    32
                }
            );
            assert_eq!(stack.pop::<usize>().unwrap(), 42_usize);
            assert_eq!(
                stack.position(),
                if cfg!(feature = "typehash_debug_name") {
                    32
                } else {
                    16
                }
            );
        }
        drop(stack);
        assert!(*dropped.borrow());

        let mut stack = DataStack::new(10240, DataStackMode::Registers);
        assert_eq!(stack.size(), 16384);
        stack.push_register::<bool>().unwrap();
        stack.drop_register();
        let a = stack.push_register_value(true).unwrap();
        assert!(*stack.access_register(a).unwrap().read::<bool>().unwrap());
        assert!(stack.access_register(a).unwrap().take::<bool>().unwrap());
        assert!(!stack.access_register(a).unwrap().has_value());
        let b = stack.push_register_value(0usize).unwrap();
        stack.access_register(b).unwrap().set(42usize);
        assert_eq!(
            *stack.access_register(b).unwrap().read::<usize>().unwrap(),
            42
        );
    }
}

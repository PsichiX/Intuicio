use crate::{type_hash::TypeHash, Finalize};
use std::{
    alloc::Layout,
    collections::{hash_map::Entry, HashMap},
    ops::Range,
};

#[derive(Debug, Copy, Clone)]
struct DataStackFinalizer {
    callback: unsafe fn(*mut ()),
    layout: Layout,
}

#[derive(Debug, Copy, Clone)]
struct DataStackRegisterTag {
    type_hash: TypeHash,
    layout: Layout,
    finalizer: Option<unsafe fn(*mut ())>,
}

pub struct DataStackToken(usize);

impl DataStackToken {
    /// # Safety
    pub unsafe fn new(position: usize) -> Self {
        Self(position)
    }
}

pub struct DataStackRegisterAccess<'a> {
    stack: &'a mut DataStack,
    position: usize,
}

impl<'a> DataStackRegisterAccess<'a> {
    pub fn type_hash(&self) -> TypeHash {
        unsafe {
            self.stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read()
                .type_hash
        }
    }

    pub fn layout(&self) -> Layout {
        unsafe {
            self.stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read()
                .layout
        }
    }

    pub fn type_hash_layout(&self) -> (TypeHash, Layout) {
        unsafe {
            let tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read();
            (tag.type_hash, tag.layout)
        }
    }

    pub fn has_value(&self) -> bool {
        unsafe {
            self.stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read()
                .finalizer
                .is_some()
        }
    }

    pub fn read<T: 'static>(&'a self) -> Option<&'a T> {
        unsafe {
            let tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read();
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

    pub fn write<T: 'static>(&'a mut self) -> Option<&'a mut T> {
        unsafe {
            let tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read();
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

    pub fn take<T: 'static>(&mut self) -> Option<T> {
        unsafe {
            let mut tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read();
            if tag.type_hash == TypeHash::of::<T>() && tag.finalizer.is_some() {
                tag.finalizer = None;
                self.stack
                    .memory
                    .as_mut_ptr()
                    .add(self.position)
                    .cast::<DataStackRegisterTag>()
                    .write(tag);
                Some(
                    self.stack
                        .memory
                        .as_ptr()
                        .add(self.position - tag.layout.size())
                        .cast::<T>()
                        .read(),
                )
            } else {
                None
            }
        }
    }

    pub fn free(&mut self) -> bool {
        unsafe {
            let mut tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read();
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
                    .write(tag);
                true
            } else {
                false
            }
        }
    }

    pub fn set<T: Finalize + 'static>(&mut self, value: T) {
        unsafe {
            let mut tag = self
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read();
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
                    .write(value);
                self.stack
                    .memory
                    .as_mut_ptr()
                    .add(self.position)
                    .cast::<DataStackRegisterTag>()
                    .write(tag);
            }
        }
    }

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
                .read();
            let other_tag = other
                .stack
                .memory
                .as_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .read();
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
                std::ptr::copy(source, target, tag.layout.size());
                self.stack
                    .memory
                    .as_mut_ptr()
                    .add(self.position)
                    .cast::<DataStackRegisterTag>()
                    .write(tag);
            }
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum DataStackMode {
    Values,
    Registers,
    #[default]
    All,
}

impl DataStackMode {
    pub fn allows_values(self) -> bool {
        matches!(self, Self::Values | Self::All)
    }

    pub fn allows_registers(self) -> bool {
        matches!(self, Self::Registers | Self::All)
    }
}

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

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn size(&self) -> usize {
        self.memory.len()
    }

    pub fn available(&self) -> usize {
        self.size().saturating_sub(self.position)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.memory[0..self.position]
    }

    pub fn visit(&self, mut f: impl FnMut(TypeHash, Layout, &[u8], Range<usize>, bool)) {
        let type_layout = Layout::new::<TypeHash>().pad_to_align();
        let tag_layout = Layout::new::<DataStackRegisterTag>().pad_to_align();
        let mut position = self.position;
        while position > 0 {
            if position < type_layout.size() {
                return;
            }
            position -= type_layout.size();
            let type_hash = unsafe { self.memory.as_ptr().add(position).cast::<TypeHash>().read() };
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
                        .read()
                };
                if position < tag.layout.size() {
                    return;
                }
                position -= tag.layout.size();
                let range = position..(position + tag.layout.size());
                f(
                    tag.type_hash,
                    tag.layout,
                    &self.memory[range.clone()],
                    range,
                    tag.finalizer.is_some(),
                );
            } else if let Some(finalizer) = self.finalizers.get(&type_hash) {
                if position < finalizer.layout.size() {
                    return;
                }
                position -= finalizer.layout.size();
                let range = position..(position + finalizer.layout.size());
                f(
                    type_hash,
                    finalizer.layout,
                    &self.memory[range.clone()],
                    range,
                    true,
                );
            }
        }
    }

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
                .write(value);
            self.position += value_layout.size();
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<TypeHash>()
                .write(type_hash);
            self.position += type_layout.size();
        }
        true
    }

    pub fn push_register<T: Finalize + 'static>(&mut self) -> Option<usize> {
        unsafe { self.push_register_raw(TypeHash::of::<T>(), Layout::new::<T>()) }
    }

    pub fn push_register_value<T: Finalize + 'static>(&mut self, value: T) -> Option<usize> {
        let result = self.push_register::<T>()?;
        let mut access = self.access_register(result)?;
        access.set(value);
        Some(result)
    }

    /// # Safety
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
        if self.position + value_layout.size() + tag_layout.size() + type_layout.size()
            > self.size()
        {
            return None;
        }
        unsafe {
            self.position += value_layout.size();
            let position = self.position;
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<DataStackRegisterTag>()
                .write(DataStackRegisterTag {
                    type_hash,
                    layout: value_layout,
                    finalizer: None,
                });
            self.position += tag_layout.size();
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<TypeHash>()
                .write(TypeHash::of::<DataStackRegisterTag>());
            self.position += type_layout.size();
            self.registers.push(position);
            Some(self.registers.len() - 1)
        }
    }

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
                .read()
        };
        if self.position + tag.layout.size() + type_layout.size() > self.size() {
            return false;
        }
        if let Entry::Vacant(e) = self.finalizers.entry(tag.type_hash) {
            if let Some(finalizer) = tag.finalizer {
                e.insert(DataStackFinalizer {
                    callback: finalizer,
                    layout: tag.layout,
                });
            }
        }
        tag.finalizer = None;
        unsafe {
            let source = register
                .stack
                .memory
                .as_ptr()
                .add(register.position - tag.layout.size());
            let target = self.memory.as_mut_ptr().add(self.position);
            std::ptr::copy(source, target, tag.layout.size());
            self.position += tag.layout.size();
            self.memory
                .as_mut_ptr()
                .add(self.position)
                .cast::<TypeHash>()
                .write(tag.type_hash);
            self.position += type_layout.size();
            register
                .stack
                .memory
                .as_mut_ptr()
                .add(register.position)
                .cast::<DataStackRegisterTag>()
                .write(tag);
        }
        true
    }

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
                .read()
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
                .read()
        };
        self.position -= value_layout.size();
        Some(result)
    }

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
                .read()
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
                .read();
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
                .read();
            self.position -= tag.layout.size();
            if let Some(finalizer) = tag.finalizer {
                (finalizer)(self.memory.as_mut_ptr().add(self.position).cast::<()>());
            }
            self.registers.pop();
        }
        true
    }

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
                    .read()
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
                .read()
        };
        let mut tag = unsafe {
            register
                .stack
                .memory
                .as_ptr()
                .add(register.position)
                .cast::<DataStackRegisterTag>()
                .read()
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
            std::ptr::copy(source, target, tag.layout.size());
            register
                .stack
                .memory
                .as_mut_ptr()
                .add(register.position)
                .cast::<DataStackRegisterTag>()
                .write(tag);
        }
        self.position -= type_layout.size();
        self.position -= tag.layout.size();
        true
    }

    pub fn store(&self) -> DataStackToken {
        DataStackToken(self.position)
    }

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
                    .read()
            };
            if type_hash == tag_type_hash {
                unsafe {
                    let tag = self
                        .memory
                        .as_ptr()
                        .add(self.position - tag_layout.size())
                        .cast::<DataStackRegisterTag>()
                        .read();
                    self.position -= tag_layout.size();
                    self.position -= tag.layout.size();
                    if let Some(finalizer) = tag.finalizer {
                        (finalizer)(self.memory.as_mut_ptr().add(self.position).cast::<()>());
                    }
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

    pub fn reverse(&mut self, token: DataStackToken) {
        let size = self.position.saturating_sub(token.0);
        if size <= 1 {
            return;
        }
        let mut memory = vec![0; size];
        memory.copy_from_slice(&self.memory[token.0..self.position]);
        let mut meta_data = vec![];
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
                    .read()
            };
            if type_hash == tag_type_hash {
                unsafe {
                    let tag = self
                        .memory
                        .as_ptr()
                        .add(self.position - tag_layout.size())
                        .cast::<DataStackRegisterTag>()
                        .read();
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
        for (source_position, size) in meta_data {
            self.memory[position..(position + size)]
                .copy_from_slice(&memory[source_position..(source_position + size)]);
            position += size;
        }
        let start = self.registers.len() - meta_registers;
        self.registers[start..].reverse();
    }

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
                .read()
        })
    }

    pub fn registers_count(&self) -> usize {
        self.registers.len()
    }

    pub fn access_register(&mut self, index: usize) -> Option<DataStackRegisterAccess> {
        let position = *self.registers.get(index)?;
        Some(DataStackRegisterAccess {
            stack: self,
            position,
        })
    }

    pub fn access_registers_pair(
        &mut self,
        a: usize,
        b: usize,
    ) -> Option<(DataStackRegisterAccess, DataStackRegisterAccess)> {
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

    /// # Safety
    pub unsafe fn prevent_drop(&mut self) {
        self.drop = false;
    }
}

pub trait DataStackPack: Sized {
    fn stack_push(self, stack: &mut DataStack);

    fn stack_push_reversed(self, stack: &mut DataStack) {
        let token = stack.store();
        self.stack_push(stack);
        stack.reverse(token);
    }

    fn stack_pop(stack: &mut DataStack) -> Self;

    fn pack_types() -> Vec<TypeHash>;
}

impl DataStackPack for () {
    fn stack_push(self, _: &mut DataStack) {}

    fn stack_pop(_: &mut DataStack) -> Self {}

    fn pack_types() -> Vec<TypeHash> {
        vec![]
    }
}

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
                ($( stack.pop::<$type>().unwrap(), )+)
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
    use crate::data_stack::{DataStack, DataStackMode};
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn test_data_stack() {
        struct Droppable(Rc<RefCell<bool>>);

        impl Drop for Droppable {
            fn drop(&mut self) {
                *self.0.borrow_mut() = true;
            }
        }

        let dropped = Rc::new(RefCell::new(false));
        let mut stack = DataStack::new(1024, DataStackMode::Values);
        assert_eq!(stack.size(), 1024);
        assert_eq!(stack.position(), 0);
        stack.push(Droppable(dropped.clone()));
        assert_eq!(stack.position(), 16);
        let token = stack.store();
        stack.push(42_usize);
        assert_eq!(stack.position(), 32);
        stack.push(true);
        assert_eq!(stack.position(), 41);
        stack.push(4.2_f32);
        assert_eq!(stack.position(), 53);
        assert_eq!(*dropped.borrow(), false);
        assert!(stack.pop::<()>().is_none());
        stack.push(());
        assert_eq!(stack.position(), 61);
        stack.reverse(token);
        let mut stack2 = stack.pop_stack(2, None);
        assert_eq!(stack.position(), 36);
        assert_eq!(stack2.size(), 32);
        assert_eq!(stack2.position(), 25);
        assert_eq!(stack2.pop::<usize>().unwrap(), 42_usize);
        assert_eq!(stack2.position(), 9);
        assert_eq!(stack2.pop::<bool>().unwrap(), true);
        assert_eq!(stack2.position(), 0);
        stack2.push(true);
        stack2.push(42_usize);
        stack.push_stack(stack2).ok().unwrap();
        assert_eq!(stack.position(), 61);
        assert_eq!(stack.pop::<usize>().unwrap(), 42_usize);
        assert_eq!(stack.position(), 45);
        assert_eq!(stack.pop::<bool>().unwrap(), true);
        assert_eq!(stack.position(), 36);
        assert_eq!(stack.pop::<f32>().unwrap(), 4.2_f32);
        assert_eq!(stack.position(), 24);
        assert_eq!(stack.pop::<()>().unwrap(), ());
        assert_eq!(stack.position(), 16);
        drop(stack);
        assert_eq!(*dropped.borrow(), true);

        let mut stack = DataStack::new(1024, DataStackMode::Registers);
        assert_eq!(stack.size(), 1024);
        assert_eq!(stack.position(), 0);
        stack.push_register::<bool>().unwrap();
        assert_eq!(stack.position(), 41);
        stack.drop_register();
        assert_eq!(stack.position(), 0);
        let a = stack.push_register_value(true).unwrap();
        assert_eq!(stack.position(), 41);
        assert_eq!(
            *stack.access_register(a).unwrap().read::<bool>().unwrap(),
            true
        );
        assert_eq!(
            stack.access_register(a).unwrap().take::<bool>().unwrap(),
            true
        );
        assert_eq!(stack.access_register(a).unwrap().has_value(), false);
        let b = stack.push_register_value(0usize).unwrap();
        assert_eq!(stack.position(), 89);
        stack.access_register(b).unwrap().set(42usize);
        assert_eq!(
            *stack.access_register(b).unwrap().read::<usize>().unwrap(),
            42
        );
    }
}

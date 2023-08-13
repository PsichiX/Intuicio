use crate::struct_type::{StructFieldQuery, StructHandle, StructQuery};
use intuicio_data::{type_hash::TypeHash, Initialize};
use std::{collections::HashMap, mem::MaybeUninit};

pub struct RuntimeObject;

impl Initialize for RuntimeObject {
    fn initialize(&mut self) {}
}

pub struct Object {
    handle: StructHandle,
    memory: Vec<u8>,
    drop: bool,
}

impl Drop for Object {
    fn drop(&mut self) {
        if self.drop {
            unsafe {
                if self.handle.is_native() {
                    self.handle.finalize(self.memory.as_mut_ptr().cast::<()>());
                } else {
                    for field in self.handle.fields() {
                        field.struct_handle().finalize(
                            self.memory
                                .as_mut_ptr()
                                .add(field.address_offset())
                                .cast::<()>(),
                        );
                    }
                }
            }
        }
    }
}

impl Initialize for Object {
    fn initialize(&mut self) {
        unsafe {
            for field in self.handle.fields() {
                field.struct_handle().initialize(
                    self.memory
                        .as_mut_ptr()
                        .add(field.address_offset())
                        .cast::<()>(),
                );
            }
        }
    }
}

impl Object {
    pub fn new(handle: StructHandle) -> Object {
        if !handle.can_initialize() {
            panic!(
                "Objects of type `{}::{}` cannot be initialized!",
                handle.module_name.as_deref().unwrap_or(""),
                handle.name
            );
        }
        let mut memory = vec![0; handle.layout().pad_to_align().size()];
        unsafe { handle.initialize(memory.as_mut_ptr().cast::<()>()) };
        Self {
            memory,
            handle,
            drop: true,
        }
    }

    pub fn try_new(handle: StructHandle) -> Option<Object> {
        if handle.can_initialize() {
            let mut memory = vec![0; handle.layout().pad_to_align().size()];
            unsafe { handle.initialize(memory.as_mut_ptr().cast::<()>()) };
            Some(Self {
                memory,
                handle,
                drop: true,
            })
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn new_uninitialized(handle: StructHandle) -> Object {
        Self {
            memory: vec![0; handle.layout().pad_to_align().size()],
            handle,
            drop: true,
        }
    }

    /// # Safety
    pub unsafe fn new_raw(handle: StructHandle, memory: Vec<u8>) -> Option<Self> {
        if memory.len() == handle.layout().pad_to_align().size() {
            Some(Self {
                memory,
                handle,
                drop: true,
            })
        } else {
            None
        }
    }

    pub fn with_value<T: 'static>(handle: StructHandle, value: T) -> Option<Self> {
        if handle.type_hash() == TypeHash::of::<T>() {
            unsafe {
                let mut result = Self::new_uninitialized(handle);
                result.as_mut_ptr().cast::<T>().write(value);
                Some(result)
            }
        } else {
            None
        }
    }

    pub fn consume<T: 'static>(mut self) -> Result<T, Self> {
        if self.handle.type_hash() == TypeHash::of::<T>() {
            self.drop = false;
            unsafe {
                let mut result = MaybeUninit::<T>::uninit();
                result
                    .as_mut_ptr()
                    .copy_from(self.memory.as_ptr().cast::<T>(), 1);
                Ok(result.assume_init())
            }
        } else {
            Err(self)
        }
    }

    /// # Safety
    pub unsafe fn into_inner(mut self) -> (StructHandle, Vec<u8>) {
        self.drop = false;
        (self.handle.clone(), std::mem::take(&mut self.memory))
    }

    pub fn struct_handle(&self) -> &StructHandle {
        &self.handle
    }

    /// # Safety
    pub unsafe fn memory(&self) -> &[u8] {
        &self.memory
    }

    /// # Safety
    pub unsafe fn memory_mut(&mut self) -> &mut [u8] {
        &mut self.memory
    }

    /// # Safety
    pub unsafe fn field_memory<'a>(&'a self, query: StructFieldQuery<'a>) -> Option<&[u8]> {
        self.handle.find_field(query).map(|field| {
            let from = field.address_offset();
            let to = from + field.struct_handle().layout().pad_to_align().size();
            &self.memory[from..to]
        })
    }

    /// # Safety
    pub unsafe fn field_memory_mut<'a>(
        &'a mut self,
        query: StructFieldQuery<'a>,
    ) -> Option<&mut [u8]> {
        self.handle.find_field(query).map(|field| {
            let from = field.address_offset();
            let to = from + field.struct_handle().layout().pad_to_align().size();
            &mut self.memory[from..to]
        })
    }

    pub fn read<T: 'static>(&self) -> Option<&T> {
        if self.handle.type_hash() == TypeHash::of::<T>() {
            unsafe { self.memory.as_ptr().cast::<T>().as_ref() }
        } else {
            None
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<&mut T> {
        if self.handle.type_hash() == TypeHash::of::<T>() {
            unsafe { self.memory.as_mut_ptr().cast::<T>().as_mut() }
        } else {
            None
        }
    }

    pub fn read_field<'a, T: 'static>(&'a self, field: &str) -> Option<&'a T> {
        let field = self.handle.find_field(StructFieldQuery {
            name: Some(field.into()),
            struct_query: Some(StructQuery {
                type_hash: Some(TypeHash::of::<T>()),
                ..Default::default()
            }),
            ..Default::default()
        })?;
        unsafe {
            self.memory
                .as_ptr()
                .add(field.address_offset())
                .cast::<T>()
                .as_ref()
        }
    }

    pub fn write_field<'a, T: 'static>(&'a mut self, field: &str) -> Option<&'a mut T> {
        let field = self.handle.find_field(StructFieldQuery {
            name: Some(field.into()),
            struct_query: Some(StructQuery {
                type_hash: Some(TypeHash::of::<T>()),
                ..Default::default()
            }),
            ..Default::default()
        })?;
        unsafe {
            self.memory
                .as_mut_ptr()
                .add(field.address_offset())
                .cast::<T>()
                .as_mut()
        }
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> *const u8 {
        self.memory.as_ptr()
    }

    /// # Safety
    pub unsafe fn as_mut_ptr(&mut self) -> *mut u8 {
        self.memory.as_mut_ptr()
    }

    /// # Safety
    pub unsafe fn prevent_drop(&mut self) {
        self.drop = false;
    }
}

#[derive(Default)]
pub struct DynamicObject {
    properties: HashMap<String, Object>,
}

impl DynamicObject {
    pub fn get(&self, name: &str) -> Option<&Object> {
        self.properties.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Object> {
        self.properties.get_mut(name)
    }

    pub fn set(&mut self, name: impl ToString, value: Object) {
        self.properties.insert(name.to_string(), value);
    }

    pub fn delete(&mut self, name: &str) -> Option<Object> {
        self.properties.remove(name)
    }

    pub fn properties(&self) -> impl Iterator<Item = (&str, &Object)> + '_ {
        self.properties
            .iter()
            .map(|(key, value)| (key.as_str(), value))
    }

    pub fn properties_mut(&mut self) -> impl Iterator<Item = (&str, &mut Object)> + '_ {
        self.properties
            .iter_mut()
            .map(|(key, value)| (key.as_str(), value))
    }

    pub fn property_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.properties.keys().map(|key| key.as_str())
    }
}

#[derive(Default)]
pub struct TypedDynamicObject {
    properties: HashMap<TypeHash, Object>,
}

impl TypedDynamicObject {
    pub fn get<T: 'static>(&self) -> Option<&Object> {
        self.properties.get(&TypeHash::of::<T>())
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut Object> {
        self.properties.get_mut(&TypeHash::of::<T>())
    }

    pub fn set<T: 'static>(&mut self, value: Object) {
        self.properties.insert(TypeHash::of::<T>(), value);
    }

    pub fn delete<T: 'static>(&mut self) -> Option<Object> {
        self.properties.remove(&TypeHash::of::<T>())
    }

    pub fn properties(&self) -> impl Iterator<Item = (&TypeHash, &Object)> + '_ {
        self.properties.iter()
    }

    pub fn properties_mut(&mut self) -> impl Iterator<Item = (&TypeHash, &mut Object)> + '_ {
        self.properties.iter_mut()
    }

    pub fn property_types(&self) -> impl Iterator<Item = &TypeHash> + '_ {
        self.properties.keys()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        object::*,
        registry::Registry,
        struct_type::*,
        utils::{object_pop_from_stack, object_push_to_stack},
    };
    use intuicio_data::prelude::*;
    use std::{
        alloc::Layout,
        rc::{Rc, Weak},
    };

    #[test]
    fn test_send() {
        fn ensure_send<T: Send>() {
            println!("{} is send!", std::any::type_name::<T>());
        }

        ensure_send::<Object>();
    }

    #[test]
    fn test_object() {
        #[derive(Default)]
        struct Droppable(Option<Weak<()>>);

        impl Drop for Droppable {
            fn drop(&mut self) {
                println!("Dropped!");
            }
        }

        let bool_handle = NativeStructBuilder::new::<bool>().build_handle();
        let usize_handle = NativeStructBuilder::new::<usize>().build_handle();
        let f32_handle = NativeStructBuilder::new::<f32>().build_handle();
        let droppable_handle = NativeStructBuilder::new::<Droppable>().build_handle();
        let handle = RuntimeStructBuilder::new("Foo")
            .field(StructField::new("a", bool_handle))
            .field(StructField::new("b", usize_handle))
            .field(StructField::new("c", f32_handle))
            .field(StructField::new("d", droppable_handle))
            .build_handle();
        assert_eq!(handle.layout().size(), 32);
        assert_eq!(handle.layout().align(), 8);
        assert_eq!(handle.fields().len(), 4);
        assert_eq!(handle.fields()[0].struct_handle().layout().size(), 1);
        assert_eq!(handle.fields()[0].struct_handle().layout().align(), 1);
        assert_eq!(handle.fields()[0].address_offset(), 0);
        assert_eq!(handle.fields()[1].struct_handle().layout().size(), 8);
        assert_eq!(handle.fields()[1].struct_handle().layout().align(), 8);
        assert_eq!(handle.fields()[1].address_offset(), 8);
        assert_eq!(handle.fields()[2].struct_handle().layout().size(), 4);
        assert_eq!(handle.fields()[2].struct_handle().layout().align(), 4);
        assert_eq!(handle.fields()[2].address_offset(), 16);
        assert_eq!(handle.fields()[3].struct_handle().layout().size(), 8);
        assert_eq!(handle.fields()[3].struct_handle().layout().align(), 8);
        assert_eq!(handle.fields()[3].address_offset(), 24);
        let mut object = Object::new(handle);
        *object.write_field::<bool>("a").unwrap() = true;
        *object.write_field::<usize>("b").unwrap() = 42;
        *object.write_field::<f32>("c").unwrap() = 4.2;
        let dropped = Rc::new(());
        let dropped_weak = Rc::downgrade(&dropped);
        object.write_field::<Droppable>("d").unwrap().0 = Some(dropped_weak);
        assert_eq!(*object.read_field::<bool>("a").unwrap(), true);
        assert_eq!(*object.read_field::<usize>("b").unwrap(), 42);
        assert_eq!(*object.read_field::<f32>("c").unwrap(), 4.2);
        assert_eq!(Rc::weak_count(&dropped), 1);
        assert!(object.read_field::<()>("d").is_none());
        drop(object);
        assert_eq!(Rc::weak_count(&dropped), 0);
    }

    #[test]
    fn test_drop() {
        type Wrapper = LifetimeRefMut;

        let lifetime = Lifetime::default();
        assert!(lifetime.state().can_write(0));
        let handle = NativeStructBuilder::new_uninitialized::<Wrapper>().build_handle();
        let object = Object::with_value(handle, lifetime.borrow_mut().unwrap()).unwrap();
        assert_eq!(lifetime.state().can_write(0), false);
        drop(object);
        assert!(lifetime.state().can_write(0));
    }

    #[test]
    fn test_inner() {
        let mut stack = DataStack::new(1024, DataStackMode::Values);
        assert_eq!(stack.position(), 0);
        let registry = Registry::default().with_basic_types();
        let handle = registry
            .find_struct(StructQuery {
                type_hash: Some(TypeHash::of::<usize>()),
                ..Default::default()
            })
            .unwrap();
        let mut object = Object::new(handle);
        *object.write::<usize>().unwrap() = 42;
        let (handle, data) = unsafe { object.into_inner() };
        assert_eq!(handle.type_hash(), TypeHash::of::<usize>());
        assert_eq!(*handle.layout(), Layout::new::<usize>());
        assert_eq!(data.len(), 8);
        let object = unsafe { Object::new_raw(handle, data).unwrap() };
        assert!(object_push_to_stack(object, &mut stack));
        assert_eq!(stack.position(), 16);
        let object = object_pop_from_stack(&mut stack, &registry).unwrap();
        assert_eq!(*object.read::<usize>().unwrap(), 42);
        assert_eq!(stack.position(), 0);
    }
}

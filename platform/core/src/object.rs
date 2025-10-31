use crate::types::{StructFieldQuery, Type, TypeHandle, TypeQuery};
use intuicio_data::{Initialize, non_zero_alloc, non_zero_dealloc, type_hash::TypeHash};
use std::collections::HashMap;

pub struct RuntimeObject;

impl Initialize for RuntimeObject {
    fn initialize() -> Self {
        Self
    }
}

pub struct Object {
    handle: TypeHandle,
    memory: *mut u8,
    drop: bool,
}

impl Drop for Object {
    fn drop(&mut self) {
        if self.drop {
            unsafe {
                if self.memory.is_null() {
                    return;
                }
                if self.handle.is_native() {
                    self.handle.finalize(self.memory.cast::<()>());
                } else {
                    match &*self.handle {
                        Type::Struct(type_) => {
                            for field in type_.fields() {
                                field
                                    .type_handle()
                                    .finalize(self.memory.add(field.address_offset()).cast::<()>());
                            }
                        }
                        Type::Enum(type_) => {
                            let discriminant = self.memory.read();
                            if let Some(variant) = type_.find_variant_by_discriminant(discriminant)
                            {
                                for field in &variant.fields {
                                    field.type_handle().finalize(
                                        self.memory.add(field.address_offset()).cast::<()>(),
                                    );
                                }
                            }
                        }
                    }
                }
                non_zero_dealloc(self.memory, *self.handle.layout());
                self.memory = std::ptr::null_mut();
            }
        }
    }
}

impl Object {
    pub fn new(handle: TypeHandle) -> Self {
        if !handle.can_initialize() {
            panic!(
                "Objects of type `{}::{}` cannot be initialized!",
                handle.module_name().unwrap_or(""),
                handle.name()
            );
        }
        let memory = unsafe { non_zero_alloc(*handle.layout()) };
        let mut result = Self {
            memory,
            handle,
            drop: true,
        };
        unsafe { result.initialize() };
        result
    }

    pub fn try_new(handle: TypeHandle) -> Option<Self> {
        if handle.can_initialize() {
            let memory = unsafe { non_zero_alloc(*handle.layout()) };
            if memory.is_null() {
                None
            } else {
                let mut result = Self {
                    memory,
                    handle,
                    drop: true,
                };
                unsafe { result.initialize() };
                Some(result)
            }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn new_uninitialized(handle: TypeHandle) -> Option<Self> {
        let memory = unsafe { non_zero_alloc(*handle.layout()) };
        if memory.is_null() {
            None
        } else {
            Some(Self {
                memory,
                handle,
                drop: true,
            })
        }
    }

    /// # Safety
    pub unsafe fn new_raw(handle: TypeHandle, memory: *mut u8) -> Self {
        Self {
            memory,
            handle,
            drop: true,
        }
    }

    /// # Safety
    pub unsafe fn from_bytes(handle: TypeHandle, bytes: &[u8]) -> Option<Self> {
        if handle.layout().size() == bytes.len() {
            let memory = unsafe { non_zero_alloc(*handle.layout()) };
            if memory.is_null() {
                None
            } else {
                unsafe { memory.copy_from(bytes.as_ptr(), bytes.len()) };
                Some(Self {
                    memory,
                    handle,
                    drop: true,
                })
            }
        } else {
            None
        }
    }

    pub fn with_value<T: 'static>(handle: TypeHandle, value: T) -> Option<Self> {
        if handle.type_hash() == TypeHash::of::<T>() {
            unsafe {
                let mut result = Self::new_uninitialized(handle)?;
                result.as_mut_ptr().cast::<T>().write(value);
                Some(result)
            }
        } else {
            None
        }
    }

    /// # Safety
    pub unsafe fn initialize(&mut self) {
        if self.handle.is_native() {
            unsafe { self.handle.initialize(self.memory.cast::<()>()) };
        } else {
            match &*self.handle {
                Type::Struct(type_) => {
                    for field in type_.fields() {
                        unsafe {
                            field
                                .type_handle()
                                .initialize(self.memory.add(field.address_offset()).cast::<()>())
                        };
                    }
                }
                Type::Enum(type_) => {
                    if let Some(variant) = type_.default_variant() {
                        unsafe { self.memory.write(variant.discriminant()) };
                        for field in &variant.fields {
                            unsafe {
                                field.type_handle().initialize(
                                    self.memory.add(field.address_offset()).cast::<()>(),
                                )
                            };
                        }
                    }
                }
            }
        }
    }

    pub fn consume<T: 'static>(mut self) -> Result<T, Self> {
        if self.handle.type_hash() == TypeHash::of::<T>() {
            self.drop = false;
            unsafe { Ok(self.memory.cast::<T>().read()) }
        } else {
            Err(self)
        }
    }

    /// # Safety
    pub unsafe fn into_inner(mut self) -> (TypeHandle, *mut u8) {
        self.drop = false;
        (self.handle.clone(), self.memory)
    }

    pub fn type_handle(&self) -> &TypeHandle {
        &self.handle
    }

    /// # Safety
    pub unsafe fn memory(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.memory, self.type_handle().layout().size()) }
    }

    /// # Safety
    pub unsafe fn memory_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.memory, self.type_handle().layout().size()) }
    }

    /// # Safety
    pub unsafe fn field_memory<'a>(&'a self, query: StructFieldQuery<'a>) -> Option<&'a [u8]> {
        match &*self.handle {
            Type::Struct(type_) => {
                let field = type_.find_field(query)?;
                Some(unsafe {
                    std::slice::from_raw_parts(
                        self.memory.add(field.address_offset()),
                        field.type_handle().layout().size(),
                    )
                })
            }
            Type::Enum(type_) => {
                let discriminant = unsafe { self.memory.read() };
                let variant = type_.find_variant_by_discriminant(discriminant)?;
                let field = variant.find_field(query)?;
                Some(unsafe {
                    std::slice::from_raw_parts(
                        self.memory.add(field.address_offset()),
                        field.type_handle().layout().size(),
                    )
                })
            }
        }
    }

    /// # Safety
    pub unsafe fn field_memory_mut<'a>(
        &'a mut self,
        query: StructFieldQuery<'a>,
    ) -> Option<&'a mut [u8]> {
        match &*self.handle {
            Type::Struct(type_) => {
                let field = type_.find_field(query)?;
                Some(unsafe {
                    std::slice::from_raw_parts_mut(
                        self.memory.add(field.address_offset()),
                        field.type_handle().layout().size(),
                    )
                })
            }
            Type::Enum(type_) => {
                let discriminant = unsafe { self.memory.read() };
                let variant = type_.find_variant_by_discriminant(discriminant)?;
                let field = variant.find_field(query)?;
                Some(unsafe {
                    std::slice::from_raw_parts_mut(
                        self.memory.add(field.address_offset()),
                        field.type_handle().layout().size(),
                    )
                })
            }
        }
    }

    pub fn read<T: 'static>(&self) -> Option<&T> {
        if self.handle.type_hash() == TypeHash::of::<T>() {
            unsafe { self.memory.cast::<T>().as_ref() }
        } else {
            None
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<&mut T> {
        if self.handle.type_hash() == TypeHash::of::<T>() {
            unsafe { self.memory.cast::<T>().as_mut() }
        } else {
            None
        }
    }

    pub fn read_field<'a, T: 'static>(&'a self, field: &str) -> Option<&'a T> {
        let query = StructFieldQuery {
            name: Some(field.into()),
            type_query: Some(TypeQuery::of::<T>()),
            ..Default::default()
        };
        let field = match &*self.handle {
            Type::Struct(type_) => type_.find_field(query),
            Type::Enum(type_) => {
                let discriminant = unsafe { self.memory.read() };
                let variant = type_.find_variant_by_discriminant(discriminant)?;
                variant.find_field(query)
            }
        }?;
        unsafe { self.memory.add(field.address_offset()).cast::<T>().as_ref() }
    }

    pub fn write_field<'a, T: 'static>(&'a mut self, field: &str) -> Option<&'a mut T> {
        let query = StructFieldQuery {
            name: Some(field.into()),
            type_query: Some(TypeQuery::of::<T>()),
            ..Default::default()
        };
        let field = match &*self.handle {
            Type::Struct(type_) => type_.find_field(query),
            Type::Enum(type_) => {
                let discriminant = unsafe { self.memory.read() };
                let variant = type_.find_variant_by_discriminant(discriminant)?;
                variant.find_field(query)
            }
        }?;
        unsafe { self.memory.add(field.address_offset()).cast::<T>().as_mut() }
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> *const u8 {
        self.memory
    }

    /// # Safety
    pub unsafe fn as_mut_ptr(&mut self) -> *mut u8 {
        self.memory
    }

    /// # Safety
    pub unsafe fn prevent_drop(&mut self) {
        self.drop = false;
    }
}

impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe {
            f.debug_struct("Object")
                .field("address", &(self.as_ptr() as usize))
                .field(
                    "type",
                    &format!(
                        "{}::{}",
                        self.handle.module_name().unwrap_or_default(),
                        self.handle.name()
                    ),
                )
                .finish()
        }
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

    pub fn drain(&mut self) -> impl Iterator<Item = (String, Object)> + '_ {
        self.properties.drain()
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

    pub fn property_values(&self) -> impl Iterator<Item = &Object> + '_ {
        self.properties.values()
    }

    pub fn property_values_mut(&mut self) -> impl Iterator<Item = &mut Object> + '_ {
        self.properties.values_mut()
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

    pub fn drain(&mut self) -> impl Iterator<Item = (TypeHash, Object)> + '_ {
        self.properties.drain()
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

    pub fn property_values(&self) -> impl Iterator<Item = &Object> + '_ {
        self.properties.values()
    }

    pub fn property_values_mut(&mut self) -> impl Iterator<Item = &mut Object> + '_ {
        self.properties.values_mut()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        object::*,
        registry::Registry,
        types::struct_type::*,
        utils::{object_pop_from_stack, object_push_to_stack},
    };
    use intuicio_data::{
        data_stack::{DataStack, DataStackMode},
        lifetime::{Lifetime, LifetimeRefMut},
    };
    use std::{
        alloc::Layout,
        rc::{Rc, Weak},
    };

    #[test]
    fn test_object() {
        struct Droppable(Option<Weak<()>>);

        impl Default for Droppable {
            fn default() -> Self {
                println!("Wrapper created!");
                Self(None)
            }
        }

        impl Drop for Droppable {
            fn drop(&mut self) {
                println!("Wrapper dropped!");
            }
        }

        struct Pass;

        impl Default for Pass {
            fn default() -> Self {
                println!("Pass created!");
                Self
            }
        }

        impl Drop for Pass {
            fn drop(&mut self) {
                println!("Pass dropped!");
            }
        }

        let bool_handle = NativeStructBuilder::new::<bool>()
            .build()
            .into_type()
            .into_handle();
        let f32_handle = NativeStructBuilder::new::<f32>()
            .build()
            .into_type()
            .into_handle();
        let usize_handle = NativeStructBuilder::new::<usize>()
            .build()
            .into_type()
            .into_handle();
        let pass_handle = NativeStructBuilder::new::<Pass>()
            .build()
            .into_type()
            .into_handle();
        let droppable_handle = NativeStructBuilder::new::<Droppable>()
            .build()
            .into_type()
            .into_handle();
        let handle = RuntimeStructBuilder::new("Foo")
            .field(StructField::new("a", bool_handle))
            .field(StructField::new("b", f32_handle))
            .field(StructField::new("c", usize_handle))
            .field(StructField::new("d", pass_handle))
            .field(StructField::new("e", droppable_handle))
            .build()
            .into_type()
            .into_handle();
        assert_eq!(handle.layout().size(), 24);
        assert_eq!(handle.layout().align(), 8);
        assert_eq!(handle.as_struct().unwrap().fields().len(), 5);
        assert_eq!(
            handle.as_struct().unwrap().fields()[0]
                .type_handle()
                .layout()
                .size(),
            1
        );
        assert_eq!(
            handle.as_struct().unwrap().fields()[0]
                .type_handle()
                .layout()
                .align(),
            1
        );
        assert_eq!(handle.as_struct().unwrap().fields()[0].address_offset(), 0);
        assert_eq!(
            handle.as_struct().unwrap().fields()[1]
                .type_handle()
                .layout()
                .size(),
            4
        );
        assert_eq!(
            handle.as_struct().unwrap().fields()[1]
                .type_handle()
                .layout()
                .align(),
            4
        );
        assert_eq!(handle.as_struct().unwrap().fields()[1].address_offset(), 4);
        assert_eq!(
            handle.as_struct().unwrap().fields()[2]
                .type_handle()
                .layout()
                .size(),
            8
        );
        assert_eq!(
            handle.as_struct().unwrap().fields()[2]
                .type_handle()
                .layout()
                .align(),
            8
        );
        assert_eq!(handle.as_struct().unwrap().fields()[2].address_offset(), 8);
        assert_eq!(
            handle.as_struct().unwrap().fields()[3]
                .type_handle()
                .layout()
                .size(),
            0
        );
        assert_eq!(
            handle.as_struct().unwrap().fields()[3]
                .type_handle()
                .layout()
                .align(),
            1
        );
        assert_eq!(handle.as_struct().unwrap().fields()[3].address_offset(), 16);
        assert_eq!(
            handle.as_struct().unwrap().fields()[4]
                .type_handle()
                .layout()
                .size(),
            8
        );
        assert_eq!(
            handle.as_struct().unwrap().fields()[4]
                .type_handle()
                .layout()
                .align(),
            8
        );
        assert_eq!(handle.as_struct().unwrap().fields()[4].address_offset(), 16);
        let mut object = Object::new(handle);
        *object.write_field::<bool>("a").unwrap() = true;
        *object.write_field::<f32>("b").unwrap() = 4.2;
        *object.write_field::<usize>("c").unwrap() = 42;
        let dropped = Rc::new(());
        let dropped_weak = Rc::downgrade(&dropped);
        object.write_field::<Droppable>("e").unwrap().0 = Some(dropped_weak);
        assert!(*object.read_field::<bool>("a").unwrap());
        assert_eq!(*object.read_field::<f32>("b").unwrap(), 4.2);
        assert_eq!(*object.read_field::<usize>("c").unwrap(), 42);
        assert_eq!(Rc::weak_count(&dropped), 1);
        assert!(object.read_field::<()>("e").is_none());
        drop(object);
        assert_eq!(Rc::weak_count(&dropped), 0);
    }

    #[test]
    fn test_drop() {
        type Wrapper = LifetimeRefMut;

        let lifetime = Lifetime::default();
        assert!(lifetime.state().can_write(0));
        let handle = NativeStructBuilder::new_uninitialized::<Wrapper>()
            .build()
            .into_type()
            .into_handle();
        let object = Object::with_value(handle, lifetime.borrow_mut().unwrap()).unwrap();
        assert!(!lifetime.state().can_write(0));
        drop(object);
        assert!(lifetime.state().can_write(0));
    }

    #[test]
    fn test_inner() {
        let mut stack = DataStack::new(10240, DataStackMode::Values);
        assert_eq!(stack.position(), 0);
        let registry = Registry::default().with_basic_types();
        let handle = registry.find_type(TypeQuery::of::<usize>()).unwrap();
        let mut object = Object::new(handle);
        *object.write::<usize>().unwrap() = 42;
        let (handle, data) = unsafe { object.into_inner() };
        assert_eq!(handle.type_hash(), TypeHash::of::<usize>());
        assert_eq!(*handle.layout(), Layout::new::<usize>().pad_to_align());
        let object = unsafe { Object::new_raw(handle, data) };
        assert!(object_push_to_stack(object, &mut stack));
        assert_eq!(
            stack.position(),
            if cfg!(feature = "typehash_debug_name") {
                32
            } else {
                16
            }
        );
        let object = object_pop_from_stack(&mut stack, &registry).unwrap();
        assert_eq!(*object.read::<usize>().unwrap(), 42);
        assert_eq!(stack.position(), 0);
    }
}

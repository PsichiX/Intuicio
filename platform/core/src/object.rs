//! Values of types the host was never compiled against.
//!
//! An [`Object`] is one heap allocation plus the [`TypeHandle`] describing
//! it. That is enough to construct, read, write and destroy a value whose
//! type a script invented at runtime.
//!
//! [`DynamicObject`] and [`TypedDynamicObject`] are a different idea: bags of
//! named or type-keyed objects, for values whose shape is not fixed at all.
use crate::types::{StructFieldQuery, Type, TypeHandle, TypeQuery};
use intuicio_data::{Initialize, non_zero_alloc, non_zero_dealloc, type_hash::TypeHash};
use std::collections::HashMap;

/// Placeholder standing in for "a type with no Rust counterpart".
///
/// Runtime types carry this type's hash, which is how
/// [`crate::types::Type::is_runtime`] tells them apart from native ones.
pub struct RuntimeObject;

impl Initialize for RuntimeObject {
    fn initialize() -> Self {
        Self
    }
}

/// A value of any registered type, held in its own allocation.
///
/// Construction and destruction go through the [`TypeHandle`]: a native type
/// runs the Rust initializer and destructor, a runtime type walks its fields
/// and does each one in turn.
///
/// Typed access is checked, so [`Object::read`] returns [`None`] unless the
/// object really holds that Rust type.
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
                self.handle.finalize(self.memory.cast::<()>());
                non_zero_dealloc(self.memory, *self.handle.layout());
                self.memory = std::ptr::null_mut();
            }
        }
    }
}

impl Object {
    /// Allocates a default value of the given type.
    ///
    /// # Panics
    ///
    /// Panics when the type has no way to create a default value. Use
    /// [`Object::try_new`] to get [`None`] instead.
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

    /// [`Object::new`] that returns [`None`] instead of panicking.
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

    /// Allocates room for a value without creating one.
    ///
    /// # Safety
    ///
    /// The memory holds garbage. It must be filled before the object is read or
    /// dropped, since dropping runs the type's destructor over whatever is
    /// there.
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

    /// Takes ownership of an existing allocation.
    ///
    /// # Safety
    ///
    /// `memory` must hold an initialized value of the type, allocated so that
    /// it can be freed with the type's layout. The object frees it on drop.
    pub unsafe fn new_raw(handle: TypeHandle, memory: *mut u8) -> Self {
        Self {
            memory,
            handle,
            drop: true,
        }
    }

    /// Allocates a value by copying a byte image of one.
    ///
    /// Returns [`None`] when `bytes` is not exactly the size of the type.
    ///
    /// # Safety
    ///
    /// `bytes` must be a valid image of a value of this type, and it is moved,
    /// so the caller must not drop the source afterwards.
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

    /// Moves a Rust value into an object, or returns [`None`] when `handle` does
    /// not describe `T`.
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

    /// Writes a default value into this object's memory.
    ///
    /// # Safety
    ///
    /// The memory must not already hold a value, since the old one is not
    /// dropped first.
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

    /// Moves the value out as a Rust value, or gives the object back on a type
    /// mismatch.
    pub fn consume<T: 'static>(mut self) -> Result<T, Self> {
        if self.handle.type_hash() == TypeHash::of::<T>() {
            self.drop = false;
            unsafe { Ok(self.memory.cast::<T>().read()) }
        } else {
            Err(self)
        }
    }

    /// Splits into the type handle and the allocation, and stops the object from
    /// freeing it.
    ///
    /// # Safety
    ///
    /// The caller becomes responsible for destroying the value and freeing the
    /// memory with the type's layout.
    pub unsafe fn into_inner(mut self) -> (TypeHandle, *mut u8) {
        self.drop = false;
        (self.handle.clone(), self.memory)
    }

    /// Returns the type of the stored value.
    pub fn type_handle(&self) -> &TypeHandle {
        &self.handle
    }

    /// Returns the value as raw bytes.
    ///
    /// # Safety
    ///
    /// Reading the bytes of a type that owns resources, or keeping them past
    /// the object's life, is on the caller.
    pub unsafe fn memory(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.memory, self.type_handle().layout().size()) }
    }

    /// Returns the value as mutable raw bytes.
    ///
    /// # Safety
    ///
    /// Writing bytes that are not a valid value of the stored type makes every
    /// later read, and the eventual drop, undefined.
    pub unsafe fn memory_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.memory, self.type_handle().layout().size()) }
    }

    /// Returns the bytes of one field, chosen by query.
    ///
    /// For an enum, the field is looked up in the variant the value currently
    /// holds.
    ///
    /// # Safety
    ///
    /// Same conditions as [`Object::memory`].
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

    /// Returns the mutable bytes of one field, chosen by query.
    ///
    /// For an enum, the field is looked up in the variant the value currently
    /// holds.
    ///
    /// # Safety
    ///
    /// Same conditions as [`Object::memory_mut`].
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

    /// Borrows the value as a `T`, or returns [`None`] on a type mismatch.
    pub fn read<T: 'static>(&self) -> Option<&T> {
        if self.handle.type_hash() == TypeHash::of::<T>() {
            unsafe { self.memory.cast::<T>().as_ref() }
        } else {
            None
        }
    }

    /// Borrows the value mutably as a `T`, or returns [`None`] on a type
    /// mismatch.
    pub fn write<T: 'static>(&mut self) -> Option<&mut T> {
        if self.handle.type_hash() == TypeHash::of::<T>() {
            unsafe { self.memory.cast::<T>().as_mut() }
        } else {
            None
        }
    }

    /// Borrows one field by name, or returns [`None`] when there is no such
    /// field of that type.
    ///
    /// This is how a runtime type's fields are reached, since there is no Rust
    /// struct to go through.
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

    /// Mutable [`Object::read_field`].
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

    /// Returns the allocation pointer.
    ///
    /// # Safety
    ///
    /// Nothing is checked. The caller takes over both typing and aliasing.
    pub unsafe fn as_ptr(&self) -> *const u8 {
        self.memory
    }

    /// Returns the mutable allocation pointer.
    ///
    /// # Safety
    ///
    /// Nothing is checked. The caller takes over both typing and aliasing.
    pub unsafe fn as_mut_ptr(&mut self) -> *mut u8 {
        self.memory
    }

    /// Stops this object from destroying its value when it is dropped.
    ///
    /// # Safety
    ///
    /// The value leaks unless its ownership was already handed to someone else.
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

/// A bag of named [`Object`] values.
///
/// For values whose shape is decided entirely at runtime, such as objects in
/// a dynamically typed language.
#[derive(Default)]
pub struct DynamicObject {
    properties: HashMap<String, Object>,
}

impl DynamicObject {
    /// Borrows a property by name.
    pub fn get(&self, name: &str) -> Option<&Object> {
        self.properties.get(name)
    }

    /// Borrows a property mutably by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Object> {
        self.properties.get_mut(name)
    }

    /// Sets a property, replacing anything already there.
    pub fn set(&mut self, name: impl ToString, value: Object) {
        self.properties.insert(name.to_string(), value);
    }

    /// Removes a property and returns it.
    pub fn delete(&mut self, name: &str) -> Option<Object> {
        self.properties.remove(name)
    }

    /// Removes and yields every property.
    pub fn drain(&mut self) -> impl Iterator<Item = (String, Object)> + '_ {
        self.properties.drain()
    }

    /// Iterates names and values.
    pub fn properties(&self) -> impl Iterator<Item = (&str, &Object)> + '_ {
        self.properties
            .iter()
            .map(|(key, value)| (key.as_str(), value))
    }

    /// Iterates names and values mutably.
    pub fn properties_mut(&mut self) -> impl Iterator<Item = (&str, &mut Object)> + '_ {
        self.properties
            .iter_mut()
            .map(|(key, value)| (key.as_str(), value))
    }

    /// Iterates property names.
    pub fn property_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.properties.keys().map(|key| key.as_str())
    }

    /// Iterates property values.
    pub fn property_values(&self) -> impl Iterator<Item = &Object> + '_ {
        self.properties.values()
    }

    /// Iterates property values mutably.
    pub fn property_values_mut(&mut self) -> impl Iterator<Item = &mut Object> + '_ {
        self.properties.values_mut()
    }
}

/// A bag of [`Object`] values keyed by type, holding at most one of each.
///
/// Useful for attaching optional data to something, the way a component map
/// does.
#[derive(Default)]
pub struct TypedDynamicObject {
    properties: HashMap<TypeHash, Object>,
}

impl TypedDynamicObject {
    /// Borrows the value stored for type `T`.
    pub fn get<T: 'static>(&self) -> Option<&Object> {
        self.properties.get(&TypeHash::of::<T>())
    }

    /// Borrows the value stored for type `T` mutably.
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut Object> {
        self.properties.get_mut(&TypeHash::of::<T>())
    }

    /// Stores a value under type `T`, replacing anything already there.
    pub fn set<T: 'static>(&mut self, value: Object) {
        self.properties.insert(TypeHash::of::<T>(), value);
    }

    /// Removes the value stored for type `T` and returns it.
    pub fn delete<T: 'static>(&mut self) -> Option<Object> {
        self.properties.remove(&TypeHash::of::<T>())
    }

    /// Removes and yields every value.
    pub fn drain(&mut self) -> impl Iterator<Item = (TypeHash, Object)> + '_ {
        self.properties.drain()
    }

    /// Iterates types and values.
    pub fn properties(&self) -> impl Iterator<Item = (&TypeHash, &Object)> + '_ {
        self.properties.iter()
    }

    /// Iterates types and values mutably.
    pub fn properties_mut(&mut self) -> impl Iterator<Item = (&TypeHash, &mut Object)> + '_ {
        self.properties.iter_mut()
    }

    /// Iterates the stored types.
    pub fn property_types(&self) -> impl Iterator<Item = &TypeHash> + '_ {
        self.properties.keys()
    }

    /// Iterates the stored values.
    pub fn property_values(&self) -> impl Iterator<Item = &Object> + '_ {
        self.properties.values()
    }

    /// Iterates the stored values mutably.
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

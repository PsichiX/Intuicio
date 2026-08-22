//! Values for a dynamically typed language.
//!
//! Everything a script holds is a [`Reference`]: a shared, counted handle to an
//! [`Object`], or null. Cloning a reference shares the value rather than
//! copying it, so two scripts can see each other's writes.
//!
//! [`Type`] and [`Function`] are the same idea for registry entries, so a
//! script can pass a type or a function around as data.
//!
//! Call [`install`] to register every type of this crate, along with the
//! primitive aliases [`Boolean`], [`Integer`], [`Real`], [`Text`], [`Array`]
//! and [`Map`].
//!
//! # Threads
//!
//! A reference is single threaded. To move one to another thread, turn it into
//! a [`Transferable`], which is the only type here that crosses a thread.
use intuicio_core::{
    Filter, define_native_struct,
    function::{FunctionHandle, FunctionQuery},
    object::Object,
    registry::Registry,
    types::{TypeHandle, TypeQuery, struct_type::NativeStructBuilder},
};
use intuicio_data::{shared::Shared, type_hash::TypeHash};
use std::{
    cell::{Ref, RefMut},
    collections::HashMap,
};

/// The boolean type scripts see.
pub type Boolean = bool;
/// The whole number type scripts see.
pub type Integer = i64;
/// The fractional number type scripts see.
pub type Real = f64;
/// The text type scripts see.
pub type Text = String;
/// The list type scripts see.
pub type Array = Vec<Reference>;
/// The map type scripts see, keyed by text.
pub type Map = HashMap<Text, Reference>;

thread_local! {
    static TRANSFERRED_STRUCT_HANDLE: TypeHandle = NativeStructBuilder::new::<Transferred>().build().into_type().into_handle();
}

#[derive(Default, Clone)]
/// A registered type, as a value a script can hold.
///
/// Empty by default, which is what [`Type::handle`] reports as [`None`].
pub struct Type {
    data: Option<TypeHandle>,
}

impl Type {
    /// Looks a type up by name and module, or returns [`None`] when there is
    /// none.
    pub fn by_name(name: &str, module_name: &str, registry: &Registry) -> Option<Self> {
        Some(Self::new(registry.find_type(TypeQuery {
            name: Some(name.into()),
            module_name: Some(module_name.into()),
            ..Default::default()
        })?))
    }

    /// Looks the Rust type `T` up, or returns [`None`] when it is not
    /// registered.
    pub fn of<T: 'static>(registry: &Registry) -> Option<Self> {
        Some(Self::new(registry.find_type(TypeQuery {
            type_hash: Some(TypeHash::of::<T>()),
            ..Default::default()
        })?))
    }

    /// Wraps a handle that was already found.
    pub fn new(handle: TypeHandle) -> Self {
        Self { data: Some(handle) }
    }

    /// Returns the handle, or [`None`] when this value is empty.
    pub fn handle(&self) -> Option<&TypeHandle> {
        self.data.as_ref()
    }

    /// Returns `true` when this is the Rust type `T`.
    pub fn is<T: 'static>(&self) -> bool {
        self.data
            .as_ref()
            .map(|data| data.type_hash() == TypeHash::of::<T>())
            .unwrap_or(false)
    }

    /// Returns `true` when both name the same type. Two empty values are not
    /// the same.
    pub fn is_same_as(&self, other: &Self) -> bool {
        if let (Some(this), Some(other)) = (self.data.as_ref(), other.data.as_ref()) {
            this == other
        } else {
            false
        }
    }

    /// Returns the runtime identity of the type, or [`None`] when this value is
    /// empty.
    pub fn type_hash(&self) -> Option<TypeHash> {
        Some(self.data.as_ref()?.type_hash())
    }
}

#[derive(Default, Clone)]
/// A registered function, as a value a script can hold.
///
/// Empty by default, which is what [`Function::handle`] reports as [`None`].
pub struct Function {
    data: Option<FunctionHandle>,
}

impl Function {
    /// Looks a function up by name and module, or returns [`None`] when there
    /// is none.
    pub fn by_name(name: &str, module_name: &str, registry: &Registry) -> Option<Self> {
        Some(Self::new(registry.find_function(FunctionQuery {
            name: Some(name.into()),
            module_name: Filter::Matching(module_name.into()),
            ..Default::default()
        })?))
    }

    /// Wraps a handle that was already found.
    pub fn new(handle: FunctionHandle) -> Self {
        Self { data: Some(handle) }
    }

    /// Returns the handle, or [`None`] when this value is empty.
    pub fn handle(&self) -> Option<&FunctionHandle> {
        self.data.as_ref()
    }

    /// Returns `true` when both have the same signature. Two empty values are
    /// not the same.
    pub fn is_same_as(&self, other: &Self) -> bool {
        if let (Some(this), Some(other)) = (self.data.as_ref(), other.data.as_ref()) {
            this.signature() == other.signature()
        } else {
            false
        }
    }
}

#[derive(Default, Clone)]
/// A shared handle to a script value, or null.
///
/// Cloning shares the value. Reads and writes are checked at runtime and
/// return [`None`] when the value is already borrowed the other way, so a
/// script reports an error instead of aborting the host.
pub struct Reference {
    data: Option<Shared<Object>>,
}

impl Reference {
    /// The null reference, which is also what [`Default`] gives.
    pub fn null() -> Self {
        Self { data: None }
    }

    /// Returns `true` when this reference holds nothing.
    pub fn is_null(&self) -> bool {
        self.data.is_none()
    }

    /// Returns `true` when the value was moved to another thread.
    ///
    /// See [`Transferable`]. Such a reference can no longer be read or written
    /// here.
    pub fn is_transferred(&self) -> bool {
        self.data
            .as_ref()
            .and_then(|data| data.read())
            .map(|data| data.read::<Transferred>().is_some())
            .unwrap_or_default()
    }

    /// Returns `true` while something else writes to the value.
    pub fn is_being_written(&mut self) -> bool {
        self.data
            .as_mut()
            .map(|data| data.write().is_none())
            .unwrap_or_default()
    }

    /// Boxes a [`Boolean`].
    ///
    /// # Panics
    ///
    /// Panics when the type is not registered. Call [`install`] first.
    pub fn new_boolean(value: Boolean, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    /// Boxes an [`Integer`]. Panics like [`Reference::new_boolean`].
    pub fn new_integer(value: Integer, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    /// Boxes a [`Real`]. Panics like [`Reference::new_boolean`].
    pub fn new_real(value: Real, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    /// Boxes a [`Text`]. Panics like [`Reference::new_boolean`].
    pub fn new_text(value: Text, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    /// Boxes an [`Array`]. Panics like [`Reference::new_boolean`].
    pub fn new_array(value: Array, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    /// Boxes a [`Map`]. Panics like [`Reference::new_boolean`].
    pub fn new_map(value: Map, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    /// Boxes a [`Type`]. Panics like [`Reference::new_boolean`].
    pub fn new_type(value: Type, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    /// Boxes a [`Function`]. Panics like [`Reference::new_boolean`].
    pub fn new_function(value: Function, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    /// Boxes any registered Rust value.
    ///
    /// # Panics
    ///
    /// Panics when `T` is not registered.
    pub fn new<T: 'static>(data: T, registry: &Registry) -> Self {
        let type_ = registry.find_type(TypeQuery::of::<T>()).unwrap_or_else(|| {
            panic!(
                "Could not make a reference of type: {}",
                std::any::type_name::<T>()
            )
        });
        let mut value = unsafe { Object::new_uninitialized(type_).unwrap() };
        unsafe { value.as_mut_ptr().cast::<T>().write(data) };
        Self::new_raw(value)
    }

    /// Boxes a value under a type given by hand, rather than looked up.
    ///
    /// # Panics
    ///
    /// Panics when `ty` is empty. The caller has to make sure `ty` really
    /// describes `T`, since nothing checks it.
    pub fn new_custom<T: 'static>(data: T, ty: &Type) -> Self {
        let mut value =
            unsafe { Object::new_uninitialized(ty.data.as_ref().unwrap().clone()).unwrap() };
        unsafe { value.as_mut_ptr().cast::<T>().write(data) };
        Self::new_raw(value)
    }

    /// Takes ownership of an [`Object`] that already exists.
    pub fn new_raw(data: Object) -> Self {
        Self {
            data: Some(Shared::new(data)),
        }
    }

    /// Takes another handle to an object that is already shared.
    pub fn new_shared(data: Shared<Object>) -> Self {
        Self { data: Some(data) }
    }

    /// Boxes a default value of `ty`.
    ///
    /// # Panics
    ///
    /// Panics when `ty` is empty or has no default value.
    pub fn initialized(ty: &Type) -> Self {
        Self::new_raw(Object::new(ty.data.as_ref().unwrap().clone()))
    }

    /// # Safety
    /// Boxes room for a value of `ty` without creating one.
    ///
    /// # Panics
    ///
    /// Panics when `ty` is empty.
    ///
    /// # Safety
    ///
    /// The memory holds garbage. Write a valid value into it before anything
    /// reads the reference or drops it, because the drop runs the destructor of
    /// the type over whatever is there.
    pub unsafe fn uninitialized(ty: &Type) -> Self {
        Self::new_raw(unsafe {
            Object::new_uninitialized(ty.data.as_ref().unwrap().clone()).unwrap()
        })
    }

    /// Returns the type of the value, or [`None`] when this reference is null
    /// or the value is being written.
    pub fn type_of(&self) -> Option<Type> {
        Some(Type::new(self.data.as_ref()?.read()?.type_handle().clone()))
    }

    /// Borrows the value as a `T`.
    ///
    /// Returns [`None`] when this reference is null, when the value is another
    /// type, or while something writes to it.
    pub fn read<T: 'static>(&'_ self) -> Option<Ref<'_, T>> {
        let result = self.data.as_ref()?.read()?;
        if result.type_handle().type_hash() == TypeHash::of::<T>() {
            Some(Ref::map(result, |data| data.read::<T>().unwrap()))
        } else {
            None
        }
    }

    /// Borrows the value as a `T` for writing.
    ///
    /// Returns [`None`] when this reference is null, when the value is another
    /// type, or while anything else reads or writes it.
    pub fn write<T: 'static>(&'_ mut self) -> Option<RefMut<'_, T>> {
        let result = self.data.as_mut()?.write()?;
        if result.type_handle().type_hash() == TypeHash::of::<T>() {
            Some(RefMut::map(result, |data| data.write::<T>().unwrap()))
        } else {
            None
        }
    }

    /// Borrows the whole object, whatever type it holds.
    pub fn read_object(&'_ self) -> Option<Ref<'_, Object>> {
        self.data.as_ref()?.read()
    }

    /// Borrows the whole object for writing, whatever type it holds.
    pub fn write_object(&'_ mut self) -> Option<RefMut<'_, Object>> {
        self.data.as_mut()?.write()
    }

    /// Puts `data` in place of the value and returns the old one.
    ///
    /// Returns [`None`] on the same terms as [`Reference::write`], and drops
    /// `data` when it does.
    pub fn swap<T: 'static>(&mut self, data: T) -> Option<T> {
        Some(std::mem::replace(
            self.data.as_mut()?.write()?.write::<T>()?,
            data,
        ))
    }

    /// Takes the object out when this is the last reference to it.
    ///
    /// Gives the reference back when others still hold it, or when it is null.
    pub fn try_consume(self) -> Result<Object, Self> {
        match self.data {
            Some(data) => match data.try_consume() {
                Ok(data) => Ok(data),
                Err(data) => Err(Self { data: Some(data) }),
            },
            None => Err(Self::null()),
        }
    }

    /// Returns how many references point at this value. `0` for null.
    pub fn references_count(&self) -> usize {
        self.data
            .as_ref()
            .map(|data| data.references_count())
            .unwrap_or(0)
    }

    /// Returns `true` when both point at the same value.
    ///
    /// `consider_null` decides what two null references answer.
    pub fn does_share_reference(&self, other: &Self, consider_null: bool) -> bool {
        match (self.data.as_ref(), other.data.as_ref()) {
            (Some(this), Some(other)) => this.does_share_reference(other),
            (None, None) => consider_null,
            _ => false,
        }
    }

    /// Moves the value out and leaves a [`Transferred`] marker in its place.
    ///
    /// Returns [`None`] when this reference is null, when the value is being
    /// written, or when the type of the value is not `Send`. Returns the address
    /// of the marker as [`Err`] when the value was already moved out.
    ///
    /// # Safety
    ///
    /// The returned object leaves the borrow tracking of this reference behind.
    /// Use [`Transferable`], which pairs this call with the rebuild on the other
    /// side.
    pub unsafe fn transfer(&self) -> Option<Result<Object, usize>> {
        let mut data = self.data.as_ref()?.write()?;
        if let Some(data) = data.read::<Transferred>() {
            return Some(Err(data.0));
        }
        if !data.type_handle().is_send() {
            return None;
        }
        let mut object = unsafe {
            Object::new_uninitialized(TRANSFERRED_STRUCT_HANDLE.with(|handle| handle.clone()))
                .unwrap()
        };
        unsafe {
            object
                .as_mut_ptr()
                .cast::<Transferred>()
                .write(Transferred(data.as_ptr() as usize))
        };
        Some(Ok(std::mem::replace(&mut *data, object)))
    }
}

impl std::fmt::Debug for Reference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.data.as_ref() {
            Some(object) => {
                if let Some(object) = object.read() {
                    f.debug_struct("Reference").field("data", &*object).finish()
                } else {
                    f.debug_struct("Reference").field("data", &()).finish()
                }
            }
            None => f.debug_struct("Reference").finish(),
        }
    }
}

impl From<Transferable> for Reference {
    fn from(value: Transferable) -> Self {
        value.reproduce()
    }
}

#[derive(Debug)]
enum TransferableObject {
    Array {
        object: Object,
        items: Vec<Option<usize>>,
    },
    Map {
        object: Object,
        pairs: HashMap<String, Option<usize>>,
    },
    Object {
        object: Object,
        fields: HashMap<String, Option<usize>>,
    },
}

#[derive(Debug)]
enum TransferableReference {
    Array {
        reference: Reference,
        items: Vec<Option<usize>>,
    },
    Map {
        reference: Reference,
        pairs: HashMap<String, Option<usize>>,
    },
    Object {
        reference: Reference,
        fields: HashMap<String, Option<usize>>,
    },
}

impl From<TransferableObject> for TransferableReference {
    fn from(value: TransferableObject) -> Self {
        match value {
            TransferableObject::Array { object, items } => TransferableReference::Array {
                reference: Reference::new_raw(object),
                items,
            },
            TransferableObject::Map { object, pairs } => TransferableReference::Map {
                reference: Reference::new_raw(object),
                pairs,
            },
            TransferableObject::Object { object, fields } => TransferableReference::Object {
                reference: Reference::new_raw(object),
                fields,
            },
        }
    }
}

impl TransferableReference {
    fn reference(&self) -> Reference {
        match self {
            TransferableReference::Array { reference, .. }
            | TransferableReference::Map { reference, .. }
            | TransferableReference::Object { reference, .. } => reference.clone(),
        }
    }
}

/// A whole graph of references, packed up to cross a thread.
///
/// A [`Reference`] is single threaded. This is the only way to move one to
/// another thread. Building a [`Transferable`] walks the graph the reference
/// leads to, takes every object out, and leaves a [`Transferred`] marker in its
/// place, so the source thread can no longer reach any of them. Turning it back
/// into a [`Reference`] on the other thread rebuilds the graph, links included.
///
/// A value whose type is not `Send` stops the walk, and the reference to it
/// comes back null.
#[derive(Debug)]
pub struct Transferable {
    /// { reference's object address as its unique ID: object behind reference}
    objects: HashMap<usize, TransferableObject>,
    root: Option<usize>,
}

unsafe impl Send for Transferable {}
unsafe impl Sync for Transferable {}

impl Transferable {
    fn produce(
        value: Reference,
        objects: &mut HashMap<usize, TransferableObject>,
    ) -> Option<usize> {
        let mut object = match unsafe { value.transfer() } {
            Some(object) => match object {
                Ok(object) => object,
                Err(address) => return Some(address),
            },
            None => return None,
        };
        let address = unsafe { object.as_ptr() as usize };
        if objects.iter().any(|object| *object.0 == address) {
            return Some(address);
        }
        if let Some(array) = object.write::<Array>() {
            let items = array
                .iter_mut()
                .map(|value| Self::produce(std::mem::replace(value, Reference::null()), objects))
                .collect();
            objects.insert(address, TransferableObject::Array { object, items });
        } else if let Some(map) = object.write::<Map>() {
            let pairs = map
                .iter_mut()
                .map(|(key, value)| {
                    (
                        key.to_owned(),
                        Self::produce(std::mem::replace(value, Reference::null()), objects),
                    )
                })
                .collect();
            objects.insert(address, TransferableObject::Map { object, pairs });
        } else {
            match &*object.type_handle().clone() {
                intuicio_core::types::Type::Struct(type_) => {
                    let fields = type_
                        .fields()
                        .iter()
                        .filter_map(|field| {
                            let value = object.write_field::<Reference>(&field.name)?;
                            Some((
                                field.name.to_owned(),
                                Self::produce(std::mem::replace(value, Reference::null()), objects),
                            ))
                        })
                        .collect();
                    objects.insert(address, TransferableObject::Object { object, fields });
                }
                intuicio_core::types::Type::Enum(type_) => {
                    let discriminant = unsafe { object.as_ptr().read() };
                    if let Some(variant) = type_.find_variant_by_discriminant(discriminant) {
                        let fields = variant
                            .fields
                            .iter()
                            .filter_map(|field| {
                                let value = object.write_field::<Reference>(&field.name)?;
                                Some((
                                    field.name.to_owned(),
                                    Self::produce(
                                        std::mem::replace(value, Reference::null()),
                                        objects,
                                    ),
                                ))
                            })
                            .collect();
                        objects.insert(address, TransferableObject::Object { object, fields });
                    }
                }
            }
        }
        Some(address)
    }

    fn reproduce(self) -> Reference {
        let Some(root) = self.root else {
            return Reference::null();
        };
        let mut results = self
            .objects
            .into_iter()
            .map(|(address, object)| (address, TransferableReference::from(object)))
            .collect::<HashMap<_, _>>();
        let references = results
            .iter()
            .map(|(address, reference)| (*address, reference.reference()))
            .collect::<HashMap<_, _>>();
        for reference in results.values_mut() {
            match reference {
                TransferableReference::Array { reference, items } => {
                    if let Some(mut array) = reference.write::<Array>() {
                        for (index, value) in array.iter_mut().enumerate() {
                            if let Some(address) = items.get(index) {
                                *value = address
                                    .and_then(|address| references.get(&address).cloned())
                                    .unwrap_or_default();
                            } else {
                                *value = Reference::null();
                            }
                        }
                    }
                }
                TransferableReference::Map { reference, pairs } => {
                    if let Some(mut map) = reference.write::<Map>() {
                        for (key, value) in map.iter_mut() {
                            if let Some(address) = pairs.get(key) {
                                *value = address
                                    .and_then(|address| references.get(&address).cloned())
                                    .unwrap_or_default();
                            } else {
                                *value = Reference::null();
                            }
                        }
                    }
                }
                TransferableReference::Object { reference, fields } => {
                    if let Some(mut object) = reference.write_object() {
                        match &**object.type_handle() {
                            intuicio_core::types::Type::Struct(type_) => {
                                let names = type_
                                    .fields()
                                    .iter()
                                    .map(|field| field.name.to_owned())
                                    .collect::<Vec<_>>();
                                for name in names {
                                    if let Some(value) = object.write_field::<Reference>(&name) {
                                        if let Some(address) = fields.get(&name) {
                                            *value = address
                                                .and_then(|address| {
                                                    references.get(&address).cloned()
                                                })
                                                .unwrap_or_default();
                                        } else {
                                            *value = Reference::null();
                                        }
                                    }
                                }
                            }
                            intuicio_core::types::Type::Enum(type_) => {
                                let discriminant = unsafe { object.as_ptr().read() };
                                if let Some(variant) =
                                    type_.find_variant_by_discriminant(discriminant)
                                {
                                    let names = variant
                                        .fields
                                        .iter()
                                        .map(|field| field.name.to_owned())
                                        .collect::<Vec<_>>();
                                    for name in names {
                                        if let Some(value) = object.write_field::<Reference>(&name)
                                        {
                                            if let Some(address) = fields.get(&name) {
                                                *value = address
                                                    .and_then(|address| {
                                                        references.get(&address).cloned()
                                                    })
                                                    .unwrap_or_default();
                                            } else {
                                                *value = Reference::null();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        references.get(&root).cloned().unwrap_or_default()
    }
}

impl From<Reference> for Transferable {
    fn from(value: Reference) -> Self {
        let mut objects = Default::default();
        let root = Transferable::produce(value, &mut objects);
        Self { objects, root }
    }
}

#[derive(Debug, Default)]
/// Marker left behind by a value that moved to another thread.
///
/// It holds the old address, which is what links the graph back together on
/// the other side. A reference holding one answers `true` to
/// [`Reference::is_transferred`] and can no longer be read.
pub struct Transferred(usize);

/// Registers every type of this crate, so scripts can hold values of them.
///
/// [`Reference`] itself is registered as neither `Send` nor `Sync`, which is
/// what keeps it on one thread.
pub fn install(registry: &mut Registry) {
    registry.add_type(define_native_struct! {
        registry => mod reflect struct Reference (Reference) {}
    });
    registry.add_type(define_native_struct! {
        registry => mod reflect struct Type (Type) {}
        [override_send = true]
        [override_sync = true]
    });
    registry.add_type(define_native_struct! {
        registry => mod reflect struct Function (Function) {}
        [override_send = true]
        [override_sync = true]
    });
    registry.add_type(define_native_struct! {
        registry => mod math struct Boolean (Boolean) {}
        [override_send = true]
        [override_sync = true]
        [override_copy = true]
    });
    registry.add_type(define_native_struct! {
        registry => mod math struct Integer (Integer) {}
        [override_send = true]
        [override_sync = true]
        [override_copy = true]
    });
    registry.add_type(define_native_struct! {
        registry => mod math struct Real (Real) {}
        [override_send = true]
        [override_sync = true]
        [override_copy = true]
    });
    registry.add_type(define_native_struct! {
        registry => mod math struct Text (Text) {}
        [override_send = true]
        [override_sync = true]
    });
    registry.add_type(define_native_struct! {
        registry => mod math struct Array (Array) {}
    });
    registry.add_type(define_native_struct! {
        registry => mod math struct Map (Map) {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use intuicio_core::{IntuicioEnum, IntuicioStruct, registry::Registry};
    use intuicio_derive::*;
    use std::thread::spawn;

    #[test]
    fn test_threading() {
        #[derive(IntuicioStruct, Default)]
        #[intuicio(name = "Foo", module_name = "test", override_send = true)]
        struct Foo {
            pub v: Reference,
            pub me: Reference,
        }

        #[derive(IntuicioEnum, Default)]
        #[intuicio(name = "Bar", module_name = "test", override_send = true)]
        #[repr(u8)]
        enum Bar {
            #[default]
            A,
            B(Reference),
        }

        let mut registry = Registry::default();
        crate::install(&mut registry);
        let foo_type = registry.add_type(Foo::define_struct(&registry));
        assert!(foo_type.is_send());
        let bar_type = registry.add_type(Bar::define_enum(&registry));
        assert!(bar_type.is_send());

        let mut value = Reference::new(
            Foo {
                v: Reference::new(Bar::B(Reference::new(0 as Integer, &registry)), &registry),
                me: Default::default(),
            },
            &registry,
        );
        let me = value.clone();
        value.write::<Foo>().unwrap().me = me;
        let transferable = Transferable::from(value.clone());
        assert!(value.is_transferred());

        let handle = spawn(|| {
            let mut registry = Registry::default();
            crate::install(&mut registry);
            let object = Reference::from(transferable);

            // we need to keep it in scope, because references being
            // actively written are not able to be transferred.
            {
                let mut value = object.clone();
                let mut value = value.write::<Foo>().unwrap();
                let mut value = value.v.write::<Bar>().unwrap();
                if let Bar::B(value) = &mut *value {
                    let mut value = value.write::<Integer>().unwrap();
                    while *value < 42 {
                        *value += 1;
                    }
                }
            }

            Transferable::from(object)
        });

        let object = Reference::from(handle.join().unwrap());
        assert!(!object.is_null());
        assert!(object.type_of().unwrap().is::<Foo>());
        let value = object.read::<Foo>().unwrap();
        assert!(!value.v.is_null());
        assert!(value.v.type_of().unwrap().is::<Bar>());
        if let Bar::B(value) = &*value.v.read::<Bar>().unwrap() {
            assert!(value.type_of().unwrap().is::<Integer>());
            assert_eq!(*value.read::<Integer>().unwrap(), 42);
        }
        assert!(!value.me.is_null());
        assert!(value.me.type_of().unwrap().is::<Foo>());
        assert!(value.me.does_share_reference(&object, true));
    }
}

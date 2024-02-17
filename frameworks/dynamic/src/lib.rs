use intuicio_core::{
    define_native_struct,
    function::{FunctionHandle, FunctionQuery},
    object::Object,
    registry::Registry,
    struct_type::{NativeStructBuilder, StructHandle, StructQuery},
};
use intuicio_data::{shared::Shared, type_hash::TypeHash};
use std::{
    cell::{Ref, RefMut},
    collections::HashMap,
};

pub type Boolean = bool;
pub type Integer = i64;
pub type Real = f64;
pub type Text = String;
pub type Array = Vec<Reference>;
pub type Map = HashMap<Text, Reference>;

thread_local! {
    static TRANSFERRED_STRUCT_HANDLE: StructHandle = NativeStructBuilder::new::<Transferred>().build_handle();
}

#[derive(Default, Clone)]
pub struct Type {
    data: Option<StructHandle>,
}

impl Type {
    pub fn by_name(name: &str, module_name: &str, registry: &Registry) -> Option<Self> {
        Some(Self::new(registry.find_struct(StructQuery {
            name: Some(name.into()),
            module_name: Some(module_name.into()),
            ..Default::default()
        })?))
    }

    pub fn of<T: 'static>(registry: &Registry) -> Option<Self> {
        Some(Self::new(registry.find_struct(StructQuery {
            type_hash: Some(TypeHash::of::<T>()),
            ..Default::default()
        })?))
    }

    pub fn new(handle: StructHandle) -> Self {
        Self { data: Some(handle) }
    }

    pub fn handle(&self) -> Option<&StructHandle> {
        self.data.as_ref()
    }

    pub fn is<T: 'static>(&self) -> bool {
        self.data
            .as_ref()
            .map(|data| data.type_hash() == TypeHash::of::<T>())
            .unwrap_or(false)
    }

    pub fn is_same_as(&self, other: &Self) -> bool {
        if let (Some(this), Some(other)) = (self.data.as_ref(), other.data.as_ref()) {
            this == other
        } else {
            false
        }
    }

    pub fn type_hash(&self) -> Option<TypeHash> {
        Some(self.data.as_ref()?.type_hash())
    }
}

#[derive(Default, Clone)]
pub struct Function {
    data: Option<FunctionHandle>,
}

impl Function {
    pub fn by_name(name: &str, module_name: &str, registry: &Registry) -> Option<Self> {
        Some(Self::new(registry.find_function(FunctionQuery {
            name: Some(name.into()),
            module_name: Some(module_name.into()),
            ..Default::default()
        })?))
    }

    pub fn new(handle: FunctionHandle) -> Self {
        Self { data: Some(handle) }
    }

    pub fn handle(&self) -> Option<&FunctionHandle> {
        self.data.as_ref()
    }

    pub fn is_same_as(&self, other: &Self) -> bool {
        if let (Some(this), Some(other)) = (self.data.as_ref(), other.data.as_ref()) {
            this.signature() == other.signature()
        } else {
            false
        }
    }
}

#[derive(Default, Clone)]
pub struct Reference {
    data: Option<Shared<Object>>,
}

impl Reference {
    pub fn null() -> Self {
        Self { data: None }
    }

    pub fn is_null(&self) -> bool {
        self.data.is_none()
    }

    pub fn is_transferred(&self) -> bool {
        self.data
            .as_ref()
            .and_then(|data| data.read())
            .map(|data| data.read::<Transferred>().is_some())
            .unwrap_or_default()
    }

    pub fn is_being_written(&mut self) -> bool {
        self.data
            .as_mut()
            .map(|data| data.write().is_none())
            .unwrap_or_default()
    }

    pub fn new_boolean(value: Boolean, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    pub fn new_integer(value: Integer, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    pub fn new_real(value: Real, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    pub fn new_text(value: Text, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    pub fn new_array(value: Array, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    pub fn new_map(value: Map, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    pub fn new_type(value: Type, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    pub fn new_function(value: Function, registry: &Registry) -> Self {
        Self::new(value, registry)
    }

    pub fn new<T: 'static>(data: T, registry: &Registry) -> Self {
        let struct_type = registry
            .find_struct(StructQuery::of::<T>())
            .unwrap_or_else(|| {
                panic!(
                    "Could not make a reference of type: {}",
                    std::any::type_name::<T>()
                )
            });
        let mut value = unsafe { Object::new_uninitialized(struct_type) };
        unsafe { value.as_mut_ptr().cast::<T>().write(data) };
        Self::new_raw(value)
    }

    pub fn new_custom<T: 'static>(data: T, ty: &Type) -> Self {
        let mut value = unsafe { Object::new_uninitialized(ty.data.as_ref().unwrap().clone()) };
        unsafe { value.as_mut_ptr().cast::<T>().write(data) };
        Self::new_raw(value)
    }

    pub fn new_raw(data: Object) -> Self {
        Self {
            data: Some(Shared::new(data)),
        }
    }

    pub fn new_shared(data: Shared<Object>) -> Self {
        Self { data: Some(data) }
    }

    pub fn initialized(ty: &Type) -> Self {
        Self::new_raw(Object::new(ty.data.as_ref().unwrap().clone()))
    }

    /// # Safety
    pub unsafe fn uninitialized(ty: &Type) -> Self {
        Self::new_raw(Object::new_uninitialized(ty.data.as_ref().unwrap().clone()))
    }

    pub fn type_of(&self) -> Option<Type> {
        Some(Type::new(
            self.data.as_ref()?.read()?.struct_handle().clone(),
        ))
    }

    pub fn read<T: 'static>(&self) -> Option<Ref<T>> {
        let result = self.data.as_ref()?.read()?;
        if result.struct_handle().type_hash() == TypeHash::of::<T>() {
            Some(Ref::map(result, |data| data.read::<T>().unwrap()))
        } else {
            None
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<RefMut<T>> {
        let result = self.data.as_mut()?.write()?;
        if result.struct_handle().type_hash() == TypeHash::of::<T>() {
            Some(RefMut::map(result, |data| data.write::<T>().unwrap()))
        } else {
            None
        }
    }

    pub fn read_object(&self) -> Option<Ref<Object>> {
        self.data.as_ref()?.read()
    }

    pub fn write_object(&mut self) -> Option<RefMut<Object>> {
        self.data.as_mut()?.write()
    }

    pub fn swap<T: 'static>(&mut self, data: T) -> Option<T> {
        Some(std::mem::replace(
            self.data.as_mut()?.write()?.write::<T>()?,
            data,
        ))
    }

    pub fn try_consume(self) -> Result<Object, Self> {
        match self.data {
            Some(data) => match data.try_consume() {
                Ok(data) => Ok(data),
                Err(data) => Err(Self { data: Some(data) }),
            },
            None => Err(Self::null()),
        }
    }

    pub fn references_count(&self) -> usize {
        self.data
            .as_ref()
            .map(|data| data.references_count())
            .unwrap_or(0)
    }

    pub fn does_share_reference(&self, other: &Self, consider_null: bool) -> bool {
        match (self.data.as_ref(), other.data.as_ref()) {
            (Some(this), Some(other)) => this.does_share_reference(other),
            (None, None) => consider_null,
            _ => false,
        }
    }

    /// # Safety
    pub unsafe fn transfer(&self) -> Option<Result<Object, usize>> {
        let mut data = self.data.as_ref()?.write()?;
        if let Some(data) = data.read::<Transferred>() {
            return Some(Err(data.0));
        }
        if !data.struct_handle().is_send() {
            return None;
        }
        let mut object =
            Object::new_uninitialized(TRANSFERRED_STRUCT_HANDLE.with(|handle| handle.clone()));
        object
            .as_mut_ptr()
            .cast::<Transferred>()
            .write(Transferred(data.as_ptr() as usize));
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

/// Normally references are single-threaded, but they can be sent between threads
/// only by means of transfer mechanism. Transfer mechanism works like this:
/// For transferred reference, we construct graph of connected unpacked objects,
/// replacing their original content objects with special Transferred type, so they
/// cannot be accessed later in original thread. We send that graph and on the other
/// thread we reconstruct objects and references from that graph and return main one.
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
            let fields = object
                .struct_handle()
                .clone()
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
                        let names = object
                            .struct_handle()
                            .fields()
                            .iter()
                            .map(|field| field.name.to_owned())
                            .collect::<Vec<_>>();
                        for name in names {
                            if let Some(value) = object.write_field::<Reference>(&name) {
                                if let Some(address) = fields.get(&name) {
                                    *value = address
                                        .and_then(|address| references.get(&address).cloned())
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
pub struct Transferred(usize);

pub fn install(registry: &mut Registry) {
    registry.add_struct(define_native_struct! {
        registry => mod reflect struct Reference (Reference) {}
        [override_send = true]
    });
    registry.add_struct(define_native_struct! {
        registry => mod reflect struct Type (Type) {}
    });
    registry.add_struct(define_native_struct! {
        registry => mod reflect struct Function (Function) {}
    });
    registry.add_struct(define_native_struct! {
        registry => mod math struct Boolean (Boolean) {}
    });
    registry.add_struct(define_native_struct! {
        registry => mod math struct Integer (Integer) {}
    });
    registry.add_struct(define_native_struct! {
        registry => mod math struct Real (Real) {}
    });
    registry.add_struct(define_native_struct! {
        registry => mod math struct Text (Text) {}
    });
    registry.add_struct(define_native_struct! {
        registry => mod math struct Array (Array) {}
    });
    registry.add_struct(define_native_struct! {
        registry => mod math struct Map (Map) {}
    });
}

#[cfg(test)]
mod tests {
    use crate::{Integer, Reference, Transferable};
    use intuicio_core::prelude::*;
    use intuicio_derive::IntuicioStruct;
    use std::thread::spawn;

    #[test]
    fn test_threading() {
        #[derive(IntuicioStruct, Default)]
        #[intuicio(name = "Foo", module_name = "test", override_send = true)]
        struct Foo {
            pub v: Reference,
            pub me: Reference,
        }

        let mut registry = Registry::default();
        crate::install(&mut registry);
        let foo_type = registry.add_struct(Foo::define_struct(&registry));
        assert!(foo_type.is_send());

        let mut value = Reference::new(
            Foo {
                v: Reference::new(0 as Integer, &registry),
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
                let mut value = value.v.write::<Integer>().unwrap();
                while *value < 42 {
                    *value += 1;
                }
            }

            Transferable::from(object)
        });

        let object = Reference::from(handle.join().unwrap());
        assert_eq!(object.is_null(), false);
        assert!(object.type_of().unwrap().is::<Foo>());
        let value = object.read::<Foo>().unwrap();
        assert_eq!(value.v.is_null(), false);
        assert!(value.v.type_of().unwrap().is::<Integer>());
        assert_eq!(*value.v.read::<Integer>().unwrap(), 42);
        assert_eq!(value.me.is_null(), false);
        assert!(value.me.type_of().unwrap().is::<Foo>());
        assert!(value.me.does_share_reference(&object, true));
    }
}

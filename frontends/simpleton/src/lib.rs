pub mod library;
pub mod parser;
pub mod script;

pub mod prelude {
    pub use crate::{library::*, script::*, *};
}

use intuicio_core::{
    crate_version,
    function::{FunctionHandle, FunctionQuery},
    object::Object,
    registry::Registry,
    struct_type::{NativeStructBuilder, StructHandle, StructQuery},
    IntuicioVersion,
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

pub fn frontend_simpleton_version() -> IntuicioVersion {
    crate_version!()
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
            .unwrap()
            .clone();
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

    pub fn initialized(ty: &Type) -> Self {
        Self::new_raw(Object::new(ty.data.as_ref().unwrap().clone()))
    }

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

    pub unsafe fn transfer(&self) -> Option<Result<Object, usize>> {
        let mut data = self.data.as_ref()?.write()?;
        if let Some(data) = data.read::<Transferred>() {
            return Some(Err(data.0));
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

impl From<Transferable> for Reference {
    fn from(mut value: Transferable) -> Self {
        let mut references = Default::default();
        value.tree.reproduce(&mut value.objects, &mut references)
    }
}

#[derive(Debug)]
enum TransferableTree {
    Null,
    Array {
        address: usize,
        items: Vec<TransferableTree>,
    },
    Map {
        address: usize,
        pairs: Vec<(String, TransferableTree)>,
    },
    Object {
        address: usize,
        fields: Vec<(String, TransferableTree)>,
    },
    Link {
        address: usize,
    },
}

impl TransferableTree {
    fn produce(value: Reference, objects: &mut HashMap<usize, Object>) -> TransferableTree {
        let mut object = match unsafe { value.transfer() } {
            Some(object) => match object {
                Ok(object) => object,
                Err(address) => return TransferableTree::Link { address },
            },
            None => return TransferableTree::Null,
        };
        let address = unsafe { object.as_ptr() as usize };
        if objects.iter().any(|object| *object.0 == address) {
            return TransferableTree::Link { address };
        }
        let result = if let Some(array) = object.write::<Array>() {
            TransferableTree::Array {
                address,
                items: array
                    .iter_mut()
                    .map(|value| {
                        Self::produce(std::mem::replace(value, Reference::null()), objects)
                    })
                    .collect(),
            }
        } else if let Some(map) = object.write::<Map>() {
            TransferableTree::Map {
                address,
                pairs: map
                    .iter_mut()
                    .map(|(key, value)| {
                        (
                            key.to_owned(),
                            Self::produce(std::mem::replace(value, Reference::null()), objects),
                        )
                    })
                    .collect(),
            }
        } else {
            let ty = object.struct_handle().clone();
            TransferableTree::Object {
                address,
                fields: ty
                    .fields()
                    .iter()
                    .filter_map(|field| {
                        let value = object.write_field::<Reference>(&field.name)?;
                        Some((
                            field.name.to_owned(),
                            Self::produce(std::mem::replace(value, Reference::null()), objects),
                        ))
                    })
                    .collect(),
            }
        };
        objects.insert(address, object);
        result
    }

    fn reproduce(
        self,
        objects: &mut HashMap<usize, Object>,
        references: &mut HashMap<usize, Reference>,
    ) -> Reference {
        match self {
            Self::Null => Reference::null(),
            Self::Array { address, items } => {
                if let Some(result) = references.get(&address) {
                    return result.clone();
                }
                let mut object = objects.remove(&address).unwrap();
                if let Some(array) = object.write::<Array>() {
                    for (item, value) in items.into_iter().zip(array.iter_mut()) {
                        let _ = std::mem::replace(value, item.reproduce(objects, references));
                    }
                }
                let result = Reference::new_raw(object);
                references.insert(address, result.clone());
                result
            }
            Self::Map { address, pairs } => {
                if let Some(result) = references.get(&address) {
                    return result.clone();
                }
                let mut object = objects.remove(&address).unwrap();
                if let Some(map) = object.write::<Map>() {
                    for (key, item) in pairs {
                        if let Some(value) = map.get_mut(&key) {
                            let _ = std::mem::replace(value, item.reproduce(objects, references));
                        }
                    }
                }
                let result = Reference::new_raw(object);
                references.insert(address, result.clone());
                result
            }
            Self::Object { address, fields } => {
                if let Some(result) = references.get(&address) {
                    return result.clone();
                }
                let mut object = objects.remove(&address).unwrap();
                for (name, item) in fields {
                    if let Some(value) = object.write_field::<Reference>(&name) {
                        let _ = std::mem::replace(value, item.reproduce(objects, references));
                    }
                }
                let result = Reference::new_raw(object);
                references.insert(address, result.clone());
                result
            }
            Self::Link { address } => references
                .get(&address)
                .map(|value| value.clone())
                .unwrap_or_default(),
        }
    }
}

pub struct Transferable {
    /// { reference's object address as its unique ID: object behind reference}
    objects: HashMap<usize, Object>,
    tree: TransferableTree,
}

impl From<Reference> for Transferable {
    fn from(value: Reference) -> Self {
        let mut objects = Default::default();
        let tree = TransferableTree::produce(value, &mut objects);
        Self { objects, tree }
    }
}

impl std::fmt::Debug for Transferable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Object<'a> {
            name: &'a str,
            module_name: Option<&'a str>,
        }

        f.debug_struct("Transferable")
            .field(
                "objects",
                &self
                    .objects
                    .iter()
                    .map(|(key, value)| {
                        (
                            key,
                            Object {
                                name: &value.struct_handle().name,
                                module_name: value.struct_handle().module_name.as_deref(),
                            },
                        )
                    })
                    .collect::<HashMap<_, _>>(),
            )
            .field("tree", &self.tree)
            .finish()
    }
}

#[derive(Debug, Default)]
pub struct Transferred(usize);

#[cfg(test)]
mod tests {
    use crate::{
        library::{array, jobs::Jobs, ObjectBuilder},
        script::{SimpletonContentParser, SimpletonPackage, SimpletonScriptExpression},
        Integer, Real, Reference, Transferable,
    };
    use intuicio_backend_vm::prelude::*;
    use intuicio_core::prelude::*;
    use std::thread::spawn;

    #[test]
    fn test_transfer() {
        let mut registry = Registry::default();
        crate::library::install(&mut registry);

        let a = Reference::new_integer(42, &registry);
        let b = Reference::new_array(vec![], &registry);
        array::push(b.clone(), a.clone());
        array::push(b.clone(), b.clone());
        let transferable = Transferable::from(b.clone());
        assert!(a.is_transferred());
        assert!(b.is_transferred());
        let result = Reference::from(transferable);
        assert!(!result.is_null());
        assert!(!result.is_transferred());
    }

    #[test]
    fn test_threading() {
        let mut registry = Registry::default();
        crate::library::install(&mut registry);

        // TODO: test if cycles make freeze.
        let value = Reference::new_integer(42, &registry);
        let transferable = Transferable::from(value.clone());
        assert!(value.is_transferred());
        let handle = spawn(|| {
            let mut registry = Registry::default();
            crate::library::install(&mut registry);
            let mut object = Reference::from(transferable);
            let mut value = object.write::<Integer>().unwrap();
            while *value < 42 {
                *value += 1;
            }
            drop(value);
            Transferable::from(object)
        });
        let value = Reference::from(handle.join().unwrap());
        assert_eq!(*value.read::<Integer>().unwrap(), 42);
    }

    #[test]
    fn test_simpleton_script() {
        let mut content_provider = FileContentProvider::new("simp", SimpletonContentParser);
        let package =
            SimpletonPackage::new("../../resources/package.simp", &mut content_provider).unwrap();
        let host_producer = HostProducer::new(move || {
            let mut registry = Registry::default();
            crate::library::install(&mut registry);
            package
                .compile()
                .install::<VmScope<SimpletonScriptExpression>>(
                    &mut registry,
                    None,
                    // Some(
                    //     PrintDebugger::full()
                    //         .basic_printables()
                    //         .stack_bytes(false)
                    //         .registers_bytes(false)
                    //         .into_handle(),
                    // ),
                );
            let context = Context::new(1024, 1024, 1024);
            Host::new(context, registry.into())
        });
        let mut vm = host_producer.produce();
        vm.context()
            .set_custom(Jobs::HOST_PRODUCER_CUSTOM, host_producer);

        let adder = Reference::new_raw(
            ObjectBuilder::new("Adder", "adder")
                .field("a", Reference::new_integer(40, vm.registry()))
                .field("b", Reference::new_integer(2, vm.registry()))
                .build(vm.registry()),
        );
        let (result,) = vm
            .call_function::<(Reference,), _>("add", "adder", None)
            .unwrap()
            .run((adder,));
        assert_eq!(vm.context().stack().position(), 0);
        assert_eq!(*result.read::<Integer>().unwrap(), 42);

        let (result,) = vm
            .call_function::<(Reference,), _>("main", "test", None)
            .unwrap()
            .run(());
        assert_eq!(vm.context().stack().position(), 0);
        assert_eq!(*result.read::<Real>().unwrap(), 42.0);
    }
}

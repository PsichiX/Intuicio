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
    struct_type::{StructHandle, StructQuery},
    IntuicioVersion,
};
use intuicio_data::shared::Shared;
use std::{
    any::TypeId,
    cell::{Ref, RefMut},
    collections::HashMap,
};

pub type Boolean = bool;
pub type Integer = i64;
pub type Real = f64;
pub type Text = String;
pub type Array = Vec<Reference>;
pub type Map = HashMap<Text, Reference>;

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
            type_id: Some(TypeId::of::<T>()),
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
            .map(|data| data.type_id() == TypeId::of::<T>())
            .unwrap_or(false)
    }

    pub fn is_same_as(&self, other: &Self) -> bool {
        if let (Some(this), Some(other)) = (self.data.as_ref(), other.data.as_ref()) {
            this == other
        } else {
            false
        }
    }

    pub fn type_id(&self) -> Option<TypeId> {
        Some(self.data.as_ref()?.type_id())
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
        *value.write().unwrap() = data;
        Self::new_raw(value)
    }

    pub fn new_custom<T: 'static>(data: T, ty: &Type) -> Self {
        let mut value = unsafe { Object::new_uninitialized(ty.data.as_ref().unwrap().clone()) };
        *value.write().unwrap() = data;
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
        if result.struct_handle().type_id() == TypeId::of::<T>() {
            Some(Ref::map(result, |data| data.read::<T>().unwrap()))
        } else {
            None
        }
    }

    pub fn write<T: 'static>(&mut self) -> Option<RefMut<T>> {
        let result = self.data.as_mut()?.write()?;
        if result.struct_handle().type_id() == TypeId::of::<T>() {
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
}

#[cfg(test)]
mod tests {
    use crate::{
        library::ObjectBuilder,
        script::{SimpletonContentParser, SimpletonPackage, SimpletonScriptExpression},
        Integer, Real, Reference,
    };
    use intuicio_backend_vm::prelude::*;
    use intuicio_core::prelude::*;

    #[test]
    fn test_simpleton_script() {
        let mut registry = Registry::default();
        crate::library::install(&mut registry);

        let mut content_provider = FileContentProvider::new("simp", SimpletonContentParser);
        SimpletonPackage::new("../../resources/package.simp", &mut content_provider)
            .unwrap()
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
        let mut vm = Host::new(Context::new(1024, 1024, 1024), registry.into());

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

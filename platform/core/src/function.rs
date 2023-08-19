use crate::{
    context::Context,
    meta::Meta,
    registry::Registry,
    struct_type::{StructHandle, StructQuery},
    Visibility,
};
use intuicio_data::data_stack::DataStackPack;
use std::{borrow::Cow, sync::Arc};

pub type FunctionHandle = Arc<Function>;
pub type FunctionMetaQuery = fn(&Meta) -> bool;

pub enum FunctionBody {
    Pointer(fn(&mut Context, &Registry)),
    #[allow(clippy::type_complexity)]
    Closure(Arc<dyn Fn(&mut Context, &Registry) + Send + Sync>),
}

impl FunctionBody {
    pub fn pointer(pointer: fn(&mut Context, &Registry)) -> Self {
        Self::Pointer(pointer)
    }

    pub fn closure<T>(closure: T) -> Self
    where
        T: Fn(&mut Context, &Registry) + Send + Sync + 'static,
    {
        Self::Closure(Arc::new(closure))
    }

    pub fn invoke(&self, context: &mut Context, registry: &Registry) {
        match self {
            Self::Pointer(pointer) => pointer(context, registry),
            Self::Closure(closure) => closure(context, registry),
        }
    }
}

impl std::fmt::Debug for FunctionBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pointer(_) => write!(f, "<Pointer>"),
            Self::Closure(_) => write!(f, "<Closure>"),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct FunctionParameter {
    pub meta: Option<Meta>,
    pub name: String,
    pub struct_handle: StructHandle,
}

impl FunctionParameter {
    pub fn new(name: impl ToString, struct_handle: StructHandle) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            struct_handle,
        }
    }
}

impl std::fmt::Debug for FunctionParameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionParameter")
            .field("meta", &self.meta)
            .field("name", &self.name)
            .field("struct_handle", &self.struct_handle.name)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct FunctionSignature {
    pub meta: Option<Meta>,
    pub name: String,
    pub module_name: Option<String>,
    pub struct_handle: Option<StructHandle>,
    pub visibility: Visibility,
    pub inputs: Vec<FunctionParameter>,
    pub outputs: Vec<FunctionParameter>,
}

impl FunctionSignature {
    pub fn new(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            module_name: None,
            struct_handle: None,
            visibility: Visibility::default(),
            inputs: vec![],
            outputs: vec![],
        }
    }

    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    pub fn with_module_name(mut self, name: impl ToString) -> Self {
        self.module_name = Some(name.to_string());
        self
    }

    pub fn with_struct_handle(mut self, handle: StructHandle) -> Self {
        self.struct_handle = Some(handle);
        self
    }

    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn with_input(mut self, parameter: FunctionParameter) -> Self {
        self.inputs.push(parameter);
        self
    }

    pub fn with_output(mut self, parameter: FunctionParameter) -> Self {
        self.outputs.push(parameter);
        self
    }
}

impl std::fmt::Debug for FunctionSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionSignature")
            .field("meta", &self.meta)
            .field("name", &self.name)
            .field("module_name", &self.module_name)
            .field(
                "struct_handle",
                match self.struct_handle.as_ref() {
                    Some(struct_handle) => &struct_handle.name,
                    None => &"!",
                },
            )
            .field("visibility", &self.visibility)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .finish()
    }
}

impl std::fmt::Display for FunctionSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(meta) = self.meta.as_ref() {
            write!(f, "#{} ", meta)?;
        }
        if let Some(module_name) = self.module_name.as_ref() {
            write!(f, "mod {} ", module_name)?;
        }
        if let Some(struct_handle) = self.struct_handle.as_ref() {
            write!(f, "struct {} ", struct_handle.type_name())?;
        }
        write!(f, "fn {}(", self.name)?;
        for (index, parameter) in self.inputs.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(
                f,
                "{}: {}",
                parameter.name,
                parameter.struct_handle.type_name()
            )?;
        }
        write!(f, ") -> (")?;
        for (index, parameter) in self.outputs.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(
                f,
                "{}: {}",
                parameter.name,
                parameter.struct_handle.type_name()
            )?;
        }
        write!(f, ")")
    }
}

#[derive(Debug)]
pub struct Function {
    signature: FunctionSignature,
    body: FunctionBody,
}

impl Function {
    pub fn new(signature: FunctionSignature, body: FunctionBody) -> Self {
        Self { signature, body }
    }

    pub fn signature(&self) -> &FunctionSignature {
        &self.signature
    }

    pub fn invoke(&self, context: &mut Context, registry: &Registry) {
        context.store_registers();
        self.body.invoke(context, registry);
        context.restore_registers();
    }

    pub fn call<O: DataStackPack, I: DataStackPack>(
        &self,
        context: &mut Context,
        registry: &Registry,
        inputs: I,
        verify: bool,
    ) -> O {
        if verify {
            let input_types = I::pack_types();
            if input_types.len() != self.signature.inputs.len() {
                panic!("Function: {} got wrong inputs number!", self.signature.name);
            }
            let output_types = O::pack_types();
            if output_types.len() != self.signature.outputs.len() {
                panic!(
                    "Function: {} got wrong outputs number!",
                    self.signature.name
                );
            }
            for (parameter, type_hash) in self.signature.inputs.iter().zip(input_types) {
                if parameter.struct_handle.type_hash() != type_hash {
                    panic!(
                        "Function: {} input parameter: {} got wrong value type!",
                        self.signature.name, parameter.name
                    );
                }
            }
            for (parameter, type_hash) in self.signature.outputs.iter().zip(output_types) {
                if parameter.struct_handle.type_hash() != type_hash {
                    panic!(
                        "Function: {} output parameter: {} got wrong value type!",
                        self.signature.name, parameter.name
                    );
                }
            }
        }
        inputs.stack_push_reversed(context.stack());
        self.invoke(context, registry);
        O::stack_pop(context.stack())
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct FunctionQueryParameter<'a> {
    pub name: Option<Cow<'a, str>>,
    pub struct_query: Option<StructQuery<'a>>,
}

impl<'a> FunctionQueryParameter<'a> {
    pub fn is_valid(&self, parameter: &FunctionParameter) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == parameter.name)
            .unwrap_or(true)
            && self
                .struct_query
                .as_ref()
                .map(|query| query.is_valid(&parameter.struct_handle))
                .unwrap_or(true)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct FunctionQuery<'a> {
    pub name: Option<Cow<'a, str>>,
    pub module_name: Option<Cow<'a, str>>,
    pub struct_query: Option<StructQuery<'a>>,
    pub visibility: Option<Visibility>,
    pub inputs: Cow<'a, [FunctionQueryParameter<'a>]>,
    pub outputs: Cow<'a, [FunctionQueryParameter<'a>]>,
    pub meta: Option<FunctionMetaQuery>,
}

impl<'a> FunctionQuery<'a> {
    pub fn is_valid(&self, signature: &FunctionSignature) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == signature.name)
            .unwrap_or(true)
            && self
                .module_name
                .as_ref()
                .map(|name| {
                    signature
                        .module_name
                        .as_ref()
                        .map(|module_name| name.as_ref() == module_name)
                        .unwrap_or(false)
                })
                .unwrap_or(true)
            && self
                .struct_query
                .as_ref()
                .map(|query| {
                    signature
                        .struct_handle
                        .as_ref()
                        .map(|handle| query.is_valid(handle))
                        .unwrap_or(false)
                })
                .unwrap_or(true)
            && self
                .visibility
                .map(|visibility| signature.visibility.is_visible(visibility))
                .unwrap_or(true)
            && self
                .inputs
                .iter()
                .zip(signature.inputs.iter())
                .all(|(query, parameter)| query.is_valid(parameter))
            && self
                .outputs
                .iter()
                .zip(signature.outputs.iter())
                .all(|(query, parameter)| query.is_valid(parameter))
            && self
                .meta
                .as_ref()
                .map(|query| signature.meta.as_ref().map(query).unwrap_or(false))
                .unwrap_or(true)
    }
}

#[macro_export]
macro_rules! function_signature {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        $(struct ($struct_type:ty))?
        fn
        $name:ident
        ($( $input_name:ident : $input_type:ty ),*)
        ->
        ($( $output_name:ident : $output_type:ty ),*)
    ) => {
        {
            let mut result = $crate::function::FunctionSignature::new(stringify!($name));
            $(
                result.module_name = Some(stringify!($module_name).to_owned());
            )?
            $(
                result.struct_handle = Some($registry.find_struct($crate::struct_type::StructQuery::of::<$struct_type>()).unwrap());
            )?
            $(
                result.inputs.push(
                    $crate::function::FunctionParameter::new(
                        stringify!($input_name).to_owned(),
                        $registry.find_struct($crate::struct_type::StructQuery::of::<$input_type>()).unwrap()
                    )
                );
            )*
            $(
                result.outputs.push(
                    $crate::function::FunctionParameter::new(
                        stringify!($output_name).to_owned(),
                        $registry.find_struct($crate::struct_type::StructQuery::of::<$output_type>()).unwrap()
                    )
                );
            )*
            result
        }
    };
}

#[macro_export]
macro_rules! define_function {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        $(struct ($struct_type:ty))?
        fn
        $name:ident
        ($( $input_name:ident : $input_type:ty),*)
        ->
        ($( $output_name:ident : $output_type:ty),*)
        $code:block
    ) => {
        $crate::function::Function::new(
            $crate::function_signature! {
                $registry
                =>
                $(mod $module_name)?
                $(struct ($struct_type))?
                fn
                $name
                ($($input_name : $input_type),*)
                ->
                ($($output_name : $output_type),*)
            },
            $crate::function::FunctionBody::closure(move |context, registry| {
                use intuicio_data::data_stack::DataStackPack;
                #[allow(unused_mut)]
                let ($(mut $input_name,)*) = <($($input_type,)*)>::stack_pop(context.stack());
                $code.stack_push_reversed(context.stack());
            }),
        )
    };
}

#[cfg(test)]
mod tests {
    use crate as intuicio_core;
    use crate::{context::*, function::*, registry::*, struct_type::*};
    use intuicio_data;
    use intuicio_derive::*;

    #[intuicio_function(meta = "foo")]
    fn function_meta() {}

    #[test]
    fn test_function() {
        fn add(context: &mut Context, _: &Registry) {
            let a = context.stack().pop::<i32>().unwrap();
            let b = context.stack().pop::<i32>().unwrap();
            context.stack().push(a + b);
        }

        let i32_handle = NativeStructBuilder::new::<i32>().build_handle();
        let signature = FunctionSignature::new("add")
            .with_input(FunctionParameter::new("a", i32_handle.clone()))
            .with_input(FunctionParameter::new("b", i32_handle.clone()))
            .with_output(FunctionParameter::new("result", i32_handle));
        let function = Function::new(signature.to_owned(), FunctionBody::pointer(add));

        assert_eq!(
            FunctionQuery {
                ..Default::default()
            }
            .is_valid(&signature),
            true
        );
        assert_eq!(
            FunctionQuery {
                name: Some("add".into()),
                ..Default::default()
            }
            .is_valid(&signature),
            true
        );
        assert_eq!(
            FunctionQuery {
                name: Some("add".into()),
                inputs: [
                    FunctionQueryParameter {
                        name: Some("a".into()),
                        ..Default::default()
                    },
                    FunctionQueryParameter {
                        name: Some("b".into()),
                        ..Default::default()
                    }
                ]
                .as_slice()
                .into(),
                outputs: [FunctionQueryParameter {
                    name: Some("result".into()),
                    ..Default::default()
                }]
                .as_slice()
                .into(),
                ..Default::default()
            }
            .is_valid(&signature),
            true
        );
        assert_eq!(
            FunctionQuery {
                name: Some("add".into()),
                inputs: [
                    FunctionQueryParameter {
                        name: Some("b".into()),
                        ..Default::default()
                    },
                    FunctionQueryParameter {
                        name: Some("a".into()),
                        ..Default::default()
                    }
                ]
                .as_slice()
                .into(),
                ..Default::default()
            }
            .is_valid(&signature),
            false
        );

        let mut context = Context::new(1024, 1024, 1024);
        let registry = Registry::default();

        context.stack().push(2);
        context.stack().push(40);
        function.invoke(&mut context, &registry);
        assert_eq!(context.stack().pop::<i32>().unwrap(), 42);

        assert_eq!(
            function_meta::define_signature(&registry).meta,
            Some(Meta::Identifier("foo".to_owned()))
        );
    }
}

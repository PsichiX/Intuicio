#![allow(unpredictable_function_pointer_comparisons)]

//! Callable units, and how to describe and find them.
//!
//! Every function, whether it came from Rust or from a script, is a
//! [`Function`]: a [`FunctionSignature`] describing it and a
//! [`FunctionBody`] doing the work. The body always has the same shape,
//! `fn(&mut Context, &Registry)`, which is why the caller cannot tell the two
//! kinds apart.
//!
//! # Argument order
//!
//! A body pops its arguments in declaration order, first to last, and pushes
//! its results in reverse. A caller therefore has to push arguments in reverse
//! order. [`Function::call`] does that for you, [`Function::invoke`] does not.
//! Prefer `call` unless you already manage the stack yourself.
use crate::{
    Filter, Visibility,
    context::Context,
    meta::Meta,
    registry::Registry,
    types::{Type, TypeHandle, TypeQuery},
};
use intuicio_data::data_stack::DataStackPack;
use rustc_hash::FxHasher;
use std::{
    borrow::Cow,
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Shared function, as a registry holds it.
pub type FunctionHandle = Arc<Function>;
/// Predicate over a function's metadata, used inside queries.
pub type FunctionMetaQuery = fn(&Meta) -> bool;

/// The code a function runs.
///
/// Both variants take the context to move data through and the registry to
/// look up anything else they need to call.
pub enum FunctionBody {
    /// A plain function pointer.
    Pointer(fn(&mut Context, &Registry)),
    /// A closure, for bodies that capture state such as a compiled script.
    #[allow(clippy::type_complexity)]
    Closure(Arc<dyn Fn(&mut Context, &Registry) + Send + Sync>),
}

impl FunctionBody {
    /// Wraps a function pointer.
    pub fn pointer(pointer: fn(&mut Context, &Registry)) -> Self {
        Self::Pointer(pointer)
    }

    /// Wraps a closure.
    pub fn closure<T>(closure: T) -> Self
    where
        T: Fn(&mut Context, &Registry) + Send + Sync + 'static,
    {
        Self::Closure(Arc::new(closure))
    }

    /// Runs the body. Prefer [`Function::invoke`], which also scopes registers.
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

/// One input or output of a function, with its name and type.
#[derive(Clone, PartialEq)]
pub struct FunctionParameter {
    /// Metadata attached to this parameter.
    pub meta: Option<Meta>,
    /// Parameter name.
    pub name: String,
    /// Type of the value this parameter carries.
    pub type_handle: TypeHandle,
}

impl FunctionParameter {
    /// Builds a parameter.
    pub fn new(name: impl ToString, type_handle: TypeHandle) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            type_handle,
        }
    }

    /// Attaches metadata, builder style.
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }
}

impl std::fmt::Debug for FunctionParameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionParameter")
            .field("meta", &self.meta)
            .field("name", &self.name)
            .field("type_handle", &self.type_handle.name())
            .finish()
    }
}

/// Everything about a function except its body.
///
/// The signature is also the identity of a function: a registry refuses to
/// hold two functions whose signatures are equal.
///
/// `type_handle` is set when the function belongs to a type, which is how
/// methods are modelled.
#[derive(Clone, PartialEq)]
pub struct FunctionSignature {
    /// Metadata attached to this function.
    pub meta: Option<Meta>,
    /// Name the function is registered under.
    pub name: String,
    /// Module the function belongs to.
    pub module_name: Option<String>,
    /// Type the function is a method of, if any.
    pub type_handle: Option<TypeHandle>,
    /// How widely the function is visible.
    pub visibility: Visibility,
    /// Arguments, in declaration order.
    pub inputs: Vec<FunctionParameter>,
    /// Results, in declaration order.
    pub outputs: Vec<FunctionParameter>,
}

impl FunctionSignature {
    /// Builds a signature with just a name.
    pub fn new(name: impl ToString) -> Self {
        Self {
            meta: None,
            name: name.to_string(),
            module_name: None,
            type_handle: None,
            visibility: Visibility::default(),
            inputs: vec![],
            outputs: vec![],
        }
    }

    /// Attaches metadata, builder style.
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Sets the owning module, builder style.
    pub fn with_module_name(mut self, name: impl ToString) -> Self {
        self.module_name = Some(name.to_string());
        self
    }

    /// Makes this a method of the given type, builder style.
    pub fn with_type_handle(mut self, handle: TypeHandle) -> Self {
        self.type_handle = Some(handle);
        self
    }

    /// Sets visibility, builder style.
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Appends an input, builder style.
    pub fn with_input(mut self, parameter: FunctionParameter) -> Self {
        self.inputs.push(parameter);
        self
    }

    /// Appends an output, builder style.
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
                "type_handle",
                &match self.type_handle.as_ref() {
                    Some(type_handle) => type_handle.name().to_owned(),
                    None => "!".to_owned(),
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
            write!(f, "#{meta} ")?;
        }
        if let Some(module_name) = self.module_name.as_ref() {
            write!(f, "mod {module_name} ")?;
        }
        if let Some(type_handle) = self.type_handle.as_ref() {
            match &**type_handle {
                Type::Struct(value) => {
                    write!(f, "struct {} ", value.type_name())?;
                }
                Type::Enum(value) => {
                    write!(f, "enum {} ", value.type_name())?;
                }
            }
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
                parameter.type_handle.type_name()
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
                parameter.type_handle.type_name()
            )?;
        }
        write!(f, ")")
    }
}

/// A callable unit: a signature plus a body.
///
/// See the [module docs](self).
#[derive(Debug)]
pub struct Function {
    signature: FunctionSignature,
    body: FunctionBody,
}

impl Function {
    /// Pairs a signature with a body.
    pub fn new(signature: FunctionSignature, body: FunctionBody) -> Self {
        Self { signature, body }
    }

    /// Returns the signature.
    pub fn signature(&self) -> &FunctionSignature {
        &self.signature
    }

    /// Runs the function on data already on the stack.
    ///
    /// Arguments must be pushed in reverse order beforehand, and results are
    /// left on the stack. Registers are scoped around the body, so the callee
    /// cannot reach the caller's.
    pub fn invoke(&self, context: &mut Context, registry: &Registry) {
        context.store_registers();
        self.body.invoke(context, registry);
        context.restore_registers();
    }

    /// Runs the function with Rust values, taking care of stack order.
    ///
    /// With `verify` set, argument and result types are checked against the
    /// signature first.
    ///
    /// # Panics
    ///
    /// Panics when `verify` is set and the types do not match, or when the
    /// results on the stack are not of type `O`.
    pub fn call<O: DataStackPack, I: DataStackPack>(
        &self,
        context: &mut Context,
        registry: &Registry,
        inputs: I,
        verify: bool,
    ) -> O {
        if verify {
            self.verify_inputs_outputs::<O, I>();
        }
        inputs.stack_push_reversed(context.stack());
        self.invoke(context, registry);
        O::stack_pop(context.stack())
    }

    /// Checks that `I` and `O` match the signature.
    ///
    /// # Panics
    ///
    /// Panics with a message naming the offending parameter when they do not.
    pub fn verify_inputs_outputs<O: DataStackPack, I: DataStackPack>(&self) {
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
            if parameter.type_handle.type_hash() != type_hash {
                panic!(
                    "Function: {} input parameter: {} got wrong value type!",
                    self.signature.name, parameter.name
                );
            }
        }
        for (parameter, type_hash) in self.signature.outputs.iter().zip(output_types) {
            if parameter.type_handle.type_hash() != type_hash {
                panic!(
                    "Function: {} output parameter: {} got wrong value type!",
                    self.signature.name, parameter.name
                );
            }
        }
    }

    /// Wraps this function in a shared handle.
    pub fn into_handle(self) -> FunctionHandle {
        self.into()
    }
}

/// Search filter for one function parameter.
///
/// Every field is optional. An empty filter matches anything.
#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct FunctionQueryParameter<'a> {
    /// Required parameter name.
    pub name: Option<Cow<'a, str>>,
    /// Filter on the parameter type.
    pub type_query: Option<TypeQuery<'a>>,
    /// Predicate the parameter metadata must satisfy.
    pub meta: Option<FunctionMetaQuery>,
}

impl FunctionQueryParameter<'_> {
    /// Returns `true` when `parameter` satisfies every set field.
    pub fn is_valid(&self, parameter: &FunctionParameter) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == parameter.name)
            .unwrap_or(true)
            && self
                .type_query
                .as_ref()
                .map(|query| query.is_valid(&parameter.type_handle))
                .unwrap_or(true)
            && self
                .meta
                .as_ref()
                .map(|query| parameter.meta.as_ref().map(query).unwrap_or(false))
                .unwrap_or(true)
    }

    /// Copies borrowed names into owned ones, so the filter can outlive them.
    pub fn to_static(&self) -> FunctionQueryParameter<'static> {
        FunctionQueryParameter {
            name: self
                .name
                .as_ref()
                .map(|name| name.as_ref().to_owned().into()),
            type_query: self.type_query.as_ref().map(|query| query.to_static()),
            meta: self.meta,
        }
    }
}

/// Search filter for a function's parameter list.
///
/// Three settings, because "the first two are ints" and "it takes exactly two
/// ints" are different questions. A bare list can only ask one of them.
///
/// [`Self::Prefix`] rejects a list **longer** than the function's. A filter
/// with no parameter to match cannot be satisfied, so it fails instead of
/// being ignored.
///
/// ```
/// # use intuicio_core::function::Parameters;
/// // Say nothing about the parameters at all.
/// let any = Parameters::default();
/// assert!(matches!(any, Parameters::Any));
/// ```
#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub enum Parameters<'a> {
    /// Matches any parameter list.
    #[default]
    Any,
    /// Matches from the front. The function may take more, but not fewer.
    Prefix(Cow<'a, [FunctionQueryParameter<'a>]>),
    /// Matches one for one. The function takes exactly these.
    Exact(Cow<'a, [FunctionQueryParameter<'a>]>),
}

/// A plain list is matched from the front. The function may take more
/// parameters.
impl<'a> From<Vec<FunctionQueryParameter<'a>>> for Parameters<'a> {
    fn from(value: Vec<FunctionQueryParameter<'a>>) -> Self {
        Self::Prefix(value.into())
    }
}

impl<'a> From<Cow<'a, [FunctionQueryParameter<'a>]>> for Parameters<'a> {
    fn from(value: Cow<'a, [FunctionQueryParameter<'a>]>) -> Self {
        Self::Prefix(value)
    }
}

impl<'a> From<&'a [FunctionQueryParameter<'a>]> for Parameters<'a> {
    fn from(value: &'a [FunctionQueryParameter<'a>]) -> Self {
        Self::Prefix(value.into())
    }
}

impl<'a> Parameters<'a> {
    /// Returns `true` when `parameters` satisfies this filter.
    pub fn is_valid(&self, parameters: &[FunctionParameter]) -> bool {
        let (queries, exact) = match self {
            Self::Any => return true,
            Self::Prefix(queries) => (queries, false),
            Self::Exact(queries) => (queries, true),
        };
        let fits = if exact {
            queries.len() == parameters.len()
        } else {
            queries.len() <= parameters.len()
        };
        fits && queries
            .iter()
            .zip(parameters.iter())
            .all(|(query, parameter)| query.is_valid(parameter))
    }

    /// The filters themselves, empty for [`Self::Any`].
    pub fn queries(&self) -> &[FunctionQueryParameter<'a>] {
        match self {
            Self::Any => &[],
            Self::Prefix(queries) | Self::Exact(queries) => queries,
        }
    }

    /// Copies borrowed names into owned ones, so the filter can outlive them.
    pub fn to_static(&self) -> Parameters<'static> {
        let queries = |queries: &Cow<'_, [FunctionQueryParameter<'_>]>| {
            queries
                .iter()
                .map(|query| query.to_static())
                .collect::<Vec<_>>()
                .into()
        };
        match self {
            Self::Any => Parameters::Any,
            Self::Prefix(inner) => Parameters::Prefix(queries(inner)),
            Self::Exact(inner) => Parameters::Exact(queries(inner)),
        }
    }
}

/// Search filter for functions in a [`crate::registry::Registry`].
///
/// Every field is optional and an empty query matches everything.
///
/// Three of the fields are a [`Filter`] rather than an `Option`, because the
/// signature's own field is optional and a query must be able to ask for its
/// **absence**. `type_query: Filter::Absent` asks for a free function, not a
/// method, which an `Option` cannot express.
///
/// ```
/// # use intuicio_core::{Filter, function::{FunctionQuery, Parameters}};
/// // A free function called `add`, taking exactly two parameters.
/// let query = FunctionQuery {
///     name: Some("add".into()),
///     module_name: Filter::Matching("lib".into()),
///     type_query: Filter::Absent,
///     inputs: Parameters::Exact(vec![Default::default(), Default::default()].into()),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Default, Clone, PartialEq, Hash)]
pub struct FunctionQuery<'a> {
    /// Required function name.
    pub name: Option<Cow<'a, str>>,
    /// Filter on the module the function belongs to.
    pub module_name: Filter<Cow<'a, str>>,
    /// Filter on the type the function is a method of.
    pub type_query: Filter<TypeQuery<'a>>,
    /// Required visibility.
    pub visibility: Option<Visibility>,
    /// Filter on the argument list.
    pub inputs: Parameters<'a>,
    /// Filter on the result list.
    pub outputs: Parameters<'a>,
    /// Predicate the function metadata must satisfy.
    pub meta: Filter<FunctionMetaQuery>,
}

impl FunctionQuery<'_> {
    /// Returns `true` when `signature` satisfies every set field.
    pub fn is_valid(&self, signature: &FunctionSignature) -> bool {
        self.name
            .as_ref()
            .map(|name| name.as_ref() == signature.name)
            .unwrap_or(true)
            && self
                .module_name
                .is_valid(signature.module_name.as_ref(), |name, module_name| {
                    name.as_ref() == module_name
                })
            && self
                .type_query
                .is_valid(signature.type_handle.as_ref(), |query, handle| {
                    query.is_valid(handle)
                })
            && self
                .visibility
                .map(|visibility| signature.visibility.is_visible(visibility))
                .unwrap_or(true)
            && self.inputs.is_valid(&signature.inputs)
            && self.outputs.is_valid(&signature.outputs)
            && self
                .meta
                .is_valid(signature.meta.as_ref(), |query, meta| query(meta))
    }

    /// Hashes the query, which is the key the registry caches results under.
    pub fn as_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Copies borrowed names into owned ones, so the query can outlive them.
    pub fn to_static(&self) -> FunctionQuery<'static> {
        FunctionQuery {
            name: self
                .name
                .as_ref()
                .map(|name| name.as_ref().to_owned().into()),
            module_name: self.module_name.map(|name| name.as_ref().to_owned().into()),
            type_query: self.type_query.map(|query| query.to_static()),
            visibility: self.visibility,
            inputs: self.inputs.to_static(),
            outputs: self.outputs.to_static(),
            meta: self.meta,
        }
    }
}

/// Builds a [`FunctionSignature`], looking every type up in a registry.
///
/// ```ignore
/// function_signature! {
///     registry => mod lib fn add(a: i32, b: i32) -> (result: i32)
/// }
/// ```
///
/// # Panics
///
/// Panics when a type used in the signature is not registered.
#[macro_export]
macro_rules! function_signature {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        $(type ($type:ty))?
        fn
        $name:ident
        ($( $input_name:ident : $input_type:ty ),*)
        ->
        ($( $output_name:ident : $output_type:ty ),*)
    ) => {{
        let mut result = $crate::function::FunctionSignature::new(stringify!($name));
        $(
            result.module_name = Some(stringify!($module_name).to_owned());
        )?
        $(
            result.type_handle = Some($registry.find_type($crate::types::TypeQuery::of::<$type>()).unwrap());
        )?
        $(
            result.inputs.push(
                $crate::function::FunctionParameter::new(
                    stringify!($input_name).to_owned(),
                    $registry.find_type($crate::types::TypeQuery::of::<$input_type>()).unwrap()
                )
            );
        )*
        $(
            result.outputs.push(
                $crate::function::FunctionParameter::new(
                    stringify!($output_name).to_owned(),
                    $registry.find_type($crate::types::TypeQuery::of::<$output_type>()).unwrap()
                )
            );
        )*
        result
    }};
}

/// Builds a whole [`Function`] from a signature and a Rust body.
///
/// The body is a block whose value is a tuple of the outputs. Arguments arrive
/// as ordinary local variables.
///
/// ```ignore
/// define_function! {
///     registry => mod lib fn add(a: i32, b: i32) -> (result: i32) {
///         (a + b,)
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_function {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        $(type ($type:ty))?
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
                $(type ($type))?
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
    use crate::{context::*, function::*, registry::*, types::struct_type::*};
    use intuicio_data;
    use intuicio_derive::*;

    #[intuicio_function(meta = "foo", args_meta(_bar = "foo"))]
    fn function_meta(_bar: bool) {}

    #[intuicio_function(name = "+", module_name = "core/ops")]
    fn function_non_ident_name(a: i32, b: i32) -> i32 {
        a + b
    }

    /// The three things `Option` and a bare parameter list could not ask, and
    /// the one they got wrong.
    #[test]
    fn test_query_filters() {
        let mut registry = Registry::default();
        registry.add_type(NativeStructBuilder::new::<i32>().build());
        let i32_type = registry.find_type(TypeQuery::of::<i32>()).unwrap();

        let free = FunctionSignature::new("f")
            .with_module_name("lib")
            .with_input(FunctionParameter::new("a", i32_type.clone()));
        let method = FunctionSignature::new("f")
            .with_module_name("lib")
            .with_type_handle(i32_type.clone())
            .with_input(FunctionParameter::new("a", i32_type.clone()));

        // Absence, which is how "a free function, not a method" is asked.
        let query = FunctionQuery {
            type_query: Filter::Absent,
            ..Default::default()
        };
        assert!(query.is_valid(&free));
        assert!(!query.is_valid(&method));

        // Ignore, the default, still matches either - so old queries behave the
        // way they did.
        let query = FunctionQuery::default();
        assert!(query.is_valid(&free));
        assert!(query.is_valid(&method));

        // Matching, on a field the signature may not have at all.
        let query = FunctionQuery {
            module_name: Filter::Matching("lib".into()),
            ..Default::default()
        };
        assert!(query.is_valid(&free));
        let query = FunctionQuery {
            module_name: Filter::Matching("other".into()),
            ..Default::default()
        };
        assert!(!query.is_valid(&free));

        let two = FunctionSignature::new("g")
            .with_input(FunctionParameter::new("a", i32_type.clone()))
            .with_input(FunctionParameter::new("b", i32_type.clone()));

        // Exact pins the count; prefix does not.
        let one_filter = vec![FunctionQueryParameter::default()];
        assert!(
            FunctionQuery {
                inputs: Parameters::Prefix(one_filter.to_owned().into()),
                ..Default::default()
            }
            .is_valid(&two)
        );
        assert!(
            !FunctionQuery {
                inputs: Parameters::Exact(one_filter.into()),
                ..Default::default()
            }
            .is_valid(&two)
        );

        // The bug: more filters than parameters used to match, because `zip`
        // stopped at the shorter list and left the extra filter unchecked.
        let three_filters = vec![
            FunctionQueryParameter::default(),
            FunctionQueryParameter::default(),
            FunctionQueryParameter::default(),
        ];
        assert!(
            !FunctionQuery {
                inputs: Parameters::Prefix(three_filters.into()),
                ..Default::default()
            }
            .is_valid(&two)
        );
    }

    #[test]
    fn test_function_non_ident_name() {
        let mut registry = Registry::default();
        registry.add_type(NativeStructBuilder::new::<i32>().build());
        let signature = function_non_ident_name::define_signature(&registry);
        assert_eq!(signature.name, "+");
        assert_eq!(signature.module_name.as_deref(), Some("core/ops"));
    }

    #[test]
    fn test_function() {
        fn add(context: &mut Context, _: &Registry) {
            let a = context.stack().pop::<i32>().unwrap();
            let b = context.stack().pop::<i32>().unwrap();
            context.stack().push(a + b);
        }

        let i32_handle = NativeStructBuilder::new::<i32>()
            .build()
            .into_type()
            .into_handle();
        let signature = FunctionSignature::new("add")
            .with_input(FunctionParameter::new("a", i32_handle.clone()))
            .with_input(FunctionParameter::new("b", i32_handle.clone()))
            .with_output(FunctionParameter::new("result", i32_handle));
        let function = Function::new(signature.to_owned(), FunctionBody::pointer(add));

        assert!(FunctionQuery::default().is_valid(&signature));
        assert!(
            FunctionQuery {
                name: Some("add".into()),
                ..Default::default()
            }
            .is_valid(&signature)
        );
        assert!(
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
            .is_valid(&signature)
        );
        assert!(
            !FunctionQuery {
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
            .is_valid(&signature)
        );

        let mut context = Context::new(10240, 10240);
        let registry = Registry::default().with_basic_types();

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

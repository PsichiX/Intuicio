//! The data a frontend produces and a backend consumes.
//!
//! This is the interface that keeps frontends and backends independent. A
//! frontend emits a [`ScriptPackage`], whatever its input looks like. A backend
//! turns the functions inside it into callable [`crate::function::Function`]
//! values. Neither has to know the other.
//!
//! # Shape of the data
//!
//! ```text
//! ScriptPackage
//!   ScriptModule           name
//!     ScriptStruct         name, fields
//!     ScriptEnum           name, variants
//!     ScriptFunction       signature + Script
//!       ScriptOperation    the actual instructions
//! ```
//!
//! # Operations
//!
//! [`ScriptOperation`] is deliberately small: define and move registers, call a
//! function, branch, loop, return, suspend. That is the common set every
//! frontend and backend can share. Anything more specific, such as pushing a
//! literal, goes into [`ScriptOperation::Expression`] and the
//! [`ScriptExpression`] type a frontend defines for itself.
//!
//! # Loading
//!
//! [`ScriptContentProvider`] is the other half. It fetches source by path, so a
//! frontend can follow imports without knowing where they live.
use crate::{
    Visibility,
    context::Context,
    function::{Function, FunctionBody, FunctionParameter, FunctionQuery, FunctionSignature},
    meta::Meta,
    registry::Registry,
    types::{
        TypeQuery,
        enum_type::{EnumVariant, RuntimeEnumBuilder},
        struct_type::{RuntimeStructBuilder, StructField},
    },
};
use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    sync::Arc,
};

/// A shared, finished [`Script`].
pub type ScriptHandle<'a, SE> = Arc<Script<'a, SE>>;
/// A straight list of operations, run from first to last.
pub type Script<'a, SE> = Vec<ScriptOperation<'a, SE>>;

/// The extra operations a frontend adds on top of the built-in set.
///
/// The built-in operations move data around but never create it, since the
/// platform has no opinion about what data is. Pushing a literal, dropping a
/// value, anything else specific to one language, goes here.
///
/// ```
/// # use intuicio_core::{context::Context, registry::Registry, script::ScriptExpression};
/// enum MyExpression {
///     Literal(i32),
///     Drop,
/// }
///
/// impl ScriptExpression for MyExpression {
///     fn evaluate(&self, context: &mut Context, _: &Registry) {
///         match self {
///             Self::Literal(value) => { context.stack().push(*value); }
///             Self::Drop => { context.stack().drop(); }
///         }
///     }
/// }
/// ```
pub trait ScriptExpression: Send + Sync {
    /// Runs this expression against the running context.
    fn evaluate(&self, context: &mut Context, registry: &Registry);
}

impl ScriptExpression for () {
    fn evaluate(&self, _: &mut Context, _: &Registry) {}
}

/// A ready-made [`ScriptExpression`] built from a closure.
///
/// Saves defining an expression type when a script is assembled in Rust
/// rather than parsed.
#[allow(clippy::type_complexity)]
pub struct InlineExpression(Arc<dyn Fn(&mut Context, &Registry) + Send + Sync>);

impl InlineExpression {
    /// An expression that pushes a copy of `value` every time it runs.
    pub fn copied<T: Copy + Send + Sync + 'static>(value: T) -> Self {
        Self(Arc::new(move |context, _| {
            context.stack().push(value);
        }))
    }

    /// An expression that pushes a clone of `value` every time it runs.
    pub fn cloned<T: Clone + Send + Sync + 'static>(value: T) -> Self {
        Self(Arc::new(move |context, _| {
            context.stack().push(value.clone());
        }))
    }

    /// An expression that runs an arbitrary closure.
    pub fn closure<F: Fn(&mut Context, &Registry) + Send + Sync + 'static>(f: F) -> Self {
        Self(Arc::new(f))
    }
}

impl ScriptExpression for InlineExpression {
    fn evaluate(&self, context: &mut Context, registry: &Registry) {
        (self.0)(context, registry);
    }
}

/// One instruction of a script.
///
/// See the [module docs](self) for why the set is this small.
#[derive(Debug)]
pub enum ScriptOperation<'a, SE: ScriptExpression> {
    /// Does nothing.
    None,
    /// Runs a frontend-defined operation. See [`ScriptExpression`].
    Expression { expression: SE },
    /// Allocates a register of the queried type.
    ///
    /// A register has to be defined before it is used, and is dropped when the
    /// scope that defined it ends.
    DefineRegister { query: TypeQuery<'a> },
    /// Destroys the value in a register, leaving it empty.
    DropRegister { index: usize },
    /// Moves a register's value onto the stack, leaving the register empty.
    PushFromRegister { index: usize },
    /// Moves the top stack value into a register.
    PopToRegister { index: usize },
    /// Moves a value from one register to another.
    MoveRegister { from: usize, to: usize },
    /// Finds a function in the registry and invokes it.
    CallFunction { query: FunctionQuery<'a> },
    /// Takes a `bool` off the stack and runs one of two scopes.
    ///
    /// The failure scope is optional, in which case a `false` does nothing.
    BranchScope {
        scope_success: ScriptHandle<'a, SE>,
        scope_failure: Option<ScriptHandle<'a, SE>>,
    },
    /// Repeats a scope while a `bool` taken off the stack is `true`.
    ///
    /// The scope must leave a fresh `bool` on the stack before it ends,
    /// otherwise the next round reads whatever happens to be there.
    LoopScope { scope: ScriptHandle<'a, SE> },
    /// Runs a nested scope and comes back when it ends.
    PushScope { scope: ScriptHandle<'a, SE> },
    /// Ends the current scope early, the way `return` does.
    PopScope,
    /// Takes a `bool` off the stack and ends the current scope when it is
    /// `false`.
    ContinueScopeConditionally,
    /// Stops stepping and reports back to the caller without finishing.
    ///
    /// For frontends building coroutines or futures. Anything to yield has to
    /// be left on the stack for the host to take.
    Suspend,
}

impl<SE: ScriptExpression> ScriptOperation<'_, SE> {
    /// Returns the operation's name, for debugging and tooling.
    pub fn label(&self) -> &str {
        match self {
            Self::None => "None",
            Self::Expression { .. } => "Expression",
            Self::DefineRegister { .. } => "DefineRegister",
            Self::DropRegister { .. } => "DropRegister",
            Self::PushFromRegister { .. } => "PushFromRegister",
            Self::PopToRegister { .. } => "PopToRegister",
            Self::MoveRegister { .. } => "MoveRegister",
            Self::CallFunction { .. } => "CallFunction",
            Self::BranchScope { .. } => "BranchScope",
            Self::LoopScope { .. } => "LoopScope",
            Self::PushScope { .. } => "PushScope",
            Self::PopScope => "PopScope",
            Self::ContinueScopeConditionally => "ContinueScopeConditionally",
            Self::Suspend => "Suspend",
        }
    }
}

/// Assembles a [`Script`] one operation at a time.
///
/// ```ignore
/// let script = ScriptBuilder::<MyExpression>::default()
///     .expression(MyExpression::Literal(40))
///     .expression(MyExpression::Literal(2))
///     .call_function(FunctionQuery { name: Some("add".into()), ..Default::default() })
///     .build();
/// ```
pub struct ScriptBuilder<'a, SE: ScriptExpression>(Script<'a, SE>);

impl<SE: ScriptExpression> Default for ScriptBuilder<'_, SE> {
    fn default() -> Self {
        Self(vec![])
    }
}

impl<'a, SE: ScriptExpression> ScriptBuilder<'a, SE> {
    /// Finishes the script and shares it.
    pub fn build(self) -> ScriptHandle<'a, SE> {
        ScriptHandle::new(self.0)
    }

    /// Appends [`ScriptOperation::Expression`].
    pub fn expression(mut self, expression: SE) -> Self {
        self.0.push(ScriptOperation::Expression { expression });
        self
    }

    /// Appends [`ScriptOperation::DefineRegister`].
    pub fn define_register(mut self, query: TypeQuery<'a>) -> Self {
        self.0.push(ScriptOperation::DefineRegister { query });
        self
    }

    /// Appends [`ScriptOperation::DropRegister`].
    pub fn drop_register(mut self, index: usize) -> Self {
        self.0.push(ScriptOperation::DropRegister { index });
        self
    }

    /// Appends [`ScriptOperation::PushFromRegister`].
    pub fn push_from_register(mut self, index: usize) -> Self {
        self.0.push(ScriptOperation::PushFromRegister { index });
        self
    }

    /// Appends [`ScriptOperation::PopToRegister`].
    pub fn pop_to_register(mut self, index: usize) -> Self {
        self.0.push(ScriptOperation::PopToRegister { index });
        self
    }

    /// Appends [`ScriptOperation::MoveRegister`].
    pub fn move_register(mut self, from: usize, to: usize) -> Self {
        self.0.push(ScriptOperation::MoveRegister { from, to });
        self
    }

    /// Appends [`ScriptOperation::CallFunction`].
    pub fn call_function(mut self, query: FunctionQuery<'a>) -> Self {
        self.0.push(ScriptOperation::CallFunction { query });
        self
    }

    /// Appends [`ScriptOperation::BranchScope`].
    pub fn branch_scope(
        mut self,
        scope_success: ScriptHandle<'a, SE>,
        scope_failure: Option<ScriptHandle<'a, SE>>,
    ) -> Self {
        self.0.push(ScriptOperation::BranchScope {
            scope_success,
            scope_failure,
        });
        self
    }

    /// Appends [`ScriptOperation::LoopScope`].
    pub fn loop_scope(mut self, scope: ScriptHandle<'a, SE>) -> Self {
        self.0.push(ScriptOperation::LoopScope { scope });
        self
    }

    /// Appends [`ScriptOperation::PushScope`].
    pub fn push_scope(mut self, scope: ScriptHandle<'a, SE>) -> Self {
        self.0.push(ScriptOperation::PushScope { scope });
        self
    }

    /// Appends [`ScriptOperation::PopScope`].
    pub fn pop_scope(mut self) -> Self {
        self.0.push(ScriptOperation::PopScope);
        self
    }

    /// Appends [`ScriptOperation::ContinueScopeConditionally`].
    pub fn continue_scope_conditionally(mut self) -> Self {
        self.0.push(ScriptOperation::ContinueScopeConditionally);
        self
    }

    /// Appends [`ScriptOperation::Suspend`].
    pub fn suspend(mut self) -> Self {
        self.0.push(ScriptOperation::Suspend);
        self
    }
}

/// A function parameter as a frontend describes it, by type query rather than
/// by resolved type.
#[derive(Debug)]
pub struct ScriptFunctionParameter<'a> {
    /// Metadata attached to this parameter.
    pub meta: Option<Meta>,
    /// Parameter name.
    pub name: String,
    /// Query that picks the parameter type.
    pub type_query: TypeQuery<'a>,
}

impl ScriptFunctionParameter<'_> {
    /// Resolves the type query against the registry.
    ///
    /// # Panics
    ///
    /// Panics when no registered type matches.
    pub fn build(&self, registry: &Registry) -> FunctionParameter {
        FunctionParameter {
            meta: self.meta.to_owned(),
            name: self.name.to_owned(),
            type_handle: registry
                .types()
                .find(|type_| self.type_query.is_valid(type_))
                .unwrap()
                .clone(),
        }
    }
}

/// A function signature as a frontend describes it, with types still
/// unresolved.
#[derive(Debug)]
pub struct ScriptFunctionSignature<'a> {
    /// Metadata attached to this function.
    pub meta: Option<Meta>,
    /// Name to register the function under.
    pub name: String,
    /// Module the function belongs to.
    pub module_name: Option<String>,
    /// Query that picks the type this function is a method of.
    pub type_query: Option<TypeQuery<'a>>,
    /// How widely the function is visible.
    pub visibility: Visibility,
    /// Arguments, in declaration order.
    pub inputs: Vec<ScriptFunctionParameter<'a>>,
    /// Results, in declaration order.
    pub outputs: Vec<ScriptFunctionParameter<'a>>,
}

impl ScriptFunctionSignature<'_> {
    /// Resolves every type query against the registry.
    ///
    /// # Panics
    ///
    /// Panics when a type query matches nothing.
    pub fn build(&self, registry: &Registry) -> FunctionSignature {
        FunctionSignature {
            meta: self.meta.to_owned(),
            name: self.name.to_owned(),
            module_name: self.module_name.to_owned(),
            type_handle: self.type_query.as_ref().map(|type_query| {
                registry
                    .types()
                    .find(|type_| type_query.is_valid(type_))
                    .unwrap()
                    .clone()
            }),
            visibility: self.visibility,
            inputs: self
                .inputs
                .iter()
                .map(|parameter| parameter.build(registry))
                .collect(),
            outputs: self
                .outputs
                .iter()
                .map(|parameter| parameter.build(registry))
                .collect(),
        }
    }
}

/// A function a frontend produced: an unresolved signature and its
/// operations.
#[derive(Debug)]
pub struct ScriptFunction<'a, SE: ScriptExpression> {
    /// Signature, with its types still unresolved.
    pub signature: ScriptFunctionSignature<'a>,
    /// Operations that make up the body.
    pub script: ScriptHandle<'a, SE>,
}

impl<SE: ScriptExpression> ScriptFunction<'static, SE> {
    /// Turns this into a real function with the given backend and registers it.
    ///
    /// Returns whatever the backend produced alongside the function, or
    /// [`None`] when the backend declined.
    pub fn install<SFG: ScriptFunctionGenerator<SE>>(
        &self,
        registry: &mut Registry,
        input: SFG::Input,
    ) -> Option<SFG::Output> {
        let (function, output) = SFG::generate_function(self, registry, input)?;
        registry.add_function(function);
        Some(output)
    }
}

/// A backend: turns script operations into a runnable function body.
///
/// A virtual machine implements this by keeping the operations and stepping
/// through them. A transpiler could emit code instead. `Input` is whatever the
/// backend needs to be given, `Output` whatever it wants to hand back.
pub trait ScriptFunctionGenerator<SE: ScriptExpression> {
    /// Configuration the backend needs, passed through at install time.
    type Input;
    /// Anything the backend wants to return alongside the function.
    type Output;

    /// Builds a body from a script, or returns [`None`] when it cannot.
    fn generate_function_body(
        script: ScriptHandle<'static, SE>,
        input: Self::Input,
    ) -> Option<(FunctionBody, Self::Output)>;

    /// Builds a whole function, resolving the signature against the registry.
    fn generate_function(
        function: &ScriptFunction<'static, SE>,
        registry: &Registry,
        input: Self::Input,
    ) -> Option<(Function, Self::Output)> {
        let (body, output) = Self::generate_function_body(function.script.clone(), input)?;
        Some((
            Function::new(function.signature.build(registry), body),
            output,
        ))
    }
}

/// A struct field as a frontend describes it, by type query rather than by
/// resolved type.
#[derive(Debug)]
pub struct ScriptStructField<'a> {
    /// Metadata attached to this field.
    pub meta: Option<Meta>,
    /// Field name.
    pub name: String,
    /// How widely the field is visible.
    pub visibility: Visibility,
    /// Query that picks the field type.
    pub type_query: TypeQuery<'a>,
}

impl ScriptStructField<'_> {
    /// Resolves the type query against the registry.
    ///
    /// # Panics
    ///
    /// Panics when no registered type matches.
    pub fn build(&self, registry: &Registry) -> StructField {
        let mut result = StructField::new(
            &self.name,
            registry
                .types()
                .find(|type_| self.type_query.is_valid(type_))
                .unwrap()
                .clone(),
        )
        .with_visibility(self.visibility);
        result.meta.clone_from(&self.meta);
        result
    }
}

/// A struct a frontend produced.
///
/// Installing takes two passes, [`ScriptStruct::declare`] then
/// [`ScriptStruct::define`], so that types can refer to each other.
#[derive(Debug)]
pub struct ScriptStruct<'a> {
    /// Metadata attached to this type.
    pub meta: Option<Meta>,
    /// Name to register the type under.
    pub name: String,
    /// Module the type belongs to.
    pub module_name: Option<String>,
    /// How widely the type is visible.
    pub visibility: Visibility,
    /// Fields, in declaration order.
    pub fields: Vec<ScriptStructField<'a>>,
}

impl ScriptStruct<'_> {
    /// Registers the type with no fields yet, so other types can name it.
    pub fn declare(&self, registry: &mut Registry) {
        let mut builder = RuntimeStructBuilder::new(&self.name);
        builder = builder.visibility(self.visibility);
        if let Some(module_name) = self.module_name.as_ref() {
            builder = builder.module_name(module_name);
        }
        if let Some(meta) = self.meta.as_ref() {
            builder = builder.meta(meta.to_owned());
        }
        registry.add_type(builder.build());
    }

    /// Fills in the fields of an already declared type, in place.
    ///
    /// Does nothing when the type was never declared.
    pub fn define(&self, registry: &mut Registry) {
        let query = TypeQuery {
            name: Some(self.name.as_str().into()),
            module_name: self
                .module_name
                .as_ref()
                .map(|module_name| module_name.into()),
            ..Default::default()
        };
        if let Some(handle) = registry.find_type(query) {
            let mut builder = RuntimeStructBuilder::new(&self.name);
            builder = builder.visibility(self.visibility);
            if let Some(module_name) = self.module_name.as_ref() {
                builder = builder.module_name(module_name);
            }
            if let Some(meta) = self.meta.as_ref() {
                builder = builder.meta(meta.to_owned());
            }
            for field in &self.fields {
                builder = builder.field(field.build(registry));
            }
            unsafe {
                let type_ = Arc::as_ptr(&handle).cast_mut();
                *type_ = builder.build().into();
            }
        }
    }

    /// Registers the type complete with fields, in one pass.
    ///
    /// Only works when every field type is already registered. Otherwise use
    /// declare and define.
    pub fn install(&self, registry: &mut Registry) {
        let mut builder = RuntimeStructBuilder::new(&self.name);
        builder = builder.visibility(self.visibility);
        if let Some(module_name) = self.module_name.as_ref() {
            builder = builder.module_name(module_name);
        }
        for field in &self.fields {
            builder = builder.field(field.build(registry));
        }
        registry.add_type(builder.build());
    }
}

/// An enum variant as a frontend describes it.
#[derive(Debug)]
pub struct ScriptEnumVariant<'a> {
    /// Metadata attached to this variant.
    pub meta: Option<Meta>,
    /// Variant name.
    pub name: String,
    /// Fields the variant carries.
    pub fields: Vec<ScriptStructField<'a>>,
    /// Discriminant to give the variant. Counted from the previous one when
    /// [`None`].
    pub discriminant: Option<u8>,
}

impl ScriptEnumVariant<'_> {
    /// Resolves the field type queries against the registry.
    ///
    /// # Panics
    ///
    /// Panics when a field type query matches nothing.
    pub fn build(&self, registry: &Registry) -> EnumVariant {
        let mut result = EnumVariant::new(&self.name);
        result.fields = self
            .fields
            .iter()
            .map(|field| field.build(registry))
            .collect();
        result.meta.clone_from(&self.meta);
        result
    }
}

/// An enum a frontend produced.
///
/// Installing takes two passes, [`ScriptEnum::declare`] then
/// [`ScriptEnum::define`], so that types can refer to each other.
#[derive(Debug)]
pub struct ScriptEnum<'a> {
    /// Metadata attached to this type.
    pub meta: Option<Meta>,
    /// Name to register the type under.
    pub name: String,
    /// Module the type belongs to.
    pub module_name: Option<String>,
    /// How widely the type is visible.
    pub visibility: Visibility,
    /// Variants, in declaration order.
    pub variants: Vec<ScriptEnumVariant<'a>>,
    /// Discriminant a default value holds.
    pub default_variant: Option<u8>,
}

impl ScriptEnum<'_> {
    /// Registers the type with no variants yet, so other types can name it.
    pub fn declare(&self, registry: &mut Registry) {
        let mut builder = RuntimeEnumBuilder::new(&self.name);
        if let Some(discriminant) = self.default_variant {
            builder = builder.set_default_variant(discriminant);
        }
        builder = builder.visibility(self.visibility);
        if let Some(module_name) = self.module_name.as_ref() {
            builder = builder.module_name(module_name);
        }
        if let Some(meta) = self.meta.as_ref() {
            builder = builder.meta(meta.to_owned());
        }
        registry.add_type(builder.build());
    }

    /// Fills in the variants of an already declared type, in place.
    ///
    /// Does nothing when the type was never declared.
    pub fn define(&self, registry: &mut Registry) {
        let query = TypeQuery {
            name: Some(self.name.as_str().into()),
            module_name: self
                .module_name
                .as_ref()
                .map(|module_name| module_name.into()),
            ..Default::default()
        };
        if let Some(handle) = registry.find_type(query) {
            let mut builder = RuntimeEnumBuilder::new(&self.name);
            if let Some(discriminant) = self.default_variant {
                builder = builder.set_default_variant(discriminant);
            }
            builder = builder.visibility(self.visibility);
            if let Some(module_name) = self.module_name.as_ref() {
                builder = builder.module_name(module_name);
            }
            if let Some(meta) = self.meta.as_ref() {
                builder = builder.meta(meta.to_owned());
            }
            for variant in &self.variants {
                if let Some(discriminant) = variant.discriminant {
                    builder =
                        builder.variant_with_discriminant(variant.build(registry), discriminant);
                } else {
                    builder = builder.variant(variant.build(registry));
                }
            }
            unsafe {
                let type_ = Arc::as_ptr(&handle).cast_mut();
                *type_ = builder.build().into();
            }
        }
    }

    /// Registers the type complete with variants, in one pass.
    ///
    /// Only works when every field type is already registered. Otherwise use
    /// declare and define.
    pub fn install(&self, registry: &mut Registry) {
        let mut builder = RuntimeEnumBuilder::new(&self.name);
        if let Some(discriminant) = self.default_variant {
            builder = builder.set_default_variant(discriminant);
        }
        builder = builder.visibility(self.visibility);
        if let Some(module_name) = self.module_name.as_ref() {
            builder = builder.module_name(module_name);
        }
        for variant in &self.variants {
            if let Some(discriminant) = variant.discriminant {
                builder = builder.variant_with_discriminant(variant.build(registry), discriminant);
            } else {
                builder = builder.variant(variant.build(registry));
            }
        }
        registry.add_type(builder.build());
    }
}

/// A named group of types and functions, as a frontend produced it.
#[derive(Debug, Default)]
pub struct ScriptModule<'a, SE: ScriptExpression> {
    /// Module name, stamped onto everything inside it.
    pub name: String,
    /// Structs the module declares.
    pub structs: Vec<ScriptStruct<'a>>,
    /// Enums the module declares.
    pub enums: Vec<ScriptEnum<'a>>,
    /// Functions the module declares.
    pub functions: Vec<ScriptFunction<'a, SE>>,
}

impl<SE: ScriptExpression> ScriptModule<'_, SE> {
    /// Stamps this module's name onto every type and function inside it.
    pub fn fix_module_names(&mut self) {
        for type_ in &mut self.structs {
            type_.module_name = Some(self.name.to_owned());
        }
        for type_ in &mut self.enums {
            type_.module_name = Some(self.name.to_owned());
        }
        for function in &mut self.functions {
            function.signature.module_name = Some(self.name.to_owned());
        }
    }

    /// First pass: registers every type without its fields.
    pub fn declare_types(&self, registry: &mut Registry) {
        for type_ in &self.structs {
            type_.declare(registry);
        }
        for type_ in &self.enums {
            type_.declare(registry);
        }
    }

    /// Second pass: fills in the fields of every declared type.
    pub fn define_types(&self, registry: &mut Registry) {
        for type_ in &self.structs {
            type_.define(registry);
        }
        for type_ in &self.enums {
            type_.define(registry);
        }
    }

    /// Runs both type passes.
    pub fn install_types(&self, registry: &mut Registry) {
        self.declare_types(registry);
        self.define_types(registry);
    }
}

impl<SE: ScriptExpression> ScriptModule<'static, SE> {
    /// Compiles every function with the given backend and registers it.
    pub fn install_functions<SFG: ScriptFunctionGenerator<SE>>(
        &self,
        registry: &mut Registry,
        input: SFG::Input,
    ) where
        SFG::Input: Clone,
    {
        for function in &self.functions {
            function.install::<SFG>(registry, input.clone());
        }
    }
}

/// A whole compilation unit: the modules a frontend produced.
#[derive(Debug, Default)]
pub struct ScriptPackage<'a, SE: ScriptExpression> {
    /// Modules this unit is made of.
    pub modules: Vec<ScriptModule<'a, SE>>,
}

impl<SE: ScriptExpression> ScriptPackage<'static, SE> {
    /// Installs everything into a registry.
    ///
    /// Every module's types go in first, across the whole package, so functions
    /// and fields can refer to types from any module.
    pub fn install<SFG: ScriptFunctionGenerator<SE>>(
        &self,
        registry: &mut Registry,
        input: SFG::Input,
    ) where
        SFG::Input: Clone,
    {
        for module in &self.modules {
            module.install_types(registry);
        }
        for module in &self.modules {
            module.install_functions::<SFG>(registry, input.clone());
        }
    }
}

/// One loaded source unit, as [`ScriptContentProvider::unpack_load`] returns
/// it.
///
/// `data` carries the load error rather than failing the whole batch, so one
/// bad file does not hide the rest.
pub struct ScriptContent<T> {
    /// Path the content was loaded from.
    pub path: String,
    /// Name to refer to this unit by, often the same as the path.
    pub name: String,
    /// The parsed content, nothing to load, or the error that stopped it.
    pub data: Result<Option<T>, Box<dyn Error>>,
}

/// Fetches script source by path.
///
/// A frontend follows imports through this trait. Where the source lives, on
/// disk, in an archive or generated on the fly, is then not the frontend's
/// problem.
pub trait ScriptContentProvider<T> {
    /// Loads and parses one unit, or returns [`None`] when there is nothing to
    /// load.
    fn load(&mut self, path: &str) -> Result<Option<T>, Box<dyn Error>>;

    /// Loads a path that may hold several units, such as an archive.
    ///
    /// Defaults to a single [`ScriptContentProvider::load`].
    fn unpack_load(&mut self, path: &str) -> Result<Vec<ScriptContent<T>>, Box<dyn Error>> {
        Ok(vec![ScriptContent {
            path: path.to_owned(),
            name: path.to_owned(),
            data: self.load(path),
        }])
    }

    /// Turns a path into the canonical form used to recognise the same unit
    /// twice.
    ///
    /// Defaults to leaving it alone.
    fn sanitize_path(&self, path: &str) -> Result<String, Box<dyn Error>> {
        Ok(path.to_owned())
    }

    /// Resolves an import path written inside `parent`.
    fn join_paths(&self, parent: &str, relative: &str) -> Result<String, Box<dyn Error>>;
}

/// Dispatches to another provider based on the file extension.
///
/// Lets one package mix source formats, for example a text language
/// importing a serialized module.
pub struct ExtensionContentProvider<S> {
    default_extension: Option<String>,
    extension_providers: HashMap<String, Box<dyn ScriptContentProvider<S>>>,
}

impl<S> Default for ExtensionContentProvider<S> {
    fn default() -> Self {
        Self {
            default_extension: None,
            extension_providers: Default::default(),
        }
    }
}

impl<S> ExtensionContentProvider<S> {
    /// Sets the extension to assume for paths that carry none.
    pub fn default_extension(mut self, extension: impl ToString) -> Self {
        self.default_extension = Some(extension.to_string());
        self
    }

    /// Routes one extension to a provider.
    pub fn extension(
        mut self,
        extension: &str,
        content_provider: impl ScriptContentProvider<S> + 'static,
    ) -> Self {
        self.extension_providers
            .insert(extension.to_owned(), Box::new(content_provider));
        self
    }
}

impl<S> ScriptContentProvider<S> for ExtensionContentProvider<S> {
    fn load(&mut self, _: &str) -> Result<Option<S>, Box<dyn Error>> {
        Ok(None)
    }

    fn unpack_load(&mut self, path: &str) -> Result<Vec<ScriptContent<S>>, Box<dyn Error>> {
        let extension = match Path::new(path).extension() {
            Some(extension) => extension.to_string_lossy().to_string(),
            None => match &self.default_extension {
                Some(extension) => extension.to_owned(),
                None => return Err(Box::new(ExtensionContentProviderError::NoDefaultExtension)),
            },
        };
        if let Some(content_provider) = self.extension_providers.get_mut(&extension) {
            content_provider.unpack_load(path)
        } else {
            Err(Box::new(
                ExtensionContentProviderError::ContentProviderForExtensionNotFound(extension),
            ))
        }
    }

    fn sanitize_path(&self, path: &str) -> Result<String, Box<dyn Error>> {
        let extension = match Path::new(path).extension() {
            Some(extension) => extension.to_string_lossy().to_string(),
            None => match &self.default_extension {
                Some(extension) => extension.to_owned(),
                None => return Err(Box::new(ExtensionContentProviderError::NoDefaultExtension)),
            },
        };
        if let Some(content_provider) = self.extension_providers.get(&extension) {
            content_provider.sanitize_path(path)
        } else {
            Err(Box::new(
                ExtensionContentProviderError::ContentProviderForExtensionNotFound(extension),
            ))
        }
    }

    fn join_paths(&self, parent: &str, relative: &str) -> Result<String, Box<dyn Error>> {
        let extension = match Path::new(relative).extension() {
            Some(extension) => extension.to_string_lossy().to_string(),
            None => match &self.default_extension {
                Some(extension) => extension.to_owned(),
                None => return Err(Box::new(ExtensionContentProviderError::NoDefaultExtension)),
            },
        };
        if let Some(content_provider) = self.extension_providers.get(&extension) {
            content_provider.join_paths(parent, relative)
        } else {
            Err(Box::new(
                ExtensionContentProviderError::ContentProviderForExtensionNotFound(extension),
            ))
        }
    }
}

/// What can go wrong while routing by extension.
#[derive(Debug)]
pub enum ExtensionContentProviderError {
    /// A path had no extension and no default was set.
    NoDefaultExtension,
    /// No provider is registered for that extension.
    ContentProviderForExtensionNotFound(String),
}

impl std::fmt::Display for ExtensionContentProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionContentProviderError::NoDefaultExtension => {
                write!(f, "No default extension set")
            }
            ExtensionContentProviderError::ContentProviderForExtensionNotFound(extension) => {
                write!(
                    f,
                    "Could not find content provider for extension: `{extension}`"
                )
            }
        }
    }
}

impl Error for ExtensionContentProviderError {}

/// A provider that loads nothing.
///
/// Useful for an extension that should be recognised but skipped.
pub struct IgnoreContentProvider;

impl<S> ScriptContentProvider<S> for IgnoreContentProvider {
    fn load(&mut self, _: &str) -> Result<Option<S>, Box<dyn Error>> {
        Ok(None)
    }

    fn join_paths(&self, parent: &str, relative: &str) -> Result<String, Box<dyn Error>> {
        Ok(format!("{parent}/{relative}"))
    }
}

/// Turns raw file bytes into whatever a frontend works with.
pub trait BytesContentParser<T> {
    /// Parses the bytes.
    fn parse(&self, bytes: Vec<u8>) -> Result<T, Box<dyn Error>>;
}

/// Loads scripts from the file system.
///
/// Paths without an extension get the configured one, and are canonicalized
/// so the same file reached by two paths is recognised as one.
pub struct FileContentProvider<T> {
    extension: String,
    parser: Box<dyn BytesContentParser<T>>,
}

impl<T> FileContentProvider<T> {
    /// Builds a provider for one extension and parser.
    pub fn new(extension: impl ToString, parser: impl BytesContentParser<T> + 'static) -> Self {
        Self {
            extension: extension.to_string(),
            parser: Box::new(parser),
        }
    }
}

impl<T> ScriptContentProvider<T> for FileContentProvider<T> {
    fn load(&mut self, path: &str) -> Result<Option<T>, Box<dyn Error>> {
        Ok(Some(self.parser.parse(std::fs::read(path)?)?))
    }

    fn sanitize_path(&self, path: &str) -> Result<String, Box<dyn Error>> {
        let mut result = PathBuf::from(path);
        if result.extension().is_none() {
            result.set_extension(&self.extension);
        }
        Ok(result.canonicalize()?.to_string_lossy().into_owned())
    }

    fn join_paths(&self, parent: &str, relative: &str) -> Result<String, Box<dyn Error>> {
        let mut path = PathBuf::from(parent);
        path.pop();
        Ok(path.join(relative).to_string_lossy().into_owned())
    }
}

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

pub type ScriptHandle<'a, SE> = Arc<Script<'a, SE>>;
pub type Script<'a, SE> = Vec<ScriptOperation<'a, SE>>;

pub trait ScriptExpression: Send + Sync {
    fn evaluate(&self, context: &mut Context, registry: &Registry);
}

impl ScriptExpression for () {
    fn evaluate(&self, _: &mut Context, _: &Registry) {}
}

#[allow(clippy::type_complexity)]
pub struct InlineExpression(Arc<dyn Fn(&mut Context, &Registry) + Send + Sync>);

impl InlineExpression {
    pub fn copied<T: Copy + Send + Sync + 'static>(value: T) -> Self {
        Self(Arc::new(move |context, _| {
            context.stack().push(value);
        }))
    }

    pub fn cloned<T: Clone + Send + Sync + 'static>(value: T) -> Self {
        Self(Arc::new(move |context, _| {
            context.stack().push(value.clone());
        }))
    }

    pub fn closure<F: Fn(&mut Context, &Registry) + Send + Sync + 'static>(f: F) -> Self {
        Self(Arc::new(f))
    }
}

impl ScriptExpression for InlineExpression {
    fn evaluate(&self, context: &mut Context, registry: &Registry) {
        (self.0)(context, registry);
    }
}

#[derive(Debug)]
pub enum ScriptOperation<'a, SE: ScriptExpression> {
    None,
    Expression {
        expression: SE,
    },
    DefineRegister {
        query: TypeQuery<'a>,
    },
    DropRegister {
        index: usize,
    },
    PushFromRegister {
        index: usize,
    },
    PopToRegister {
        index: usize,
    },
    MoveRegister {
        from: usize,
        to: usize,
    },
    CallFunction {
        query: FunctionQuery<'a>,
    },
    BranchScope {
        scope_success: ScriptHandle<'a, SE>,
        scope_failure: Option<ScriptHandle<'a, SE>>,
    },
    LoopScope {
        scope: ScriptHandle<'a, SE>,
    },
    PushScope {
        scope: ScriptHandle<'a, SE>,
    },
    PopScope,
    ContinueScopeConditionally,
    Suspend,
}

impl<SE: ScriptExpression> ScriptOperation<'_, SE> {
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

pub struct ScriptBuilder<'a, SE: ScriptExpression>(Script<'a, SE>);

impl<SE: ScriptExpression> Default for ScriptBuilder<'_, SE> {
    fn default() -> Self {
        Self(vec![])
    }
}

impl<'a, SE: ScriptExpression> ScriptBuilder<'a, SE> {
    pub fn build(self) -> ScriptHandle<'a, SE> {
        ScriptHandle::new(self.0)
    }

    pub fn expression(mut self, expression: SE) -> Self {
        self.0.push(ScriptOperation::Expression { expression });
        self
    }

    pub fn define_register(mut self, query: TypeQuery<'a>) -> Self {
        self.0.push(ScriptOperation::DefineRegister { query });
        self
    }

    pub fn drop_register(mut self, index: usize) -> Self {
        self.0.push(ScriptOperation::DropRegister { index });
        self
    }

    pub fn push_from_register(mut self, index: usize) -> Self {
        self.0.push(ScriptOperation::PushFromRegister { index });
        self
    }

    pub fn pop_to_register(mut self, index: usize) -> Self {
        self.0.push(ScriptOperation::PopToRegister { index });
        self
    }

    pub fn move_register(mut self, from: usize, to: usize) -> Self {
        self.0.push(ScriptOperation::MoveRegister { from, to });
        self
    }

    pub fn call_function(mut self, query: FunctionQuery<'a>) -> Self {
        self.0.push(ScriptOperation::CallFunction { query });
        self
    }

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

    pub fn loop_scope(mut self, scope: ScriptHandle<'a, SE>) -> Self {
        self.0.push(ScriptOperation::LoopScope { scope });
        self
    }

    pub fn push_scope(mut self, scope: ScriptHandle<'a, SE>) -> Self {
        self.0.push(ScriptOperation::PushScope { scope });
        self
    }

    pub fn pop_scope(mut self) -> Self {
        self.0.push(ScriptOperation::PopScope);
        self
    }

    pub fn continue_scope_conditionally(mut self) -> Self {
        self.0.push(ScriptOperation::ContinueScopeConditionally);
        self
    }

    pub fn suspend(mut self) -> Self {
        self.0.push(ScriptOperation::Suspend);
        self
    }
}

#[derive(Debug)]
pub struct ScriptFunctionParameter<'a> {
    pub meta: Option<Meta>,
    pub name: String,
    pub type_query: TypeQuery<'a>,
}

impl ScriptFunctionParameter<'_> {
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

#[derive(Debug)]
pub struct ScriptFunctionSignature<'a> {
    pub meta: Option<Meta>,
    pub name: String,
    pub module_name: Option<String>,
    pub type_query: Option<TypeQuery<'a>>,
    pub visibility: Visibility,
    pub inputs: Vec<ScriptFunctionParameter<'a>>,
    pub outputs: Vec<ScriptFunctionParameter<'a>>,
}

impl ScriptFunctionSignature<'_> {
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

#[derive(Debug)]
pub struct ScriptFunction<'a, SE: ScriptExpression> {
    pub signature: ScriptFunctionSignature<'a>,
    pub script: ScriptHandle<'a, SE>,
}

impl<SE: ScriptExpression> ScriptFunction<'static, SE> {
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

pub trait ScriptFunctionGenerator<SE: ScriptExpression> {
    type Input;
    type Output;

    fn generate_function_body(
        script: ScriptHandle<'static, SE>,
        input: Self::Input,
    ) -> Option<(FunctionBody, Self::Output)>;

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

#[derive(Debug)]
pub struct ScriptStructField<'a> {
    pub meta: Option<Meta>,
    pub name: String,
    pub visibility: Visibility,
    pub type_query: TypeQuery<'a>,
}

impl ScriptStructField<'_> {
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

#[derive(Debug)]
pub struct ScriptStruct<'a> {
    pub meta: Option<Meta>,
    pub name: String,
    pub module_name: Option<String>,
    pub visibility: Visibility,
    pub fields: Vec<ScriptStructField<'a>>,
}

impl ScriptStruct<'_> {
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

#[derive(Debug)]
pub struct ScriptEnumVariant<'a> {
    pub meta: Option<Meta>,
    pub name: String,
    pub fields: Vec<ScriptStructField<'a>>,
    pub discriminant: Option<u8>,
}

impl ScriptEnumVariant<'_> {
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

#[derive(Debug)]
pub struct ScriptEnum<'a> {
    pub meta: Option<Meta>,
    pub name: String,
    pub module_name: Option<String>,
    pub visibility: Visibility,
    pub variants: Vec<ScriptEnumVariant<'a>>,
    pub default_variant: Option<u8>,
}

impl ScriptEnum<'_> {
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

#[derive(Debug, Default)]
pub struct ScriptModule<'a, SE: ScriptExpression> {
    pub name: String,
    pub structs: Vec<ScriptStruct<'a>>,
    pub enums: Vec<ScriptEnum<'a>>,
    pub functions: Vec<ScriptFunction<'a, SE>>,
}

impl<SE: ScriptExpression> ScriptModule<'_, SE> {
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

    pub fn declare_types(&self, registry: &mut Registry) {
        for type_ in &self.structs {
            type_.declare(registry);
        }
        for type_ in &self.enums {
            type_.declare(registry);
        }
    }

    pub fn define_types(&self, registry: &mut Registry) {
        for type_ in &self.structs {
            type_.define(registry);
        }
        for type_ in &self.enums {
            type_.define(registry);
        }
    }

    pub fn install_types(&self, registry: &mut Registry) {
        self.declare_types(registry);
        self.define_types(registry);
    }
}

impl<SE: ScriptExpression> ScriptModule<'static, SE> {
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

#[derive(Debug, Default)]
pub struct ScriptPackage<'a, SE: ScriptExpression> {
    pub modules: Vec<ScriptModule<'a, SE>>,
}

impl<SE: ScriptExpression> ScriptPackage<'static, SE> {
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

pub struct ScriptContent<T> {
    pub path: String,
    pub name: String,
    pub data: Result<Option<T>, Box<dyn Error>>,
}

pub trait ScriptContentProvider<T> {
    fn load(&mut self, path: &str) -> Result<Option<T>, Box<dyn Error>>;

    fn unpack_load(&mut self, path: &str) -> Result<Vec<ScriptContent<T>>, Box<dyn Error>> {
        Ok(vec![ScriptContent {
            path: path.to_owned(),
            name: path.to_owned(),
            data: self.load(path),
        }])
    }

    fn sanitize_path(&self, path: &str) -> Result<String, Box<dyn Error>> {
        Ok(path.to_owned())
    }

    fn join_paths(&self, parent: &str, relative: &str) -> Result<String, Box<dyn Error>>;
}

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
    pub fn default_extension(mut self, extension: impl ToString) -> Self {
        self.default_extension = Some(extension.to_string());
        self
    }

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

#[derive(Debug)]
pub enum ExtensionContentProviderError {
    NoDefaultExtension,
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

pub struct IgnoreContentProvider;

impl<S> ScriptContentProvider<S> for IgnoreContentProvider {
    fn load(&mut self, _: &str) -> Result<Option<S>, Box<dyn Error>> {
        Ok(None)
    }

    fn join_paths(&self, parent: &str, relative: &str) -> Result<String, Box<dyn Error>> {
        Ok(format!("{parent}/{relative}"))
    }
}

pub trait BytesContentParser<T> {
    fn parse(&self, bytes: Vec<u8>) -> Result<T, Box<dyn Error>>;
}

pub struct FileContentProvider<T> {
    extension: String,
    parser: Box<dyn BytesContentParser<T>>,
}

impl<T> FileContentProvider<T> {
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

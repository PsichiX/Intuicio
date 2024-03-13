use intuicio_core::{
    context::Context,
    crate_version,
    function::FunctionQuery,
    meta::Meta,
    registry::Registry,
    script::{
        ScriptContentProvider, ScriptEnum, ScriptEnumVariant, ScriptExpression, ScriptFunction,
        ScriptFunctionParameter, ScriptFunctionSignature, ScriptHandle, ScriptModule,
        ScriptOperation, ScriptPackage, ScriptStruct, ScriptStructField,
    },
    types::TypeQuery,
    IntuicioVersion, Visibility,
};
use intuicio_nodes::nodes::{
    Node, NodeDefinition, NodeGraphVisitor, NodePin, NodeSuggestion, NodeTypeInfo, PropertyValue,
    ResponseSuggestionNode,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error};

pub type SerdeScript = Vec<SerdeOperation>;

pub fn frontend_serde_version() -> IntuicioVersion {
    crate_version!()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SerdeLiteral {
    Unit,
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    Isize(isize),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Usize(usize),
    F32(f32),
    F64(f64),
    Char(char),
    String(String),
}

impl SerdeLiteral {
    fn evaluate(&self, context: &mut Context) {
        match self {
            Self::Unit => context.stack().push(()),
            Self::Bool(value) => context.stack().push(*value),
            Self::I8(value) => context.stack().push(*value),
            Self::I16(value) => context.stack().push(*value),
            Self::I32(value) => context.stack().push(*value),
            Self::I64(value) => context.stack().push(*value),
            Self::I128(value) => context.stack().push(*value),
            Self::Isize(value) => context.stack().push(*value),
            Self::U8(value) => context.stack().push(*value),
            Self::U16(value) => context.stack().push(*value),
            Self::U32(value) => context.stack().push(*value),
            Self::U64(value) => context.stack().push(*value),
            Self::U128(value) => context.stack().push(*value),
            Self::Usize(value) => context.stack().push(*value),
            Self::F32(value) => context.stack().push(*value),
            Self::F64(value) => context.stack().push(*value),
            Self::Char(value) => context.stack().push(*value),
            Self::String(value) => context.stack().push(value.to_owned()),
        };
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SerdeExpression {
    Literal(SerdeLiteral),
    StackDrop,
}

impl ScriptExpression for SerdeExpression {
    fn evaluate(&self, context: &mut Context, _: &Registry) {
        match self {
            Self::Literal(literal) => {
                literal.evaluate(context);
            }
            Self::StackDrop => {
                context.stack().drop();
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SerdeOperation {
    Expression(SerdeExpression),
    MakeRegister {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        module_name: Option<String>,
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
    CallFunction {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        module_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        type_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visibility: Option<Visibility>,
    },
    BranchScope {
        script_success: SerdeScript,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        script_failure: Option<SerdeScript>,
    },
    LoopScope {
        script: SerdeScript,
    },
    PushScope {
        script: SerdeScript,
    },
    PopScope,
}

fn build_script(script: &SerdeScript) -> ScriptHandle<'static, SerdeExpression> {
    ScriptHandle::new(
        script
            .iter()
            .map(|operation| match operation {
                SerdeOperation::Expression(expression) => ScriptOperation::Expression {
                    expression: expression.to_owned(),
                },
                SerdeOperation::MakeRegister { name, module_name } => {
                    ScriptOperation::DefineRegister {
                        query: TypeQuery {
                            name: Some(name.to_owned().into()),
                            module_name: module_name.as_ref().map(|name| name.to_owned().into()),
                            ..Default::default()
                        },
                    }
                }
                SerdeOperation::DropRegister { index } => {
                    ScriptOperation::DropRegister { index: *index }
                }
                SerdeOperation::PushFromRegister { index } => {
                    ScriptOperation::PushFromRegister { index: *index }
                }
                SerdeOperation::PopToRegister { index } => {
                    ScriptOperation::PopToRegister { index: *index }
                }
                SerdeOperation::CallFunction {
                    name,
                    module_name,
                    type_name,
                    visibility,
                } => ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some(name.to_owned().into()),
                        module_name: module_name.as_ref().map(|name| name.to_owned().into()),
                        type_query: type_name.as_ref().map(|name| TypeQuery {
                            name: Some(name.to_owned().into()),
                            module_name: module_name.as_ref().map(|name| name.to_owned().into()),
                            ..Default::default()
                        }),
                        visibility: *visibility,
                        ..Default::default()
                    },
                },
                SerdeOperation::BranchScope {
                    script_success: operations_success,
                    script_failure: operations_failure,
                } => ScriptOperation::BranchScope {
                    scope_success: build_script(operations_success),
                    scope_failure: operations_failure.as_ref().map(build_script),
                },
                SerdeOperation::LoopScope { script: operations } => ScriptOperation::LoopScope {
                    scope: build_script(operations),
                },
                SerdeOperation::PushScope { script: operations } => ScriptOperation::PushScope {
                    scope: build_script(operations),
                },
                SerdeOperation::PopScope => ScriptOperation::PopScope,
            })
            .collect(),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeFunctionParameter {
    pub meta: Option<Meta>,
    pub name: String,
    pub module_name: Option<String>,
    pub type_name: String,
}

impl SerdeFunctionParameter {
    pub fn compile(&self) -> ScriptFunctionParameter<'static> {
        ScriptFunctionParameter {
            meta: self.meta.to_owned(),
            name: self.name.to_owned(),
            type_query: TypeQuery {
                name: Some(self.type_name.to_owned().into()),
                module_name: self
                    .module_name
                    .as_ref()
                    .map(|module_name| module_name.to_owned().into()),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeFunction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default)]
    pub visibility: Visibility,
    #[serde(default)]
    pub inputs: Vec<SerdeFunctionParameter>,
    #[serde(default)]
    pub outputs: Vec<SerdeFunctionParameter>,
    pub script: SerdeScript,
}

impl SerdeFunction {
    pub fn compile(&self, module_name: &str) -> ScriptFunction<'static, SerdeExpression> {
        ScriptFunction {
            signature: ScriptFunctionSignature {
                meta: self.meta.to_owned(),
                name: self.name.to_owned(),
                module_name: Some(module_name.to_owned()),
                type_query: self.type_name.as_ref().map(|type_name| TypeQuery {
                    name: Some(type_name.to_owned().into()),
                    ..Default::default()
                }),
                visibility: self.visibility,
                inputs: self
                    .inputs
                    .iter()
                    .map(|parameter| parameter.compile())
                    .collect(),
                outputs: self
                    .outputs
                    .iter()
                    .map(|parameter| parameter.compile())
                    .collect(),
            },
            script: build_script(&self.script),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeStructField {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Visibility::is_public")]
    pub visibility: Visibility,
    pub module_name: Option<String>,
    pub type_name: String,
}

impl SerdeStructField {
    pub fn compile(&self) -> ScriptStructField<'static> {
        ScriptStructField {
            meta: self.meta.to_owned(),
            name: self.name.to_owned(),
            visibility: self.visibility,
            type_query: TypeQuery {
                name: Some(self.type_name.to_owned().into()),
                module_name: self
                    .module_name
                    .as_ref()
                    .map(|module_name| module_name.to_owned().into()),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeStruct {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Visibility::is_public")]
    pub visibility: Visibility,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<SerdeStructField>,
}

impl SerdeStruct {
    pub fn compile(&self, module_name: &str) -> ScriptStruct<'static> {
        ScriptStruct {
            meta: self.meta.to_owned(),
            name: self.name.to_owned(),
            module_name: Some(module_name.to_owned()),
            visibility: self.visibility,
            fields: self.fields.iter().map(|field| field.compile()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeEnumVariant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub name: String,
    pub fields: Vec<SerdeStructField>,
    pub discriminant: Option<u8>,
}

impl SerdeEnumVariant {
    pub fn compile(&self) -> ScriptEnumVariant<'static> {
        ScriptEnumVariant {
            meta: self.meta.to_owned(),
            name: self.name.to_owned(),
            fields: self.fields.iter().map(|field| field.compile()).collect(),
            discriminant: self.discriminant,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeEnum {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Visibility::is_public")]
    pub visibility: Visibility,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<SerdeEnumVariant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_variant: Option<u8>,
}

impl SerdeEnum {
    pub fn compile(&self, module_name: &str) -> ScriptEnum<'static> {
        ScriptEnum {
            meta: self.meta.to_owned(),
            name: self.name.to_owned(),
            module_name: Some(module_name.to_owned()),
            visibility: self.visibility,
            variants: self
                .variants
                .iter()
                .map(|variant| variant.compile())
                .collect(),
            default_variant: self.default_variant,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeModule {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structs: Vec<SerdeStruct>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enums: Vec<SerdeEnum>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<SerdeFunction>,
}

impl SerdeModule {
    pub fn compile(&self) -> ScriptModule<'static, SerdeExpression> {
        ScriptModule {
            name: self.name.to_owned(),
            structs: self
                .structs
                .iter()
                .map(|struct_type| struct_type.compile(&self.name))
                .collect(),
            enums: self
                .enums
                .iter()
                .map(|enum_type| enum_type.compile(&self.name))
                .collect(),
            functions: self
                .functions
                .iter()
                .map(|function| function.compile(&self.name))
                .collect(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SerdeFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<SerdeModule>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SerdePackage {
    pub files: HashMap<String, SerdeFile>,
}

impl SerdePackage {
    pub fn new<CP>(path: &str, content_provider: &mut CP) -> Result<Self, Box<dyn Error>>
    where
        CP: ScriptContentProvider<SerdeFile>,
    {
        let mut result = Self::default();
        result.load(path, content_provider)?;
        Ok(result)
    }

    pub fn load<CP>(&mut self, path: &str, content_provider: &mut CP) -> Result<(), Box<dyn Error>>
    where
        CP: ScriptContentProvider<SerdeFile>,
    {
        let path = content_provider.sanitize_path(path)?;
        if self.files.contains_key(&path) {
            return Ok(());
        }
        for content in content_provider.unpack_load(&path)? {
            if let Some(file) = content.data? {
                let dependencies = file.dependencies.to_owned();
                self.files.insert(content.name, file);
                for relative in dependencies {
                    let path = content_provider.join_paths(&content.path, &relative)?;
                    self.load(&path, content_provider)?;
                }
            }
        }
        Ok(())
    }

    pub fn compile(&self) -> ScriptPackage<'static, SerdeExpression> {
        ScriptPackage {
            modules: self
                .files
                .values()
                .flat_map(|file| file.modules.iter())
                .map(|module| module.compile())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerdeNodeTypeInfo {
    pub name: String,
    pub module_name: Option<String>,
}

impl SerdeNodeTypeInfo {
    pub fn new(name: impl ToString, module_name: Option<impl ToString>) -> Self {
        Self {
            name: name.to_string(),
            module_name: module_name.map(|name| name.to_string()),
        }
    }
}

impl NodeTypeInfo for SerdeNodeTypeInfo {
    fn type_query(&self) -> TypeQuery {
        TypeQuery {
            name: Some(self.name.as_str().into()),
            module_name: self.module_name.as_ref().map(|name| name.into()),
            ..Default::default()
        }
    }

    fn are_compatible(&self, other: &Self) -> bool {
        self == other
    }
}

impl std::fmt::Display for SerdeNodeTypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}::{}",
            self.name,
            self.module_name.as_deref().unwrap_or("")
        )
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub enum SerdeNodes {
    #[default]
    Start,
    Operation(SerdeOperation),
}

impl NodeDefinition for SerdeNodes {
    type TypeInfo = SerdeNodeTypeInfo;

    fn node_label(&self, _: &Registry) -> String {
        match self {
            Self::Start => "Start".to_owned(),
            Self::Operation(operation) => match operation {
                SerdeOperation::Expression(expression) => match expression {
                    SerdeExpression::Literal(literal) => match literal {
                        SerdeLiteral::Unit => "Unit literal".to_owned(),
                        SerdeLiteral::Bool(_) => "Boolean literal".to_owned(),
                        SerdeLiteral::I8(_) => "Signed 8-bit literal".to_owned(),
                        SerdeLiteral::I16(_) => "Signed 16-bit literal".to_owned(),
                        SerdeLiteral::I32(_) => "Signed 32-bit literal".to_owned(),
                        SerdeLiteral::I64(_) => "Signed 64-bit literal".to_owned(),
                        SerdeLiteral::I128(_) => "Signed 128-bit literal".to_owned(),
                        SerdeLiteral::Isize(_) => "Signed size literal".to_owned(),
                        SerdeLiteral::U8(_) => "Unsigned 8-bit literal".to_owned(),
                        SerdeLiteral::U16(_) => "Unsigned 16-bit literal".to_owned(),
                        SerdeLiteral::U32(_) => "Unsigned 32-bit literal".to_owned(),
                        SerdeLiteral::U64(_) => "Unsigned 64-bit literal".to_owned(),
                        SerdeLiteral::U128(_) => "Unsigned 128-bit literal".to_owned(),
                        SerdeLiteral::Usize(_) => "Unsigned size literal".to_owned(),
                        SerdeLiteral::F32(_) => "32-bit float literal".to_owned(),
                        SerdeLiteral::F64(_) => "64-bit float literal".to_owned(),
                        SerdeLiteral::Char(_) => "Character literal".to_owned(),
                        SerdeLiteral::String(_) => "String literal".to_owned(),
                    },
                    SerdeExpression::StackDrop => "Stack drop".to_owned(),
                },
                SerdeOperation::MakeRegister { .. } => "Make register".to_owned(),
                SerdeOperation::DropRegister { .. } => "Drop register".to_owned(),
                SerdeOperation::PushFromRegister { .. } => {
                    "Push data from register to stack".to_owned()
                }
                SerdeOperation::PopToRegister { .. } => {
                    "Pop data from stack to register".to_owned()
                }
                SerdeOperation::CallFunction {
                    name, module_name, ..
                } => format!(
                    "Call function: `{}::{}`",
                    module_name.as_deref().unwrap_or(""),
                    name
                ),
                SerdeOperation::BranchScope { .. } => "Branch scope".to_owned(),
                SerdeOperation::LoopScope { .. } => "Loop scope".to_owned(),
                SerdeOperation::PushScope { .. } => "Push scope".to_owned(),
                SerdeOperation::PopScope => "Pop scope".to_owned(),
            },
        }
    }

    fn node_pins_in(&self, _: &Registry) -> Vec<NodePin<Self::TypeInfo>> {
        match self {
            Self::Start => vec![],
            Self::Operation(operation) => match operation {
                SerdeOperation::Expression(expression) => match expression {
                    SerdeExpression::Literal(literal) => match literal {
                        SerdeLiteral::Unit => vec![NodePin::execute("In", false)],
                        _ => vec![NodePin::execute("In", false), NodePin::property("Value")],
                    },
                    SerdeExpression::StackDrop => vec![NodePin::execute("In", false)],
                },
                SerdeOperation::MakeRegister { .. } => vec![
                    NodePin::execute("In", false),
                    NodePin::property("Type name"),
                    NodePin::property("Type module name"),
                ],
                SerdeOperation::DropRegister { .. }
                | SerdeOperation::PushFromRegister { .. }
                | SerdeOperation::PopToRegister { .. } => {
                    vec![NodePin::execute("In", false), NodePin::property("Index")]
                }
                SerdeOperation::CallFunction { .. } => vec![
                    NodePin::execute("In", false),
                    NodePin::property("Name"),
                    NodePin::property("Module name"),
                    NodePin::property("Type name"),
                    NodePin::property("Visibility"),
                ],
                _ => vec![NodePin::execute("In", false)],
            },
        }
    }

    fn node_pins_out(&self, _: &Registry) -> Vec<NodePin<Self::TypeInfo>> {
        match self {
            Self::Start => vec![NodePin::execute("Out", false)],
            Self::Operation(operation) => match operation {
                SerdeOperation::BranchScope { .. } => vec![
                    NodePin::execute("Out", false),
                    NodePin::execute("Success body", true),
                    NodePin::execute("Failure body", true),
                ],
                SerdeOperation::LoopScope { .. } | SerdeOperation::PushScope { .. } => vec![
                    NodePin::execute("Out", false),
                    NodePin::execute("Body", true),
                ],
                SerdeOperation::PopScope => vec![],
                _ => vec![NodePin::execute("Out", false)],
            },
        }
    }

    fn node_is_start(&self, _: &Registry) -> bool {
        matches!(self, SerdeNodes::Start)
    }

    fn node_suggestions(
        x: i64,
        y: i64,
        _: NodeSuggestion<Self>,
        registry: &Registry,
    ) -> Vec<ResponseSuggestionNode<Self>> {
        vec![
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::Unit,
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::Bool(true),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::I8(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::I16(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::I32(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::I64(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::I128(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::Isize(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::U8(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::U16(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::U32(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::U64(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::U128(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::Usize(0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::F32(0.0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::F64(0.0),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::Char('@'),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::String("text".to_owned()),
                    ))),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Expression",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::StackDrop)),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Register",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::MakeRegister {
                        name: "Type".to_owned(),
                        module_name: None,
                    }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Register",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::DropRegister { index: 0 }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Register",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::PushFromRegister { index: 0 }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Register",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::PopToRegister { index: 0 }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Call",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::CallFunction {
                        name: "Function".to_owned(),
                        module_name: None,
                        type_name: None,
                        visibility: None,
                    }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Scope",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::BranchScope {
                        script_success: vec![],
                        script_failure: None,
                    }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Scope",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::LoopScope { script: vec![] }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Scope",
                Node::new(
                    x,
                    y,
                    SerdeNodes::Operation(SerdeOperation::PushScope { script: vec![] }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Scope",
                Node::new(x, y, SerdeNodes::Operation(SerdeOperation::PopScope)),
                registry,
            ),
        ]
    }

    fn get_property(&self, property_name: &str) -> Option<PropertyValue> {
        match self {
            Self::Operation(operation) => match operation {
                SerdeOperation::Expression(SerdeExpression::Literal(literal)) => {
                    match property_name {
                        "Value" => match literal {
                            SerdeLiteral::Unit => PropertyValue::new(&()).ok(),
                            SerdeLiteral::Bool(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::I8(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::I16(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::I32(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::I64(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::I128(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::Isize(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::U8(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::U16(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::U32(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::U64(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::U128(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::Usize(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::F32(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::F64(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::Char(value) => PropertyValue::new(value).ok(),
                            SerdeLiteral::String(value) => PropertyValue::new(value).ok(),
                        },
                        _ => None,
                    }
                }
                SerdeOperation::MakeRegister { name, module_name } => match property_name {
                    "Type name" => PropertyValue::new(name).ok(),
                    "Type module name" => module_name
                        .as_ref()
                        .and_then(|name| PropertyValue::new(name).ok()),
                    _ => None,
                },
                SerdeOperation::DropRegister { index } => match property_name {
                    "Index" => PropertyValue::new(index).ok(),
                    _ => None,
                },
                SerdeOperation::PushFromRegister { index } => match property_name {
                    "Index" => PropertyValue::new(index).ok(),
                    _ => None,
                },
                SerdeOperation::PopToRegister { index } => match property_name {
                    "Index" => PropertyValue::new(index).ok(),
                    _ => None,
                },
                SerdeOperation::CallFunction {
                    name,
                    module_name,
                    type_name,
                    visibility,
                } => match property_name {
                    "Name" => PropertyValue::new(name).ok(),
                    "Module name" => module_name
                        .as_ref()
                        .and_then(|name| PropertyValue::new(name).ok()),
                    "Type name" => type_name
                        .as_ref()
                        .and_then(|name| PropertyValue::new(name).ok()),
                    "Visibility" => visibility
                        .as_ref()
                        .and_then(|name| PropertyValue::new(name).ok()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    fn set_property(&mut self, property_name: &str, property_value: PropertyValue) {
        if let Self::Operation(operation) = self {
            match operation {
                SerdeOperation::Expression(SerdeExpression::Literal(literal)) => {
                    if property_name == "Value" {
                        match literal {
                            SerdeLiteral::Unit => {}
                            SerdeLiteral::Bool(value) => {
                                if let Ok(v) = property_value.get_exact::<bool>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::I8(value) => {
                                if let Ok(v) = property_value.get_exact::<i8>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::I16(value) => {
                                if let Ok(v) = property_value.get_exact::<i16>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::I32(value) => {
                                if let Ok(v) = property_value.get_exact::<i32>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::I64(value) => {
                                if let Ok(v) = property_value.get_exact::<i64>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::I128(value) => {
                                if let Ok(v) = property_value.get_exact::<i128>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::Isize(value) => {
                                if let Ok(v) = property_value.get_exact::<isize>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::U8(value) => {
                                if let Ok(v) = property_value.get_exact::<u8>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::U16(value) => {
                                if let Ok(v) = property_value.get_exact::<u16>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::U32(value) => {
                                if let Ok(v) = property_value.get_exact::<u32>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::U64(value) => {
                                if let Ok(v) = property_value.get_exact::<u64>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::U128(value) => {
                                if let Ok(v) = property_value.get_exact::<u128>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::Usize(value) => {
                                if let Ok(v) = property_value.get_exact::<usize>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::F32(value) => {
                                if let Ok(v) = property_value.get_exact::<f32>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::F64(value) => {
                                if let Ok(v) = property_value.get_exact::<f64>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::Char(value) => {
                                if let Ok(v) = property_value.get_exact::<char>() {
                                    *value = v;
                                }
                            }
                            SerdeLiteral::String(value) => {
                                if let Ok(v) = property_value.get_exact::<String>() {
                                    *value = v;
                                }
                            }
                        }
                    }
                }
                SerdeOperation::MakeRegister { name, module_name } => match property_name {
                    "Type name" => {
                        if let Ok(v) = property_value.get_exact::<String>() {
                            *name = v;
                        }
                    }
                    "Type module name" => {
                        *module_name = if let Ok(v) = property_value.get_exact::<String>() {
                            Some(v)
                        } else {
                            None
                        };
                    }
                    _ => {}
                },
                SerdeOperation::DropRegister { index } => {
                    if property_name == "Index" {
                        if let Ok(v) = property_value.get_exact::<usize>() {
                            *index = v;
                        }
                    }
                }
                SerdeOperation::PushFromRegister { index } => {
                    if property_name == "Index" {
                        if let Ok(v) = property_value.get_exact::<usize>() {
                            *index = v;
                        }
                    }
                }
                SerdeOperation::PopToRegister { index } => {
                    if property_name == "Index" {
                        if let Ok(v) = property_value.get_exact::<usize>() {
                            *index = v;
                        }
                    }
                }
                SerdeOperation::CallFunction {
                    name,
                    module_name,
                    type_name,
                    visibility,
                } => match property_name {
                    "Name" => {
                        if let Ok(v) = property_value.get_exact::<String>() {
                            *name = v;
                        }
                    }
                    "Module name" => {
                        *module_name = if let Ok(v) = property_value.get_exact::<String>() {
                            Some(v)
                        } else {
                            None
                        };
                    }
                    "Type name" => {
                        *type_name = if let Ok(v) = property_value.get_exact::<String>() {
                            Some(v)
                        } else {
                            None
                        };
                    }
                    "Visibility" => {
                        *visibility = if let Ok(v) = property_value.get_exact::<Visibility>() {
                            Some(v)
                        } else {
                            None
                        };
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

pub struct CompileSerdeNodeGraphVisitor;

impl NodeGraphVisitor<SerdeNodes> for CompileSerdeNodeGraphVisitor {
    type Input = ();
    type Output = SerdeOperation;

    fn visit_statement(
        &mut self,
        node: &Node<SerdeNodes>,
        _: HashMap<String, Self::Input>,
        mut scopes: HashMap<String, Vec<Self::Output>>,
        result: &mut Vec<Self::Output>,
    ) -> bool {
        if let SerdeNodes::Operation(operation) = &node.data {
            match operation {
                SerdeOperation::BranchScope { .. } => {
                    if let Some(script_success) = scopes.remove("Success body") {
                        result.push(SerdeOperation::BranchScope {
                            script_success,
                            script_failure: scopes.remove("Failure body"),
                        });
                    }
                }
                SerdeOperation::LoopScope { .. } => {
                    if let Some(script) = scopes.remove("Body") {
                        result.push(SerdeOperation::LoopScope { script });
                    }
                }
                SerdeOperation::PushScope { .. } => {
                    if let Some(script) = scopes.remove("Body") {
                        result.push(SerdeOperation::PushScope { script });
                    }
                }
                _ => result.push(operation.to_owned()),
            }
        }
        true
    }

    fn visit_expression(
        &mut self,
        _: &Node<SerdeNodes>,
        _: HashMap<String, Self::Input>,
    ) -> Option<Self::Input> {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use intuicio_backend_vm::prelude::*;
    use intuicio_core::prelude::*;
    use intuicio_nodes::nodes::*;

    pub struct LexprContentParser;

    impl BytesContentParser<SerdeFile> for LexprContentParser {
        fn parse(&self, bytes: Vec<u8>) -> Result<SerdeFile, Box<dyn Error>> {
            let content = String::from_utf8(bytes)?;
            Ok(serde_lexpr::from_str::<SerdeFile>(&content)?)
        }
    }

    #[test]
    fn test_frontend_lexpr() {
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(define_function! {
            registry => mod intrinsics fn add(a: usize, b: usize) -> (result: usize) {
                (a + b,)
            }
        });
        let mut content_provider = FileContentProvider::new("lexpr", LexprContentParser);
        SerdePackage::new("../../resources/package.lexpr", &mut content_provider)
            .unwrap()
            .compile()
            .install::<VmScope<SerdeExpression>>(
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
        assert!(registry
            .find_function(FunctionQuery {
                name: Some("main".into()),
                module_name: Some("test".into()),
                ..Default::default()
            })
            .is_some());
        let mut host = Host::new(Context::new(10240, 10240), RegistryHandle::new(registry));
        let (result,) = host
            .call_function::<(usize,), _>("main", "test", None)
            .unwrap()
            .run(());
        assert_eq!(result, 42);
    }

    #[test]
    fn test_nodes() {
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(define_function! {
            registry => mod intrinsics fn add(a: usize, b: usize) -> (result: usize) {
                (a + b,)
            }
        });
        let mut graph = NodeGraph::default();
        let start = graph
            .add_node(Node::new(0, 0, SerdeNodes::Start), &registry)
            .unwrap();
        let literal_a = graph
            .add_node(
                Node::new(
                    0,
                    0,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::I32(2),
                    ))),
                ),
                &registry,
            )
            .unwrap();
        let literal_b = graph
            .add_node(
                Node::new(
                    0,
                    0,
                    SerdeNodes::Operation(SerdeOperation::Expression(SerdeExpression::Literal(
                        SerdeLiteral::I32(40),
                    ))),
                ),
                &registry,
            )
            .unwrap();
        let call_add = graph
            .add_node(
                Node::new(
                    0,
                    0,
                    SerdeNodes::Operation(SerdeOperation::CallFunction {
                        name: "add".to_owned(),
                        module_name: Some("intrinsics".to_owned()),
                        type_name: None,
                        visibility: None,
                    }),
                ),
                &registry,
            )
            .unwrap();
        graph.connect_nodes(NodeConnection::new(start, literal_a, "Out", "In"));
        graph.connect_nodes(NodeConnection::new(literal_a, literal_b, "Out", "In"));
        graph.connect_nodes(NodeConnection::new(literal_b, call_add, "Out", "In"));
        graph.validate(&registry).unwrap();
        assert_eq!(
            graph.visit(&mut CompileSerdeNodeGraphVisitor, &registry),
            vec![
                SerdeOperation::Expression(SerdeExpression::Literal(SerdeLiteral::I32(2))),
                SerdeOperation::Expression(SerdeExpression::Literal(SerdeLiteral::I32(40))),
                SerdeOperation::CallFunction {
                    name: "add".to_owned(),
                    module_name: Some("intrinsics".to_owned()),
                    type_name: None,
                    visibility: None,
                }
            ]
        );

        {
            let mut graph = graph.clone();
            graph.connect_nodes(NodeConnection {
                from_node: call_add,
                to_node: call_add,
                from_pin: "Out".to_owned(),
                to_pin: "In".to_owned(),
            });
            assert!(matches!(
                graph.validate(&registry).unwrap_err()[0],
                NodeGraphError::Connection(ConnectionError::InternalConnection(_))
            ));
        }

        {
            let mut graph = graph.clone();
            graph.connect_nodes(NodeConnection {
                from_node: literal_a,
                to_node: literal_b,
                from_pin: "Out".to_owned(),
                to_pin: "Body".to_owned(),
            });
            assert!(matches!(
                graph.validate(&registry).unwrap_err()[0],
                NodeGraphError::Connection(ConnectionError::TargetPinNotFound { .. })
            ));
        }

        {
            let mut graph = graph.clone();
            graph.connect_nodes(NodeConnection {
                from_node: literal_a,
                to_node: literal_b,
                from_pin: "Out".to_owned(),
                to_pin: "Value".to_owned(),
            });
            assert!(matches!(
                graph.validate(&registry).unwrap_err()[0],
                NodeGraphError::Connection(ConnectionError::MismatchPins { .. })
            ));
        }

        {
            let mut graph = graph.clone();
            graph.connect_nodes(NodeConnection {
                from_node: call_add,
                to_node: literal_a,
                from_pin: "Out".to_owned(),
                to_pin: "In".to_owned(),
            });
            assert!(matches!(
                graph.validate(&registry).unwrap_err()[0],
                NodeGraphError::Connection(ConnectionError::CycleNodeFound { .. })
            ));
        }
    }
}

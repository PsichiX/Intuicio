use intuicio_core::{
    context::Context,
    function::FunctionQuery,
    registry::Registry,
    script::{
        ScriptExpression, ScriptFunction, ScriptFunctionParameter, ScriptFunctionSignature,
        ScriptHandle, ScriptModule, ScriptOperation, ScriptPackage, ScriptStruct,
        ScriptStructField,
    },
    struct_type::StructQuery,
    Visibility,
};
use serde::{Deserialize, Serialize};

pub type SerdeScript = Vec<SerdeOperation>;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerdeExpression {
    Literal(SerdeLiteral),
}

impl ScriptExpression for SerdeExpression {
    fn evaluate(&self, context: &mut Context, _: &Registry) {
        match self {
            Self::Literal(literal) => {
                literal.evaluate(context);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeOperationFunctionQueryParameter {
    pub name: Option<String>,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        struct_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        visibility: Option<Visibility>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        inputs: Vec<SerdeOperationFunctionQueryParameter>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        outputs: Vec<SerdeOperationFunctionQueryParameter>,
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
                        query: StructQuery {
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
                    struct_name,
                    visibility,
                    ..
                } => ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some(name.to_owned().into()),
                        module_name: module_name.as_ref().map(|name| name.to_owned().into()),
                        struct_query: struct_name.as_ref().map(|name| StructQuery {
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
                    scope_failure: operations_failure
                        .as_ref()
                        .map(|script| build_script(script)),
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
    pub name: String,
    pub struct_name: String,
}

impl SerdeFunctionParameter {
    pub fn compile(&self) -> ScriptFunctionParameter<'static> {
        ScriptFunctionParameter {
            name: self.name.to_owned(),
            struct_query: StructQuery {
                name: Some(self.struct_name.as_str().to_owned().into()),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeFunction {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub struct_name: Option<String>,
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
                name: self.name.to_owned(),
                module_name: Some(module_name.to_owned().into()),
                struct_query: self.struct_name.as_ref().map(|struct_name| StructQuery {
                    name: Some(struct_name.to_owned().into()),
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
    pub name: String,
    #[serde(default, skip_serializing_if = "Visibility::is_public")]
    pub visibility: Visibility,
    pub struct_name: String,
}

impl SerdeStructField {
    pub fn compile(&self) -> ScriptStructField<'static> {
        ScriptStructField {
            name: self.name.to_owned(),
            visibility: self.visibility,
            struct_query: StructQuery {
                name: Some(self.struct_name.as_str().to_owned().into()),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeStruct {
    pub name: String,
    #[serde(default, skip_serializing_if = "Visibility::is_public")]
    pub visibility: Visibility,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<SerdeStructField>,
}

impl SerdeStruct {
    pub fn compile(&self, module_name: &str) -> ScriptStruct<'static> {
        ScriptStruct {
            name: self.name.to_owned(),
            module_name: Some(module_name.to_owned()),
            visibility: self.visibility,
            fields: self.fields.iter().map(|field| field.compile()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeModule {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structs: Vec<SerdeStruct>,
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
            functions: self
                .functions
                .iter()
                .map(|function| function.compile(&self.name))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdePackage {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<SerdeModule>,
}

impl SerdePackage {
    pub fn compile(&self) -> ScriptPackage<'static, SerdeExpression> {
        ScriptPackage {
            modules: self.modules.iter().map(|module| module.compile()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use intuicio_backend_vm::prelude::*;
    use intuicio_core::prelude::*;

    #[test]
    fn test_frontend_lexpr() {
        let content = std::fs::read_to_string("../../resources/package.lexpr").unwrap();
        let package = serde_lexpr::from_str::<SerdePackage>(&content).unwrap();
        println!("LEXPR: {}", serde_lexpr::to_string(&package).unwrap());
        let mut registry = Registry::default().with_basic_types();
        let i32_handle = registry.find_struct(StructQuery::of::<i32>()).unwrap();
        registry.add_function(Function::new(
            FunctionSignature::new("add")
                .with_module_name("intrinsics")
                .with_input(FunctionParameter::new("a", i32_handle.clone()))
                .with_input(FunctionParameter::new("b", i32_handle.clone()))
                .with_output(FunctionParameter::new("result", i32_handle)),
            FunctionBody::closure(|context, _| {
                let a = context.stack().pop::<usize>().unwrap();
                let b = context.stack().pop::<usize>().unwrap();
                context.stack().push(a + b);
            }),
        ));
        package
            .compile()
            .install::<VmScope<SerdeExpression>>(&mut registry, None);
        assert!(registry
            .find_function(FunctionQuery {
                name: Some("main".into()),
                module_name: Some("test".into()),
                ..Default::default()
            })
            .is_some());
        let mut host = Host::new(
            Context::new(1024, 1024, 1024),
            RegistryHandle::new(registry),
        );
        let (result,) = host
            .call_function::<(usize,), _>("main", "test", None)
            .unwrap()
            .run(());
        assert_eq!(result, 42);
    }
}

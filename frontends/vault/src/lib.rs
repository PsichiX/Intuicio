use intuicio_core::{
    context::Context,
    function::{FunctionQuery, FunctionQueryParameter},
    registry::Registry,
    script::{
        BytesContentParser, ScriptContentProvider, ScriptExpression, ScriptFunction,
        ScriptFunctionParameter, ScriptFunctionSignature, ScriptHandle, ScriptModule,
        ScriptOperation, ScriptPackage, ScriptStruct, ScriptStructField,
    },
    struct_type::StructQuery,
    Visibility,
};
use serde::{Deserialize, Serialize};
use std::{any::TypeId, collections::HashMap, error::Error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VaultLiteral {
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

impl VaultLiteral {
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

#[derive(Debug)]
pub enum VaultScriptExpression {
    Literal(VaultLiteral),
    StackDrop,
    StackProduce { name: String },
}

impl ScriptExpression for VaultScriptExpression {
    fn evaluate(&self, context: &mut Context, registry: &Registry) {
        match self {
            Self::Literal(literal) => {
                literal.evaluate(context);
            }
            Self::StackDrop => {
                context.stack().drop();
            }
            Self::StackProduce { name } => {
                let type_id = context.stack().peek().unwrap();
                registry
                    .find_function(FunctionQuery {
                        name: Some(name.into()),
                        struct_query: Some(StructQuery {
                            type_id: Some(type_id),
                            ..Default::default()
                        }),
                        inputs: [FunctionQueryParameter {
                            struct_query: Some(StructQuery {
                                type_id: Some(type_id),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }]
                        .as_slice()
                        .into(),
                        ..Default::default()
                    })
                    .unwrap()
                    .invoke(context, registry);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VaultExpression {
    DefineVariable {
        name: String,
    },
    TakeVariable {
        name: String,
    },
    CloneVariable {
        name: String,
    },
    VariableRef {
        name: String,
    },
    VariableRefMut {
        name: String,
    },
    Literal(VaultLiteral),
    CallFunction {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        module_name: Option<String>,
        name: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        arguments: Vec<VaultExpression>,
    },
    CallMethod {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        module_name: Option<String>,
        struct_name: String,
        name: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        arguments: Vec<VaultExpression>,
    },
    If {
        condition: Box<VaultExpression>,
        success: Vec<VaultStatement>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure: Option<Vec<VaultStatement>>,
    },
}

impl VaultExpression {
    pub fn compile(
        &self,
        result: &mut Vec<ScriptOperation<VaultScriptExpression>>,
        registers: &mut Vec<String>,
    ) {
        match self {
            Self::DefineVariable { name } => {
                if let Some(item) = registers.iter_mut().find(|n| n == &name) {
                    *item = name.to_owned();
                } else {
                    registers.push(name.to_owned());
                }
            }
            Self::TakeVariable { name } => {
                result.push(ScriptOperation::PushFromRegister {
                    index: registers.iter().position(|n| n == name.as_str()).unwrap(),
                });
            }
            Self::CloneVariable { name } => {
                result.push(ScriptOperation::PushFromRegister {
                    index: registers.iter().position(|n| n == name.as_str()).unwrap(),
                });
                result.push(ScriptOperation::Expression {
                    expression: VaultScriptExpression::StackProduce {
                        name: "clone".to_owned(),
                    },
                });
                result.push(ScriptOperation::PopToRegister {
                    index: registers.iter().position(|n| n == name.as_str()).unwrap(),
                });
            }
            Self::VariableRef { name } => {
                result.push(ScriptOperation::PushFromRegister {
                    index: registers.iter().position(|n| n == name.as_str()).unwrap(),
                });
                result.push(ScriptOperation::Expression {
                    expression: VaultScriptExpression::StackProduce {
                        name: "ref".to_owned(),
                    },
                });
                result.push(ScriptOperation::PopToRegister {
                    index: registers.iter().position(|n| n == name.as_str()).unwrap(),
                });
            }
            Self::VariableRefMut { name } => {
                result.push(ScriptOperation::PushFromRegister {
                    index: registers.iter().position(|n| n == name.as_str()).unwrap(),
                });
                result.push(ScriptOperation::Expression {
                    expression: VaultScriptExpression::StackProduce {
                        name: "ref_mut".to_owned(),
                    },
                });
                result.push(ScriptOperation::PopToRegister {
                    index: registers.iter().position(|n| n == name.as_str()).unwrap(),
                });
            }
            Self::Literal(literal) => {
                result.push(ScriptOperation::Expression {
                    expression: VaultScriptExpression::Literal(literal.to_owned()),
                });
            }
            Self::CallFunction {
                module_name,
                name,
                arguments,
            } => {
                for argument in arguments.iter().rev() {
                    argument.compile(result, registers);
                }
                result.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some(name.to_owned().into()),
                        module_name: module_name.as_ref().map(|name| name.to_owned().into()),
                        ..Default::default()
                    },
                });
            }
            Self::CallMethod {
                module_name,
                struct_name,
                name,
                arguments,
            } => {
                for argument in arguments.iter().rev() {
                    argument.compile(result, registers);
                }
                result.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some(name.to_owned().into()),
                        struct_query: Some(StructQuery {
                            name: Some(struct_name.to_owned().into()),
                            ..Default::default()
                        }),
                        module_name: module_name.as_ref().map(|name| name.to_owned().into()),
                        ..Default::default()
                    },
                });
            }
            Self::If {
                condition,
                success,
                failure,
            } => {
                condition.compile(result, registers);
                let mut success_operations = vec![];
                for statement in success {
                    statement.compile(&mut success_operations, registers);
                }
                let failure_handle = failure.as_ref().map(|failure| {
                    let mut operations = vec![];
                    for statement in failure {
                        statement.compile(&mut operations, registers);
                    }
                    ScriptHandle::new(operations)
                });
                result.push(ScriptOperation::BranchScope {
                    scope_success: ScriptHandle::new(success_operations),
                    scope_failure: failure_handle,
                });
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VaultStatement {
    MakeVariable {
        name: String,
        expression: VaultExpression,
    },
    Expression(VaultExpression),
    Return(VaultExpression),
    Scope(Vec<VaultStatement>),
    While {
        condition: Box<VaultExpression>,
        statements: Vec<VaultStatement>,
    },
    For {
        setup: Vec<VaultStatement>,
        condition: Box<VaultExpression>,
        advancement: Vec<VaultStatement>,
        statements: Vec<VaultStatement>,
    },
}

impl VaultStatement {
    pub fn compile(
        &self,
        result: &mut Vec<ScriptOperation<VaultScriptExpression>>,
        registers: &mut Vec<String>,
    ) {
        match self {
            Self::MakeVariable { name, expression } => {
                expression.compile(result, registers);
                result.push(ScriptOperation::PopToRegister {
                    index: registers.iter().position(|n| n == name.as_str()).unwrap(),
                });
            }
            Self::Expression(expression) => {
                expression.compile(result, registers);
                result.push(ScriptOperation::Expression {
                    expression: VaultScriptExpression::StackDrop,
                });
            }
            Self::Return(expression) => {
                expression.compile(result, registers);
                result.push(ScriptOperation::Expression {
                    expression: VaultScriptExpression::Literal(VaultLiteral::Bool(false)),
                });
                result.push(ScriptOperation::ContinueScopeConditionally);
            }
            Self::Scope(expressions) => {
                let mut operations = vec![];
                for statement in expressions {
                    statement.compile(&mut operations, registers);
                }
                result.push(ScriptOperation::PushScope {
                    scope: ScriptHandle::new(operations),
                });
            }
            Self::While {
                condition,
                statements,
            } => {
                let mut operations = vec![];
                condition.compile(&mut operations, registers);
                operations.push(ScriptOperation::ContinueScopeConditionally);
                for statement in statements {
                    statement.compile(&mut operations, registers);
                }
                result.push(ScriptOperation::LoopScope {
                    scope: ScriptHandle::new(operations),
                });
            }
            Self::For {
                setup,
                condition,
                advancement,
                statements,
            } => {
                for statement in setup {
                    statement.compile(result, registers);
                }
                let mut operations = vec![];
                condition.compile(&mut operations, registers);
                operations.push(ScriptOperation::ContinueScopeConditionally);
                for statement in statements {
                    statement.compile(&mut operations, registers);
                }
                for statement in advancement {
                    statement.compile(result, registers);
                }
                result.push(ScriptOperation::LoopScope {
                    scope: ScriptHandle::new(operations),
                });
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultFunctionParameter {
    pub name: String,
    pub arg_type: String,
}

impl VaultFunctionParameter {
    pub fn build(&self) -> ScriptFunctionParameter<'static> {
        ScriptFunctionParameter {
            name: self.name.to_owned(),
            struct_query: StructQuery {
                name: Some(self.arg_type.as_str().to_owned().into()),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultFunction {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<VaultFunctionParameter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    pub statements: Vec<VaultStatement>,
}

impl VaultFunction {
    pub fn compile(
        &self,
        module_name: &str,
        struct_query: Option<StructQuery<'static>>,
    ) -> ScriptFunction<'static, VaultScriptExpression> {
        let signature = ScriptFunctionSignature {
            name: self.name.to_owned(),
            module_name: module_name.to_owned().into(),
            struct_query,
            visibility: Visibility::Public,
            inputs: self.arguments.iter().map(|input| input.build()).collect(),
            outputs: vec![ScriptFunctionParameter {
                name: "result".to_owned(),
                struct_query: if let Some(return_type) = &self.return_type {
                    StructQuery {
                        name: Some(return_type.to_owned().into()),
                        ..Default::default()
                    }
                } else {
                    StructQuery {
                        type_id: Some(TypeId::of::<()>()),
                        ..Default::default()
                    }
                },
            }],
        };
        let mut registers = Vec::new();
        let mut operations = vec![];
        for argument in &self.arguments {
            if let Some(item) = registers.iter_mut().find(|n| *n == &argument.name) {
                *item = argument.name.to_owned();
            } else {
                registers.push(argument.name.to_owned());
            }
            operations.push(ScriptOperation::DefineRegister {
                query: StructQuery {
                    name: Some(argument.arg_type.to_owned().into()),
                    ..Default::default()
                },
            });
            operations.push(ScriptOperation::PopToRegister {
                index: registers.iter().position(|n| n == &argument.name).unwrap(),
            });
        }
        for statement in &self.statements {
            statement.compile(&mut operations, &mut registers);
        }
        ScriptFunction {
            signature,
            script: ScriptHandle::new(operations),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStructField {
    pub name: String,
    pub struct_name: String,
}

impl VaultStructField {
    pub fn build(&self) -> ScriptStructField<'static> {
        ScriptStructField {
            name: self.name.to_owned(),
            visibility: Visibility::Public,
            struct_query: StructQuery {
                name: Some(self.struct_name.as_str().to_owned().into()),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStruct {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<VaultStructField>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<VaultFunction>,
}

impl VaultStruct {
    pub fn compile_struct(&self, module_name: &str) -> ScriptStruct<'static> {
        ScriptStruct {
            name: self.name.to_owned(),
            module_name: Some(module_name.to_owned()),
            visibility: Visibility::Public,
            fields: self.fields.iter().map(|field| field.build()).collect(),
        }
    }

    pub fn compile_methods(
        &self,
        module_name: &str,
    ) -> Vec<ScriptFunction<'static, VaultScriptExpression>> {
        let struct_query = StructQuery {
            name: Some(self.name.as_str().to_owned().into()),
            module_name: Some(module_name.to_owned().into()),
            ..Default::default()
        };
        self.methods
            .iter()
            .map(|method| method.compile(module_name, Some(struct_query.clone())))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VaultDefinition {
    Function(VaultFunction),
    Struct(VaultStruct),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultModule {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub definitions: Vec<VaultDefinition>,
}

impl VaultModule {
    pub fn parse(content: &str) -> Result<Self, serde_lexpr::Error> {
        serde_lexpr::from_str(content)
    }

    pub fn compile(&self) -> ScriptModule<'static, VaultScriptExpression> {
        ScriptModule {
            name: self.name.to_owned(),
            structs: self
                .definitions
                .iter()
                .filter_map(|definition| match definition {
                    VaultDefinition::Struct(struct_type) => {
                        Some(struct_type.compile_struct(&self.name))
                    }
                    _ => None,
                })
                .collect(),
            functions: self
                .definitions
                .iter()
                .filter_map(|definition| match definition {
                    VaultDefinition::Function(function) => Some(function.compile(&self.name, None)),
                    _ => None,
                })
                .chain(
                    self.definitions
                        .iter()
                        .filter_map(|definition| match definition {
                            VaultDefinition::Struct(struct_type) => {
                                Some(struct_type.compile_methods(&self.name))
                            }
                            _ => None,
                        })
                        .flatten(),
                )
                .collect(),
        }
    }
}

#[derive(Default)]
pub struct VaultPackage {
    pub modules: HashMap<String, VaultModule>,
}

impl VaultPackage {
    pub fn new<CP>(path: &str, content_provider: &mut CP) -> Result<Self, Box<dyn Error>>
    where
        CP: ScriptContentProvider<VaultModule>,
    {
        let mut result = Self::default();
        result.load(path, content_provider)?;
        Ok(result)
    }

    pub fn load<CP>(&mut self, path: &str, content_provider: &mut CP) -> Result<(), Box<dyn Error>>
    where
        CP: ScriptContentProvider<VaultModule>,
    {
        let path = content_provider.sanitize_path(path)?;
        if self.modules.contains_key(&path) {
            return Ok(());
        }
        if let Some(module) = content_provider.load(&path)? {
            let dependencies = module.dependencies.to_owned();
            self.modules.insert(path.to_owned(), module);
            for relative in dependencies {
                let path = content_provider.join_paths(&path, &relative)?;
                self.load(&path, content_provider)?;
            }
        }
        Ok(())
    }

    pub fn compile(&self) -> ScriptPackage<'static, VaultScriptExpression> {
        ScriptPackage {
            modules: self
                .modules
                .values()
                .map(|module| module.compile())
                .collect(),
        }
    }
}

pub struct VaultContentParser;

impl BytesContentParser<VaultModule> for VaultContentParser {
    fn parse(&self, bytes: Vec<u8>) -> Result<VaultModule, Box<dyn Error>> {
        let content = String::from_utf8(bytes)?;
        Ok(VaultModule::parse(&content)?)
    }
}

#[macro_export]
macro_rules! define_vault_function {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        fn
        $name:ident
        ($( $argument_name:ident : $argument_type:ty),*)
        ->
        $return_type:ty
        $code:block
    ) => {
        intuicio_core::function::Function::new(
            intuicio_core::function_signature! {
                $registry
                =>
                $(mod $module_name)?
                fn
                $name
                ($($argument_name : $argument_type),*)
                ->
                (result : $return_type)
            },
            intuicio_core::function::FunctionBody::closure(move |context, #[allow(unused_variables)] registry| {
                use intuicio_data::data_stack::DataStackPack;
                #[allow(unused_mut)]
                let ($(mut $argument_name,)*) = <($($argument_type,)*)>::stack_pop(context.stack());
                context.stack().push($code);
            }),
        )
    };
}

#[macro_export]
macro_rules! define_vault_method {
    (
        $registry:expr
        =>
        $(mod $module_name:ident)?
        struct ($struct_type:ty)
        fn
        $name:ident
        ($( $argument_name:ident : $argument_type:ty),*)
        ->
        $return_type:ty
        $code:block
    ) => {
        intuicio_core::function::Function::new(
            intuicio_core::function_signature! {
                $registry
                =>
                $(mod $module_name)?
                struct ($struct_type)
                fn
                $name
                ($($argument_name : $argument_type),*)
                ->
                (result : $return_type)
            },
            intuicio_core::function::FunctionBody::closure(move |context, registry| {
                use intuicio_data::data_stack::DataStackPack;
                #[allow(unused_mut)]
                let ($(mut $argument_name,)*) = <($($argument_type,)*)>::stack_pop(context.stack());
                context.stack().push($code);
            }),
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use intuicio_backend_vm::prelude::*;
    use intuicio_core::prelude::*;

    #[test]
    fn test_vault_script() {
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn print(content: String) -> () {
                println!("PRINT: {}", content);
            }
        });
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn add(a: usize, b: usize) -> usize {
                a + b
            }
        });
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn sub(a: usize, b: usize) -> usize {
                a - b
            }
        });
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn less_than(a: usize, b: usize) -> bool {
                a < b
            }
        });
        registry.add_function(define_function! {
            registry => mod intrinsics struct (usize) fn clone(this: usize) -> (original: usize, clone: usize) {
                (this, this)
            }
        });
        let mut content_provider = FileContentProvider::new("vault", VaultContentParser);
        VaultPackage::new("../../resources/package.vault", &mut content_provider)
            .unwrap()
            .compile()
            .install::<VmScope<VaultScriptExpression>>(
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
        let (result,) = vm
            .call_function::<(usize,), _>("main", "test", None)
            .unwrap()
            .run(());
        assert_eq!(vm.context().stack().position(), 0);
        assert_eq!(result, 42);
        let (result,) = vm
            .call_function::<(usize,), (usize,)>("fib", "test", None)
            .unwrap()
            .run((20,));
        assert_eq!(vm.context().stack().position(), 0);
        assert_eq!(result, 6765);
    }
}

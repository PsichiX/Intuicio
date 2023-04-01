pub mod library;
pub mod parser;

use intuicio_core::{
    context::Context,
    crate_version,
    function::{FunctionHandle, FunctionQuery},
    object::Object,
    registry::Registry,
    script::{
        ScriptExpression, ScriptFunction, ScriptFunctionParameter, ScriptFunctionSignature,
        ScriptHandle, ScriptModule, ScriptOperation, ScriptPackage, ScriptStruct,
        ScriptStructField,
    },
    struct_type::{StructHandle, StructQuery},
    IntuicioVersion, Visibility,
};
use intuicio_data::shared::Shared;
use std::{
    any::TypeId,
    cell::{Ref, RefMut},
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
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

const CLOSURES: &str = "_closures";

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

#[derive(Debug, Clone)]
pub enum SimpletonScriptLiteral {
    Null,
    Boolean(Boolean),
    Integer(Integer),
    Real(Real),
    Text(Text),
    Array {
        items_count: usize,
    },
    Map {
        items_count: usize,
    },
    Object {
        name: String,
        module_name: String,
        fields_count: usize,
    },
}

impl SimpletonScriptLiteral {
    fn evaluate(&self, context: &mut Context, registry: &Registry) {
        match self {
            Self::Null => context.stack().push(Reference::null()),
            Self::Boolean(value) => context
                .stack()
                .push(Reference::new_boolean(*value, registry)),
            Self::Integer(value) => context
                .stack()
                .push(Reference::new_integer(*value, registry)),
            Self::Real(value) => context.stack().push(Reference::new_real(*value, registry)),
            Self::Text(value) => context
                .stack()
                .push(Reference::new_text(value.to_owned(), registry)),
            Self::Array { items_count } => {
                let mut result = Array::with_capacity(*items_count);
                for _ in 0..*items_count {
                    result.push(context.stack().pop::<Reference>().unwrap());
                }
                context.stack().push(Reference::new_array(result, registry))
            }
            Self::Map { items_count } => {
                let mut result = Map::with_capacity(*items_count);
                for _ in 0..*items_count {
                    let key = context
                        .stack()
                        .pop::<Reference>()
                        .unwrap()
                        .read::<Text>()
                        .unwrap()
                        .to_owned();
                    let value = context.stack().pop::<Reference>().unwrap();
                    result.insert(key, value);
                }
                context.stack().push(Reference::new_map(result, registry))
            }
            Self::Object {
                name,
                module_name,
                fields_count,
            } => {
                let struct_type = registry
                    .find_struct(StructQuery {
                        name: Some(name.into()),
                        module_name: Some(module_name.into()),
                        ..Default::default()
                    })
                    .unwrap();
                let mut result = Object::new(struct_type);
                for _ in 0..*fields_count {
                    let name = context.stack().pop::<Reference>().unwrap();

                    *result
                        .write_field::<Reference>(name.read::<Text>().unwrap().as_str())
                        .unwrap() = context.stack().pop::<Reference>().unwrap();
                }
                context.stack().push(Reference::new_raw(result))
            }
        };
    }
}

#[derive(Debug)]
pub enum SimpletonScriptExpression {
    FindStruct { name: String, module_name: String },
    FindFunction { name: String, module_name: String },
    Literal(SimpletonScriptLiteral),
    StackDrop,
    StackDuplicate,
    StackSwap,
    StackUnwrapBoolean,
    StackValueOr(bool),
    GetField { name: String },
    SetField { name: String },
}

impl ScriptExpression for SimpletonScriptExpression {
    fn evaluate(&self, context: &mut Context, registry: &Registry) {
        match self {
            Self::FindStruct { name, module_name } => {
                context.stack().push(Reference::new_type(
                    Type::new(
                        registry
                            .find_struct(StructQuery {
                                name: Some(name.into()),
                                module_name: Some(module_name.into()),
                                ..Default::default()
                            })
                            .unwrap(),
                    ),
                    registry,
                ));
            }
            Self::FindFunction { name, module_name } => {
                context.stack().push(Reference::new_function(
                    Function::new(
                        registry
                            .find_function(FunctionQuery {
                                name: Some(name.into()),
                                module_name: Some(module_name.into()),
                                ..Default::default()
                            })
                            .unwrap(),
                    ),
                    registry,
                ));
            }
            Self::Literal(literal) => {
                literal.evaluate(context, registry);
            }
            Self::StackDrop => {
                context.stack().drop();
            }
            Self::StackDuplicate => {
                let object = context.stack().pop::<Reference>().unwrap();
                context.stack().push(object.clone());
                context.stack().push(object);
            }
            Self::StackSwap => {
                let a = context.stack().pop::<Reference>().unwrap();
                let b = context.stack().pop::<Reference>().unwrap();
                context.stack().push(a);
                context.stack().push(b);
            }
            Self::StackUnwrapBoolean => {
                let value = context.stack().pop::<Reference>().unwrap();
                context.stack().push(*value.read::<Boolean>().unwrap());
            }
            Self::StackValueOr(value) => {
                let object = context.stack().pop::<Reference>().unwrap();
                if object.is_null() {
                    context.stack().push(*value);
                } else {
                    context.stack().push(object);
                    context.stack().push(!*value);
                }
            }
            Self::GetField { name } => {
                let object = context.stack().pop::<Reference>().unwrap();
                let value = object
                    .read_object()
                    .unwrap()
                    .read_field::<Reference>(name)
                    .unwrap()
                    .clone();
                context.stack().push(value);
            }
            Self::SetField { name } => {
                let mut object = context.stack().pop::<Reference>().unwrap();
                let value = context.stack().pop::<Reference>().unwrap();
                *object
                    .write_object()
                    .unwrap()
                    .write_field::<Reference>(name)
                    .unwrap() = value;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum SimpletonLiteral {
    Null,
    Boolean(Boolean),
    Integer(Integer),
    Real(Real),
    Text(Text),
    Array {
        items: Vec<SimpletonExpressionStart>,
    },
    Map {
        items: Vec<(String, SimpletonExpressionStart)>,
    },
    Object {
        name: String,
        module_name: String,
        fields: Vec<(String, SimpletonExpressionStart)>,
    },
}

impl SimpletonLiteral {
    pub fn compile(
        &self,
        result: &mut Vec<ScriptOperation<SimpletonScriptExpression>>,
        registers: &mut Vec<String>,
        closures: &mut Vec<SimpletonFunction>,
        closures_index: &mut usize,
    ) -> SimpletonScriptLiteral {
        match self {
            Self::Null => SimpletonScriptLiteral::Null,
            Self::Boolean(value) => SimpletonScriptLiteral::Boolean(*value),
            Self::Integer(value) => SimpletonScriptLiteral::Integer(*value),
            Self::Real(value) => SimpletonScriptLiteral::Real(*value),
            Self::Text(value) => SimpletonScriptLiteral::Text(value.to_owned()),
            Self::Array { items } => {
                for item in items.iter().rev() {
                    item.compile(result, registers, closures, closures_index);
                }
                SimpletonScriptLiteral::Array {
                    items_count: items.len(),
                }
            }
            Self::Map { items } => {
                for (key, value) in items.iter().rev() {
                    value.compile(result, registers, closures, closures_index);
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::Literal(
                            SimpletonScriptLiteral::Text(key.to_owned()),
                        ),
                    });
                }
                SimpletonScriptLiteral::Map {
                    items_count: items.len(),
                }
            }
            Self::Object {
                name,
                module_name,
                fields,
            } => {
                for (key, value) in fields.iter().rev() {
                    value.compile(result, registers, closures, closures_index);
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::Literal(
                            SimpletonScriptLiteral::Text(key.to_owned()),
                        ),
                    });
                }
                SimpletonScriptLiteral::Object {
                    name: name.to_owned(),
                    module_name: module_name.to_owned(),
                    fields_count: fields.len(),
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum SimpletonExpressionStart {
    FindStruct {
        name: String,
        module_name: String,
        next: Option<SimpletonExpressionNext>,
    },
    FindFunction {
        name: String,
        module_name: String,
        next: Option<SimpletonExpressionNext>,
    },
    Closure {
        captures: Vec<String>,
        arguments: Vec<String>,
        statements: Vec<SimpletonStatement>,
        next: Option<SimpletonExpressionNext>,
    },
    Literal {
        literal: SimpletonLiteral,
        next: Option<SimpletonExpressionNext>,
    },
    GetVariable {
        name: String,
        next: Option<SimpletonExpressionNext>,
    },
    CallFunction {
        name: String,
        module_name: String,
        arguments: Vec<SimpletonExpressionStart>,
        next: Option<SimpletonExpressionNext>,
    },
}

impl SimpletonExpressionStart {
    pub fn compile(
        &self,
        result: &mut Vec<ScriptOperation<SimpletonScriptExpression>>,
        registers: &mut Vec<String>,
        closures: &mut Vec<SimpletonFunction>,
        closures_index: &mut usize,
    ) {
        match self {
            Self::FindStruct {
                name,
                module_name,
                next,
            } => {
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::FindStruct {
                        name: name.to_owned(),
                        module_name: module_name.to_owned(),
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                }
            }
            Self::FindFunction {
                name,
                module_name,
                next,
            } => {
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::FindFunction {
                        name: name.to_owned(),
                        module_name: module_name.to_owned(),
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                }
            }
            Self::Closure {
                captures,
                arguments,
                statements,
                next,
            } => {
                let name = format!("_{}", *closures_index);
                *closures_index += 1;
                closures.push(SimpletonFunction {
                    name: name.to_owned(),
                    arguments: captures.iter().chain(arguments.iter()).cloned().collect(),
                    statements: statements.to_owned(),
                });
                for capture in captures.iter().rev() {
                    let index = registers.iter().position(|n| n == capture).unwrap();
                    result.push(ScriptOperation::PushFromRegister { index });
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::StackDuplicate,
                    });
                    result.push(ScriptOperation::PopToRegister { index });
                }
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::Literal(SimpletonScriptLiteral::Array {
                        items_count: captures.len(),
                    }),
                });
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::FindFunction {
                        name,
                        module_name: CLOSURES.to_owned(),
                    },
                });
                result.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some("new".to_owned().into()),
                        module_name: Some("closure".to_owned().into()),
                        ..Default::default()
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                }
            }
            Self::Literal { literal, next } => {
                let literal = literal.compile(result, registers, closures, closures_index);
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::Literal(literal),
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                }
            }
            Self::GetVariable { name, next } => {
                let index = registers.iter().position(|n| n == name.as_str()).unwrap();
                result.push(ScriptOperation::PushFromRegister { index });
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackDuplicate,
                });
                result.push(ScriptOperation::PopToRegister { index });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                }
            }
            Self::CallFunction {
                name,
                module_name,
                arguments,
                next,
            } => {
                for argument in arguments.iter().rev() {
                    argument.compile(result, registers, closures, closures_index);
                }
                result.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some(name.to_owned().into()),
                        module_name: Some(module_name.to_owned().into()),
                        ..Default::default()
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                }
            }
        }
    }

    pub fn compile_assign(
        &self,
        result: &mut Vec<ScriptOperation<SimpletonScriptExpression>>,
        registers: &mut Vec<String>,
        closures: &mut Vec<SimpletonFunction>,
        closures_index: &mut usize,
    ) {
        match self {
            Self::FindStruct {
                name,
                module_name,
                next,
            } => {
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::FindStruct {
                        name: name.to_owned(),
                        module_name: module_name.to_owned(),
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                } else {
                    panic!("Trying to assign value to structure type!");
                }
            }
            Self::FindFunction {
                name,
                module_name,
                next,
            } => {
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::FindFunction {
                        name: name.to_owned(),
                        module_name: module_name.to_owned(),
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                } else {
                    panic!("Trying to assign value to function type!");
                }
            }
            Self::Closure {
                captures,
                arguments,
                statements,
                next,
            } => {
                let name = format!("_{}", *closures_index);
                *closures_index += 1;
                closures.push(SimpletonFunction {
                    name: name.to_owned(),
                    arguments: captures.iter().chain(arguments.iter()).cloned().collect(),
                    statements: statements.to_owned(),
                });
                for capture in captures.iter().rev() {
                    let index = registers.iter().position(|n| n == capture).unwrap();
                    result.push(ScriptOperation::PushFromRegister { index });
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::StackDuplicate,
                    });
                    result.push(ScriptOperation::PopToRegister { index });
                }
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::Literal(SimpletonScriptLiteral::Array {
                        items_count: captures.len(),
                    }),
                });
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::FindFunction {
                        name,
                        module_name: CLOSURES.to_owned(),
                    },
                });
                result.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some("new".to_owned().into()),
                        module_name: Some("closure".to_owned().into()),
                        ..Default::default()
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                } else {
                    panic!("Trying to assign value to closure!");
                }
            }
            Self::Literal { literal, next } => {
                let literal = literal.compile(result, registers, closures, closures_index);
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::Literal(literal),
                });
                if let Some(next) = next {
                    next.compile_assign(result, registers, closures, closures_index);
                } else {
                    panic!("Trying to assign value to literal!");
                }
            }
            Self::GetVariable { name, next } => {
                let index = registers.iter().position(|n| n == name.as_str()).unwrap();
                if let Some(next) = next {
                    result.push(ScriptOperation::PushFromRegister { index });
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::StackDuplicate,
                    });
                    result.push(ScriptOperation::PopToRegister { index });
                    next.compile_assign(result, registers, closures, closures_index);
                } else {
                    result.push(ScriptOperation::PopToRegister { index });
                }
            }
            Self::CallFunction {
                name,
                module_name,
                arguments,
                next,
            } => {
                for argument in arguments.iter().rev() {
                    argument.compile(result, registers, closures, closures_index);
                }
                result.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some(name.to_owned().into()),
                        module_name: Some(module_name.to_owned().into()),
                        ..Default::default()
                    },
                });
                if let Some(next) = next {
                    next.compile_assign(result, registers, closures, closures_index);
                } else {
                    panic!("Trying to assign value to function call!");
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum SimpletonExpressionNext {
    GetField {
        name: String,
        next: Option<Box<SimpletonExpressionNext>>,
    },
    GetArrayItem {
        index: Box<SimpletonExpressionStart>,
        next: Option<Box<SimpletonExpressionNext>>,
    },
    GetMapItem {
        index: Box<SimpletonExpressionStart>,
        next: Option<Box<SimpletonExpressionNext>>,
    },
}

impl SimpletonExpressionNext {
    pub fn compile(
        &self,
        result: &mut Vec<ScriptOperation<SimpletonScriptExpression>>,
        registers: &mut Vec<String>,
        closures: &mut Vec<SimpletonFunction>,
        closures_index: &mut usize,
    ) {
        match self {
            Self::GetField { name, next } => {
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::GetField {
                        name: name.to_owned(),
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                }
            }
            Self::GetArrayItem { index, next } => {
                index.compile(result, registers, closures, closures_index);
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackSwap,
                });
                result.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some("get".into()),
                        module_name: Some("array".into()),
                        ..Default::default()
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                }
            }
            Self::GetMapItem { index, next } => {
                index.compile(result, registers, closures, closures_index);
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackSwap,
                });
                result.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some("get".into()),
                        module_name: Some("map".into()),
                        ..Default::default()
                    },
                });
                if let Some(next) = next {
                    next.compile(result, registers, closures, closures_index);
                }
            }
        }
    }

    pub fn compile_assign(
        &self,
        result: &mut Vec<ScriptOperation<SimpletonScriptExpression>>,
        registers: &mut Vec<String>,
        closures: &mut Vec<SimpletonFunction>,
        closures_index: &mut usize,
    ) {
        match self {
            Self::GetField { name, next } => {
                if let Some(next) = next {
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::GetField {
                            name: name.to_owned(),
                        },
                    });
                    next.compile_assign(result, registers, closures, closures_index);
                } else {
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::SetField {
                            name: name.to_owned(),
                        },
                    });
                }
            }
            Self::GetArrayItem { index, next } => {
                if let Some(next) = next {
                    index.compile(result, registers, closures, closures_index);
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::StackSwap,
                    });
                    result.push(ScriptOperation::CallFunction {
                        query: FunctionQuery {
                            name: Some("get".into()),
                            module_name: Some("array".into()),
                            ..Default::default()
                        },
                    });
                    next.compile_assign(result, registers, closures, closures_index);
                } else {
                    index.compile(result, registers, closures, closures_index);
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::StackSwap,
                    });
                    result.push(ScriptOperation::CallFunction {
                        query: FunctionQuery {
                            name: Some("set".into()),
                            module_name: Some("array".into()),
                            ..Default::default()
                        },
                    });
                }
            }
            Self::GetMapItem { index, next } => {
                if let Some(next) = next {
                    index.compile(result, registers, closures, closures_index);
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::StackSwap,
                    });
                    result.push(ScriptOperation::CallFunction {
                        query: FunctionQuery {
                            name: Some("get".into()),
                            module_name: Some("map".into()),
                            ..Default::default()
                        },
                    });
                    next.compile_assign(result, registers, closures, closures_index);
                } else {
                    index.compile(result, registers, closures, closures_index);
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::StackSwap,
                    });
                    result.push(ScriptOperation::CallFunction {
                        query: FunctionQuery {
                            name: Some("set".into()),
                            module_name: Some("map".into()),
                            ..Default::default()
                        },
                    });
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum SimpletonStatement {
    CreateVariable {
        name: String,
        value: SimpletonExpressionStart,
    },
    AssignValue {
        object: SimpletonExpressionStart,
        value: SimpletonExpressionStart,
    },
    Expression(SimpletonExpressionStart),
    Return(SimpletonExpressionStart),
    IfElse {
        condition: SimpletonExpressionStart,
        success: Vec<SimpletonStatement>,
        failure: Option<Vec<SimpletonStatement>>,
    },
    While {
        condition: SimpletonExpressionStart,
        statements: Vec<SimpletonStatement>,
    },
    For {
        variable: String,
        iterator: SimpletonExpressionStart,
        statements: Vec<SimpletonStatement>,
    },
}

impl SimpletonStatement {
    pub fn recursive_any(&self, f: &impl Fn(&Self) -> bool) -> bool {
        if f(self) {
            return true;
        }
        match self {
            Self::IfElse {
                success, failure, ..
            } => {
                for item in success {
                    if item.recursive_any(f) {
                        return true;
                    }
                }
                if let Some(failure) = failure.as_ref() {
                    for item in failure {
                        if item.recursive_any(f) {
                            return true;
                        }
                    }
                }
            }
            Self::While { statements, .. } => {
                for item in statements {
                    if item.recursive_any(f) {
                        return true;
                    }
                }
            }
            Self::For { statements, .. } => {
                for item in statements {
                    if item.recursive_any(f) {
                        return true;
                    }
                }
            }
            _ => {}
        }
        false
    }

    pub fn compile(
        &self,
        result: &mut Vec<ScriptOperation<SimpletonScriptExpression>>,
        registers: &mut Vec<String>,
        closures: &mut Vec<SimpletonFunction>,
        closures_index: &mut usize,
        subscope_level: usize,
    ) {
        match self {
            Self::CreateVariable { name, value } => {
                if !registers.iter().any(|n| n == name) {
                    registers.push(name.to_owned());
                }
                result.push(ScriptOperation::DefineRegister {
                    query: StructQuery::of::<Reference>(),
                });
                value.compile(result, registers, closures, closures_index);
                result.push(ScriptOperation::PopToRegister {
                    index: registers.iter().position(|n| n == name).unwrap(),
                });
            }
            Self::AssignValue { object, value } => {
                value.compile(result, registers, closures, closures_index);
                object.compile_assign(result, registers, closures, closures_index);
            }
            Self::Expression(expression) => {
                expression.compile(result, registers, closures, closures_index);
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackDrop,
                });
            }
            Self::Return(expression) => {
                expression.compile(result, registers, closures, closures_index);
                for _ in 0..(subscope_level + 1) {
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::Literal(
                            SimpletonScriptLiteral::Boolean(false),
                        ),
                    });
                    result.push(ScriptOperation::Expression {
                        expression: SimpletonScriptExpression::StackUnwrapBoolean,
                    });
                }
                result.push(ScriptOperation::ContinueScopeConditionally);
            }
            Self::IfElse {
                condition,
                success,
                failure,
            } => {
                condition.compile(result, registers, closures, closures_index);
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackUnwrapBoolean,
                });
                // success body
                let mut success_operations = vec![];
                for statement in success {
                    statement.compile(
                        &mut success_operations,
                        registers,
                        closures,
                        closures_index,
                        subscope_level + 1,
                    );
                }
                success_operations.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::Literal(
                        SimpletonScriptLiteral::Boolean(true),
                    ),
                });
                success_operations.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackUnwrapBoolean,
                });
                // failure body
                let mut failure_operations = vec![];
                if let Some(failure) = failure {
                    for statement in failure {
                        statement.compile(
                            &mut failure_operations,
                            registers,
                            closures,
                            closures_index,
                            subscope_level + 1,
                        );
                    }
                }
                failure_operations.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::Literal(
                        SimpletonScriptLiteral::Boolean(true),
                    ),
                });
                failure_operations.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackUnwrapBoolean,
                });
                // main body
                result.push(ScriptOperation::BranchScope {
                    scope_success: ScriptHandle::new(success_operations),
                    scope_failure: Some(ScriptHandle::new(failure_operations)),
                });
                result.push(ScriptOperation::ContinueScopeConditionally);
            }
            Self::While {
                condition,
                statements,
            } => {
                let mut operations = vec![];
                // loop body
                for statement in statements {
                    if statement.recursive_any(&|statement| {
                        matches!(statement, SimpletonStatement::Return(_))
                    }) {
                        panic!("Cannot return values inside while loops!");
                    }
                    statement.compile(&mut operations, registers, closures, closures_index, 0);
                }
                condition.compile(&mut operations, registers, closures, closures_index);
                operations.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackUnwrapBoolean,
                });
                // main body
                condition.compile(result, registers, closures, closures_index);
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackUnwrapBoolean,
                });
                result.push(ScriptOperation::LoopScope {
                    scope: ScriptHandle::new(operations),
                });
            }
            Self::For {
                variable,
                iterator,
                statements,
            } => {
                let mut operations = vec![];
                // loop body
                if !registers.iter().any(|n| n == variable) {
                    registers.push(variable.to_owned());
                }
                operations.push(ScriptOperation::DefineRegister {
                    query: StructQuery::of::<Reference>(),
                });
                let index = registers
                    .iter()
                    .position(|n| n == variable.as_str())
                    .unwrap();
                operations.push(ScriptOperation::PopToRegister { index });
                for statement in statements {
                    if statement.recursive_any(&|statement| {
                        matches!(statement, SimpletonStatement::Return(_))
                    }) {
                        panic!("Cannot return values inside for loops!");
                    }
                    statement.compile(&mut operations, registers, closures, closures_index, 0);
                }
                operations.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackDuplicate,
                });
                operations.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some("next".to_owned().into()),
                        module_name: Some("iter".to_owned().into()),
                        ..Default::default()
                    },
                });
                operations.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackValueOr(false),
                });
                // main body
                iterator.compile(result, registers, closures, closures_index);
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackDuplicate,
                });
                result.push(ScriptOperation::CallFunction {
                    query: FunctionQuery {
                        name: Some("next".to_owned().into()),
                        module_name: Some("iter".to_owned().into()),
                        ..Default::default()
                    },
                });
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackValueOr(false),
                });
                result.push(ScriptOperation::LoopScope {
                    scope: ScriptHandle::new(operations),
                });
                result.push(ScriptOperation::Expression {
                    expression: SimpletonScriptExpression::StackDrop,
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimpletonFunction {
    pub name: String,
    pub arguments: Vec<String>,
    pub statements: Vec<SimpletonStatement>,
}

impl SimpletonFunction {
    pub fn compile(
        &self,
        module_name: &str,
        closures: &mut Vec<SimpletonFunction>,
        closures_index: &mut usize,
    ) -> ScriptFunction<'static, SimpletonScriptExpression> {
        let signature = ScriptFunctionSignature {
            name: self.name.to_owned(),
            module_name: Some(module_name.to_owned()),
            struct_query: None,
            visibility: Visibility::Public,
            inputs: self
                .arguments
                .iter()
                .map(|name| ScriptFunctionParameter {
                    name: name.to_owned(),
                    struct_query: StructQuery::of::<Reference>(),
                })
                .collect(),
            outputs: vec![ScriptFunctionParameter {
                name: "result".to_owned(),
                struct_query: StructQuery::of::<Reference>(),
            }],
        };
        let mut registers = Vec::new();
        let mut operations = vec![];
        for name in &self.arguments {
            if !registers.iter().any(|n| n == name) {
                registers.push(name.to_owned());
            }
            operations.push(ScriptOperation::DefineRegister {
                query: StructQuery::of::<Reference>(),
            });
            operations.push(ScriptOperation::PopToRegister {
                index: registers.iter().position(|n| n == name).unwrap(),
            });
        }
        for statement in &self.statements {
            statement.compile(&mut operations, &mut registers, closures, closures_index, 0);
        }
        operations.push(ScriptOperation::Expression {
            expression: SimpletonScriptExpression::Literal(SimpletonScriptLiteral::Null),
        });
        ScriptFunction {
            signature,
            script: ScriptHandle::new(operations),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimpletonStruct {
    pub name: String,
    pub fields: Vec<String>,
}

impl SimpletonStruct {
    pub fn compile(&self, module_name: &str) -> ScriptStruct<'static> {
        ScriptStruct {
            name: self.name.to_owned(),
            module_name: Some(module_name.to_owned()),
            visibility: Visibility::Public,
            fields: self
                .fields
                .iter()
                .map(|name| ScriptStructField {
                    name: name.to_owned(),
                    visibility: Visibility::Public,
                    struct_query: StructQuery::of::<Reference>(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimpletonModule {
    pub name: String,
    pub dependencies: Vec<String>,
    pub structs: Vec<SimpletonStruct>,
    pub functions: Vec<SimpletonFunction>,
}

impl SimpletonModule {
    pub fn parse(content: &str) -> Result<Self, String> {
        parser::parse(content)
    }

    pub fn compile(
        &self,
        closures: &mut Vec<SimpletonFunction>,
        closures_index: &mut usize,
    ) -> ScriptModule<'static, SimpletonScriptExpression> {
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
                .map(|function| function.compile(&self.name, closures, closures_index))
                .collect(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SimpletonPackage {
    pub modules: HashMap<String, SimpletonModule>,
}

impl SimpletonPackage {
    pub fn new<CP>(path: &str, content_provider: &mut CP) -> Result<Self, Box<dyn Error>>
    where
        CP: SimpletonContentProvider,
    {
        let mut result = Self::default();
        result.load(path, content_provider)?;
        Ok(result)
    }

    pub fn load<CP>(&mut self, path: &str, content_provider: &mut CP) -> Result<(), Box<dyn Error>>
    where
        CP: SimpletonContentProvider,
    {
        let path = content_provider.sanitize_path(path)?;
        if self.modules.contains_key(&path) {
            return Ok(());
        }
        if let Some(content) = content_provider.load(&path)? {
            let module = SimpletonModule::parse(&content)?;
            let dependencies = module.dependencies.to_owned();
            self.modules.insert(path.to_owned(), module);
            for relative in dependencies {
                let path = content_provider.join_paths(&path, &relative)?;
                self.load(&path, content_provider)?;
            }
        }
        Ok(())
    }

    pub fn compile(&self) -> ScriptPackage<'static, SimpletonScriptExpression> {
        let mut closures = vec![];
        let mut closures_index = 0;
        let mut modules: Vec<ScriptModule<SimpletonScriptExpression>> = self
            .modules
            .values()
            .map(|module| module.compile(&mut closures, &mut closures_index))
            .collect();
        let mut closure_functions = vec![];
        loop {
            let mut result = vec![];
            for closure in &closures {
                closure_functions.push(closure.compile(CLOSURES, &mut result, &mut closures_index));
            }
            if result.is_empty() {
                break;
            }
            closures = result;
        }
        modules.push(ScriptModule {
            name: CLOSURES.to_owned(),
            structs: vec![],
            functions: closure_functions,
        });
        ScriptPackage { modules }
    }
}

pub trait SimpletonContentProvider {
    fn load(&mut self, path: &str) -> Result<Option<String>, Box<dyn Error>>;

    fn sanitize_path(&self, path: &str) -> Result<String, Box<dyn Error>> {
        Ok(path.to_owned())
    }

    fn join_paths(&self, parent: &str, relative: &str) -> Result<String, Box<dyn Error>>;
}

pub struct ExtensionContentProvider {
    extension_providers: HashMap<String, Box<dyn SimpletonContentProvider>>,
}

impl Default for ExtensionContentProvider {
    fn default() -> Self {
        Self {
            extension_providers: Default::default(),
        }
        .with("simp", FileContentProvider)
    }
}

impl ExtensionContentProvider {
    pub fn with(
        mut self,
        extension: &str,
        content_provider: impl SimpletonContentProvider + 'static,
    ) -> Self {
        self.extension_providers
            .insert(extension.to_owned(), Box::new(content_provider));
        self
    }
}

impl SimpletonContentProvider for ExtensionContentProvider {
    fn load(&mut self, path: &str) -> Result<Option<String>, Box<dyn Error>> {
        let extension = match Path::new(path).extension() {
            Some(extension) => extension.to_string_lossy().to_string(),
            None => "simp".to_owned(),
        };
        if let Some(content_provider) = self.extension_providers.get_mut(&extension) {
            content_provider.load(path)
        } else {
            Err(Box::new(
                ExtensionContentProviderError::ContentProviderForExtensionNotFound(extension),
            ))
        }
    }

    fn sanitize_path(&self, path: &str) -> Result<String, Box<dyn Error>> {
        let extension = match Path::new(path).extension() {
            Some(extension) => extension.to_string_lossy().to_string(),
            None => "simp".to_owned(),
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
            None => "simp".to_owned(),
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
    ContentProviderForExtensionNotFound(String),
}

impl std::fmt::Display for ExtensionContentProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionContentProviderError::ContentProviderForExtensionNotFound(extension) => {
                write!(
                    f,
                    "Could not find content provider for extension: `{}`",
                    extension
                )
            }
        }
    }
}

impl Error for ExtensionContentProviderError {}

pub struct IgnoreContentProvider;

impl SimpletonContentProvider for IgnoreContentProvider {
    fn load(&mut self, _: &str) -> Result<Option<String>, Box<dyn Error>> {
        Ok(None)
    }

    fn join_paths(&self, parent: &str, relative: &str) -> Result<String, Box<dyn Error>> {
        Ok(format!("{}/{}", parent, relative))
    }
}

pub struct FileContentProvider;

impl SimpletonContentProvider for FileContentProvider {
    fn load(&mut self, path: &str) -> Result<Option<String>, Box<dyn Error>> {
        Ok(Some(std::fs::read_to_string(path)?))
    }

    fn sanitize_path(&self, path: &str) -> Result<String, Box<dyn Error>> {
        let mut result = PathBuf::from(path);
        result.set_extension("simp");
        Ok(result.canonicalize()?.to_string_lossy().into_owned())
    }

    fn join_paths(&self, parent: &str, relative: &str) -> Result<String, Box<dyn Error>> {
        let mut path = PathBuf::from(parent);
        path.pop();
        Ok(path.join(relative).to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        library::ObjectBuilder, FileContentProvider, Integer, Real, Reference, SimpletonPackage,
        SimpletonScriptExpression,
    };
    use intuicio_backend_vm::prelude::*;
    use intuicio_core::prelude::*;

    #[test]
    fn test_simpleton_script() {
        let mut registry = Registry::default();
        crate::library::install(&mut registry);

        SimpletonPackage::new("../../resources/package.simp", &mut FileContentProvider)
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

use intuicio_core::prelude::*;
use std::{error::Error, str::FromStr};

#[derive(Debug)]
pub enum CustomExpression {
    Literal(i32),
}

impl ScriptExpression for CustomExpression {
    fn evaluate(&self, context: &mut Context, _: &Registry) {
        match self {
            Self::Literal(value) => {
                context.stack().push(*value);
            }
        }
    }
}

#[derive(Debug)]
pub struct CustomOperationError {
    pub operation: String,
}

impl std::fmt::Display for CustomOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unsupported operation: `{}`", self.operation)
    }
}

impl Error for CustomOperationError {}

pub type CustomScript = Vec<CustomOperation>;

pub enum CustomOperation {
    Comment { content: String },
    Push { value: i32 },
    Call { name: String, module_name: String },
}

impl FromStr for CustomOperation {
    type Err = CustomOperationError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Self::Comment {
                content: "".to_owned(),
            });
        }
        if line.starts_with('#') {
            return Ok(Self::Comment {
                content: line.to_owned(),
            });
        }
        let mut tokens = line.split_ascii_whitespace();
        match tokens.next() {
            Some("push") => {
                let value = tokens.next().unwrap().parse::<i32>().unwrap();
                Ok(Self::Push { value })
            }
            Some("call") => {
                let module_name = tokens.next().unwrap().to_owned();
                let name = tokens.next().unwrap().to_owned();
                Ok(Self::Call { name, module_name })
            }
            _ => Err(CustomOperationError {
                operation: line.to_owned(),
            }),
        }
    }
}

impl CustomOperation {
    pub fn compile_operation(&self) -> Option<ScriptOperation<'static, CustomExpression>> {
        match self {
            Self::Comment { .. } => None,
            Self::Push { value } => Some(ScriptOperation::Expression {
                expression: CustomExpression::Literal(*value),
            }),
            Self::Call { name, module_name } => Some(ScriptOperation::CallFunction {
                query: FunctionQuery {
                    name: Some(name.to_owned().into()),
                    module_name: Some(module_name.to_owned().into()),
                    ..Default::default()
                },
            }),
        }
    }

    pub fn compile_script(
        operations: &[CustomOperation],
    ) -> ScriptHandle<'static, CustomExpression> {
        operations
            .iter()
            .rev()
            .filter_map(|operation| operation.compile_operation())
            .collect::<Vec<_>>()
            .into()
    }
}

pub struct CustomContentParser;

impl BytesContentParser<CustomScript> for CustomContentParser {
    fn parse(&self, bytes: Vec<u8>) -> Result<CustomScript, Box<dyn Error>> {
        Ok(String::from_utf8(bytes)?
            .lines()
            .filter(|line| !line.is_empty())
            .map(CustomOperation::from_str)
            .collect::<Result<CustomScript, _>>()?)
    }
}

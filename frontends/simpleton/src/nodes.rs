use crate::{Reference, script::SimpletonLiteral};
use intuicio_core::{function::FunctionQuery, registry::Registry, types::TypeQuery};
use intuicio_nodes::nodes::{
    Node, NodeDefinition, NodePin, NodeSuggestion, NodeTypeInfo, PropertyValue,
    ResponseSuggestionNode,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimpletonNodeTypeInfo;

impl SimpletonNodeTypeInfo {}

impl NodeTypeInfo for SimpletonNodeTypeInfo {
    fn type_query(&self) -> TypeQuery {
        TypeQuery::of::<Reference>()
    }

    fn are_compatible(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Display for SimpletonNodeTypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "reflect::Reference",)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimpletonExpressionNodes {
    FindStruct {
        name: String,
        module_name: String,
    },
    FindFunction {
        name: String,
        module_name: String,
    },
    Closure {
        captures: Vec<String>,
        arguments: Vec<String>,
    },
    Literal(SimpletonLiteral),
    GetVariable {
        name: String,
    },
    CallFunction {
        name: String,
        module_name: String,
    },
    GetField {
        name: String,
    },
    GetArrayItem,
    GetMapIndex,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum SimpletonNodes {
    #[default]
    Start,
    CreateVariable {
        name: String,
    },
    AssignValue,
    Expression(SimpletonExpressionNodes),
    Return,
    IfElse,
    While,
    For {
        variable: String,
    },
}

impl NodeDefinition for SimpletonNodes {
    type TypeInfo = SimpletonNodeTypeInfo;

    fn node_label(&self, _: &Registry) -> String {
        match self {
            SimpletonNodes::Start => "Start".to_owned(),
            SimpletonNodes::CreateVariable { .. } => "Create variable".to_owned(),
            SimpletonNodes::AssignValue => "Assign value".to_owned(),
            SimpletonNodes::Expression(expression) => match expression {
                SimpletonExpressionNodes::FindStruct { .. } => "Find struct".to_owned(),
                SimpletonExpressionNodes::FindFunction { .. } => "Find function".to_owned(),
                SimpletonExpressionNodes::Closure { .. } => "Closure".to_owned(),
                SimpletonExpressionNodes::Literal(literal) => match literal {
                    SimpletonLiteral::Null => "Null literal".to_owned(),
                    SimpletonLiteral::Boolean(_) => "Boolean literal".to_owned(),
                    SimpletonLiteral::Integer(_) => "Integer literal".to_owned(),
                    SimpletonLiteral::Real(_) => "Real number literal".to_owned(),
                    SimpletonLiteral::Text(_) => "Text literal".to_owned(),
                    SimpletonLiteral::Array { .. } => "Array literal".to_owned(),
                    SimpletonLiteral::Map { .. } => "Map literal".to_owned(),
                    SimpletonLiteral::Object { .. } => "Object literal".to_owned(),
                },
                SimpletonExpressionNodes::GetVariable { .. } => "Get variable".to_owned(),
                SimpletonExpressionNodes::CallFunction { .. } => "Call function".to_owned(),
                SimpletonExpressionNodes::GetField { .. } => "Get field".to_owned(),
                SimpletonExpressionNodes::GetArrayItem => "Get array item".to_owned(),
                SimpletonExpressionNodes::GetMapIndex => "Get map item".to_owned(),
            },
            SimpletonNodes::Return => "Return value".to_owned(),
            SimpletonNodes::IfElse => "If-else branch".to_owned(),
            SimpletonNodes::While => "While loop".to_owned(),
            SimpletonNodes::For { .. } => "For loop".to_owned(),
        }
    }

    fn node_pins_in(&self, registry: &Registry) -> Vec<NodePin<Self::TypeInfo>> {
        match self {
            SimpletonNodes::Start => vec![],
            SimpletonNodes::CreateVariable { .. } => {
                vec![
                    NodePin::execute("In", false),
                    NodePin::parameter("Value", SimpletonNodeTypeInfo),
                    NodePin::property("Name"),
                ]
            }
            SimpletonNodes::AssignValue => vec![
                NodePin::execute("In", false),
                NodePin::parameter("Object", SimpletonNodeTypeInfo),
                NodePin::parameter("Value", SimpletonNodeTypeInfo),
            ],
            SimpletonNodes::Expression(expression) => match expression {
                SimpletonExpressionNodes::FindStruct { .. } => {
                    vec![NodePin::property("Name"), NodePin::property("Module name")]
                }
                SimpletonExpressionNodes::FindFunction { .. } => {
                    vec![NodePin::property("Name"), NodePin::property("Module name")]
                }
                SimpletonExpressionNodes::Closure { .. } => vec![
                    NodePin::property("Captures"),
                    NodePin::property("Arguments"),
                ],
                SimpletonExpressionNodes::Literal(literal) => match literal {
                    SimpletonLiteral::Null => vec![],
                    SimpletonLiteral::Array { items } => (0..items.len())
                        .map(|index| {
                            NodePin::parameter(format!("Value #{index}"), SimpletonNodeTypeInfo)
                        })
                        .collect(),
                    SimpletonLiteral::Map { items } => (0..items.len())
                        .flat_map(|index| {
                            [
                                NodePin::property(format!("Key #{index}")),
                                NodePin::parameter(
                                    format!("Value #{index}"),
                                    SimpletonNodeTypeInfo,
                                ),
                            ]
                        })
                        .collect(),
                    SimpletonLiteral::Object { fields, .. } => {
                        let mut result =
                            vec![NodePin::property("Name"), NodePin::property("Module name")];
                        result.extend((0..fields.len()).flat_map(|index| {
                            [
                                NodePin::property(format!("Field #{index}")),
                                NodePin::parameter(
                                    format!("Value #{index}"),
                                    SimpletonNodeTypeInfo,
                                ),
                            ]
                        }));
                        result
                    }
                    _ => vec![NodePin::property("Value")],
                },
                SimpletonExpressionNodes::GetVariable { .. } => vec![NodePin::property("Name")],
                SimpletonExpressionNodes::CallFunction { name, module_name } => {
                    let mut result =
                        vec![NodePin::property("Name"), NodePin::property("Module name")];
                    if let Some(function) = registry.find_function(FunctionQuery {
                        name: Some(name.into()),
                        module_name: Some(module_name.into()),
                        ..Default::default()
                    }) {
                        result.extend(function.signature().inputs.iter().flat_map(|input| {
                            [NodePin::parameter(&input.name, SimpletonNodeTypeInfo)]
                        }));
                    }
                    result
                }
                SimpletonExpressionNodes::GetField { .. } => vec![NodePin::property("Name")],
                SimpletonExpressionNodes::GetArrayItem => {
                    vec![NodePin::parameter("Index", SimpletonNodeTypeInfo)]
                }
                SimpletonExpressionNodes::GetMapIndex => {
                    vec![NodePin::parameter("Key", SimpletonNodeTypeInfo)]
                }
            },
            SimpletonNodes::Return => vec![
                NodePin::execute("In", false),
                NodePin::parameter("Value", SimpletonNodeTypeInfo),
            ],
            SimpletonNodes::IfElse => vec![
                NodePin::execute("In", false),
                NodePin::parameter("Condition", SimpletonNodeTypeInfo),
            ],
            SimpletonNodes::While => vec![
                NodePin::execute("In", false),
                NodePin::parameter("Condition", SimpletonNodeTypeInfo),
            ],
            SimpletonNodes::For { .. } => vec![
                NodePin::execute("In", false),
                NodePin::parameter("Iterator", SimpletonNodeTypeInfo),
                NodePin::property("Variable"),
            ],
        }
    }

    fn node_pins_out(&self, _: &Registry) -> Vec<NodePin<Self::TypeInfo>> {
        match self {
            SimpletonNodes::Expression(_) => {
                vec![NodePin::parameter("Result", SimpletonNodeTypeInfo)]
            }
            SimpletonNodes::Return => vec![],
            SimpletonNodes::IfElse => vec![
                NodePin::execute("Out", false),
                NodePin::execute("Success body", true),
                NodePin::execute("Failure body", true),
            ],
            SimpletonNodes::While | SimpletonNodes::For { .. } => vec![
                NodePin::execute("Out", false),
                NodePin::execute("Iteration body", true),
            ],
            _ => vec![NodePin::execute("Out", false)],
        }
    }

    fn node_is_start(&self, _: &Registry) -> bool {
        matches!(self, Self::Start)
    }

    fn node_suggestions(
        x: i64,
        y: i64,
        _: NodeSuggestion<Self>,
        registry: &Registry,
    ) -> Vec<ResponseSuggestionNode<Self>> {
        vec![
            ResponseSuggestionNode::new(
                "Variable",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::CreateVariable {
                        name: "variable".to_owned(),
                    },
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Variable",
                Node::new(x, y, SimpletonNodes::AssignValue),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Type",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::FindStruct {
                        name: "Integer".to_owned(),
                        module_name: "math".to_owned(),
                    }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Type",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::FindFunction {
                        name: "add".to_owned(),
                        module_name: "math".to_owned(),
                    }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Type",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::Closure {
                        captures: vec![],
                        arguments: vec![],
                    }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::Literal(
                        SimpletonLiteral::Null,
                    )),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::Literal(
                        SimpletonLiteral::Boolean(false),
                    )),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::Literal(
                        SimpletonLiteral::Integer(0),
                    )),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::Literal(
                        SimpletonLiteral::Real(0.0),
                    )),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::Literal(
                        SimpletonLiteral::Text("text".to_owned()),
                    )),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::Literal(
                        SimpletonLiteral::Array { items: vec![] },
                    )),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::Literal(
                        SimpletonLiteral::Map { items: vec![] },
                    )),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Literal",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::Literal(
                        SimpletonLiteral::Object {
                            name: "Integer".to_owned(),
                            module_name: "math".to_owned(),
                            fields: vec![],
                        },
                    )),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Access",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::GetVariable {
                        name: "variable".to_owned(),
                    }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Call",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::CallFunction {
                        name: "add".to_owned(),
                        module_name: "math".to_owned(),
                    }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Access",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::GetField {
                        name: "field".to_owned(),
                    }),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Access",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::GetArrayItem),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Access",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::Expression(SimpletonExpressionNodes::GetMapIndex),
                ),
                registry,
            ),
            ResponseSuggestionNode::new(
                "Statement",
                Node::new(x, y, SimpletonNodes::Return),
                registry,
            ),
            ResponseSuggestionNode::new("Scope", Node::new(x, y, SimpletonNodes::IfElse), registry),
            ResponseSuggestionNode::new("Scope", Node::new(x, y, SimpletonNodes::While), registry),
            ResponseSuggestionNode::new(
                "Scope",
                Node::new(
                    x,
                    y,
                    SimpletonNodes::For {
                        variable: "item".to_owned(),
                    },
                ),
                registry,
            ),
        ]
    }

    fn get_property(&self, property_name: &str) -> Option<PropertyValue> {
        match self {
            SimpletonNodes::CreateVariable { name } => match property_name {
                "Name" => PropertyValue::new(name).ok(),
                _ => None,
            },
            SimpletonNodes::Expression(expression) => match expression {
                SimpletonExpressionNodes::FindStruct { name, module_name }
                | SimpletonExpressionNodes::FindFunction { name, module_name }
                | SimpletonExpressionNodes::CallFunction { name, module_name } => {
                    match property_name {
                        "Name" => PropertyValue::new(name).ok(),
                        "Module name" => PropertyValue::new(module_name).ok(),
                        _ => None,
                    }
                }
                SimpletonExpressionNodes::Closure {
                    captures,
                    arguments,
                } => match property_name {
                    "Captures" => PropertyValue::new(captures).ok(),
                    "Arguments" => PropertyValue::new(arguments).ok(),
                    _ => None,
                },
                SimpletonExpressionNodes::Literal(literal) => match literal {
                    SimpletonLiteral::Null => None,
                    SimpletonLiteral::Boolean(value) => match property_name {
                        "Value" => PropertyValue::new(value).ok(),
                        _ => None,
                    },
                    SimpletonLiteral::Integer(value) => match property_name {
                        "Value" => PropertyValue::new(value).ok(),
                        _ => None,
                    },
                    SimpletonLiteral::Real(value) => match property_name {
                        "Value" => PropertyValue::new(value).ok(),
                        _ => None,
                    },
                    SimpletonLiteral::Text(value) => match property_name {
                        "Value" => PropertyValue::new(value).ok(),
                        _ => None,
                    },
                    SimpletonLiteral::Array { .. } => None,
                    SimpletonLiteral::Map { items } => property_name
                        .strip_prefix("Key #")
                        .and_then(|property_name| {
                            property_name
                                .parse::<usize>()
                                .ok()
                                .and_then(|index| items.get(index))
                                .and_then(|(value, _)| PropertyValue::new(value).ok())
                        }),
                    SimpletonLiteral::Object {
                        name,
                        module_name,
                        fields,
                    } => match property_name {
                        "Name" => PropertyValue::new(name).ok(),
                        "Module name" => PropertyValue::new(module_name).ok(),
                        _ => property_name
                            .strip_prefix("Field #")
                            .and_then(|property_name| {
                                property_name
                                    .parse::<usize>()
                                    .ok()
                                    .and_then(|index| fields.get(index))
                                    .and_then(|(value, _)| PropertyValue::new(value).ok())
                            }),
                    },
                },
                SimpletonExpressionNodes::GetVariable { name }
                | SimpletonExpressionNodes::GetField { name } => match property_name {
                    "Name" => PropertyValue::new(name).ok(),
                    _ => None,
                },
                _ => None,
            },
            SimpletonNodes::For { variable } => match property_name {
                "Variable" => PropertyValue::new(variable).ok(),
                _ => None,
            },
            _ => None,
        }
    }

    fn set_property(&mut self, property_name: &str, property_value: PropertyValue) {
        match self {
            SimpletonNodes::CreateVariable { name } => {
                if property_name == "Name" {
                    if let Ok(v) = property_value.get_exact() {
                        *name = v;
                    }
                }
            }
            SimpletonNodes::Expression(expression) => match expression {
                SimpletonExpressionNodes::FindStruct { name, module_name }
                | SimpletonExpressionNodes::FindFunction { name, module_name }
                | SimpletonExpressionNodes::CallFunction { name, module_name } => {
                    match property_name {
                        "Name" => {
                            if let Ok(v) = property_value.get_exact() {
                                *name = v;
                            }
                        }
                        "Module name" => {
                            if let Ok(v) = property_value.get_exact() {
                                *module_name = v;
                            }
                        }
                        _ => {}
                    }
                }
                SimpletonExpressionNodes::Closure {
                    captures,
                    arguments,
                } => match property_name {
                    "Captures" => {
                        if let Ok(v) = property_value.get_exact() {
                            *captures = v;
                        }
                    }
                    "Arguments" => {
                        if let Ok(v) = property_value.get_exact() {
                            *arguments = v;
                        }
                    }
                    _ => {}
                },
                SimpletonExpressionNodes::Literal(literal) => match literal {
                    SimpletonLiteral::Null => {}
                    SimpletonLiteral::Boolean(value) => {
                        if property_name == "Value" {
                            if let Ok(v) = property_value.get_exact() {
                                *value = v;
                            }
                        }
                    }
                    SimpletonLiteral::Integer(value) => {
                        if property_name == "Value" {
                            if let Ok(v) = property_value.get_exact() {
                                *value = v;
                            }
                        }
                    }
                    SimpletonLiteral::Real(value) => {
                        if property_name == "Value" {
                            if let Ok(v) = property_value.get_exact() {
                                *value = v;
                            }
                        }
                    }
                    SimpletonLiteral::Text(value) => {
                        if property_name == "Value" {
                            if let Ok(v) = property_value.get_exact() {
                                *value = v;
                            }
                        }
                    }
                    SimpletonLiteral::Array { .. } => {}
                    SimpletonLiteral::Map { items } => {
                        if let Some(property_name) = property_name.strip_prefix("Key #") {
                            if let Ok(v) = property_value.get_exact() {
                                if let Some((value, _)) = property_name
                                    .parse::<usize>()
                                    .ok()
                                    .and_then(|index| items.get_mut(index))
                                {
                                    *value = v;
                                }
                            }
                        }
                    }
                    SimpletonLiteral::Object {
                        name,
                        module_name,
                        fields,
                    } => match property_name {
                        "Name" => {
                            if let Ok(v) = property_value.get_exact() {
                                *name = v;
                            }
                        }
                        "Module name" => {
                            if let Ok(v) = property_value.get_exact() {
                                *module_name = v;
                            }
                        }
                        _ => {
                            if let Some(property_name) = property_name.strip_prefix("Field #") {
                                if let Ok(v) = property_value.get_exact() {
                                    if let Some((value, _)) = property_name
                                        .parse::<usize>()
                                        .ok()
                                        .and_then(|index| fields.get_mut(index))
                                    {
                                        *value = v;
                                    }
                                }
                            }
                        }
                    },
                },
                SimpletonExpressionNodes::GetVariable { name }
                | SimpletonExpressionNodes::GetField { name } => {
                    if property_name == "Name" {
                        if let Ok(v) = property_value.get_exact::<String>() {
                            *name = v;
                        }
                    }
                }
                _ => {}
            },
            SimpletonNodes::For { variable } => {
                if property_name == "Variable" {
                    if let Ok(v) = property_value.get_exact::<String>() {
                        *variable = v;
                    }
                }
            }
            _ => {}
        }
    }
}

// pub struct CompileSimpletonNodeGraphVisitor;

// pub enum CompileSimpletonNodeGraphVisitorInput {
//     Start(SimpletonExpressionStart),
//     Next(SimpletonExpressionNext),
// }

// impl CompileSimpletonNodeGraphVisitorInput {
//     fn into_start(self) -> Option<SimpletonExpressionStart> {
//         match self {
//             Self::Start(result) => Some(result),
//             _ => None,
//         }
//     }

//     fn into_next(self) -> Option<SimpletonExpressionNext> {
//         match self {
//             Self::Next(result) => Some(result),
//             _ => None,
//         }
//     }
// }

// impl NodeGraphVisitor<SimpletonNodes> for CompileSimpletonNodeGraphVisitor {
//     type Input = CompileSimpletonNodeGraphVisitorInput;
//     type Output = SimpletonStatement;

//     fn visit_statement(
//         &mut self,
//         node: &Node<SimpletonNodes>,
//         inputs: HashMap<String, Self::Input>,
//         scopes: HashMap<String, Vec<Self::Output>>,
//         result: &mut Vec<Self::Output>,
//     ) -> bool {
//         match &node.data {
//             SimpletonNodes::Start => {}
//             SimpletonNodes::CreateVariable { name } => {
//                 // result.push(SimpletonStatement::CreateVariable { name, value: () });
//                 todo!()
//             }
//             SimpletonNodes::AssignValue => todo!(),
//             SimpletonNodes::Expression(_) => todo!(),
//             SimpletonNodes::Return => todo!(),
//             SimpletonNodes::IfElse => todo!(),
//             SimpletonNodes::While => todo!(),
//             SimpletonNodes::For { variable } => todo!(),
//         }
//         true
//     }

//     fn visit_expression(
//         &mut self,
//         node: &Node<SimpletonNodes>,
//         mut inputs: HashMap<String, Self::Input>,
//     ) -> Option<Self::Input> {
//         match &node.data {
//             SimpletonNodes::Expression(expression) => match expression {
//                 SimpletonExpressionNodes::FindStruct { name, module_name } => {
//                     Some(CompileSimpletonNodeGraphVisitorInput::Start(
//                         SimpletonExpressionStart::FindStruct {
//                             name: name.to_owned(),
//                             module_name: module_name.to_owned(),
//                             next: inputs.remove("Result").and_then(|next| next.into_next()),
//                         },
//                     ))
//                 }
//                 SimpletonExpressionNodes::FindFunction { name, module_name } => {
//                     Some(CompileSimpletonNodeGraphVisitorInput::Start(
//                         SimpletonExpressionStart::FindFunction {
//                             name: name.to_owned(),
//                             module_name: module_name.to_owned(),
//                             next: inputs.remove("Result").and_then(|next| next.into_next()),
//                         },
//                     ))
//                 }
//                 SimpletonExpressionNodes::Closure {
//                     captures,
//                     arguments,
//                 } => todo!(),
//                 SimpletonExpressionNodes::Literal(_) => todo!(),
//                 SimpletonExpressionNodes::GetVariable { name } => todo!(),
//                 SimpletonExpressionNodes::CallFunction { name, module_name } => todo!(),
//                 SimpletonExpressionNodes::GetField { name } => {
//                     Some(CompileSimpletonNodeGraphVisitorInput::Next(
//                         SimpletonExpressionNext::GetField {
//                             name: name.to_owned(),
//                             next: inputs
//                                 .remove("Result")
//                                 .and_then(|next| next.into_next())
//                                 .map(|next| next.into()),
//                         },
//                     ))
//                 }
//                 // SimpletonExpressionNodes::GetArrayItem => Some(CompileSimpletonNodeGraphVisitorInput::Next(
//                 //     SimpletonExpressionNext::GetArrayItem {
//                 //         index: inputs
//                 //         .remove("Result")
//                 //         .and_then(|next| next.into_start())
//                 //         .map(|next| Box::new(next)),
//                 //         next: inputs
//                 //         .remove("Result")
//                 //         .and_then(|next| next.into_next())
//                 //         .map(|next| next.into()),
//                 //     },
//                 // )),
//                 SimpletonExpressionNodes::GetMapIndex => todo!(),
//                 _ => todo!(),
//             },
//             _ => None,
//         }
//     }
// }

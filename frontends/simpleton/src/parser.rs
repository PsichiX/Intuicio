use crate::{
    Integer, Real, SimpletonExpressionNext, SimpletonExpressionStart, SimpletonFunction,
    SimpletonLiteral, SimpletonModule, SimpletonStatement, SimpletonStruct, Text,
};
use pest::{iterators::Pair, Parser};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct SimpletonParser;

pub fn parse(content: &str) -> Result<SimpletonModule, String> {
    match SimpletonParser::parse(Rule::file, content) {
        Ok(mut pairs) => {
            let pair = pairs.next().unwrap();
            match pair.as_rule() {
                Rule::file => Ok(parse_module(pair.into_inner().next().unwrap())),
                rule => unreachable!("{:?}", rule),
            }
        }
        Err(error) => return Err(format!("{}", error)),
    }
}

fn parse_module(pair: Pair<Rule>) -> SimpletonModule {
    let mut result = SimpletonModule {
        name: Default::default(),
        dependencies: vec![],
        structs: vec![],
        functions: vec![],
    };
    let mut pairs = pair.into_inner();
    result.name = parse_identifier(pairs.next().unwrap());
    for pair in pairs {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::import => {
                result
                    .dependencies
                    .push(parse_text(pair.into_inner().next().unwrap()));
            }
            Rule::structure => {
                result.structs.push(parse_structure(pair));
            }
            Rule::function => {
                result.functions.push(parse_function(pair));
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    result
}

fn parse_structure(pair: Pair<Rule>) -> SimpletonStruct {
    let mut pairs = pair.into_inner();
    let mut result = SimpletonStruct {
        name: Default::default(),
        fields: vec![],
    };
    result.name = parse_identifier(pairs.next().unwrap());
    for pair in pairs {
        match pair.as_rule() {
            Rule::structure_field => {
                result.fields.push(parse_identifier(pair));
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    result
}

fn parse_function(pair: Pair<Rule>) -> SimpletonFunction {
    let mut pairs = pair.into_inner();
    let mut result = SimpletonFunction {
        name: Default::default(),
        arguments: vec![],
        statements: vec![],
    };
    result.name = parse_identifier(pairs.next().unwrap());
    for pair in pairs {
        match pair.as_rule() {
            Rule::function_argument => {
                result.arguments.push(parse_identifier(pair));
            }
            Rule::statement => {
                result.statements.push(parse_statement(pair));
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    result
}

fn parse_statement(pair: Pair<Rule>) -> SimpletonStatement {
    let pair = pair.into_inner().next().unwrap();
    match pair.as_rule() {
        Rule::create_variable => parse_create_variable(pair),
        Rule::assign_value => parse_assign_value(pair),
        Rule::expression => SimpletonStatement::Expression(parse_expression_start(
            pair.into_inner().next().unwrap(),
        )),
        Rule::return_value => {
            SimpletonStatement::Return(parse_expression_start(pair.into_inner().next().unwrap()))
        }
        Rule::if_else => {
            let mut pairs = pair.into_inner();
            let condition = parse_expression_start(pairs.next().unwrap());
            let mut success = vec![];
            let mut failure = None;
            for pair in pairs {
                match pair.as_rule() {
                    Rule::if_else_success => {
                        success = pair
                            .into_inner()
                            .map(|pair| parse_statement(pair))
                            .collect();
                    }
                    Rule::if_else_failure => {
                        failure = Some(
                            pair.into_inner()
                                .map(|pair| parse_statement(pair))
                                .collect(),
                        );
                    }
                    rule => unreachable!("{:?}", rule),
                }
            }
            SimpletonStatement::IfElse {
                condition,
                success,
                failure,
            }
        }
        Rule::while_loop => {
            let mut pairs = pair.into_inner();
            let condition = parse_expression_start(pairs.next().unwrap());
            let statements = pairs.map(|pair| parse_statement(pair)).collect();
            SimpletonStatement::While {
                condition,
                statements,
            }
        }
        Rule::for_loop => {
            let mut pairs = pair.into_inner();
            let variable = parse_identifier(pairs.next().unwrap());
            let iterator = parse_expression_start(pairs.next().unwrap());
            let statements = pairs.map(|pair| parse_statement(pair)).collect();
            SimpletonStatement::For {
                variable,
                iterator,
                statements,
            }
        }
        rule => unreachable!("{:?}", rule),
    }
}

fn parse_create_variable(pair: Pair<Rule>) -> SimpletonStatement {
    let mut pairs = pair.into_inner();
    let name = parse_identifier(pairs.next().unwrap());
    let value = parse_expression_start(pairs.next().unwrap());
    SimpletonStatement::CreateVariable { name, value }
}

fn parse_assign_value(pair: Pair<Rule>) -> SimpletonStatement {
    let mut pairs = pair.into_inner();
    let object = parse_expression_start(pairs.next().unwrap());
    let value = parse_expression_start(pairs.next().unwrap());
    SimpletonStatement::AssignValue { object, value }
}

fn parse_expression_start(pair: Pair<Rule>) -> SimpletonExpressionStart {
    let pair = pair.into_inner().next().unwrap();
    match pair.as_rule() {
        Rule::find_structure => {
            let mut pairs = pair.into_inner();
            let (name, module_name) = parse_path(pairs.next().unwrap());
            let next = if let Some(pair) = pairs.next() {
                Some(parse_expression_next(pair))
            } else {
                None
            };
            SimpletonExpressionStart::FindStruct {
                name,
                module_name,
                next,
            }
        }
        Rule::find_function => {
            let mut pairs = pair.into_inner();
            let (name, module_name) = parse_path(pairs.next().unwrap());
            let next = if let Some(pair) = pairs.next() {
                Some(parse_expression_next(pair))
            } else {
                None
            };
            SimpletonExpressionStart::FindFunction {
                name,
                module_name,
                next,
            }
        }
        Rule::closure => {
            let pairs = pair.into_inner();
            let mut captures = vec![];
            let mut arguments = vec![];
            let mut statements = vec![];
            let mut next = None;
            for pair in pairs {
                match pair.as_rule() {
                    Rule::closure_capture => {
                        captures.push(parse_identifier(pair));
                    }
                    Rule::function_argument => {
                        arguments.push(parse_identifier(pair));
                    }
                    Rule::statement => {
                        statements.push(parse_statement(pair));
                    }
                    Rule::expression_next => {
                        next = Some(parse_expression_next(pair));
                    }
                    rule => unreachable!("{:?}", rule),
                }
            }
            SimpletonExpressionStart::Closure {
                captures,
                arguments,
                statements,
                next,
            }
        }
        Rule::literal => {
            let mut pairs = pair.into_inner();
            let literal = parse_literal(pairs.next().unwrap());
            let next = if let Some(pair) = pairs.next() {
                Some(parse_expression_next(pair))
            } else {
                None
            };
            SimpletonExpressionStart::Literal { literal, next }
        }
        Rule::get_variable => {
            let mut pairs = pair.into_inner();
            let name = parse_identifier(pairs.next().unwrap());
            let next = if let Some(pair) = pairs.next() {
                Some(parse_expression_next(pair))
            } else {
                None
            };
            SimpletonExpressionStart::GetVariable { name, next }
        }
        Rule::call_function => {
            let mut pairs = pair.into_inner();
            let (name, module_name) = parse_path(pairs.next().unwrap());
            let mut arguments = vec![];
            let mut next = None;
            for pair in pairs {
                match pair.as_rule() {
                    Rule::call_argument => {
                        arguments.push(parse_expression_start(pair.into_inner().next().unwrap()));
                    }
                    Rule::expression_next => {
                        next = Some(parse_expression_next(pair));
                    }
                    rule => unreachable!("{:?}", rule),
                }
            }
            SimpletonExpressionStart::CallFunction {
                name,
                module_name,
                arguments,
                next,
            }
        }
        rule => unreachable!("{:?}", rule),
    }
}

fn parse_expression_next(pair: Pair<Rule>) -> SimpletonExpressionNext {
    let pair = pair.into_inner().next().unwrap();
    match pair.as_rule() {
        Rule::get_field => {
            let mut pairs = pair.into_inner();
            let name = parse_identifier(pairs.next().unwrap());
            let next = if let Some(pair) = pairs.next() {
                Some(Box::new(parse_expression_next(pair)))
            } else {
                None
            };
            SimpletonExpressionNext::GetField { name, next }
        }
        Rule::get_array_item => {
            let mut pairs = pair.into_inner();
            let index = Box::new(parse_expression_start(pairs.next().unwrap()));
            let next = if let Some(pair) = pairs.next() {
                Some(Box::new(parse_expression_next(pair)))
            } else {
                None
            };
            SimpletonExpressionNext::GetArrayItem { index, next }
        }
        Rule::get_map_item => {
            let mut pairs = pair.into_inner();
            let index = Box::new(parse_expression_start(pairs.next().unwrap()));
            let next = if let Some(pair) = pairs.next() {
                Some(Box::new(parse_expression_next(pair)))
            } else {
                None
            };
            SimpletonExpressionNext::GetMapItem { index, next }
        }
        rule => unreachable!("{:?}", rule),
    }
}

fn parse_literal(pair: Pair<Rule>) -> SimpletonLiteral {
    match pair.as_rule() {
        Rule::null => SimpletonLiteral::Null,
        Rule::bool_true => SimpletonLiteral::Boolean(true),
        Rule::bool_false => SimpletonLiteral::Boolean(false),
        Rule::integer => SimpletonLiteral::Integer(parse_integer(pair)),
        Rule::hex_inner => SimpletonLiteral::Integer(parse_hex(pair)),
        Rule::binary_inner => SimpletonLiteral::Integer(parse_binary(pair)),
        Rule::real => SimpletonLiteral::Real(parse_real(pair)),
        Rule::text => SimpletonLiteral::Text(parse_text(pair)),
        Rule::array => SimpletonLiteral::Array {
            items: parse_array(pair),
        },
        Rule::map => SimpletonLiteral::Map {
            items: parse_map(pair),
        },
        Rule::object => {
            let (name, module_name, fields) = parse_object(pair);
            SimpletonLiteral::Object {
                name,
                module_name,
                fields,
            }
        }
        rule => unreachable!("{:?}", rule),
    }
}

fn parse_integer(pair: Pair<Rule>) -> Integer {
    pair.as_str().parse::<Integer>().unwrap()
}

fn parse_hex(pair: Pair<Rule>) -> Integer {
    Integer::from_str_radix(pair.as_str(), 16).unwrap()
}

fn parse_binary(pair: Pair<Rule>) -> Integer {
    Integer::from_str_radix(pair.as_str(), 2).unwrap()
}

fn parse_real(pair: Pair<Rule>) -> Real {
    pair.as_str().parse::<Real>().unwrap()
}

fn parse_text(pair: Pair<Rule>) -> Text {
    snailquote::unescape(pair.as_str()).unwrap()
}

fn parse_array(pair: Pair<Rule>) -> Vec<SimpletonExpressionStart> {
    pair.into_inner()
        .map(|pair| parse_expression_start(pair))
        .collect()
}

fn parse_map(pair: Pair<Rule>) -> Vec<(String, SimpletonExpressionStart)> {
    pair.into_inner()
        .map(|pair| {
            let mut pairs = pair.into_inner();
            let key = parse_identifier(pairs.next().unwrap());
            let value = if let Some(pair) = pairs.next() {
                parse_expression_start(pair)
            } else {
                SimpletonExpressionStart::GetVariable {
                    name: key.to_owned(),
                    next: None,
                }
            };
            (key, value)
        })
        .collect()
}

fn parse_object(pair: Pair<Rule>) -> (String, String, Vec<(String, SimpletonExpressionStart)>) {
    let mut pairs = pair.into_inner();
    let (name, module_name) = parse_path(pairs.next().unwrap());
    let fields = pairs
        .map(|pair| {
            let mut pairs = pair.into_inner();
            let key = parse_identifier(pairs.next().unwrap());
            let value = if let Some(pair) = pairs.next() {
                parse_expression_start(pair)
            } else {
                SimpletonExpressionStart::GetVariable {
                    name: key.to_owned(),
                    next: None,
                }
            };
            (key, value)
        })
        .collect();
    (name, module_name, fields)
}

fn parse_path(pair: Pair<Rule>) -> (String, String) {
    let mut pairs = pair.into_inner();
    let module_name = parse_identifier(pairs.next().unwrap());
    let name = parse_identifier(pairs.next().unwrap());
    (name, module_name)
}

fn parse_identifier(pair: Pair<Rule>) -> String {
    pair.as_str().to_owned()
}

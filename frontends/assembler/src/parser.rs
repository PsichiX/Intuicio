use std::{fmt::Debug, str::FromStr};

use crate::{
    AsmExpression, AsmFile, AsmFunction, AsmFunctionParameter, AsmLiteral, AsmModule, AsmOperation,
    AsmStruct, AsmStructField,
};
use intuicio_core::Visibility;
use pest::{iterators::Pair, Parser};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct AsmParser;

pub fn parse(content: &str) -> Result<AsmFile, String> {
    match AsmParser::parse(Rule::file, content) {
        Ok(mut pairs) => {
            let pair = pairs.next().unwrap();
            match pair.as_rule() {
                Rule::file => Ok(parse_file(pair)),
                rule => unreachable!("{:?}", rule),
            }
        }
        Err(error) => return Err(format!("{}", error)),
    }
}

fn parse_file(pair: Pair<Rule>) -> AsmFile {
    let mut result = AsmFile::default();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::file_item => {
                let pair = pair.into_inner().next().unwrap();
                match pair.as_rule() {
                    Rule::import => {
                        result
                            .dependencies
                            .push(parse_string(pair.into_inner().next().unwrap()));
                    }
                    Rule::module => {
                        result.modules.push(parse_module(pair));
                    }
                    rule => unreachable!("{:?}", rule),
                }
            }
            Rule::EOI => {}
            rule => unreachable!("{:?}", rule),
        }
    }
    result
}

fn parse_module(pair: Pair<Rule>) -> AsmModule {
    let mut pairs = pair.into_inner();
    let mut result = AsmModule {
        name: parse_identifier(pairs.next().unwrap()),
        structs: vec![],
        functions: vec![],
    };
    for pair in pairs {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::function => {
                result.functions.push(parse_function(pair));
            }
            Rule::structure => {
                result.structs.push(parse_structure(pair));
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    result
}

fn parse_structure(pair: Pair<Rule>) -> AsmStruct {
    let mut pairs = pair.into_inner();
    let mut result = parse_struct_header(pairs.next().unwrap());
    result.fields = parse_struct_fields(pairs.next().unwrap());
    result
}

fn parse_struct_fields(pair: Pair<Rule>) -> Vec<AsmStructField> {
    pair.into_inner()
        .map(|pair| parse_struct_field(pair))
        .collect()
}

fn parse_struct_field(pair: Pair<Rule>) -> AsmStructField {
    let pairs = pair.into_inner();
    let mut result = AsmStructField {
        name: Default::default(),
        visibility: Visibility::Public,
        module_name: None,
        struct_name: Default::default(),
    };
    for pair in pairs {
        match pair.as_rule() {
            Rule::visibility => {
                result.visibility = parse_visibility(pair);
            }
            Rule::identifier => {
                result.name = parse_identifier(pair);
            }
            Rule::path_module => {
                result.module_name = Some(parse_path_name(pair));
            }
            Rule::path_struct => {
                result.name = parse_path_name(pair);
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    result
}

fn parse_struct_header(pair: Pair<Rule>) -> AsmStruct {
    let pairs = pair.into_inner();
    let mut result = AsmStruct {
        name: Default::default(),
        visibility: Visibility::Public,
        fields: vec![],
    };
    for pair in pairs {
        match pair.as_rule() {
            Rule::visibility => {
                result.visibility = parse_visibility(pair);
            }
            Rule::path_struct => {
                result.name = parse_path_name(pair);
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    result
}

fn parse_function(pair: Pair<Rule>) -> AsmFunction {
    let mut pairs = pair.into_inner();
    let mut result = parse_function_header(pairs.next().unwrap());
    result.inputs = parse_function_parameters(pairs.next().unwrap());
    result.outputs = parse_function_parameters(pairs.next().unwrap());
    result.script = parse_scope(pairs.next().unwrap());
    result
}

fn parse_scope(pair: Pair<Rule>) -> Vec<AsmOperation> {
    pair.into_inner()
        .map(|pair| parse_operation(pair))
        .collect()
}

fn parse_operation(pair: Pair<Rule>) -> AsmOperation {
    let pair = pair.into_inner().next().unwrap();
    match pair.as_rule() {
        Rule::push_literal => parse_push_literal(pair),
        Rule::stack_drop => AsmOperation::Expression(AsmExpression::StackDrop),
        Rule::make_register => parse_make_register(pair),
        Rule::drop_register => parse_drop_register(pair),
        Rule::push_from_register => parse_push_from_register(pair),
        Rule::pop_to_register => parse_pop_to_register(pair),
        Rule::call_function => parse_call_function(pair),
        Rule::branch_scope => parse_branch_scope(pair),
        Rule::loop_scope => parse_loop_scope(pair),
        Rule::pop_scope => AsmOperation::PopScope,
        rule => unreachable!("{:?}", rule),
    }
}

fn parse_loop_scope(pair: Pair<Rule>) -> AsmOperation {
    AsmOperation::LoopScope {
        script: parse_scope(pair.into_inner().next().unwrap()),
    }
}

fn parse_branch_scope(pair: Pair<Rule>) -> AsmOperation {
    let mut pairs = pair.into_inner();
    AsmOperation::BranchScope {
        script_success: parse_scope(pairs.next().unwrap()),
        script_failure: pairs.next().map(|pair| parse_scope(pair)),
    }
}

fn parse_call_function(pair: Pair<Rule>) -> AsmOperation {
    let pairs = pair.into_inner();
    let mut name = Default::default();
    let mut module_name = None;
    let mut struct_name = None;
    let mut visibility = None;
    for pair in pairs {
        match pair.as_rule() {
            Rule::visibility => {
                visibility = Some(parse_visibility(pair));
            }
            Rule::path_module => {
                module_name = Some(parse_path_name(pair));
            }
            Rule::path_struct => {
                struct_name = Some(parse_path_name(pair));
            }
            Rule::path_function => {
                name = parse_path_name(pair);
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    AsmOperation::CallFunction {
        name,
        module_name,
        struct_name,
        visibility,
    }
}

fn parse_pop_to_register(pair: Pair<Rule>) -> AsmOperation {
    AsmOperation::PopToRegister {
        index: parse_literal::<usize>(pair),
    }
}

fn parse_push_from_register(pair: Pair<Rule>) -> AsmOperation {
    AsmOperation::PushFromRegister {
        index: parse_literal::<usize>(pair),
    }
}

fn parse_drop_register(pair: Pair<Rule>) -> AsmOperation {
    AsmOperation::DropRegister {
        index: parse_literal::<usize>(pair),
    }
}

fn parse_make_register(pair: Pair<Rule>) -> AsmOperation {
    let pairs = pair.into_inner();
    let mut name = Default::default();
    let mut module_name = None;
    for pair in pairs {
        match pair.as_rule() {
            Rule::path_module => {
                module_name = Some(parse_path_name(pair));
            }
            Rule::path_struct => {
                name = parse_path_name(pair);
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    AsmOperation::MakeRegister { name, module_name }
}

fn parse_push_literal(pair: Pair<Rule>) -> AsmOperation {
    let pair = pair
        .into_inner()
        .next()
        .unwrap()
        .into_inner()
        .next()
        .unwrap();
    match pair.as_rule() {
        Rule::literal_unit => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::Unit)),
        Rule::literal_bool_false => {
            AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::Bool(false)))
        }
        Rule::literal_bool_true => {
            AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::Bool(true)))
        }
        Rule::literal_i8 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::I8(
            parse_literal::<i8>(pair),
        ))),
        Rule::literal_i16 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::I16(
            parse_literal::<i16>(pair),
        ))),
        Rule::literal_i32 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::I32(
            parse_literal::<i32>(pair),
        ))),
        Rule::literal_i64 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::I64(
            parse_literal::<i64>(pair),
        ))),
        Rule::literal_i128 => {
            AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::I128(parse_literal::<
                i128,
            >(pair))))
        }
        Rule::literal_isize => {
            AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::Isize(parse_literal::<
                isize,
            >(
                pair
            ))))
        }
        Rule::literal_u8 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::U8(
            parse_literal::<u8>(pair),
        ))),
        Rule::literal_u16 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::U16(
            parse_literal::<u16>(pair),
        ))),
        Rule::literal_u32 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::U32(
            parse_literal::<u32>(pair),
        ))),
        Rule::literal_u64 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::U64(
            parse_literal::<u64>(pair),
        ))),
        Rule::literal_u128 => {
            AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::U128(parse_literal::<
                u128,
            >(pair))))
        }
        Rule::literal_usize => {
            AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::Usize(parse_literal::<
                usize,
            >(
                pair
            ))))
        }
        Rule::literal_f32 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::F32(
            parse_literal::<f32>(pair),
        ))),
        Rule::literal_f64 => AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::F64(
            parse_literal::<f64>(pair),
        ))),
        Rule::literal_char => {
            AsmOperation::Expression(AsmExpression::Literal(AsmLiteral::Char(parse_literal::<
                char,
            >(pair))))
        }
        Rule::literal_string => AsmOperation::Expression(AsmExpression::Literal(
            AsmLiteral::String(parse_string(pair)),
        )),
        rule => unreachable!("{:?}", rule),
    }
}

fn parse_function_parameters(pair: Pair<Rule>) -> Vec<AsmFunctionParameter> {
    pair.into_inner()
        .map(|pair| parse_function_parameter(pair))
        .collect()
}

fn parse_function_parameter(pair: Pair<Rule>) -> AsmFunctionParameter {
    let mut pairs = pair.into_inner();
    let mut result = AsmFunctionParameter {
        name: parse_identifier(pairs.next().unwrap()),
        module_name: None,
        struct_name: Default::default(),
    };
    for pair in pairs {
        match pair.as_rule() {
            Rule::path_module => {
                result.module_name = Some(parse_path_name(pair));
            }
            Rule::path_struct => {
                result.struct_name = parse_path_name(pair);
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    result
}

fn parse_function_header(pair: Pair<Rule>) -> AsmFunction {
    let pairs = pair.into_inner();
    let mut result = AsmFunction {
        name: Default::default(),
        struct_name: None,
        visibility: Visibility::Public,
        inputs: vec![],
        outputs: vec![],
        script: vec![],
    };
    for pair in pairs {
        match pair.as_rule() {
            Rule::visibility => {
                result.visibility = parse_visibility(pair);
            }
            Rule::path_struct => {
                result.struct_name = Some(parse_path_name(pair));
            }
            Rule::path_function => {
                result.name = parse_path_name(pair);
            }
            rule => unreachable!("{:?}", rule),
        }
    }
    result
}

fn parse_path_name(pair: Pair<Rule>) -> String {
    parse_identifier(pair.into_inner().next().unwrap())
}

fn parse_visibility(pair: Pair<Rule>) -> Visibility {
    match pair.into_inner().next().unwrap().as_rule() {
        Rule::visibility_public => Visibility::Public,
        Rule::visibility_internal => Visibility::Module,
        Rule::visibility_private => Visibility::Private,
        rule => unreachable!("{:?}", rule),
    }
}

fn parse_literal<T>(pair: Pair<Rule>) -> T
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    pair.into_inner()
        .next()
        .unwrap()
        .as_str()
        .parse::<T>()
        .unwrap()
}

fn parse_string(pair: Pair<Rule>) -> String {
    snailquote::unescape(pair.as_str()).unwrap()
}

fn parse_identifier(pair: Pair<Rule>) -> String {
    pair.as_str().to_owned()
}

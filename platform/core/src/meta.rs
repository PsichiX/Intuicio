use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Write};

#[derive(Parser)]
#[grammar = "meta.pest"]
struct MetaParser;

impl MetaParser {
    fn parse_main(content: &str) -> Result<Meta, String> {
        match Self::parse(Rule::main, content) {
            Ok(mut pairs) => {
                let pair = pairs.next().unwrap().into_inner().next().unwrap();
                match pair.as_rule() {
                    Rule::meta => Ok(Self::parse_meta(pair)),
                    rule => unreachable!("{:?}", rule),
                }
            }
            Err(error) => Err(format!("{}", error)),
        }
    }

    fn parse_meta(pair: Pair<Rule>) -> Meta {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::identifier => Meta::Identifier(Self::parse_identifier(pair)),
            Rule::value => Meta::Value(Self::parse_value(pair)),
            Rule::array => Meta::Array(Self::parse_array(pair)),
            Rule::map => Meta::Map(Self::parse_map(pair)),
            rule => unreachable!("{:?}", rule),
        }
    }

    fn parse_identifier(pair: Pair<Rule>) -> String {
        pair.as_str().to_owned()
    }

    fn parse_value(pair: Pair<Rule>) -> MetaValue {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::literal_bool => MetaValue::Bool(pair.as_str().parse::<bool>().unwrap()),
            Rule::literal_integer => MetaValue::Integer(pair.as_str().parse::<i64>().unwrap()),
            Rule::literal_float => MetaValue::Float(pair.as_str().parse::<f64>().unwrap()),
            Rule::literal_string => {
                MetaValue::String(pair.into_inner().next().unwrap().as_str().to_owned())
            }
            rule => unreachable!("{:?}", rule),
        }
    }

    fn parse_array(pair: Pair<Rule>) -> Vec<Meta> {
        pair.into_inner()
            .map(|pair| Self::parse_meta(pair))
            .collect()
    }

    fn parse_map(pair: Pair<Rule>) -> HashMap<String, Meta> {
        pair.into_inner()
            .map(|pair| {
                let mut pairs = pair.into_inner();
                (
                    Self::parse_identifier(pairs.next().unwrap()),
                    Self::parse_meta(pairs.next().unwrap()),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetaValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

impl MetaValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match self {
            Self::String(value) => Some(value.to_owned()),
            _ => None,
        }
    }
}

impl std::fmt::Display for MetaValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(value) => value.fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Float(value) => value.fmt(f),
            Self::String(value) => f.write_fmt(format_args!("{:?}", value)),
        }
    }
}

impl From<bool> for MetaValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for MetaValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for MetaValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<&str> for MetaValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Meta {
    Identifier(String),
    Value(MetaValue),
    Array(Vec<Meta>),
    Map(HashMap<String, Meta>),
}

impl Meta {
    pub fn parse(content: &str) -> Result<Self, String> {
        MetaParser::parse_main(content)
    }

    pub fn as_identifier(&self) -> Option<&str> {
        match self {
            Self::Identifier(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn as_value(&self) -> Option<&MetaValue> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Meta>> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&HashMap<String, Meta>> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }
}

impl std::fmt::Display for Meta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identifier(value) => value.fmt(f),
            Self::Value(value) => value.fmt(f),
            Self::Array(value) => {
                f.write_char('[')?;
                for (index, value) in value.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    value.fmt(f)?;
                }
                f.write_char(']')
            }
            Self::Map(value) => {
                f.write_char('{')?;
                for (index, (key, value)) in value.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    key.fmt(f)?;
                    f.write_str(": ")?;
                    value.fmt(f)?;
                }
                f.write_char('}')
            }
        }
    }
}

#[macro_export]
macro_rules! meta {
    (@item { $( $key:ident : $item:tt ),* }) => {{
        #[allow(unused_mut)]
        let mut result = std::collections::HashMap::default();
        $(
            result.insert(
                stringify!($key).to_owned(),
                $crate::meta!(@item $item),
            );
        )*
        $crate::meta::Meta::Map(result)
    }};
    (@item [ $( $item:tt ),* ]) => {
        $crate::meta::Meta::Array(vec![ $( $crate::meta!(@item $item) ),* ])
    };
    (@item $value:literal) => {
        $crate::meta::Meta::Value($crate::meta::MetaValue::from($value))
    };
    (@item $value:ident) => {
        $crate::meta::Meta::Identifier(stringify!($value).to_owned())
    };
    ($tree:tt) => {
        $crate::meta!(@item $tree)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser() {
        println!("{}", MetaParser::parse_main("foo").unwrap());
        println!("{}", MetaParser::parse_main("true").unwrap());
        println!("{}", MetaParser::parse_main("42").unwrap());
        println!("{}", MetaParser::parse_main("4.2").unwrap());
        println!("{}", MetaParser::parse_main("'foo'").unwrap());
        println!(
            "{}",
            MetaParser::parse_main("[true, 42, 4.2, 'foo']").unwrap()
        );
        println!(
            "{}",
            MetaParser::parse_main("{bool: true, integer: 42, float: 4.2, string: 'foo'}").unwrap()
        );
    }

    #[test]
    fn test_meta() {
        let meta = crate::meta!(foo);
        assert!(matches!(meta, Meta::Identifier(_)));
        assert_eq!(meta.as_identifier().unwrap(), "foo");
        let meta = crate::meta!(true);
        assert!(matches!(meta, Meta::Value(MetaValue::Bool(_))));
        assert_eq!(meta.as_value().unwrap().as_bool().unwrap(), true);
        let meta = crate::meta!(42);
        assert!(matches!(meta, Meta::Value(MetaValue::Integer(_))));
        assert_eq!(meta.as_value().unwrap().as_integer().unwrap(), 42);
        let meta = crate::meta!(4.2);
        assert!(matches!(meta, Meta::Value(MetaValue::Float(_))));
        assert_eq!(meta.as_value().unwrap().as_float().unwrap(), 4.2);
        let meta = crate::meta!("foo");
        assert!(matches!(meta, Meta::Value(MetaValue::String(_))));
        assert_eq!(meta.as_value().unwrap().as_str().unwrap(), "foo");
        let meta = crate::meta!([]);
        assert!(matches!(meta, Meta::Array(_)));
        let meta = crate::meta!([true, 42, 4.2, "foo"]);
        assert_eq!(
            meta.as_array().unwrap()[0]
                .as_value()
                .unwrap()
                .as_bool()
                .unwrap(),
            true
        );
        assert_eq!(
            meta.as_array().unwrap()[1]
                .as_value()
                .unwrap()
                .as_integer()
                .unwrap(),
            42
        );
        assert_eq!(
            meta.as_array().unwrap()[2]
                .as_value()
                .unwrap()
                .as_float()
                .unwrap(),
            4.2
        );
        assert_eq!(
            meta.as_array().unwrap()[3]
                .as_value()
                .unwrap()
                .as_str()
                .unwrap(),
            "foo"
        );
        let meta = crate::meta!({});
        assert!(matches!(meta, Meta::Map(_)));
        let meta = crate::meta!({bool: true, integer: 42, float: 4.2, string: "foo"});
        assert_eq!(
            meta.as_map().unwrap()["bool"]
                .as_value()
                .unwrap()
                .as_bool()
                .unwrap(),
            true
        );
        assert_eq!(
            meta.as_map().unwrap()["integer"]
                .as_value()
                .unwrap()
                .as_integer()
                .unwrap(),
            42
        );
        assert_eq!(
            meta.as_map().unwrap()["float"]
                .as_value()
                .unwrap()
                .as_float()
                .unwrap(),
            4.2
        );
        assert_eq!(
            meta.as_map().unwrap()["string"]
                .as_value()
                .unwrap()
                .as_str()
                .unwrap(),
            "foo"
        );
    }
}

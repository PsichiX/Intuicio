use pest::{Parser, iterators::Pair};
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
            Rule::named => {
                let (id, meta) = Self::parse_named(pair);
                Meta::Named(id, Box::new(meta))
            }
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
        pair.into_inner().map(Self::parse_meta).collect()
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

    fn parse_named(pair: Pair<Rule>) -> (String, Meta) {
        let mut pairs = pair.into_inner();
        (
            Self::parse_identifier(pairs.next().unwrap()),
            Self::parse_meta(pairs.next().unwrap()),
        )
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
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
    Named(String, Box<Meta>),
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

    pub fn as_named(&self) -> Option<(&str, &Meta)> {
        match self {
            Self::Named(name, value) => Some((name.as_str(), value)),
            _ => None,
        }
    }

    pub fn has_id(&self, name: &str) -> bool {
        match self {
            Self::Identifier(value) => value == name,
            Self::Value(value) => value.as_str() == Some(name),
            Self::Array(values) => values.iter().any(|meta| meta.has_id(name)),
            Self::Map(values) => values.iter().any(|(key, _)| key == name),
            Self::Named(key, _) => key == name,
        }
    }

    pub fn extract_by_id(&self, name: &str) -> Option<MetaExtract> {
        match self {
            Self::Identifier(value) => {
                if value == name {
                    Some(MetaExtract::Identifier(value.as_str()))
                } else {
                    None
                }
            }
            Self::Value(value) => {
                if value.as_str() == Some(name) {
                    Some(MetaExtract::Value(value))
                } else {
                    None
                }
            }
            Self::Array(values) => values
                .iter()
                .filter_map(|meta| meta.extract_by_id(name))
                .next(),
            Self::Map(values) => values
                .iter()
                .filter_map(|(key, value)| {
                    if key == name {
                        Some(MetaExtract::Meta(value))
                    } else {
                        None
                    }
                })
                .next(),
            Self::Named(key, value) => {
                if key == name {
                    Some(MetaExtract::Meta(value))
                } else {
                    None
                }
            }
        }
    }

    pub fn items_iter(&self) -> MetaExtractIter {
        match self {
            Self::Identifier(name) => {
                MetaExtractIter::new(std::iter::once(MetaExtract::Identifier(name.as_str())))
            }
            Self::Value(value) => MetaExtractIter::new(std::iter::once(MetaExtract::Value(value))),
            Self::Array(values) => MetaExtractIter::new(values.iter().map(MetaExtract::Meta)),
            Self::Map(values) => MetaExtractIter::new(values.values().map(MetaExtract::Meta)),
            Self::Named(_, value) => {
                MetaExtractIter::new(std::iter::once(MetaExtract::Meta(value)))
            }
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
            Self::Named(name, value) => {
                f.write_str(name)?;
                f.write_str(" = ")?;
                value.fmt(f)
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MetaExtract<'a> {
    Undefined,
    Identifier(&'a str),
    Meta(&'a Meta),
    Value(&'a MetaValue),
}

impl MetaExtract<'_> {
    pub fn is_undefined(&self) -> bool {
        matches!(self, Self::Undefined)
    }

    pub fn as_identifier(&self) -> Option<&str> {
        match self {
            Self::Identifier(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_value(&self) -> Option<&MetaValue> {
        match self {
            Self::Value(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_meta(&self) -> Option<&Meta> {
        match self {
            Self::Meta(value) => Some(*value),
            _ => None,
        }
    }
}

pub struct MetaExtractIter<'a>(Box<dyn Iterator<Item = MetaExtract<'a>> + 'a>);

impl<'a> MetaExtractIter<'a> {
    fn new(iter: impl Iterator<Item = MetaExtract<'a>> + 'a) -> Self {
        Self(Box::new(iter))
    }
}

impl<'a> Iterator for MetaExtractIter<'a> {
    type Item = MetaExtract<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
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
    (@item ( $name:ident = $item:tt )) => {
        $crate::meta::Meta::Named(
            stringify!($name).to_owned(),
            Box::new($crate::meta!(@item $item))
        )
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
        println!("{}", Meta::parse("foo").unwrap());
        println!("{}", Meta::parse("true").unwrap());
        println!("{}", Meta::parse("42").unwrap());
        println!("{}", Meta::parse("4.2").unwrap());
        println!("{}", Meta::parse("'foo'").unwrap());
        println!("{}", Meta::parse("foo = true").unwrap());
        println!(
            "{}",
            Meta::parse("[true, 42, 4.2, 'foo', foo = true]").unwrap()
        );
        println!(
            "{}",
            Meta::parse("{bool: true, integer: 42, float: 4.2, string: 'foo', named: foo = true}")
                .unwrap()
        );
    }

    #[test]
    fn test_macro() {
        let meta = crate::meta!(foo);
        assert!(matches!(meta, Meta::Identifier(_)));
        assert_eq!(meta.as_identifier().unwrap(), "foo");

        let meta = crate::meta!(true);
        assert!(matches!(meta, Meta::Value(MetaValue::Bool(_))));
        assert!(meta.as_value().unwrap().as_bool().unwrap());

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
        assert!(
            meta.as_array().unwrap()[0]
                .as_value()
                .unwrap()
                .as_bool()
                .unwrap()
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
        assert!(
            meta.as_map().unwrap()["bool"]
                .as_value()
                .unwrap()
                .as_bool()
                .unwrap()
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

        let meta = crate::meta!((foo = true));
        assert!(matches!(meta, Meta::Named(_, _)));
        assert_eq!(meta.as_named().unwrap().0, "foo");
        assert!(
            meta.as_named()
                .unwrap()
                .1
                .as_value()
                .unwrap()
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn test_meta_extract() {
        let meta = crate::meta!(foo);
        assert!(meta.has_id("foo"));
        assert_eq!(
            meta.extract_by_id("foo").unwrap(),
            MetaExtract::Identifier("foo")
        );
        assert_eq!(
            meta.items_iter().collect::<Vec<_>>(),
            vec![MetaExtract::Identifier("foo")]
        );

        let meta = crate::meta!("foo");
        assert!(meta.has_id("foo"));
        assert_eq!(
            meta.extract_by_id("foo").unwrap(),
            MetaExtract::Value(&MetaValue::String("foo".to_owned()))
        );
        assert_eq!(
            meta.items_iter().collect::<Vec<_>>(),
            vec![MetaExtract::Value(&MetaValue::String("foo".to_owned()))]
        );

        let meta = crate::meta!([true, 42, 4.2, "foo"]);
        assert!(meta.has_id("foo"));
        assert_eq!(
            meta.extract_by_id("foo").unwrap(),
            MetaExtract::Value(&MetaValue::String("foo".to_owned()))
        );
        assert_eq!(
            meta.items_iter().collect::<Vec<_>>(),
            vec![
                MetaExtract::Meta(&Meta::Value(MetaValue::Bool(true))),
                MetaExtract::Meta(&Meta::Value(MetaValue::Integer(42))),
                MetaExtract::Meta(&Meta::Value(MetaValue::Float(4.2))),
                MetaExtract::Meta(&Meta::Value(MetaValue::String("foo".to_owned()))),
            ]
        );

        let meta = crate::meta!({bool: true, integer: 42, float: 4.2, string: "foo"});
        assert!(meta.has_id("bool"));
        assert!(meta.has_id("integer"));
        assert!(meta.has_id("float"));
        assert!(meta.has_id("string"));
        assert_eq!(
            meta.extract_by_id("bool").unwrap(),
            MetaExtract::Meta(&Meta::Value(MetaValue::Bool(true)))
        );
        assert_eq!(
            meta.extract_by_id("integer").unwrap(),
            MetaExtract::Meta(&Meta::Value(MetaValue::Integer(42)))
        );
        assert_eq!(
            meta.extract_by_id("float").unwrap(),
            MetaExtract::Meta(&Meta::Value(MetaValue::Float(4.2)))
        );
        assert_eq!(
            meta.extract_by_id("string").unwrap(),
            MetaExtract::Meta(&Meta::Value(MetaValue::String("foo".to_owned())))
        );
        let mut result = meta.items_iter().collect::<Vec<_>>();
        result.sort_by(|a, b| {
            let a = match a {
                MetaExtract::Meta(Meta::Value(MetaValue::Bool(_))) => 0,
                MetaExtract::Meta(Meta::Value(MetaValue::Integer(_))) => 1,
                MetaExtract::Meta(Meta::Value(MetaValue::Float(_))) => 2,
                MetaExtract::Meta(Meta::Value(MetaValue::String(_))) => 3,
                _ => 4,
            };
            let b = match b {
                MetaExtract::Meta(Meta::Value(MetaValue::Bool(_))) => 0,
                MetaExtract::Meta(Meta::Value(MetaValue::Integer(_))) => 1,
                MetaExtract::Meta(Meta::Value(MetaValue::Float(_))) => 2,
                MetaExtract::Meta(Meta::Value(MetaValue::String(_))) => 3,
                _ => 4,
            };
            a.cmp(&b)
        });
        assert_eq!(
            result,
            vec![
                MetaExtract::Meta(&Meta::Value(MetaValue::Bool(true))),
                MetaExtract::Meta(&Meta::Value(MetaValue::Integer(42))),
                MetaExtract::Meta(&Meta::Value(MetaValue::Float(4.2))),
                MetaExtract::Meta(&Meta::Value(MetaValue::String("foo".to_owned()))),
            ]
        );

        let meta = crate::meta!((foo = true));
        assert!(meta.has_id("foo"));
        assert_eq!(
            meta.extract_by_id("foo").unwrap(),
            MetaExtract::Meta(&Meta::Value(MetaValue::Bool(true)))
        );
        assert_eq!(
            meta.items_iter().collect::<Vec<_>>(),
            vec![MetaExtract::Meta(&Meta::Value(MetaValue::Bool(true)))]
        );
    }
}

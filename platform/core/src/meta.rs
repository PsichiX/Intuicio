use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Write};

pub enum ParseMetaError {}

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
    use crate::meta::{Meta, MetaValue};

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

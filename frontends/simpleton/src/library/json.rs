use crate::{Array, Boolean, Integer, Map, Real, Reference, Text};
use intuicio_core::registry::Registry;
use intuicio_derive::intuicio_function;
use serde_json::{Number, Value};

fn to_value(value: &Reference) -> Value {
    if let Some(value) = value.read::<Boolean>() {
        Value::Bool(*value)
    } else if let Some(value) = value.read::<Integer>() {
        Value::Number((*value).into())
    } else if let Some(value) = value.read::<Real>() {
        Value::Number(Number::from_f64(*value).unwrap())
    } else if let Some(value) = value.read::<Text>() {
        Value::String(value.to_owned())
    } else if let Some(value) = value.read::<Array>() {
        Value::Array(value.iter().map(to_value).collect())
    } else if let Some(value) = value.read::<Map>() {
        Value::Object(
            value
                .iter()
                .map(|(key, value)| (key.to_owned(), to_value(value)))
                .collect(),
        )
    } else {
        Value::Null
    }
}

fn from_value(value: Value, registry: &Registry) -> Reference {
    match value {
        Value::Null => Reference::null(),
        Value::Bool(value) => Reference::new_boolean(value, registry),
        Value::Number(value) => {
            if let Some(value) = value.as_f64() {
                Reference::new_real(value, registry)
            } else if let Some(value) = value.as_u64() {
                Reference::new_integer(value as Integer, registry)
            } else if let Some(value) = value.as_i64() {
                Reference::new_integer(value, registry)
            } else {
                Reference::null()
            }
        }
        Value::String(value) => Reference::new_text(value, registry),
        Value::Array(value) => Reference::new_array(
            value
                .into_iter()
                .map(|value| from_value(value, registry))
                .collect(),
            registry,
        ),
        Value::Object(value) => Reference::new_map(
            value
                .into_iter()
                .map(|(key, value)| (key, from_value(value, registry)))
                .collect(),
            registry,
        ),
    }
}

#[intuicio_function(module_name = "json", use_registry)]
pub fn serialize(registry: &Registry, value: Reference) -> Reference {
    Reference::new_text(serde_json::to_string(&to_value(&value)).unwrap(), registry)
}

#[intuicio_function(module_name = "json", use_registry)]
pub fn serialize_pretty(registry: &Registry, value: Reference) -> Reference {
    Reference::new_text(
        serde_json::to_string_pretty(&to_value(&value)).unwrap(),
        registry,
    )
}

#[intuicio_function(module_name = "json", use_registry)]
pub fn deserialize(registry: &Registry, text: Reference) -> Reference {
    from_value(
        serde_json::from_str::<Value>(text.read::<Text>().unwrap().as_str()).unwrap(),
        registry,
    )
}

pub fn install(registry: &mut Registry) {
    registry.add_function(serialize::define_function(registry));
    registry.add_function(serialize_pretty::define_function(registry));
    registry.add_function(deserialize::define_function(registry));
}

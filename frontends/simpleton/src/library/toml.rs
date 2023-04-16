use crate::{Array, Boolean, Integer, Map, Real, Reference, Text};
use intuicio_core::registry::Registry;
use intuicio_derive::intuicio_function;
use toml::Value;

fn to_value(value: &Reference) -> Value {
    if let Some(value) = value.read::<Boolean>() {
        Value::Boolean(*value)
    } else if let Some(value) = value.read::<Integer>() {
        Value::Integer(*value)
    } else if let Some(value) = value.read::<Real>() {
        Value::Float(*value)
    } else if let Some(value) = value.read::<Text>() {
        Value::String(value.to_owned())
    } else if let Some(value) = value.read::<Array>() {
        Value::Array(value.iter().map(to_value).collect())
    } else if let Some(value) = value.read::<Map>() {
        Value::Table(
            value
                .iter()
                .map(|(key, value)| (key.to_owned(), to_value(value)))
                .collect(),
        )
    } else {
        panic!("Cannot serialize null!")
    }
}

fn from_value(value: Value, registry: &Registry) -> Reference {
    match value {
        Value::String(value) => Reference::new_text(value, registry),
        Value::Integer(value) => Reference::new_integer(value as Integer, registry),
        Value::Float(value) => Reference::new_real(value, registry),
        Value::Boolean(value) => Reference::new_boolean(value, registry),
        Value::Datetime(_) => {
            panic!("Cannot deserialize date time!");
        }
        Value::Array(value) => Reference::new_array(
            value
                .into_iter()
                .map(|value| from_value(value, registry))
                .collect(),
            registry,
        ),
        Value::Table(value) => Reference::new_map(
            value
                .into_iter()
                .map(|(key, value)| (key, from_value(value, registry)))
                .collect(),
            registry,
        ),
    }
}

#[intuicio_function(module_name = "toml", use_registry)]
pub fn serialize(registry: &Registry, value: Reference) -> Reference {
    Reference::new_text(toml::to_string(&to_value(&value)).unwrap(), registry)
}

#[intuicio_function(module_name = "toml", use_registry)]
pub fn serialize_pretty(registry: &Registry, value: Reference) -> Reference {
    Reference::new_text(toml::to_string_pretty(&to_value(&value)).unwrap(), registry)
}

#[intuicio_function(module_name = "toml", use_registry)]
pub fn deserialize(registry: &Registry, text: Reference) -> Reference {
    from_value(
        toml::from_str::<Value>(text.read::<Text>().unwrap().as_str()).unwrap(),
        registry,
    )
}

pub fn install(registry: &mut Registry) {
    registry.add_function(serialize::define_function(registry));
    registry.add_function(serialize_pretty::define_function(registry));
    registry.add_function(deserialize::define_function(registry));
}

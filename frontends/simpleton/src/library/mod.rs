pub mod array;
pub mod bytes;
pub mod closure;
#[cfg(feature = "console")]
pub mod console;
pub mod debug;
pub mod event;
#[cfg(feature = "ffi")]
pub mod ffi;
#[cfg(feature = "fs")]
pub mod fs;
pub mod iter;
#[cfg(feature = "jobs")]
pub mod jobs;
pub mod json;
pub mod map;
pub mod math;
#[cfg(feature = "net")]
pub mod net;
#[cfg(feature = "process")]
pub mod process;
pub mod promise;
pub mod reflect;
pub mod text;
pub mod toml;

use crate::{Map, Reference};
use intuicio_core::{object::Object, registry::Registry, types::TypeQuery};

pub struct ObjectBuilder {
    name: String,
    module_name: String,
    fields: Map,
}

impl ObjectBuilder {
    pub fn new(name: impl ToString, module_name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            module_name: module_name.to_string(),
            fields: Map::new(),
        }
    }

    pub fn field(mut self, name: impl ToString, value: Reference) -> Self {
        self.fields.insert(name.to_string(), value);
        self
    }

    pub fn build(self, registry: &Registry) -> Object {
        let type_ = registry
            .find_type(TypeQuery {
                name: Some(self.name.into()),
                module_name: Some(self.module_name.into()),
                ..Default::default()
            })
            .unwrap();
        let mut result = Object::new(type_);
        for (key, value) in self.fields {
            *result.write_field::<Reference>(&key).unwrap() = value;
        }
        result
    }
}

pub fn install(registry: &mut Registry) {
    reflect::install(registry);
    math::install(registry);
    text::install(registry);
    array::install(registry);
    map::install(registry);
    #[cfg(feature = "console")]
    console::install(registry);
    #[cfg(feature = "fs")]
    fs::install(registry);
    #[cfg(feature = "ffi")]
    ffi::install(registry);
    #[cfg(feature = "process")]
    process::install(registry);
    #[cfg(feature = "net")]
    net::install(registry);
    bytes::install(registry);
    json::install(registry);
    toml::install(registry);
    debug::install(registry);
    closure::install(registry);
    iter::install(registry);
    promise::install(registry);
    event::install(registry);
    #[cfg(feature = "jobs")]
    jobs::install(registry);
}

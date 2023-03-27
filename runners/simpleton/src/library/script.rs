use crate::Config;
use intuicio_core::registry::Registry;
use intuicio_derive::intuicio_function;
use intuicio_frontend_simpleton::{Array, Integer, Map, Reference, Text};

#[intuicio_function(module_name = "script", use_registry)]
pub fn run(registry: &Registry, entry: Reference, config: Reference, args: Reference) -> Reference {
    let entry = entry.read::<Text>().unwrap();
    let args = if let Some(args) = args.read::<Array>() {
        args.iter()
            .map(|arg| arg.read::<Text>().unwrap().to_owned())
            .collect()
    } else {
        vec![]
    };
    let config = if let Some(config) = config.read::<Map>() {
        Config {
            name: config
                .get("name")
                .map(|value| value.read::<Text>().unwrap().to_owned()),
            module_name: config
                .get("module_name")
                .map(|value| value.read::<Text>().unwrap().to_owned()),
            stack_capacity: config
                .get("stack_capacity")
                .map(|value| *value.read::<Integer>().unwrap() as usize),
            registers_capacity: config
                .get("registers_capacity")
                .map(|value| *value.read::<Integer>().unwrap() as usize),
            heap_page_capacity: config
                .get("heap_page_capacity")
                .map(|value| *value.read::<Integer>().unwrap() as usize),
            into_code: None,
            into_intuicio: None,
        }
    } else {
        Config::default()
    };
    Reference::new_integer(
        crate::execute(&entry, config, args.into_iter()) as Integer,
        registry,
    )
}

pub fn install(registry: &mut Registry) {
    registry.add_function(run::define_function(registry));
}

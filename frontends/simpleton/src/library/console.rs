use std::io::Write;

use crate::{Reference, Text};
use intuicio_core::registry::Registry;
use intuicio_derive::intuicio_function;

#[intuicio_function(module_name = "console")]
pub fn log(text: Reference) -> Reference {
    print!("{}", text.read::<Text>().unwrap().as_str());
    Reference::null()
}

#[intuicio_function(module_name = "console")]
pub fn log_line(text: Reference) -> Reference {
    println!("{}", text.read::<Text>().unwrap().as_str());
    Reference::null()
}

#[intuicio_function(module_name = "console")]
pub fn error(text: Reference) -> Reference {
    eprint!("{}", text.read::<Text>().unwrap().as_str());
    Reference::null()
}

#[intuicio_function(module_name = "console")]
pub fn error_line(text: Reference) -> Reference {
    eprintln!("{}", text.read::<Text>().unwrap().as_str());
    Reference::null()
}

#[intuicio_function(module_name = "console", use_registry)]
pub fn read_line(registry: &Registry) -> Reference {
    let mut result = String::new();
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut result).unwrap();
    Reference::new_text(result, registry)
}

pub fn install(registry: &mut Registry) {
    registry.add_function(log::define_function(registry));
    registry.add_function(log_line::define_function(registry));
    registry.add_function(error::define_function(registry));
    registry.add_function(error_line::define_function(registry));
    registry.add_function(read_line::define_function(registry));
}

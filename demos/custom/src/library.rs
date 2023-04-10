use intuicio_core::prelude::*;
use intuicio_derive::*;

#[intuicio_function(module_name = "lib")]
fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[intuicio_function(module_name = "lib")]
fn sub(a: i32, b: i32) -> i32 {
    a - b
}

#[intuicio_function(module_name = "lib")]
fn mul(a: i32, b: i32) -> i32 {
    a * b
}

#[intuicio_function(module_name = "lib")]
fn div(a: i32, b: i32) -> i32 {
    a / b
}

pub fn install(registry: &mut Registry) {
    registry.add_function(add::define_function(registry));
    registry.add_function(sub::define_function(registry));
    registry.add_function(mul::define_function(registry));
    registry.add_function(div::define_function(registry));
}

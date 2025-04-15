use intuicio_core::{IntuicioVersion, registry::Registry};
use intuicio_derive::*;
use intuicio_frontend_simpleton::{Integer, Reference};

#[intuicio_function(module_name = "plugin", use_registry)]
pub fn fib(registry: &Registry, n: Reference) -> Reference {
    Reference::new_integer(fib_inner(*n.read::<Integer>().unwrap()), registry)
}

fn fib_inner(n: Integer) -> Integer {
    match n {
        0 => 0,
        1 => 1,
        n => fib_inner(n - 1) + fib_inner(n - 2),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn version() -> IntuicioVersion {
    intuicio_core::core_version()
}

#[unsafe(no_mangle)]
pub extern "C" fn install(registry: &mut Registry) {
    registry.add_function(fib::define_function(registry));
}

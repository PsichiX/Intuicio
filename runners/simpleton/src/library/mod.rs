pub mod script;

use intuicio_core::registry::Registry;

pub fn install(registry: &mut Registry) {
    script::install(registry);
}

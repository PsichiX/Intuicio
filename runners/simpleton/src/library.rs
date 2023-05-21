use crate::ENTRY_DIR;
use intuicio_core::prelude::*;
use intuicio_derive::intuicio_function;
use intuicio_frontend_simpleton::prelude::*;

#[intuicio_function(module_name = "simpleton", use_context, use_registry)]
pub fn get_entry_dir(context: &mut Context, registry: &Registry) -> Reference {
    context
        .custom::<String>(ENTRY_DIR)
        .map(|value| Reference::new_text(value.to_owned(), registry))
        .unwrap_or_default()
}

pub fn install(registry: &mut Registry) {
    registry.add_function(get_entry_dir::define_function(registry));
}

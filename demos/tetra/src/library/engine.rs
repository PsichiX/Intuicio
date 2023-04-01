use intuicio_core::prelude::*;
use intuicio_data::prelude::*;
use intuicio_derive::*;
use tetra::Context as TetraContext;

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Engine", module_name = "engine")]
pub struct Engine {
    #[intuicio(ignore)]
    pub(crate) assets: String,
    #[intuicio(ignore)]
    pub(crate) tetra_context: Option<ManagedRefMut<TetraContext>>,
}

#[intuicio_methods(module_name = "engine")]
impl Engine {
    pub fn new(assets: &str, tetra_context: ManagedRefMut<TetraContext>) -> Self {
        Self {
            assets: assets.to_owned(),
            tetra_context: Some(tetra_context),
        }
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Engine::define_struct(registry));
}

pub mod color;
pub mod image;
pub mod image_pipeline;
pub mod vec2;

use intuicio_core::registry::Registry;

pub fn install(registry: &mut Registry) {
    color::install(registry);
    vec2::install(registry);
    image::install(registry);
    image_pipeline::install(registry);
}

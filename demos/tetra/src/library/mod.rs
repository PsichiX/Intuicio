use intuicio_core::registry::Registry;

pub mod color;
pub mod font;
pub mod image;
pub mod renderer;
pub mod vec2;

pub fn install(registry: &mut Registry) {
    vec2::install(registry);
    color::install(registry);
    image::install(registry);
    font::install(registry);
    renderer::install(registry);
}

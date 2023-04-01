use intuicio_core::registry::Registry;

pub mod color;
pub mod engine;
pub mod font;
pub mod gui;
pub mod image;
pub mod input;
pub mod rect;
pub mod vec2;

pub fn install(registry: &mut Registry) {
    vec2::install(registry);
    rect::install(registry);
    color::install(registry);
    image::install(registry);
    font::install(registry);
    engine::install(registry);
    input::install(registry);
    gui::install(registry);
}

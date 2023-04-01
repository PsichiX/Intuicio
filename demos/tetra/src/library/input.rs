use super::{engine::Engine, vec2::Vec2};
use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::*;
use tetra::input::{
    get_mouse_position, get_text_input, is_mouse_button_pressed, is_mouse_button_released,
    MouseButton,
};

#[intuicio_function(module_name = "input", use_registry)]
pub fn is_action_pressed(registry: &Registry, engine: Reference) -> Reference {
    let engine = engine.read::<Engine>().unwrap();
    let ctx = engine.tetra_context.as_ref().unwrap();
    let ctx = ctx.read().unwrap();
    Reference::new_boolean(is_mouse_button_pressed(&ctx, MouseButton::Left), registry)
}

#[intuicio_function(module_name = "input", use_registry)]
pub fn is_action_released(registry: &Registry, engine: Reference) -> Reference {
    let engine = engine.read::<Engine>().unwrap();
    let ctx = engine.tetra_context.as_ref().unwrap();
    let ctx = ctx.read().unwrap();
    Reference::new_boolean(is_mouse_button_released(&ctx, MouseButton::Left), registry)
}

#[intuicio_function(module_name = "input", use_registry)]
pub fn is_context_pressed(registry: &Registry, engine: Reference) -> Reference {
    let engine = engine.read::<Engine>().unwrap();
    let ctx = engine.tetra_context.as_ref().unwrap();
    let ctx = ctx.read().unwrap();
    Reference::new_boolean(is_mouse_button_pressed(&ctx, MouseButton::Right), registry)
}

#[intuicio_function(module_name = "input", use_registry)]
pub fn is_context_released(registry: &Registry, engine: Reference) -> Reference {
    let engine = engine.read::<Engine>().unwrap();
    let ctx = engine.tetra_context.as_ref().unwrap();
    let ctx = ctx.read().unwrap();
    Reference::new_boolean(is_mouse_button_released(&ctx, MouseButton::Right), registry)
}

#[intuicio_function(module_name = "input", use_registry)]
pub fn pointer_position(registry: &Registry, engine: Reference) -> Reference {
    let engine = engine.read::<Engine>().unwrap();
    let ctx = engine.tetra_context.as_ref().unwrap();
    let ctx = ctx.read().unwrap();
    Reference::new(
        Vec2::from_tetra(get_mouse_position(&ctx), registry),
        registry,
    )
}

#[intuicio_function(module_name = "input", use_registry)]
pub fn text(registry: &Registry, engine: Reference) -> Reference {
    let engine = engine.read::<Engine>().unwrap();
    let ctx = engine.tetra_context.as_ref().unwrap();
    let ctx = ctx.read().unwrap();
    Reference::new_text(get_text_input(&ctx).unwrap_or("").to_owned(), registry)
}

pub fn install(registry: &mut Registry) {
    registry.add_function(is_action_pressed::define_function(registry));
    registry.add_function(is_action_released::define_function(registry));
    registry.add_function(is_context_pressed::define_function(registry));
    registry.add_function(is_context_released::define_function(registry));
    registry.add_function(pointer_position::define_function(registry));
    registry.add_function(text::define_function(registry));
}

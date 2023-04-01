use super::{color::Color, engine::Engine, vec2::Vec2};
use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::*;
use tetra::{
    graphics::{
        text::{Font as TetraFont, Text as TetraText},
        DrawParams,
    },
    window,
};

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Font", module_name = "font")]
pub struct Font {
    #[intuicio(ignore)]
    pub(crate) font: Option<TetraFont>,
}

#[intuicio_methods(module_name = "font")]
impl Font {
    #[intuicio_method(use_registry)]
    pub fn load(
        registry: &Registry,
        mut engine: Reference,
        path: Reference,
        size: Reference,
    ) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let path = path.read::<Text>().unwrap();
        let path = format!("{}/{}", engine.assets, path.as_str());
        let size = *size.read::<Real>().unwrap() as f32;
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        let result = Self {
            font: Some(TetraFont::vector(&mut ctx, path.as_str(), size).unwrap()),
        };
        Reference::new(result, registry)
    }

    #[intuicio_method()]
    pub fn draw(
        mut engine: Reference,
        font: Reference,
        content: Reference,
        position: Reference,
        color: Reference,
    ) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let font = font.read::<Font>().unwrap();
        let content = content.read::<Text>().unwrap();
        let position = position.read::<Vec2>().unwrap().into_tetra();
        let color = color.read::<Color>().unwrap().into_tetra();
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        TetraText::new(content.as_str(), font.font.as_ref().unwrap().clone()).draw(
            &mut ctx,
            DrawParams {
                position,
                color,
                ..Default::default()
            },
        );
        Reference::null()
    }

    #[intuicio_method()]
    #[allow(clippy::too_many_arguments)]
    pub fn draw_advanced(
        mut engine: Reference,
        font: Reference,
        content: Reference,
        position: Reference,
        scale: Reference,
        origin: Reference,
        rotation: Reference,
        color: Reference,
    ) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let font = font.read::<Font>().unwrap();
        let content = content.read::<Text>().unwrap();
        let position = position.read::<Vec2>().unwrap().into_tetra();
        let scale = scale.read::<Vec2>().unwrap().into_tetra();
        let origin = origin.read::<Vec2>().unwrap().into_tetra();
        let rotation = *rotation.read::<Real>().unwrap() as f32;
        let color = color.read::<Color>().unwrap().into_tetra();
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        TetraText::new(content.as_str(), font.font.as_ref().unwrap().clone()).draw(
            &mut ctx,
            DrawParams {
                position,
                scale,
                origin,
                rotation,
                color,
            },
        );
        Reference::null()
    }

    #[intuicio_method()]
    pub fn draw_screen(
        mut engine: Reference,
        font: Reference,
        content: Reference,
        factor: Reference,
        color: Reference,
    ) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let font = font.read::<Font>().unwrap();
        let content = content.read::<Text>().unwrap();
        let factor = factor.read::<Vec2>().unwrap().into_tetra();
        let color = color.read::<Color>().unwrap().into_tetra();
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        let mut text = TetraText::new(content.as_str(), font.font.as_ref().unwrap().clone());
        let bounds = text.get_bounds(&mut ctx).unwrap();
        let screen_width = window::get_width(&ctx) as f32;
        let screen_height = window::get_height(&ctx) as f32;
        let position = (screen_width * factor.x, screen_height * factor.y).into();
        let origin = (bounds.width * factor.x, bounds.height * factor.y).into();
        text.draw(
            &mut ctx,
            DrawParams {
                position,
                origin,
                color,
                ..Default::default()
            },
        );
        Reference::null()
    }

    #[intuicio_method()]
    #[allow(clippy::too_many_arguments)]
    pub fn draw_screen_advanced(
        mut engine: Reference,
        font: Reference,
        content: Reference,
        factor: Reference,
        scale: Reference,
        origin: Reference,
        rotation: Reference,
        color: Reference,
    ) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let font = font.read::<Font>().unwrap();
        let content = content.read::<Text>().unwrap();
        let factor = factor.read::<Vec2>().unwrap().into_tetra();
        let scale = scale.read::<Vec2>().unwrap().into_tetra();
        let origin = origin.read::<Vec2>().unwrap().into_tetra();
        let rotation = *rotation.read::<Real>().unwrap() as f32;
        let color = color.read::<Color>().unwrap().into_tetra();
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        let mut text = TetraText::new(content.as_str(), font.font.as_ref().unwrap().clone());
        let bounds = text.get_bounds(&mut ctx).unwrap();
        let screen_width = window::get_width(&ctx) as f32;
        let screen_height = window::get_height(&ctx) as f32;
        let position = (screen_width * factor.x, screen_height * factor.y).into();
        let origin = (bounds.width * origin.x, bounds.height * origin.y).into();
        text.draw(
            &mut ctx,
            DrawParams {
                position,
                scale,
                origin,
                rotation,
                color,
            },
        );
        Reference::null()
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Font::define_struct(registry));
    registry.add_function(Font::load__define_function(registry));
    registry.add_function(Font::draw__define_function(registry));
    registry.add_function(Font::draw_advanced__define_function(registry));
    registry.add_function(Font::draw_screen__define_function(registry));
    registry.add_function(Font::draw_screen_advanced__define_function(registry));
}

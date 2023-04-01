use super::{color::Color, engine::Engine, vec2::Vec2};
use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::*;
use tetra::{
    graphics::{DrawParams, Texture},
    window,
};

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Image", module_name = "image")]
pub struct Image {
    #[intuicio(ignore)]
    pub(crate) texture: Option<Texture>,
}

#[intuicio_methods(module_name = "image")]
impl Image {
    #[intuicio_method(use_registry)]
    pub fn load(registry: &Registry, mut engine: Reference, path: Reference) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let path = path.read::<Text>().unwrap();
        let path = format!("{}/{}", engine.assets, path.as_str());
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        let result = Self {
            texture: Some(Texture::new(&mut ctx, path.as_str()).unwrap()),
        };
        Reference::new(result, registry)
    }

    #[intuicio_method()]
    pub fn draw(
        mut engine: Reference,
        image: Reference,
        position: Reference,
        color: Reference,
    ) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let image = image.read::<Image>().unwrap();
        let position = position.read::<Vec2>().unwrap().into_tetra();
        let color = color.read::<Color>().unwrap().into_tetra();
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        image.texture.as_ref().unwrap().draw(
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
    pub fn draw_advanced(
        mut engine: Reference,
        image: Reference,
        position: Reference,
        scale: Reference,
        origin: Reference,
        rotation: Reference,
        color: Reference,
    ) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let image = image.read::<Image>().unwrap();
        let position = position.read::<Vec2>().unwrap().into_tetra();
        let scale = scale.read::<Vec2>().unwrap().into_tetra();
        let origin = origin.read::<Vec2>().unwrap().into_tetra();
        let rotation = *rotation.read::<Real>().unwrap() as f32;
        let color = color.read::<Color>().unwrap().into_tetra();
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        image.texture.as_ref().unwrap().draw(
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
        image: Reference,
        factor: Reference,
        color: Reference,
    ) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let image = image.read::<Image>().unwrap();
        let factor = factor.read::<Vec2>().unwrap().into_tetra();
        let color = color.read::<Color>().unwrap().into_tetra();
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        let screen_width = window::get_width(&ctx) as f32;
        let screen_height = window::get_height(&ctx) as f32;
        let texture = image.texture.as_ref().unwrap();
        let image_width = texture.width() as f32;
        let image_height = texture.height() as f32;
        let position = (screen_width * factor.x, screen_height * factor.y).into();
        let origin = (image_width * factor.x, image_height * factor.y).into();
        texture.draw(
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
    pub fn draw_screen_advanced(
        mut engine: Reference,
        image: Reference,
        factor: Reference,
        scale: Reference,
        origin: Reference,
        rotation: Reference,
        color: Reference,
    ) -> Reference {
        let engine = &mut *engine.write::<Engine>().unwrap();
        let image = image.read::<Image>().unwrap();
        let factor = factor.read::<Vec2>().unwrap().into_tetra();
        let scale = scale.read::<Vec2>().unwrap().into_tetra();
        let origin = origin.read::<Vec2>().unwrap().into_tetra();
        let rotation = *rotation.read::<Real>().unwrap() as f32;
        let color = color.read::<Color>().unwrap().into_tetra();
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        let screen_width = window::get_width(&ctx) as f32;
        let screen_height = window::get_height(&ctx) as f32;
        let texture = image.texture.as_ref().unwrap();
        let image_width = texture.width() as f32;
        let image_height = texture.height() as f32;
        let position = (screen_width * factor.x, screen_height * factor.y).into();
        let origin = (image_width * origin.x, image_height * origin.y).into();
        texture.draw(
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
    registry.add_struct(Image::define_struct(registry));
    registry.add_function(Image::load__define_function(registry));
    registry.add_function(Image::draw__define_function(registry));
    registry.add_function(Image::draw_advanced__define_function(registry));
    registry.add_function(Image::draw_screen__define_function(registry));
    registry.add_function(Image::draw_screen_advanced__define_function(registry));
}

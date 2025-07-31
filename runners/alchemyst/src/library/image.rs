use crate::library::color::Color;
use image::{
    imageops::{FilterType, crop_imm, resize},
    *,
};
use intuicio_core::{IntuicioStruct, context::Context, registry::Registry};
use intuicio_derive::*;
use intuicio_frontend_simpleton::{
    Array, Boolean, Integer, Real, Reference, Text, library::closure::Closure,
};

#[derive(IntuicioStruct, Default, Clone)]
#[intuicio(
    name = "ImageProcessingConfig",
    module_name = "image",
    override_send = true
)]
pub struct ImageProcessingConfig {
    pub col: Reference,
    pub row: Reference,
    pub cols: Reference,
    pub rows: Reference,
    pub uv_space: Reference,
}

#[derive(IntuicioStruct, Default, Clone)]
#[intuicio(name = "Image", module_name = "image")]
pub struct Image {
    #[intuicio(ignore)]
    pub(crate) buffer: Rgba32FImage,
}

#[intuicio_methods(module_name = "image")]
impl Image {
    #[allow(clippy::new_ret_no_self)]
    #[intuicio_method(use_registry)]
    pub fn new(
        registry: &Registry,
        width: Reference,
        height: Reference,
        color: Reference,
    ) -> Reference {
        let width = *width.read::<Integer>().unwrap() as u32;
        let height = *height.read::<Integer>().unwrap() as u32;
        let color = color
            .read::<Color>()
            .map(|color| color.to_pixel())
            .unwrap_or_else(|| Rgba([0.0, 0.0, 0.0, 0.0]));
        Reference::new(
            Image {
                buffer: Rgba32FImage::from_pixel(width, height, color),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn clone(registry: &Registry, image: Reference) -> Reference {
        Reference::new(image.read::<Image>().unwrap().clone(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn resize(
        registry: &Registry,
        image: Reference,
        width: Reference,
        height: Reference,
    ) -> Reference {
        let image = image.read::<Image>().unwrap();
        let width = *width.read::<Integer>().unwrap() as u32;
        let height = *height.read::<Integer>().unwrap() as u32;
        Reference::new(
            Image {
                buffer: resize(&image.buffer, width, height, FilterType::CatmullRom),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn crop(
        registry: &Registry,
        image: Reference,
        x: Reference,
        y: Reference,
        width: Reference,
        height: Reference,
    ) -> Reference {
        let image = image.read::<Image>().unwrap();
        let x = *x.read::<Integer>().unwrap() as u32;
        let y = *y.read::<Integer>().unwrap() as u32;
        let width = *width.read::<Integer>().unwrap() as u32;
        let height = *height.read::<Integer>().unwrap() as u32;
        Reference::new(
            Image {
                buffer: crop_imm(&image.buffer, x, y, width, height).to_image(),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn open(registry: &Registry, path: Reference) -> Reference {
        let path = path.read::<Text>().unwrap();
        if let Ok(image) = open(path.as_str()) {
            return Reference::new(
                Image {
                    buffer: image.to_rgba32f(),
                },
                registry,
            );
        }
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn save_hdr(registry: &Registry, image: Reference, path: Reference) -> Reference {
        let image = image.read::<Image>().unwrap();
        let path = path.read::<Text>().unwrap();
        Reference::new_boolean(image.buffer.save(path.as_str()).is_ok(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn save_ldr(registry: &Registry, image: Reference, path: Reference) -> Reference {
        let image = image.read::<Image>().unwrap();
        let path = path.read::<Text>().unwrap();
        Reference::new_boolean(
            DynamicImage::from(image.buffer.clone())
                .to_rgba8()
                .save(path.as_str())
                .is_ok(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn from_array(
        registry: &Registry,
        pixels: Reference,
        cols: Reference,
        rows: Reference,
    ) -> Reference {
        let pixels = pixels.read::<Array>().unwrap();
        let cols = *cols.read::<Integer>().unwrap() as u32;
        let rows = *rows.read::<Integer>().unwrap() as u32;
        let pixels = pixels
            .iter()
            .flat_map(|color| {
                let color = color.read::<Color>().unwrap();
                [
                    *color.r.read::<Real>().unwrap() as f32,
                    *color.g.read::<Real>().unwrap() as f32,
                    *color.b.read::<Real>().unwrap() as f32,
                    *color.a.read::<Real>().unwrap() as f32,
                ]
            })
            .collect();
        Rgba32FImage::from_vec(cols, rows, pixels)
            .map(|buffer| Reference::new(Self { buffer }, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_registry)]
    pub fn to_array(registry: &Registry, image: Reference) -> Reference {
        let image = image.read::<Image>().unwrap();
        Reference::new_array(
            image
                .buffer
                .pixels()
                .map(|pixel| Reference::new(Color::from_pixel(pixel, registry), registry))
                .collect(),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn width(registry: &Registry, image: Reference) -> Reference {
        let image = image.read::<Image>().unwrap();
        Reference::new_integer(image.buffer.width() as Integer, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn height(registry: &Registry, image: Reference) -> Reference {
        let image = image.read::<Image>().unwrap();
        Reference::new_integer(image.buffer.height() as Integer, registry)
    }

    pub(crate) fn sample_inner(
        &self,
        mut u: f32,
        mut v: f32,
        wrap: bool,
        interpolate: bool,
    ) -> Rgba<f32> {
        if wrap {
            u = if u < 0.0 { 1.0 } else { 0.0 } + u.fract();
            v = if v < 0.0 { 1.0 } else { 0.0 } + v.fract();
        } else {
            u = u.clamp(0.0, 1.0);
            v = v.clamp(0.0, 1.0);
        }
        let width = self.buffer.width().saturating_sub(1);
        let height = self.buffer.height().saturating_sub(1);
        let col = u * width as f32;
        let row = v * height as f32;
        if interpolate {
            fn lerp(from: f32, to: f32, factor: f32) -> f32 {
                from + (to - from) * factor
            }
            u = col.fract();
            v = row.fract();
            let col = col.floor() as u32;
            let row = row.floor() as u32;
            let top_left = self
                .buffer
                .get_pixel_checked(col, row)
                .copied()
                .unwrap_or(Rgba([0.0, 0.0, 0.0, 0.0]));
            let top_right = self
                .buffer
                .get_pixel_checked(col + 1, row)
                .copied()
                .unwrap_or(Rgba([0.0, 0.0, 0.0, 0.0]));
            let bottom_right = self
                .buffer
                .get_pixel_checked(col + 1, row + 1)
                .copied()
                .unwrap_or(Rgba([0.0, 0.0, 0.0, 0.0]));
            let bottom_left = self
                .buffer
                .get_pixel_checked(col, row + 1)
                .copied()
                .unwrap_or(Rgba([0.0, 0.0, 0.0, 0.0]));
            let top = top_left.map2(&top_right, |a, b| lerp(a, b, u));
            let bottom = bottom_left.map2(&bottom_right, |a, b| lerp(a, b, u));
            top.map2(&bottom, |a, b| lerp(a, b, v))
        } else {
            let col = col.round() as u32;
            let row = row.round() as u32;
            self.buffer
                .get_pixel_checked(col, row)
                .copied()
                .unwrap_or(Rgba([0.0, 0.0, 0.0, 0.0]))
        }
    }

    #[intuicio_method(use_registry)]
    pub fn sample(
        registry: &Registry,
        image: Reference,
        u: Reference,
        v: Reference,
        interpolate: Reference,
        wrap: Reference,
    ) -> Reference {
        let image = image.read::<Image>().unwrap();
        let u = *u.read::<Real>().unwrap() as f32;
        let v = *v.read::<Real>().unwrap() as f32;
        let wrap = *wrap.read::<Boolean>().unwrap();
        let interpolate = *interpolate.read::<Boolean>().unwrap();
        let result = image.sample_inner(u, v, wrap, interpolate);
        Reference::new(Color::from_pixel(&result, registry), registry)
    }

    pub(crate) fn get_pixel_inner(&self, col: u32, row: u32) -> Rgba<f32> {
        self.buffer
            .get_pixel_checked(col, row)
            .copied()
            .unwrap_or(Rgba([0.0, 0.0, 0.0, 0.0]))
    }

    #[intuicio_method(use_registry)]
    pub fn get_pixel(
        registry: &Registry,
        image: Reference,
        col: Reference,
        row: Reference,
    ) -> Reference {
        let image = image.read::<Image>().unwrap();
        let col = *col.read::<Integer>().unwrap() as u32;
        let row = *row.read::<Integer>().unwrap() as u32;
        let result = image.get_pixel_inner(col, row);
        Reference::new(Color::from_pixel(&result, registry), registry)
    }

    #[intuicio_method()]
    pub fn set_pixel(
        mut image: Reference,
        col: Reference,
        row: Reference,
        color: Reference,
    ) -> Reference {
        let mut image = image.write::<Image>().unwrap();
        let col = *col.read::<Integer>().unwrap() as u32;
        let row = *row.read::<Integer>().unwrap() as u32;
        let color = color.read::<Color>().unwrap();
        image.buffer.put_pixel(col, row, color.to_pixel());
        Reference::null()
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn process(
        context: &mut Context,
        registry: &Registry,
        mut image: Reference,
        config: Reference,
        closure: Reference,
    ) -> Reference {
        let mut image = image.write::<Image>().unwrap();
        let closure = closure.read::<Closure>().unwrap();
        let config = config
            .read::<ImageProcessingConfig>()
            .map(|config| config.clone())
            .unwrap_or_default();
        let col = config
            .col
            .read::<Integer>()
            .map(|value| *value as u32)
            .unwrap_or_default();
        let row = config
            .row
            .read::<Integer>()
            .map(|value| *value as u32)
            .unwrap_or_default();
        let cols = config
            .cols
            .read::<Integer>()
            .map(|value| *value as u32)
            .unwrap_or_else(|| image.buffer.width());
        let rows = config
            .rows
            .read::<Integer>()
            .map(|value| *value as u32)
            .unwrap_or_else(|| image.buffer.height());
        let uv_space = config
            .uv_space
            .read::<Boolean>()
            .map(|value| *value)
            .unwrap_or_default();
        let width = (image.buffer.width().saturating_sub(1)) as Real;
        let height = (image.buffer.height().saturating_sub(1)) as Real;
        for (x, y, pixel) in image.buffer.enumerate_pixels_mut() {
            if x < col || y < row || x >= col + cols || y >= row + rows {
                continue;
            }
            let args = if uv_space {
                [
                    Reference::new_real(x as Real / width, registry),
                    Reference::new_real(y as Real / height, registry),
                    Reference::new(Color::from_pixel(pixel, registry), registry),
                ]
            } else {
                [
                    Reference::new_integer(x as Integer, registry),
                    Reference::new_integer(y as Integer, registry),
                    Reference::new(Color::from_pixel(pixel, registry), registry),
                ]
            };
            if let Some(color) = closure.invoke(context, registry, &args).read::<Color>() {
                *pixel = color.to_pixel();
            }
        }
        Reference::null()
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_type(ImageProcessingConfig::define_struct(registry));
    registry.add_type(Image::define_struct(registry));
    registry.add_function(Image::new__define_function(registry));
    registry.add_function(Image::clone__define_function(registry));
    registry.add_function(Image::resize__define_function(registry));
    registry.add_function(Image::crop__define_function(registry));
    registry.add_function(Image::open__define_function(registry));
    registry.add_function(Image::save_hdr__define_function(registry));
    registry.add_function(Image::save_ldr__define_function(registry));
    registry.add_function(Image::from_array__define_function(registry));
    registry.add_function(Image::to_array__define_function(registry));
    registry.add_function(Image::width__define_function(registry));
    registry.add_function(Image::height__define_function(registry));
    registry.add_function(Image::sample__define_function(registry));
    registry.add_function(Image::get_pixel__define_function(registry));
    registry.add_function(Image::set_pixel__define_function(registry));
    registry.add_function(Image::process__define_function(registry));
}

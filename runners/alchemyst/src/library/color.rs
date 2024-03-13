use image::*;
use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::prelude::*;

#[derive(IntuicioStruct, Default, Clone)]
#[intuicio(name = "Color", module_name = "color", override_send = true)]
pub struct Color {
    pub r: Reference,
    pub g: Reference,
    pub b: Reference,
    pub a: Reference,
}

#[intuicio_methods(module_name = "color")]
impl Color {
    pub fn from_pixel(pixel: &Rgba<f32>, registry: &Registry) -> Self {
        Self {
            r: Reference::new_real(pixel.0[0] as f64, registry),
            g: Reference::new_real(pixel.0[1] as f64, registry),
            b: Reference::new_real(pixel.0[2] as f64, registry),
            a: Reference::new_real(pixel.0[3] as f64, registry),
        }
    }

    pub fn to_pixel(&self) -> Rgba<f32> {
        Rgba([
            *self.r.read::<Real>().unwrap() as f32,
            *self.g.read::<Real>().unwrap() as f32,
            *self.b.read::<Real>().unwrap() as f32,
            *self.a.read::<Real>().unwrap() as f32,
        ])
    }

    #[intuicio_method(use_registry)]
    pub fn from_array(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Array>().unwrap();
        Reference::new(
            Color {
                r: value[0].clone(),
                g: value[1].clone(),
                b: value[2].clone(),
                a: value[3].clone(),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn to_array(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Color>().unwrap();
        Reference::new_array(
            vec![
                value.r.clone(),
                value.g.clone(),
                value.b.clone(),
                value.a.clone(),
            ],
            registry,
        )
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_type(Color::define_struct(registry));
    registry.add_function(Color::from_array__define_function(registry));
    registry.add_function(Color::to_array__define_function(registry));
}

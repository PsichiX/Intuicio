use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::*;

pub type TetraColor = tetra::graphics::Color;

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
    pub fn from_tetra(value: TetraColor, registry: &Registry) -> Self {
        Self {
            r: Reference::new_real(value.r as Real, registry),
            g: Reference::new_real(value.g as Real, registry),
            b: Reference::new_real(value.b as Real, registry),
            a: Reference::new_real(value.a as Real, registry),
        }
    }

    pub fn to_tetra(&self) -> TetraColor {
        TetraColor {
            r: *self.r.read::<Real>().unwrap() as f32,
            g: *self.g.read::<Real>().unwrap() as f32,
            b: *self.b.read::<Real>().unwrap() as f32,
            a: *self.a.read::<Real>().unwrap() as f32,
        }
    }
    #[intuicio_method(use_registry)]
    pub fn transparent(registry: &Registry) -> Reference {
        Reference::new(Color::from_tetra(TetraColor::default(), registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn black(registry: &Registry) -> Reference {
        Reference::new(Color::from_tetra(TetraColor::BLACK, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn white(registry: &Registry) -> Reference {
        Reference::new(Color::from_tetra(TetraColor::WHITE, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn red(registry: &Registry) -> Reference {
        Reference::new(Color::from_tetra(TetraColor::RED, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn green(registry: &Registry) -> Reference {
        Reference::new(Color::from_tetra(TetraColor::GREEN, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn blue(registry: &Registry) -> Reference {
        Reference::new(Color::from_tetra(TetraColor::BLUE, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn hex(registry: &Registry, text: Reference) -> Reference {
        let text = text.read::<Text>().unwrap();
        Reference::new(
            Color::from_tetra(TetraColor::hex(text.as_str()), registry),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn saturate(registry: &Registry, color: Reference) -> Reference {
        let color = color.read::<Color>().unwrap().to_tetra();
        Reference::new(Color::from_tetra(color.clamp(), registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn premultiply(registry: &Registry, color: Reference) -> Reference {
        let color = color.read::<Color>().unwrap().to_tetra();
        Reference::new(
            Color::from_tetra(color.to_premultiplied(), registry),
            registry,
        )
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Color::define_struct(registry));
    registry.add_function(Color::transparent__define_function(registry));
    registry.add_function(Color::black__define_function(registry));
    registry.add_function(Color::white__define_function(registry));
    registry.add_function(Color::red__define_function(registry));
    registry.add_function(Color::green__define_function(registry));
    registry.add_function(Color::blue__define_function(registry));
    registry.add_function(Color::hex__define_function(registry));
    registry.add_function(Color::saturate__define_function(registry));
    registry.add_function(Color::premultiply__define_function(registry));
}

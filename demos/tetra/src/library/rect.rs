use super::vec2::Vec2;
use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::*;

pub type TetraRect = tetra::graphics::Rectangle;

#[derive(Default, IntuicioStruct, Clone)]
#[intuicio(name = "Rect", module_name = "rect", override_send = true)]
pub struct Rect {
    pub x: Reference,
    pub y: Reference,
    pub w: Reference,
    pub h: Reference,
}

#[intuicio_methods(module_name = "rect")]
impl Rect {
    pub fn from_tetra(value: TetraRect, registry: &Registry) -> Self {
        Self {
            x: Reference::new_real(value.x as Real, registry),
            y: Reference::new_real(value.y as Real, registry),
            w: Reference::new_real(value.width as Real, registry),
            h: Reference::new_real(value.height as Real, registry),
        }
    }

    pub fn into_tetra(&self) -> TetraRect {
        TetraRect {
            x: *self.x.read::<Real>().unwrap() as f32,
            y: *self.y.read::<Real>().unwrap() as f32,
            width: *self.w.read::<Real>().unwrap() as f32,
            height: *self.h.read::<Real>().unwrap() as f32,
        }
    }

    #[intuicio_method(use_registry)]
    pub fn left(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Rect>().unwrap();
        let x = *value.x.read::<Real>().unwrap();
        Reference::new_real(x, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn right(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Rect>().unwrap();
        let x = *value.x.read::<Real>().unwrap();
        let w = *value.w.read::<Real>().unwrap();
        Reference::new_real(x + w, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn top(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Rect>().unwrap();
        let y = *value.y.read::<Real>().unwrap();
        Reference::new_real(y, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn bottom(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Rect>().unwrap();
        let y = *value.y.read::<Real>().unwrap();
        let h = *value.h.read::<Real>().unwrap();
        Reference::new_real(y + h, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn does_contain_point(
        registry: &Registry,
        value: Reference,
        point: Reference,
    ) -> Reference {
        let value = value.read::<Rect>().unwrap().into_tetra();
        let point = point.read::<Vec2>().unwrap().into_tetra();
        Reference::new_boolean(value.contains_point(point), registry)
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Rect::define_struct(registry));
    registry.add_function(Rect::left__define_function(registry));
    registry.add_function(Rect::right__define_function(registry));
    registry.add_function(Rect::top__define_function(registry));
    registry.add_function(Rect::bottom__define_function(registry));
    registry.add_function(Rect::does_contain_point__define_function(registry));
}

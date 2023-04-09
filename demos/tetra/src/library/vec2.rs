use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::*;

pub type TetraVec2 = tetra::math::Vec2<f32>;

#[derive(Default, IntuicioStruct, Clone)]
#[intuicio(name = "Vec2", module_name = "vec2", override_send = true)]
pub struct Vec2 {
    pub x: Reference,
    pub y: Reference,
}

#[intuicio_methods(module_name = "vec2")]
impl Vec2 {
    pub fn from_tetra(value: TetraVec2, registry: &Registry) -> Self {
        Self {
            x: Reference::new_real(value.x as Real, registry),
            y: Reference::new_real(value.y as Real, registry),
        }
    }

    pub fn into_tetra(&self) -> TetraVec2 {
        TetraVec2 {
            x: *self.x.read::<Real>().unwrap() as f32,
            y: *self.y.read::<Real>().unwrap() as f32,
        }
    }

    #[intuicio_method(use_registry)]
    pub fn zero(registry: &Registry) -> Reference {
        Reference::new(Vec2::from_tetra(0.0.into(), registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn one(registry: &Registry) -> Reference {
        Reference::new(Vec2::from_tetra(1.0.into(), registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn add(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().into_tetra();
        let b = b.read::<Vec2>().unwrap().into_tetra();
        Reference::new(Vec2::from_tetra(a + b, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn sub(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().into_tetra();
        let b = b.read::<Vec2>().unwrap().into_tetra();
        Reference::new(Vec2::from_tetra(a - b, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn mul(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().into_tetra();
        let b = b.read::<Vec2>().unwrap().into_tetra();
        Reference::new(Vec2::from_tetra(a * b, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn div(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().into_tetra();
        let b = b.read::<Vec2>().unwrap().into_tetra();
        Reference::new(Vec2::from_tetra(a / b, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn dot(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().into_tetra();
        let b = b.read::<Vec2>().unwrap().into_tetra();
        Reference::new(a.dot(b) as Real, registry)
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Vec2::define_struct(registry));
    registry.add_function(Vec2::zero__define_function(registry));
    registry.add_function(Vec2::one__define_function(registry));
    registry.add_function(Vec2::add__define_function(registry));
    registry.add_function(Vec2::sub__define_function(registry));
    registry.add_function(Vec2::mul__define_function(registry));
    registry.add_function(Vec2::div__define_function(registry));
    registry.add_function(Vec2::dot__define_function(registry));
}

use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::prelude::*;

type VekVec2 = vek::vec::repr_c::vec2::Vec2<f64>;

#[derive(IntuicioStruct, Default, Clone)]
#[intuicio(name = "Vec2", odule_name = "vec2", override_send = true)]
pub struct Vec2 {
    pub x: Reference,
    pub y: Reference,
}

#[intuicio_methods(module_name = "vec2")]
impl Vec2 {
    pub fn from_vek(value: VekVec2, registry: &Registry) -> Self {
        Self {
            x: Reference::new_real(value.x, registry),
            y: Reference::new_real(value.y, registry),
        }
    }

    pub fn to_vek(&self) -> VekVec2 {
        VekVec2::new(
            *self.x.read::<Real>().unwrap(),
            *self.y.read::<Real>().unwrap(),
        )
    }

    #[intuicio_method(use_registry)]
    pub fn zero(registry: &Registry) -> Reference {
        Reference::new(
            Vec2 {
                x: Reference::new_real(0.0, registry),
                y: Reference::new_real(0.0, registry),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn one(registry: &Registry) -> Reference {
        Reference::new(
            Vec2 {
                x: Reference::new_real(1.0, registry),
                y: Reference::new_real(1.0, registry),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn from_array(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Array>().unwrap();
        Reference::new(
            Vec2 {
                x: value[0].clone(),
                y: value[1].clone(),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn to_array(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Vec2>().unwrap();
        Reference::new_array(vec![value.x.clone(), value.y.clone()], registry)
    }

    #[intuicio_method(use_registry)]
    pub fn magnitude(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Vec2>().unwrap().to_vek();
        Reference::new_real(value.magnitude(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn magnitude_squared(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Vec2>().unwrap().to_vek();
        Reference::new_real(value.magnitude_squared(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn add(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().to_vek();
        let b = b.read::<Vec2>().unwrap().to_vek();
        let result = Vec2::from_vek(a + b, registry);
        Reference::new(result, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn sub(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().to_vek();
        let b = b.read::<Vec2>().unwrap().to_vek();
        let result = Vec2::from_vek(a - b, registry);
        Reference::new(result, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn mul(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().to_vek();
        let b = b.read::<Vec2>().unwrap().to_vek();
        let result = Vec2::from_vek(a * b, registry);
        Reference::new(result, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn div(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().to_vek();
        let b = b.read::<Vec2>().unwrap().to_vek();
        let result = Vec2::from_vek(a / b, registry);
        Reference::new(result, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn negate(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Vec2>().unwrap().to_vek();
        let result = Vec2::from_vek(-value, registry);
        Reference::new(result, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn normalize(registry: &Registry, value: Reference) -> Reference {
        let value = value.read::<Vec2>().unwrap().to_vek();
        let result = Vec2::from_vek(value.normalized(), registry);
        Reference::new(result, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn dot(registry: &Registry, a: Reference, b: Reference) -> Reference {
        let a = a.read::<Vec2>().unwrap().to_vek();
        let b = b.read::<Vec2>().unwrap().to_vek();
        Reference::new_real(a.dot(b), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn reflect(registry: &Registry, value: Reference, normal: Reference) -> Reference {
        let value = value.read::<Vec2>().unwrap().to_vek();
        let normal = normal.read::<Vec2>().unwrap().to_vek();
        Reference::new(Vec2::from_vek(value.reflected(normal), registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn refract(
        registry: &Registry,
        value: Reference,
        normal: Reference,
        eta: Reference,
    ) -> Reference {
        let value = value.read::<Vec2>().unwrap().to_vek();
        let normal = normal.read::<Vec2>().unwrap().to_vek();
        let eta = *eta.read::<Real>().unwrap();
        Reference::new(
            Vec2::from_vek(value.refracted(normal, eta), registry),
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn lerp(
        registry: &Registry,
        from: Reference,
        to: Reference,
        factor: Reference,
    ) -> Reference {
        let from = from.read::<Vec2>().unwrap().to_vek();
        let to = to.read::<Vec2>().unwrap().to_vek();
        let factor = *factor.read::<Real>().unwrap();
        Reference::new(
            Vec2::from_vek(VekVec2::lerp_unclamped(from, to, factor), registry),
            registry,
        )
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_type(Vec2::define_struct(registry));
    registry.add_function(Vec2::zero__define_function(registry));
    registry.add_function(Vec2::one__define_function(registry));
    registry.add_function(Vec2::from_array__define_function(registry));
    registry.add_function(Vec2::to_array__define_function(registry));
    registry.add_function(Vec2::magnitude__define_function(registry));
    registry.add_function(Vec2::magnitude_squared__define_function(registry));
    registry.add_function(Vec2::add__define_function(registry));
    registry.add_function(Vec2::sub__define_function(registry));
    registry.add_function(Vec2::mul__define_function(registry));
    registry.add_function(Vec2::div__define_function(registry));
    registry.add_function(Vec2::negate__define_function(registry));
    registry.add_function(Vec2::normalize__define_function(registry));
    registry.add_function(Vec2::dot__define_function(registry));
    registry.add_function(Vec2::reflect__define_function(registry));
    registry.add_function(Vec2::refract__define_function(registry));
    registry.add_function(Vec2::lerp__define_function(registry));
}

use crate::{Boolean, Integer, Real, Reference};
use intuicio_core::{define_native_struct, registry::Registry};
use intuicio_derive::intuicio_function;
use rand::Rng;
use std::ops::Rem;

#[intuicio_function(module_name = "math", use_registry)]
pub fn add(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(*a + *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_real(*a + *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn sub(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(*a - *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_real(*a - *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn mul(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(*a * *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_real(*a * *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn div(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(*a / *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_real(*a / *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn negate(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Integer>() {
        return Reference::new_integer(-*value, registry);
    }
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(-*value, registry);
    }
    if let Some(value) = value.read::<Boolean>() {
        return Reference::new_boolean(!*value, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn abs(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Integer>() {
        return Reference::new_integer(value.abs(), registry);
    }
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.abs(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn pow(registry: &Registry, value: Reference, exp: Reference) -> Reference {
    if let (Some(value), Some(exp)) = (value.read::<Integer>(), exp.read::<Integer>()) {
        return Reference::new_integer(value.pow(*exp as _), registry);
    }
    if let (Some(value), Some(exp)) = (value.read::<Real>(), exp.read::<Real>()) {
        return Reference::new_real(value.powf(*exp), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn modulo(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(a.rem(*b), registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_real(a.rem(*b), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn log(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(a.ilog(*b) as Integer, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_real(a.log(*b), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn signum(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Integer>() {
        return Reference::new_integer(value.signum(), registry);
    }
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.signum(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn and(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Boolean>(), b.read::<Boolean>()) {
        return Reference::new_boolean(*a && *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(*a & *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn or(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Boolean>(), b.read::<Boolean>()) {
        return Reference::new_boolean(*a || *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(*a | *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn xor(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(*a ^ *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn shift_left(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(*a << *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn shift_right(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(*a >> *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn random_boolean(registry: &Registry) -> Reference {
    Reference::new_boolean(rand::thread_rng().gen::<Boolean>(), registry)
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn random_integer(registry: &Registry) -> Reference {
    Reference::new_integer(rand::thread_rng().gen::<Integer>(), registry)
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn random_real(registry: &Registry) -> Reference {
    Reference::new_real(rand::thread_rng().gen::<Real>(), registry)
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn equals(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Boolean>(), b.read::<Boolean>()) {
        return Reference::new_boolean(*a == *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_boolean(*a == *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_boolean(*a == *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn not_equals(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Boolean>(), b.read::<Boolean>()) {
        return Reference::new_boolean(*a != *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_boolean(*a != *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_boolean(*a != *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn less_than(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_boolean(*a < *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_boolean(*a < *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn less_than_or_equal(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_boolean(*a <= *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_boolean(*a <= *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn greater_than(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_boolean(*a > *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_boolean(*a > *b, registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn greater_than_or_equal(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_boolean(*a >= *b, registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_boolean(*a >= *b, registry);
    }
    Reference::null()
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(define_native_struct! {
        registry => mod math struct Boolean (Boolean) {}
    });
    registry.add_struct(define_native_struct! {
        registry => mod math struct Integer (Integer) {}
    });
    registry.add_struct(define_native_struct! {
        registry => mod math struct Real (Real) {}
    });
    registry.add_function(add::define_function(registry));
    registry.add_function(sub::define_function(registry));
    registry.add_function(mul::define_function(registry));
    registry.add_function(div::define_function(registry));
    registry.add_function(negate::define_function(registry));
    registry.add_function(abs::define_function(registry));
    registry.add_function(pow::define_function(registry));
    registry.add_function(modulo::define_function(registry));
    registry.add_function(log::define_function(registry));
    registry.add_function(signum::define_function(registry));
    registry.add_function(and::define_function(registry));
    registry.add_function(or::define_function(registry));
    registry.add_function(xor::define_function(registry));
    registry.add_function(shift_left::define_function(registry));
    registry.add_function(shift_right::define_function(registry));
    registry.add_function(random_boolean::define_function(registry));
    registry.add_function(random_integer::define_function(registry));
    registry.add_function(random_real::define_function(registry));
    registry.add_function(equals::define_function(registry));
    registry.add_function(not_equals::define_function(registry));
    registry.add_function(less_than::define_function(registry));
    registry.add_function(less_than_or_equal::define_function(registry));
    registry.add_function(greater_than::define_function(registry));
    registry.add_function(greater_than_or_equal::define_function(registry));
}

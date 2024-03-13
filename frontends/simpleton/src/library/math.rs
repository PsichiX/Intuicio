use crate::{Boolean, Integer, Real, Reference};
use intuicio_core::{define_native_struct, registry::Registry};
use intuicio_derive::intuicio_function;
use rand::Rng;
use std::ops::Rem;

#[intuicio_function(module_name = "math", use_registry)]
pub fn min(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(a.min(*b), registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_real(a.min(*b), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn max(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Integer>(), b.read::<Integer>()) {
        return Reference::new_integer(a.max(*b), registry);
    }
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_real(a.max(*b), registry);
    }
    Reference::null()
}

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
pub fn sin(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.sin(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn cos(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.cos(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn tan(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.tan(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn asin(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.asin(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn acos(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.acos(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn atan(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.atan(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn atan2(registry: &Registry, a: Reference, b: Reference) -> Reference {
    if let (Some(a), Some(b)) = (a.read::<Real>(), b.read::<Real>()) {
        return Reference::new_real(a.atan2(*b), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn degrees(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.to_degrees(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn radians(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.to_radians(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn sqrt(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.sqrt(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn floor(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.floor(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn ceil(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.ceil(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn round(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.round(), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn clamp(registry: &Registry, value: Reference, from: Reference, to: Reference) -> Reference {
    if let (Some(value), Some(from), Some(to)) = (
        value.read::<Integer>(),
        from.read::<Integer>(),
        to.read::<Integer>(),
    ) {
        return Reference::new_integer(value.clamp(*from, *to), registry);
    }
    if let (Some(value), Some(from), Some(to)) =
        (value.read::<Real>(), from.read::<Real>(), to.read::<Real>())
    {
        return Reference::new_real(value.clamp(*from, *to), registry);
    }
    Reference::null()
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn fract(registry: &Registry, value: Reference) -> Reference {
    if let Some(value) = value.read::<Real>() {
        return Reference::new_real(value.fract(), registry);
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
pub fn random_integer(registry: &Registry, from: Reference, to: Reference) -> Reference {
    let from = from
        .read::<Integer>()
        .map(|value| *value)
        .unwrap_or(Integer::MIN);
    let to = to
        .read::<Integer>()
        .map(|value| *value)
        .unwrap_or(Integer::MAX);
    let result = rand::thread_rng().gen_range(from..to);
    Reference::new_integer(result, registry)
}

#[intuicio_function(module_name = "math", use_registry)]
pub fn random_real(registry: &Registry, from: Reference, to: Reference) -> Reference {
    let from = from.read::<Real>().map(|value| *value).unwrap_or(Real::MIN);
    let to = to.read::<Real>().map(|value| *value).unwrap_or(Real::MAX);
    let result = rand::thread_rng().gen_range(from..to);
    Reference::new_real(result, registry)
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
    registry.add_type(define_native_struct! {
        registry => mod math struct Boolean (Boolean) {}
    });
    registry.add_type(define_native_struct! {
        registry => mod math struct Integer (Integer) {}
    });
    registry.add_type(define_native_struct! {
        registry => mod math struct Real (Real) {}
    });
    registry.add_function(min::define_function(registry));
    registry.add_function(max::define_function(registry));
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
    registry.add_function(sin::define_function(registry));
    registry.add_function(cos::define_function(registry));
    registry.add_function(tan::define_function(registry));
    registry.add_function(asin::define_function(registry));
    registry.add_function(acos::define_function(registry));
    registry.add_function(atan::define_function(registry));
    registry.add_function(atan2::define_function(registry));
    registry.add_function(degrees::define_function(registry));
    registry.add_function(radians::define_function(registry));
    registry.add_function(sqrt::define_function(registry));
    registry.add_function(floor::define_function(registry));
    registry.add_function(ceil::define_function(registry));
    registry.add_function(round::define_function(registry));
    registry.add_function(clamp::define_function(registry));
    registry.add_function(fract::define_function(registry));
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

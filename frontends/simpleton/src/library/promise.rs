use crate::{library::closure::Closure, Reference};
use intuicio_core::{context::Context, registry::Registry, IntuicioStruct};
use intuicio_derive::{intuicio_method, intuicio_methods, IntuicioStruct};

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Promise", module_name = "promise", override_send = false)]
pub struct Promise {
    #[intuicio(ignore)]
    pub resolved: Reference,
    #[intuicio(ignore)]
    pub rejected: Reference,
    #[intuicio(ignore)]
    pub next: Reference,
}

#[intuicio_methods(module_name = "promise")]
impl Promise {
    #[allow(clippy::new_ret_no_self)]
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry, resolved: Reference, rejected: Reference) -> Reference {
        Reference::new(
            Promise {
                resolved,
                rejected,
                next: Reference::null(),
            },
            registry,
        )
    }

    fn then_impl(promise: &mut Promise, then: Reference) {
        if let Some(mut next) = promise.next.write::<Promise>() {
            return Self::then_impl(&mut next, then);
        }
        promise.next = then;
    }

    #[intuicio_method()]
    pub fn then(mut promise: Reference, then: Reference) -> Reference {
        let mut promise = promise.write::<Promise>().unwrap();
        Self::then_impl(&mut promise, then);
        Reference::null()
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn resolve(
        context: &mut Context,
        registry: &Registry,
        mut promise: Reference,
        value: Reference,
    ) -> Reference {
        let mut promise = match promise.write::<Promise>() {
            Some(promise) => promise,
            None => return Reference::null(),
        };
        if !promise.resolved.is_null() {
            promise.resolved.read::<Closure>().unwrap().invoke(
                context,
                registry,
                &[promise.next.clone(), value],
            );
        }
        promise.resolved = Reference::null();
        promise.rejected = Reference::null();
        promise.next = Reference::null();
        Reference::null()
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn reject(
        context: &mut Context,
        registry: &Registry,
        mut promise: Reference,
        value: Reference,
    ) -> Reference {
        let mut promise = match promise.write::<Promise>() {
            Some(promise) => promise,
            None => return Reference::null(),
        };
        if !promise.rejected.is_null() {
            promise.rejected.read::<Closure>().unwrap().invoke(
                context,
                registry,
                &[promise.next.clone(), value],
            );
        }
        promise.resolved = Reference::null();
        promise.rejected = Reference::null();
        promise.next = Reference::null();
        Reference::null()
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_type(Promise::define_struct(registry));
    registry.add_function(Promise::new__define_function(registry));
    registry.add_function(Promise::then__define_function(registry));
    registry.add_function(Promise::resolve__define_function(registry));
    registry.add_function(Promise::reject__define_function(registry));
}

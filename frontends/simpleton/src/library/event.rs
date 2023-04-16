use crate::{Array, Function, Reference};
use intuicio_core::{context::Context, registry::Registry, IntuicioStruct};
use intuicio_derive::{intuicio_method, intuicio_methods, IntuicioStruct};

use super::{closure::Closure, promise::Promise};

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Event", module_name = "event", override_send = false)]
pub struct Event {
    #[intuicio(ignore)]
    pub persistent: Array,
    #[intuicio(ignore)]
    pub oneshot: Array,
}

#[intuicio_methods(module_name = "event")]
impl Event {
    #[intuicio_method()]
    pub fn bind(mut event: Reference, target: Reference) -> Reference {
        let mut event = event.write::<Event>().unwrap();
        if target.read::<Promise>().is_some() {
            event.oneshot.push(target);
        } else {
            event.persistent.push(target);
        }
        Reference::null()
    }

    #[intuicio_method()]
    pub fn bind_once(mut event: Reference, target: Reference) -> Reference {
        let mut event = event.write::<Event>().unwrap();
        event.oneshot.push(target);
        Reference::null()
    }

    #[intuicio_method()]
    pub fn unbind(mut event: Reference, target: Reference) -> Reference {
        let mut event = event.write::<Event>().unwrap();
        if target.is_null() {
            event.persistent.clear();
            event.oneshot.clear();
        } else {
            while let Some(index) = event
                .persistent
                .iter()
                .position(|item| crate::library::reflect::are_same_impl(item, &target))
            {
                event.persistent.swap_remove(index);
            }
            while let Some(index) = event
                .oneshot
                .iter()
                .position(|item| crate::library::reflect::are_same_impl(item, &target))
            {
                event.oneshot.swap_remove(index);
            }
        }
        Reference::null()
    }

    fn dispatch_impl(
        context: &mut Context,
        registry: &Registry,
        target: Reference,
        arguments: Reference,
    ) {
        if target.read::<Function>().is_some() {
            crate::library::reflect::call(context, registry, target, arguments);
        } else if target.read::<Closure>().is_some() {
            Closure::call(context, registry, target, arguments);
        } else if target.read::<Promise>().is_some() {
            Promise::resolve(context, registry, target, arguments);
        }
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn dispatch(
        context: &mut Context,
        registry: &Registry,
        mut event: Reference,
        arguments: Reference,
    ) -> Reference {
        assert!(arguments.read::<Array>().is_some());
        let mut event = event.write::<Event>().unwrap();
        for target in &event.persistent {
            Self::dispatch_impl(context, registry, target.clone(), arguments.clone())
        }
        for target in &event.oneshot {
            Self::dispatch_impl(context, registry, target.clone(), arguments.clone())
        }
        event.oneshot.clear();
        Reference::null()
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Event::define_struct(registry));
    registry.add_function(Event::bind__define_function(registry));
    registry.add_function(Event::bind_once__define_function(registry));
    registry.add_function(Event::unbind__define_function(registry));
    registry.add_function(Event::dispatch__define_function(registry));
}

use crate::{Array, Function, Reference};
use intuicio_core::{context::Context, registry::Registry, IntuicioStruct};
use intuicio_derive::{intuicio_method, intuicio_methods, IntuicioStruct};

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Closure", module_name = "closure", override_send = false)]
pub struct Closure {
    #[intuicio(ignore)]
    pub function: Function,
    #[intuicio(ignore)]
    pub captured: Array,
}

#[intuicio_methods(module_name = "closure")]
impl Closure {
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry, function: Reference, captured: Reference) -> Reference {
        Reference::new(
            Closure {
                function: function.read::<Function>().unwrap().clone(),
                captured: captured.read::<Array>().unwrap().clone(),
            },
            registry,
        )
    }

    pub fn invoke(
        &self,
        context: &mut Context,
        registry: &Registry,
        arguments: &[Reference],
    ) -> Reference {
        for argument in arguments.iter().rev() {
            context.stack().push(argument.clone());
        }
        for argument in self.captured.iter().rev() {
            context.stack().push(argument.clone());
        }
        self.function.handle().unwrap().invoke(context, registry);
        context.stack().pop::<Reference>().unwrap_or_default()
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn call(
        context: &mut Context,
        registry: &Registry,
        closure: Reference,
        arguments: Reference,
    ) -> Reference {
        closure.read::<Closure>().unwrap().invoke(
            context,
            registry,
            arguments.read::<Array>().as_ref().unwrap(),
        )
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Closure::define_struct(registry));
    registry.add_function(Closure::new__define_function(registry));
    registry.add_function(Closure::call__define_function(registry));
}

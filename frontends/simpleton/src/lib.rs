pub mod library;
pub mod nodes;
pub mod parser;
pub mod script;

pub mod prelude {
    pub use crate::{library::*, script::*, *};
}

use intuicio_core::{crate_version, IntuicioVersion};

pub use intuicio_framework_dynamic::{
    Array, Boolean, Function, Integer, Map, Real, Reference, Text, Transferable, Transferred, Type,
};

pub fn frontend_simpleton_version() -> IntuicioVersion {
    crate_version!()
}

#[cfg(test)]
mod tests {
    use crate::{
        library::{jobs::Jobs, ObjectBuilder},
        script::{SimpletonContentParser, SimpletonPackage, SimpletonScriptExpression},
        Integer, Real, Reference,
    };
    use intuicio_backend_vm::prelude::*;
    use intuicio_core::prelude::*;

    #[test]
    fn test_simpleton_script() {
        let mut content_provider = FileContentProvider::new("simp", SimpletonContentParser);
        let package =
            SimpletonPackage::new("../../resources/package.simp", &mut content_provider).unwrap();
        let host_producer = HostProducer::new(move || {
            let mut registry = Registry::default();
            crate::library::install(&mut registry);
            package
                .compile()
                .install::<VmScope<SimpletonScriptExpression>>(
                    &mut registry,
                    None,
                    // Some(
                    //     PrintDebugger::full()
                    //         .basic_printables()
                    //         .stack_bytes(false)
                    //         .registers_bytes(false)
                    //         .into_handle(),
                    // ),
                );
            let context = Context::new(10240, 10240);
            Host::new(context, registry.into())
        });
        let mut vm = host_producer.produce();
        vm.context()
            .set_custom(Jobs::HOST_PRODUCER_CUSTOM, host_producer);

        let adder = Reference::new_raw(
            ObjectBuilder::new("Adder", "adder")
                .field("a", Reference::new_integer(40, vm.registry()))
                .field("b", Reference::new_integer(2, vm.registry()))
                .build(vm.registry()),
        );
        let (result,) = vm
            .call_function::<(Reference,), _>("add", "adder", None)
            .unwrap()
            .run((adder,));
        assert_eq!(vm.context().stack().position(), 0);
        assert_eq!(*result.read::<Integer>().unwrap(), 42);

        let (result,) = vm
            .call_function::<(Reference,), _>("main", "test", None)
            .unwrap()
            .run(());
        assert_eq!(vm.context().stack().position(), 0);
        assert_eq!(*result.read::<Real>().unwrap(), 42.0);
    }
}

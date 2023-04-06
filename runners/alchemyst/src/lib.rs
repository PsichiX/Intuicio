mod library;

use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_frontend_simpleton::{
    library::jobs::Jobs,
    script::{SimpletonModule, SimpletonPackage, SimpletonScriptExpression},
    Reference,
};

pub fn execute<CP>(
    entry: &str,
    args: impl IntoIterator<Item = String>,
    content_provider: &mut CP,
) -> Reference
where
    CP: ScriptContentProvider<SimpletonModule>,
{
    let package = SimpletonPackage::new(entry, content_provider).unwrap();
    let host_producer = HostProducer::new(move || {
        let mut registry = Registry::default();
        intuicio_frontend_simpleton::library::install(&mut registry);
        crate::library::install(&mut registry);
        package
            .compile()
            .install::<VmScope<SimpletonScriptExpression>>(&mut registry, None);
        let context = Context::new(1024 * 128, 1024 * 128, 0);
        Host::new(context, registry.into())
    });
    let mut host = host_producer.produce();
    #[cfg(feature = "jobs")]
    {
        host.context()
            .set_custom(Jobs::HOST_PRODUCER_CUSTOM, host_producer);
    }
    let args = Reference::new_array(
        args.into_iter()
            .map(|arg| Reference::new_text(arg, host.registry()))
            .collect(),
        host.registry(),
    );
    host.call_function::<(Reference,), _>("main", "main", None)
        .unwrap()
        .run((args,))
        .0
}

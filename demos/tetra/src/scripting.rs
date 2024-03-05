use crate::library::engine::Engine;
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_data::prelude::*;
use intuicio_frontend_simpleton::prelude::{jobs::Jobs, *};
use tetra::{time::get_delta_time, Context as TetraContext};

pub struct Scripting {
    host: Host,
    state: Reference,
    _context_lifetime: Lifetime,
}

impl Drop for Scripting {
    fn drop(&mut self) {
        self.cleanup();
    }
}

impl Scripting {
    pub fn new(
        assets: &str,
        stack_capacity: usize,
        registers_capacity: usize,
        entry: &str,
        tetra_context: &mut TetraContext,
    ) -> Self {
        let entry = format!("{}/{}", assets, entry);
        let mut content_provider = ExtensionContentProvider::<SimpletonModule>::default()
            .extension(
                "simp",
                FileContentProvider::new("simp", SimpletonContentParser),
            )
            .extension("plugin", IgnoreContentProvider)
            .default_extension("simp");
        let package = SimpletonPackage::new(&entry, &mut content_provider).unwrap();
        let host_producer = HostProducer::new(move || {
            let mut registry = Registry::default();
            intuicio_frontend_simpleton::library::install(&mut registry);
            crate::library::install(&mut registry);
            package.install_plugins(&mut registry, &["./", "../../target/debug"]);
            package
                .compile()
                .install::<VmScope<SimpletonScriptExpression>>(&mut registry, None);
            let context = Context::new(stack_capacity, registers_capacity);
            Host::new(context, registry.into())
        });
        let mut host = host_producer.produce();
        host.context()
            .set_custom(Jobs::HOST_PRODUCER_CUSTOM, host_producer);
        let context_lifetime = Lifetime::default();
        let tetra_context =
            ManagedRefMut::new(tetra_context, context_lifetime.borrow_mut().unwrap());
        let engine = Reference::new(Engine::new(assets, tetra_context), host.registry());
        let state = Reference::new_map(
            map! {
                engine: engine,
                assets: Reference::new_text(assets.to_owned(), host.registry()),
            },
            host.registry(),
        );
        Self {
            host,
            state,
            _context_lifetime: context_lifetime,
        }
    }

    pub fn initialize(&mut self) {
        if let Some(call) = self
            .host
            .call_function::<(Reference,), _>("initialize", "game", None)
        {
            call.run((self.state.clone(),));
        }
    }

    pub fn cleanup(&mut self) {
        if let Some(call) = self
            .host
            .call_function::<(Reference,), _>("cleanup", "game", None)
        {
            call.run((self.state.clone(),));
        }
    }

    pub fn update(&mut self, ctx: &TetraContext) {
        let dt = Reference::new_real(
            get_delta_time(ctx).as_secs_f32() as Real,
            self.host.registry(),
        );
        if let Some(call) = self
            .host
            .call_function::<(Reference,), _>("update", "game", None)
        {
            call.run((dt, self.state.clone()));
        }
    }

    pub fn draw(&mut self, ctx: &TetraContext) {
        let dt = Reference::new_real(
            get_delta_time(ctx).as_secs_f32() as Real,
            self.host.registry(),
        );
        if let Some(call) = self
            .host
            .call_function::<(Reference,), _>("draw", "game", None)
        {
            call.run((dt, self.state.clone()));
        }
    }
}

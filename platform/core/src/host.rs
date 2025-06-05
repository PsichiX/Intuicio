use crate::{
    context::Context,
    function::{FunctionHandle, FunctionQuery, FunctionQueryParameter},
    registry::{Registry, RegistryHandle},
    types::TypeQuery,
};
use intuicio_data::data_stack::DataStackPack;
use std::{cell::RefCell, marker::PhantomData, sync::Arc};
use typid::ID;

thread_local! {
    static GLOBAL_HOST_STACK: RefCell<Vec<(HostId, Host)>> = const{ RefCell::new(vec![]) };
}

pub type HostId = ID<Host>;

#[derive(Clone)]
pub struct HostProducer {
    producer: Arc<Box<dyn Fn() -> Host + Send + Sync>>,
}

impl HostProducer {
    pub fn new(f: impl Fn() -> Host + Send + Sync + 'static) -> Self {
        Self {
            producer: Arc::new(Box::new(f)),
        }
    }

    pub fn produce(&self) -> Host {
        (self.producer)()
    }
}

pub struct Host {
    context: Context,
    registry: RegistryHandle,
}

impl Host {
    pub fn new(context: Context, registry: RegistryHandle) -> Self {
        Self { context, registry }
    }

    pub fn fork(&self) -> Self {
        Self {
            context: self.context.fork(),
            registry: self.registry.clone(),
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn push_global(self) -> Result<HostId, Self> {
        GLOBAL_HOST_STACK.with(|host| match host.try_borrow_mut() {
            Ok(mut stack) => {
                let id = HostId::new();
                stack.push((id, self));
                Ok(id)
            }
            Err(_) => Err(self),
        })
    }

    pub fn pop_global() -> Option<Self> {
        GLOBAL_HOST_STACK.with(move |stack| Some(stack.try_borrow_mut().ok()?.pop()?.1))
    }

    pub fn remove_global(id: HostId) -> Option<Self> {
        GLOBAL_HOST_STACK.with(move |stack| {
            let mut stack = stack.try_borrow_mut().ok()?;
            let index = stack.iter().position(|(host_id, _)| host_id == &id)?;
            Some(stack.remove(index).1)
        })
    }

    pub fn with_global<T>(f: impl FnOnce(&mut Self) -> T) -> Option<T> {
        GLOBAL_HOST_STACK.with(move |stack| {
            let mut stack = stack.try_borrow_mut().ok()?;
            let host = &mut stack.last_mut()?.1;
            Some(f(host))
        })
    }

    pub fn context(&mut self) -> &mut Context {
        &mut self.context
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn context_and_registry(&mut self) -> (&mut Context, &Registry) {
        (&mut self.context, &self.registry)
    }

    pub fn find_function(
        &self,
        name: &str,
        module_name: &str,
        type_name: Option<&str>,
    ) -> Option<FunctionHandle> {
        self.registry.find_function(FunctionQuery {
            name: Some(name.into()),
            module_name: Some(module_name.into()),
            type_query: type_name.map(|type_name| TypeQuery {
                name: Some(type_name.into()),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    pub fn call_function<O: DataStackPack, I: DataStackPack>(
        &mut self,
        name: &str,
        module_name: &str,
        type_name: Option<&str>,
    ) -> Option<HostFunctionCall<I, O>> {
        let inputs_query = I::pack_types()
            .into_iter()
            .map(|type_hash| FunctionQueryParameter {
                type_query: Some(TypeQuery {
                    type_hash: Some(type_hash),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let outputs_query = O::pack_types()
            .into_iter()
            .map(|type_hash| FunctionQueryParameter {
                type_query: Some(TypeQuery {
                    type_hash: Some(type_hash),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let handle = self.registry.find_function(FunctionQuery {
            name: Some(name.into()),
            module_name: Some(module_name.into()),
            type_query: type_name.map(|type_name| TypeQuery {
                name: Some(type_name.into()),
                ..Default::default()
            }),
            inputs: inputs_query.into(),
            outputs: outputs_query.into(),
            ..Default::default()
        })?;
        Some(HostFunctionCall {
            context: &mut self.context,
            registry: &self.registry,
            handle,
            _phantom: Default::default(),
        })
    }
}

pub struct HostFunctionCall<'a, I: DataStackPack, O: DataStackPack> {
    context: &'a mut Context,
    registry: &'a Registry,
    handle: FunctionHandle,
    _phantom: PhantomData<(I, O)>,
}

impl<I: DataStackPack, O: DataStackPack> HostFunctionCall<'_, I, O> {
    pub fn run(self, inputs: I) -> O {
        self.handle.call(self.context, self.registry, inputs, false)
    }
}

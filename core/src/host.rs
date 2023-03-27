use crate::{
    context::Context,
    function::FunctionQuery,
    prelude::{FunctionHandle, FunctionQueryParameter},
    registry::{Registry, RegistryHandle},
    struct_type::StructQuery,
};
use intuicio_data::data_stack::DataStackPack;
use std::{cell::RefCell, marker::PhantomData};
use typid::ID;

thread_local! {
    static GLOBAL_HOST_STACK: RefCell<Vec<(HostId, Host)>> = RefCell::new(vec![]);
}

pub type HostId = ID<Host>;

pub struct Host {
    context: Context,
    registry: RegistryHandle,
}

impl Host {
    pub fn new(context: Context, registry: RegistryHandle) -> Self {
        Self { context, registry }
    }

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

    pub fn with_global<T>(mut f: impl FnMut(&mut Self) -> T) -> Option<T> {
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
        struct_name: Option<&str>,
    ) -> Option<FunctionHandle> {
        self.registry.find_function(FunctionQuery {
            name: Some(name.into()),
            module_name: Some(module_name.into()),
            struct_query: struct_name.map(|struct_name| StructQuery {
                name: Some(struct_name.into()),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    pub fn call_function<O: DataStackPack, I: DataStackPack>(
        &mut self,
        name: &str,
        module_name: &str,
        struct_name: Option<&str>,
    ) -> Option<HostFunctionCall<I, O>> {
        let inputs_query = I::pack_types()
            .into_iter()
            .map(|type_id| FunctionQueryParameter {
                struct_query: Some(StructQuery {
                    type_id: Some(type_id),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let outputs_query = O::pack_types()
            .into_iter()
            .map(|type_id| FunctionQueryParameter {
                struct_query: Some(StructQuery {
                    type_id: Some(type_id),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let handle = self.registry.find_function(FunctionQuery {
            name: Some(name.into()),
            module_name: Some(module_name.into()),
            struct_query: struct_name.map(|struct_name| StructQuery {
                name: Some(struct_name.into()),
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

impl<'a, I: DataStackPack, O: DataStackPack> HostFunctionCall<'a, I, O> {
    pub fn run(self, inputs: I) -> O {
        self.handle.call(self.context, self.registry, inputs, false)
    }
}

//! The native side of a scripting solution.
//!
//! See [`Host`].
use crate::{
    Filter,
    context::Context,
    function::{FunctionHandle, FunctionQuery, FunctionQueryParameter, Parameters},
    registry::{Registry, RegistryHandle},
    types::TypeQuery,
};
use intuicio_data::data_stack::DataStackPack;
use std::{cell::RefCell, marker::PhantomData, sync::Arc};
use typid::ID;

thread_local! {
    /// Per-thread stack of hosts installed with [`Host::push_global`].
    static GLOBAL_HOST_STACK: RefCell<Vec<(HostId, Host)>> = const{ RefCell::new(vec![]) };
}

/// Identifies a host pushed onto the global stack.
pub type HostId = ID<Host>;

/// A cloneable factory for hosts.
///
/// A [`Host`] cannot cross threads, but a producer can. Store one in the
/// context's custom data and every worker thread can build a host of its own
/// with the same registry.
#[derive(Clone)]
pub struct HostProducer {
    producer: Arc<Box<dyn Fn() -> Host + Send + Sync>>,
}

impl HostProducer {
    /// Wraps a closure that builds a host.
    pub fn new(f: impl Fn() -> Host + Send + Sync + 'static) -> Self {
        Self {
            producer: Arc::new(Box::new(f)),
        }
    }

    /// Builds a new host.
    pub fn produce(&self) -> Host {
        (self.producer)()
    }
}

/// A [`Context`] and a [`Registry`] paired up, ready to call functions.
///
/// This is the application side of a scripting solution: register your native
/// types and functions, install your scripts, then call in.
///
/// The registry is shared through an [`Arc`], so forks and worker threads all
/// see the same definitions. It must be complete before the first call, since
/// a registry in use can no longer be modified.
pub struct Host {
    context: Context,
    registry: RegistryHandle,
}

impl Host {
    /// Pairs a context with a registry.
    pub fn new(context: Context, registry: RegistryHandle) -> Self {
        Self { context, registry }
    }

    /// Builds a host with a fresh context of the same capacities, sharing this
    /// registry.
    pub fn fork(&self) -> Self {
        Self {
            context: self.context.fork(),
            registry: self.registry.clone(),
        }
    }

    /// Pushes this host onto the current thread's global stack and returns its
    /// id.
    ///
    /// Gives the host back when the stack is already borrowed. Useful for code
    /// that cannot pass a host down by argument, such as a native callback.
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

    /// Takes the topmost host off the current thread's global stack.
    pub fn pop_global() -> Option<Self> {
        GLOBAL_HOST_STACK.with(move |stack| Some(stack.try_borrow_mut().ok()?.pop()?.1))
    }

    /// Takes a specific host off the current thread's global stack.
    pub fn remove_global(id: HostId) -> Option<Self> {
        GLOBAL_HOST_STACK.with(move |stack| {
            let mut stack = stack.try_borrow_mut().ok()?;
            let index = stack.iter().position(|(host_id, _)| host_id == &id)?;
            Some(stack.remove(index).1)
        })
    }

    /// Runs `f` with the topmost host of the current thread.
    ///
    /// Returns [`None`] when the stack is empty or already borrowed.
    pub fn with_global<T>(f: impl FnOnce(&mut Self) -> T) -> Option<T> {
        GLOBAL_HOST_STACK.with(move |stack| {
            let mut stack = stack.try_borrow_mut().ok()?;
            let host = &mut stack.last_mut()?.1;
            Some(f(host))
        })
    }

    /// Returns the context.
    pub fn context(&mut self) -> &mut Context {
        &mut self.context
    }

    /// Returns the registry.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Returns context and registry at once, as function bodies need both.
    pub fn context_and_registry(&mut self) -> (&mut Context, &Registry) {
        (&mut self.context, &self.registry)
    }

    /// Looks a function up by name, module and optionally owning type.
    pub fn find_function(
        &self,
        name: &str,
        module_name: &str,
        type_name: Option<&str>,
    ) -> Option<FunctionHandle> {
        self.registry.find_function(FunctionQuery {
            name: Some(name.into()),
            module_name: Filter::Matching(module_name.into()),
            type_query: type_name
                .map(|type_name| TypeQuery {
                    name: Some(type_name.into()),
                    ..Default::default()
                })
                .into(),
            ..Default::default()
        })
    }

    /// Prepares a call to a function, matching it on argument and result types
    /// as well as its name.
    ///
    /// Returns [`None`] when no function matches. Call
    /// [`HostFunctionCall::run`] on the result with the arguments.
    ///
    /// ```no_run
    /// # use intuicio_core::host::Host;
    /// # fn example(host: &mut Host) {
    /// let (result,) = host
    ///     .call_function::<(i32,), _>("add", "lib", None)
    ///     .unwrap()
    ///     .run((40_i32, 2_i32));
    /// # }
    /// ```
    pub fn call_function<O: DataStackPack, I: DataStackPack>(
        &'_ mut self,
        name: &str,
        module_name: &str,
        type_name: Option<&str>,
    ) -> Option<HostFunctionCall<'_, I, O>> {
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
            module_name: Filter::Matching(module_name.into()),
            type_query: type_name
                .map(|type_name| TypeQuery {
                    name: Some(type_name.into()),
                    ..Default::default()
                })
                .into(),
            inputs: Parameters::Exact(inputs_query.into()),
            outputs: Parameters::Exact(outputs_query.into()),
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

/// A function found by [`Host::call_function`], waiting for its arguments.
pub struct HostFunctionCall<'a, I: DataStackPack, O: DataStackPack> {
    context: &'a mut Context,
    registry: &'a Registry,
    handle: FunctionHandle,
    _phantom: PhantomData<(I, O)>,
}

impl<I: DataStackPack, O: DataStackPack> HostFunctionCall<'_, I, O> {
    /// Pushes the arguments, runs the function and pops the results.
    pub fn run(self, inputs: I) -> O {
        self.handle.call(self.context, self.registry, inputs, false)
    }
}

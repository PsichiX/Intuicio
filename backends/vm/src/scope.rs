use crate::debugger::VmDebuggerHandle;
use intuicio_core::{
    context::Context,
    function::FunctionBody,
    registry::{Registry, RegistryHandle},
    script::{ScriptExpression, ScriptFunctionGenerator, ScriptHandle, ScriptOperation},
};
use intuicio_data::managed::{ManagedLazy, ManagedRefMut};
use typid::ID;

pub type VmScopeSymbol = ID<()>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmScopeResult {
    Continue,
    Completed,
    Suspended,
}

impl VmScopeResult {
    pub fn can_continue(self) -> bool {
        self == VmScopeResult::Continue
    }

    pub fn is_completed(self) -> bool {
        self == VmScopeResult::Completed
    }

    pub fn is_suspended(self) -> bool {
        self == VmScopeResult::Suspended
    }

    pub fn can_progress(self) -> bool {
        !self.is_completed()
    }
}

pub struct VmScope<'a, SE: ScriptExpression> {
    handle: ScriptHandle<'a, SE>,
    symbol: VmScopeSymbol,
    position: usize,
    child: Option<Box<Self>>,
    debugger: Option<VmDebuggerHandle<SE>>,
}

impl<'a, SE: ScriptExpression> VmScope<'a, SE> {
    pub fn new(handle: ScriptHandle<'a, SE>, symbol: VmScopeSymbol) -> Self {
        Self {
            handle,
            symbol,
            position: 0,
            child: None,
            debugger: None,
        }
    }

    /// # Safety
    pub unsafe fn restore(mut self, position: usize, child: Option<Self>) -> Self {
        self.position = position;
        self.child = child.map(Box::new);
        self
    }

    pub fn with_debugger(mut self, debugger: Option<VmDebuggerHandle<SE>>) -> Self {
        self.debugger = debugger;
        self
    }

    #[allow(clippy::type_complexity)]
    pub fn into_inner(
        self,
    ) -> (
        ScriptHandle<'a, SE>,
        VmScopeSymbol,
        usize,
        Option<Box<Self>>,
        Option<VmDebuggerHandle<SE>>,
    ) {
        (
            self.handle,
            self.symbol,
            self.position,
            self.child,
            self.debugger,
        )
    }

    pub fn symbol(&self) -> VmScopeSymbol {
        self.symbol
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn has_completed(&self) -> bool {
        self.position >= self.handle.len()
    }

    pub fn child(&self) -> Option<&Self> {
        self.child.as_deref()
    }

    pub fn run(&mut self, context: &mut Context, registry: &Registry) {
        while self.step(context, registry).can_progress() {}
    }

    pub fn run_until_suspended(
        &mut self,
        context: &mut Context,
        registry: &Registry,
    ) -> VmScopeResult {
        loop {
            match self.step(context, registry) {
                VmScopeResult::Continue => {}
                result => return result,
            }
        }
    }

    pub fn step(&mut self, context: &mut Context, registry: &Registry) -> VmScopeResult {
        if let Some(child) = &mut self.child {
            match child.step(context, registry) {
                VmScopeResult::Completed => {
                    self.child = None;
                }
                result => return result,
            }
        }
        if self.position == 0
            && let Some(debugger) = self.debugger.as_ref()
            && let Ok(mut debugger) = debugger.try_write()
        {
            debugger.on_enter_scope(self, context, registry);
        }
        let result = if let Some(operation) = self.handle.get(self.position) {
            if let Some(debugger) = self.debugger.as_ref()
                && let Ok(mut debugger) = debugger.try_write()
            {
                debugger.on_enter_operation(self, operation, self.position, context, registry);
            }
            let position = self.position;
            let result = match operation {
                ScriptOperation::None => {
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::Expression { expression } => {
                    expression.evaluate(context, registry);
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::DefineRegister { query } => {
                    let handle = registry
                        .types()
                        .find(|handle| query.is_valid(handle))
                        .unwrap_or_else(|| {
                            panic!("Could not define register for non-existent type: {query:#?}")
                        });
                    unsafe {
                        context
                            .registers()
                            .push_register_raw(handle.type_hash(), *handle.layout())
                    };
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::DropRegister { index } => {
                    let index = context.absolute_register_index(*index);
                    context
                        .registers()
                        .access_register(index)
                        .unwrap_or_else(|| {
                            panic!("Could not access non-existent register: {index}")
                        })
                        .free();
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::PushFromRegister { index } => {
                    let index = context.absolute_register_index(*index);
                    let (stack, registers) = context.stack_and_registers();
                    let mut register = registers.access_register(index).unwrap_or_else(|| {
                        panic!("Could not access non-existent register: {index}")
                    });
                    if !stack.push_from_register(&mut register) {
                        panic!("Could not push data from register: {index}");
                    }
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::PopToRegister { index } => {
                    let index = context.absolute_register_index(*index);
                    let (stack, registers) = context.stack_and_registers();
                    let mut register = registers.access_register(index).unwrap_or_else(|| {
                        panic!("Could not access non-existent register: {index}")
                    });
                    if !stack.pop_to_register(&mut register) {
                        panic!("Could not pop data to register: {index}");
                    }
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::MoveRegister { from, to } => {
                    let from = context.absolute_register_index(*from);
                    let to = context.absolute_register_index(*to);
                    let (mut source, mut target) = context
                        .registers()
                        .access_registers_pair(from, to)
                        .unwrap_or_else(|| {
                            panic!("Could not access non-existent registers pair: {from} and {to}")
                        });
                    source.move_to(&mut target);
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::CallFunction { query } => {
                    let handle = registry
                        .functions()
                        .find(|handle| query.is_valid(handle.signature()))
                        .unwrap_or_else(|| {
                            panic!("Could not call non-existent function: {query:#?}")
                        });
                    handle.invoke(context, registry);
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::BranchScope {
                    scope_success,
                    scope_failure,
                } => {
                    if context.stack().pop::<bool>().unwrap() {
                        self.child = Some(Box::new(
                            Self::new(scope_success.clone(), self.symbol)
                                .with_debugger(self.debugger.clone()),
                        ));
                    } else if let Some(scope_failure) = scope_failure {
                        self.child = Some(Box::new(
                            Self::new(scope_failure.clone(), self.symbol)
                                .with_debugger(self.debugger.clone()),
                        ));
                    }
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::LoopScope { scope } => {
                    if !context.stack().pop::<bool>().unwrap() {
                        self.position += 1;
                    } else {
                        self.child = Some(Box::new(
                            Self::new(scope.clone(), self.symbol)
                                .with_debugger(self.debugger.clone()),
                        ));
                    }
                    VmScopeResult::Continue
                }
                ScriptOperation::PushScope { scope } => {
                    context.store_registers();
                    self.child = Some(Box::new(
                        Self::new(scope.clone(), self.symbol).with_debugger(self.debugger.clone()),
                    ));
                    self.position += 1;
                    VmScopeResult::Continue
                }
                ScriptOperation::PopScope => {
                    context.restore_registers();
                    self.position = self.handle.len();
                    VmScopeResult::Completed
                }
                ScriptOperation::ContinueScopeConditionally => {
                    if context.stack().pop::<bool>().unwrap() {
                        self.position += 1;
                        VmScopeResult::Continue
                    } else {
                        self.position = self.handle.len();
                        VmScopeResult::Completed
                    }
                }
                ScriptOperation::Suspend => {
                    self.position += 1;
                    VmScopeResult::Suspended
                }
            };
            if let Some(debugger) = self.debugger.as_ref()
                && let Ok(mut debugger) = debugger.try_write()
            {
                debugger.on_exit_operation(self, operation, position, context, registry);
            }
            result
        } else {
            VmScopeResult::Completed
        };
        if (!result.can_progress() || self.position >= self.handle.len())
            && let Some(debugger) = self.debugger.as_ref()
            && let Ok(mut debugger) = debugger.try_write()
        {
            debugger.on_exit_scope(self, context, registry);
        }
        result
    }
}

impl<SE: ScriptExpression + 'static> ScriptFunctionGenerator<SE> for VmScope<'static, SE> {
    type Input = Option<VmDebuggerHandle<SE>>;
    type Output = VmScopeSymbol;

    fn generate_function_body(
        script: ScriptHandle<'static, SE>,
        debugger: Self::Input,
    ) -> Option<(FunctionBody, Self::Output)> {
        let symbol = VmScopeSymbol::new();
        Some((
            FunctionBody::closure(move |context, registry| {
                Self::new(script.clone(), symbol)
                    .with_debugger(debugger.clone())
                    .run(context, registry);
            }),
            symbol,
        ))
    }
}

impl<SE: ScriptExpression> Clone for VmScope<'_, SE> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            symbol: self.symbol,
            position: self.position,
            child: self.child.as_ref().map(|child| Box::new((**child).clone())),
            debugger: self.debugger.clone(),
        }
    }
}

pub enum VmScopeFutureContext {
    Owned(Box<Context>),
    RefMut(ManagedRefMut<Context>),
    Lazy(ManagedLazy<Context>),
}

impl From<Box<Context>> for VmScopeFutureContext {
    fn from(value: Box<Context>) -> Self {
        Self::Owned(value)
    }
}

impl From<Context> for VmScopeFutureContext {
    fn from(value: Context) -> Self {
        Self::Owned(Box::new(value))
    }
}

impl From<ManagedRefMut<Context>> for VmScopeFutureContext {
    fn from(value: ManagedRefMut<Context>) -> Self {
        Self::RefMut(value)
    }
}

impl From<ManagedLazy<Context>> for VmScopeFutureContext {
    fn from(value: ManagedLazy<Context>) -> Self {
        Self::Lazy(value)
    }
}

pub struct VmScopeFuture<'a, SE: ScriptExpression> {
    pub scope: VmScope<'a, SE>,
    pub context: VmScopeFutureContext,
    pub registry: RegistryHandle,
    pub operations_per_poll: usize,
}

impl<'a, SE: ScriptExpression> VmScopeFuture<'a, SE> {
    pub fn new(
        scope: VmScope<'a, SE>,
        context: impl Into<VmScopeFutureContext>,
        registry: RegistryHandle,
    ) -> Self {
        Self {
            scope,
            context: context.into(),
            registry,
            operations_per_poll: usize::MAX,
        }
    }

    pub fn operations_per_poll(mut self, value: usize) -> Self {
        self.operations_per_poll = value;
        self
    }

    fn step(&mut self) -> Option<VmScopeResult> {
        match &mut self.context {
            VmScopeFutureContext::Owned(context) => {
                Some(self.scope.step(&mut *context, &self.registry))
            }
            VmScopeFutureContext::RefMut(context) => {
                let mut context = context.write()?;
                Some(self.scope.step(&mut context, &self.registry))
            }
            VmScopeFutureContext::Lazy(context) => {
                let mut context = context.write()?;
                Some(self.scope.step(&mut context, &self.registry))
            }
        }
    }
}

impl<SE: ScriptExpression> Future for VmScopeFuture<'_, SE> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        for _ in 0..self.operations_per_poll {
            match self.step() {
                None => return std::task::Poll::Pending,
                Some(VmScopeResult::Completed) => return std::task::Poll::Ready(()),
                Some(VmScopeResult::Suspended) => return std::task::Poll::Pending,
                Some(VmScopeResult::Continue) => {}
            }
        }
        std::task::Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use crate::scope::*;
    use intuicio_core::{
        Visibility,
        function::{Function, FunctionParameter, FunctionQuery, FunctionSignature},
        script::{ScriptBuilder, ScriptFunction, ScriptFunctionParameter, ScriptFunctionSignature},
        types::{TypeQuery, struct_type::NativeStructBuilder},
    };
    use intuicio_data::managed::Managed;

    #[test]
    fn test_async() {
        fn is_async<T: Send + Sync>() {}

        is_async::<VmScope<()>>();
        is_async::<VmScopeFuture<()>>();
        is_async::<VmScopeFutureContext>();
    }

    #[test]
    fn test_vm_scope() {
        let i32_handle = NativeStructBuilder::new::<i32>()
            .build()
            .into_type()
            .into_handle();
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(Function::new(
            FunctionSignature::new("add")
                .with_input(FunctionParameter::new("a", i32_handle.clone()))
                .with_input(FunctionParameter::new("b", i32_handle.clone()))
                .with_output(FunctionParameter::new("result", i32_handle.clone())),
            FunctionBody::closure(|context, _| {
                let a = context.stack().pop::<i32>().unwrap();
                let b = context.stack().pop::<i32>().unwrap();
                context.stack().push(a + b);
            }),
        ));
        registry.add_function(
            VmScope::<()>::generate_function(
                &ScriptFunction {
                    signature: ScriptFunctionSignature {
                        meta: None,
                        name: "add_script".to_owned(),
                        module_name: None,
                        type_query: None,
                        visibility: Visibility::Public,
                        inputs: vec![
                            ScriptFunctionParameter {
                                meta: None,
                                name: "a".to_owned(),
                                type_query: TypeQuery::of::<i32>(),
                            },
                            ScriptFunctionParameter {
                                meta: None,
                                name: "b".to_owned(),
                                type_query: TypeQuery::of::<i32>(),
                            },
                        ],
                        outputs: vec![ScriptFunctionParameter {
                            meta: None,
                            name: "result".to_owned(),
                            type_query: TypeQuery::of::<i32>(),
                        }],
                    },
                    script: ScriptBuilder::<()>::default()
                        .define_register(TypeQuery::of::<i32>())
                        .pop_to_register(0)
                        .push_from_register(0)
                        .call_function(FunctionQuery {
                            name: Some("add".into()),
                            ..Default::default()
                        })
                        .build(),
                },
                &registry,
                None,
            )
            .unwrap()
            .0,
        );
        registry.add_type_handle(i32_handle);
        let mut context = Context::new(10240, 10240);
        let (result,) = registry
            .find_function(FunctionQuery {
                name: Some("add".into()),
                ..Default::default()
            })
            .unwrap()
            .call::<(i32,), _>(&mut context, &registry, (40, 2), true);
        assert_eq!(result, 42);
        assert_eq!(context.stack().position(), 0);
        assert_eq!(context.registers().position(), 0);
        let (result,) = registry
            .find_function(FunctionQuery {
                name: Some("add_script".into()),
                ..Default::default()
            })
            .unwrap()
            .call::<(i32,), _>(&mut context, &registry, (40, 2), true);
        assert_eq!(result, 42);
        assert_eq!(context.stack().position(), 0);
        assert_eq!(context.registers().position(), 0);
    }

    #[test]
    fn test_vm_scope_future() {
        enum Expression {
            Literal(i32),
            Increment,
        }

        impl ScriptExpression for Expression {
            fn evaluate(&self, context: &mut Context, _registry: &Registry) {
                match self {
                    Expression::Literal(value) => {
                        context.stack().push(*value);
                    }
                    Expression::Increment => {
                        let value = context.stack().pop::<i32>().unwrap();
                        context.stack().push(value + 1);
                    }
                }
            }
        }

        let mut context = Managed::new(Context::new(10240, 10240));
        let registry = RegistryHandle::default();

        let script = ScriptBuilder::<Expression>::default()
            .expression(Expression::Literal(42))
            .suspend()
            .expression(Expression::Increment)
            .build();
        let scope = VmScope::new(script, VmScopeSymbol::new());
        let mut future = VmScopeFuture::new(scope, context.lazy(), registry);
        let mut future = std::pin::Pin::new(&mut future);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        assert_eq!(context.write().unwrap().stack().position(), 0);

        assert_eq!(future.as_mut().poll(&mut cx), std::task::Poll::Pending);
        assert_eq!(
            context.write().unwrap().stack().position(),
            if cfg!(feature = "typehash_debug_name") {
                28
            } else {
                12
            }
        );
        assert_eq!(context.write().unwrap().stack().pop::<i32>().unwrap(), 42);
        context.write().unwrap().stack().push(1);

        assert_eq!(future.as_mut().poll(&mut cx), std::task::Poll::Ready(()));
        assert_eq!(context.write().unwrap().stack().pop::<i32>().unwrap(), 2);
    }
}

use crate::debugger::VmDebuggerHandle;
use intuicio_core::{
    context::Context,
    function::FunctionBody,
    registry::Registry,
    script::{ScriptExpression, ScriptFunctionGenerator, ScriptHandle, ScriptOperation},
};
use typid::ID;

pub type VmScopeSymbol = ID<()>;

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

    pub fn with_debugger(mut self, debugger: Option<VmDebuggerHandle<SE>>) -> Self {
        self.debugger = debugger;
        self
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

    pub fn run(&mut self, context: &mut Context, registry: &Registry) {
        while self.step(context, registry) {}
    }

    pub fn step(&mut self, context: &mut Context, registry: &Registry) -> bool {
        if let Some(child) = &mut self.child {
            if child.step(context, registry) {
                return true;
            } else {
                self.child = None;
            }
        }
        if self.position == 0 {
            if let Some(debugger) = self.debugger.as_ref() {
                if let Ok(mut debugger) = debugger.try_write() {
                    debugger.on_enter_scope(self, context, registry);
                }
            }
        }
        let result = if let Some(operation) = self.handle.get(self.position) {
            if let Some(debugger) = self.debugger.as_ref() {
                if let Ok(mut debugger) = debugger.try_write() {
                    debugger.on_enter_operation(self, operation, self.position, context, registry);
                }
            }
            let position = self.position;
            let result = match operation {
                ScriptOperation::None => {
                    self.position += 1;
                    true
                }
                ScriptOperation::Expression { expression } => {
                    expression.evaluate(context, registry);
                    self.position += 1;
                    true
                }
                ScriptOperation::DefineRegister { query } => {
                    let handle = registry
                        .structs()
                        .find(|handle| query.is_valid(handle))
                        .unwrap_or_else(|| {
                            panic!(
                                "Could not define register for non-existent struct: {:#?}",
                                query
                            )
                        });
                    unsafe {
                        context
                            .registers()
                            .push_register_raw(handle.type_id(), *handle.layout())
                    };
                    self.position += 1;
                    true
                }
                ScriptOperation::DropRegister { index } => {
                    let index = context.absolute_register_index(*index);
                    context
                        .registers()
                        .access_register(index)
                        .unwrap_or_else(|| {
                            panic!("Could not access non-existent register: {}", index)
                        })
                        .free();
                    self.position += 1;
                    true
                }
                ScriptOperation::PushFromRegister { index } => {
                    let index = context.absolute_register_index(*index);
                    let (stack, registers) = context.stack_and_registers();
                    let mut register = registers.access_register(index).unwrap_or_else(|| {
                        panic!("Could not access non-existent register: {}", index)
                    });
                    if !stack.push_from_register(&mut register) {
                        panic!("Could not push data from register: {}", index);
                    }
                    self.position += 1;
                    true
                }
                ScriptOperation::PopToRegister { index } => {
                    let index = context.absolute_register_index(*index);
                    let (stack, registers) = context.stack_and_registers();
                    let mut register = registers.access_register(index).unwrap_or_else(|| {
                        panic!("Could not access non-existent register: {}", index)
                    });
                    if !stack.pop_to_register(&mut register) {
                        panic!("Could not pop data to register: {}", index);
                    }
                    self.position += 1;
                    true
                }
                ScriptOperation::MoveRegister { from, to } => {
                    let from = context.absolute_register_index(*from);
                    let to = context.absolute_register_index(*to);
                    let (mut source, mut target) = context
                        .registers()
                        .access_registers_pair(from, to)
                        .unwrap_or_else(|| {
                            panic!(
                                "Could not access non-existent registers pair: {} and {}",
                                from, to
                            )
                        });
                    source.move_to(&mut target);
                    self.position += 1;
                    true
                }
                ScriptOperation::CallFunction { query } => {
                    let handle = registry
                        .functions()
                        .find(|handle| query.is_valid(handle.signature()))
                        .unwrap_or_else(|| {
                            panic!("Could not call non-existent function: {:#?}", query)
                        });
                    handle.invoke(context, registry);
                    self.position += 1;
                    true
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
                    true
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
                    true
                }
                ScriptOperation::PushScope { scope } => {
                    context.store_registers();
                    self.child = Some(Box::new(
                        Self::new(scope.clone(), self.symbol).with_debugger(self.debugger.clone()),
                    ));
                    self.position += 1;
                    true
                }
                ScriptOperation::PopScope => {
                    context.restore_registers();
                    self.position = self.handle.len();
                    false
                }
                ScriptOperation::ContinueScopeConditionally => {
                    let result = context.stack().pop::<bool>().unwrap();
                    if result {
                        self.position += 1;
                    } else {
                        self.position = self.handle.len();
                    }
                    result
                }
            };
            if let Some(debugger) = self.debugger.as_ref() {
                if let Ok(mut debugger) = debugger.try_write() {
                    debugger.on_exit_operation(self, operation, position, context, registry);
                }
            }
            result
        } else {
            false
        };
        if !result || self.position >= self.handle.len() {
            if let Some(debugger) = self.debugger.as_ref() {
                if let Ok(mut debugger) = debugger.try_write() {
                    debugger.on_exit_scope(self, context, registry);
                }
            }
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

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use crate::scope::*;
    use intuicio_core::prelude::*;

    #[test]
    fn test_vm_scope() {
        let i32_handle = NativeStructBuilder::new::<i32>().build_handle();
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
                        name: "add_script".to_owned(),
                        module_name: None,
                        struct_query: None,
                        visibility: Visibility::Public,
                        inputs: vec![
                            ScriptFunctionParameter {
                                name: "a".to_owned(),
                                struct_query: StructQuery {
                                    type_id: Some(TypeId::of::<i32>()),
                                    ..Default::default()
                                },
                            },
                            ScriptFunctionParameter {
                                name: "b".to_owned(),
                                struct_query: StructQuery {
                                    type_id: Some(TypeId::of::<i32>()),
                                    ..Default::default()
                                },
                            },
                        ],
                        outputs: vec![ScriptFunctionParameter {
                            name: "result".to_owned(),
                            struct_query: StructQuery {
                                type_id: Some(TypeId::of::<i32>()),
                                ..Default::default()
                            },
                        }],
                    },
                    script: ScriptBuilder::<()>::default()
                        .define_register(StructQuery::of::<i32>())
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
        registry.add_struct_handle(i32_handle);
        let mut context = Context::new(1024, 1024, 1024);
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
}

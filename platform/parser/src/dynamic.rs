use crate::ParserHandle;
use intuicio_core::{
    context::Context,
    function::{Function, FunctionHandle, FunctionQuery},
    host::Host,
    registry::{Registry, RegistryHandle},
    types::struct_type::NativeStructBuilder,
};
use intuicio_data::{
    lifetime::Lifetime,
    managed::{DynamicManaged, DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut},
    managed_gc::DynamicManagedGc,
};
use std::sync::{Arc, RwLock, RwLockWriteGuard};

pub mod shorthand {
    use super::*;
    use crate::{
        pratt::{PrattParserAssociativity, PrattParserRule},
        shorthand::{inspect, map_err, omap, pratt},
    };

    pub fn dyn_inspect(parser: ParserHandle, function_name: impl ToString) -> ParserHandle {
        let function_name = function_name.to_string();
        dynamic_extension(move |extension| {
            let function_name = function_name.clone();
            inspect(parser.clone(), move |value| {
                extension
                    .call(&function_name)
                    .unwrap()
                    .arg(value.borrow().unwrap())
                    .call_no_return();
            })
        })
    }

    pub fn dyn_map(parser: ParserHandle, function_name: impl ToString) -> ParserHandle {
        let function_name = function_name.to_string();
        dynamic_extension(move |extension| {
            let function_name = function_name.clone();
            omap(parser.clone(), move |value| {
                extension
                    .call(&function_name)
                    .unwrap()
                    .arg(value)
                    .call_return()
            })
        })
    }

    pub fn dyn_map_err(parser: ParserHandle, function_name: impl ToString) -> ParserHandle {
        let function_name = function_name.to_string();
        dynamic_extension(move |extension| {
            let function_name = function_name.clone();
            map_err(parser.clone(), move |error| {
                extension
                    .call(&function_name)
                    .unwrap()
                    .arg_owned(error)
                    .call_return()
                    .consume()
                    .ok()
                    .unwrap()
            })
        })
    }

    #[derive(Debug, Clone)]
    pub enum DynamicPrattParserRule {
        Prefix {
            operator_function_name: String,
            transformer_function_name: String,
        },
        PrefixOp {
            operator: String,
            transformer_function_name: String,
        },
        Postfix {
            operator_function_name: String,
            transformer_function_name: String,
        },
        PostfixOp {
            operator: String,
            transformer_function_name: String,
        },
        Infix {
            operator_function_name: String,
            transformer_function_name: String,
            associativity: PrattParserAssociativity,
        },
        InfixOp {
            operator: String,
            transformer_function_name: String,
            associativity: PrattParserAssociativity,
        },
    }

    pub fn dyn_pratt(
        tokenizer_parser: ParserHandle,
        rules: Vec<Vec<DynamicPrattParserRule>>,
    ) -> ParserHandle {
        dynamic_extension(move |extension| {
            let rules = rules
                .clone()
                .into_iter()
                .map(move |rules| {
                    rules
                        .into_iter()
                        .map(|rule| match rule {
                            DynamicPrattParserRule::Prefix {
                                operator_function_name,
                                transformer_function_name,
                            } => {
                                let extension_o = extension.clone();
                                let extension_t = extension.clone();
                                PrattParserRule::prefx_raw(
                                    move |operator| {
                                        extension_o
                                            .call(&operator_function_name)
                                            .unwrap()
                                            .arg(operator.borrow().unwrap())
                                            .call_return()
                                            .consume()
                                            .ok()
                                            .unwrap()
                                    },
                                    move |value| {
                                        extension_t
                                            .call(&transformer_function_name)
                                            .unwrap()
                                            .arg(value)
                                            .call_return()
                                    },
                                )
                            }
                            DynamicPrattParserRule::PrefixOp {
                                operator,
                                transformer_function_name,
                            } => {
                                let extension_t = extension.clone();
                                PrattParserRule::prefx_raw(
                                    move |token| {
                                        token
                                            .read::<String>()
                                            .map(|op| *op == operator)
                                            .unwrap_or_default()
                                    },
                                    move |value| {
                                        extension_t
                                            .call(&transformer_function_name)
                                            .unwrap()
                                            .arg(value)
                                            .call_return()
                                    },
                                )
                            }
                            DynamicPrattParserRule::Postfix {
                                operator_function_name,
                                transformer_function_name,
                            } => {
                                let extension_o = extension.clone();
                                let extension_t = extension.clone();
                                PrattParserRule::postfix_raw(
                                    move |operator| {
                                        extension_o
                                            .call(&operator_function_name)
                                            .unwrap()
                                            .arg(operator.borrow().unwrap())
                                            .call_return()
                                            .consume()
                                            .ok()
                                            .unwrap()
                                    },
                                    move |value| {
                                        extension_t
                                            .call(&transformer_function_name)
                                            .unwrap()
                                            .arg(value)
                                            .call_return()
                                    },
                                )
                            }
                            DynamicPrattParserRule::PostfixOp {
                                operator,
                                transformer_function_name,
                            } => {
                                let extension_t = extension.clone();
                                PrattParserRule::postfix_raw(
                                    move |token| {
                                        token
                                            .read::<String>()
                                            .map(|op| *op == operator)
                                            .unwrap_or_default()
                                    },
                                    move |value| {
                                        extension_t
                                            .call(&transformer_function_name)
                                            .unwrap()
                                            .arg(value)
                                            .call_return()
                                    },
                                )
                            }
                            DynamicPrattParserRule::Infix {
                                operator_function_name,
                                transformer_function_name,
                                associativity,
                            } => {
                                let extension_o = extension.clone();
                                let extension_t = extension.clone();
                                PrattParserRule::infix_raw(
                                    move |operator| {
                                        extension_o
                                            .call(&operator_function_name)
                                            .unwrap()
                                            .arg(operator.borrow().unwrap())
                                            .call_return()
                                            .consume()
                                            .ok()
                                            .unwrap()
                                    },
                                    move |lhs, rhs| {
                                        extension_t
                                            .call(&transformer_function_name)
                                            .unwrap()
                                            .arg(lhs)
                                            .arg(rhs)
                                            .call_return()
                                    },
                                    associativity,
                                )
                            }
                            DynamicPrattParserRule::InfixOp {
                                operator,
                                transformer_function_name,
                                associativity,
                            } => {
                                let extension_t = extension.clone();
                                PrattParserRule::infix_raw(
                                    move |token| {
                                        token
                                            .read::<String>()
                                            .map(|op| *op == operator)
                                            .unwrap_or_default()
                                    },
                                    move |lhs, rhs| {
                                        extension_t
                                            .call(&transformer_function_name)
                                            .unwrap()
                                            .arg(lhs)
                                            .arg(rhs)
                                            .call_return()
                                    },
                                    associativity,
                                )
                            }
                        })
                        .collect()
                })
                .collect();
            pratt(tokenizer_parser.clone(), rules)
        })
    }
}

pub struct DynamicExtensionBuilder {
    registry: Registry,
}

impl Default for DynamicExtensionBuilder {
    fn default() -> Self {
        Self {
            registry: Registry::default()
                .with_type(
                    NativeStructBuilder::new_named_uninitialized::<DynamicManaged>(
                        "DynamicManaged",
                    )
                    .build(),
                )
                .with_type(
                    NativeStructBuilder::new_named_uninitialized::<DynamicManagedRef>(
                        "DynamicManagedRef",
                    )
                    .build(),
                )
                .with_type(
                    NativeStructBuilder::new_named_uninitialized::<DynamicManagedRefMut>(
                        "DynamicManagedRefMut",
                    )
                    .build(),
                )
                .with_type(
                    NativeStructBuilder::new_named_uninitialized::<DynamicManagedLazy>(
                        "DynamicManagedLazy",
                    )
                    .build(),
                )
                .with_type(
                    NativeStructBuilder::new_named_uninitialized::<DynamicManagedGc>(
                        "DynamicManagedGc",
                    )
                    .build(),
                ),
        }
    }
}

impl DynamicExtensionBuilder {
    pub fn with(mut self, f: impl FnOnce(&Registry) -> Function) -> Self {
        self.add(f);
        self
    }

    pub fn add(&mut self, f: impl FnOnce(&Registry) -> Function) {
        self.registry.add_function(f(&self.registry));
    }

    pub fn build(self) -> DynamicExtension {
        DynamicExtension {
            host: Arc::new(RwLock::new(Host::new(
                Context::new(10240, 10240),
                RegistryHandle::new(self.registry),
            ))),
        }
    }
}

pub struct DynamicExtension {
    host: Arc<RwLock<Host>>,
}

impl DynamicExtension {
    pub fn call<'a>(&'a self, name: &str) -> Option<DynamicExtensionCall<'a>> {
        let host = self.host.write().ok()?;
        let handle = host.registry().find_function(FunctionQuery {
            name: Some(name.into()),
            ..Default::default()
        })?;
        Some(DynamicExtensionCall {
            host,
            handle,
            args: vec![],
            lifetimes: vec![],
        })
    }
}

pub enum Value {
    Owned(DynamicManaged),
    Ref(DynamicManagedRef),
    RefMut(DynamicManagedRefMut),
    Lazy(DynamicManagedLazy),
    Gc(DynamicManagedGc),
}

impl From<DynamicManaged> for Value {
    fn from(value: DynamicManaged) -> Self {
        Self::Owned(value)
    }
}

impl From<DynamicManagedRef> for Value {
    fn from(value: DynamicManagedRef) -> Self {
        Self::Ref(value)
    }
}

impl From<DynamicManagedRefMut> for Value {
    fn from(value: DynamicManagedRefMut) -> Self {
        Self::RefMut(value)
    }
}

impl From<DynamicManagedLazy> for Value {
    fn from(value: DynamicManagedLazy) -> Self {
        Self::Lazy(value)
    }
}

impl From<DynamicManagedGc> for Value {
    fn from(value: DynamicManagedGc) -> Self {
        Self::Gc(value)
    }
}

pub struct DynamicExtensionCall<'a> {
    host: RwLockWriteGuard<'a, Host>,
    handle: FunctionHandle,
    args: Vec<Value>,
    lifetimes: Vec<Lifetime>,
}

impl DynamicExtensionCall<'_> {
    pub fn arg(mut self, value: impl Into<Value>) -> Self {
        self.args.push(value.into());
        self
    }

    pub fn arg_owned<T>(mut self, value: T) -> Self {
        let value = DynamicManaged::new(value).ok().unwrap();
        self.args.push(Value::Owned(value));
        self
    }

    pub fn arg_ref<T>(mut self, value: &T) -> Self {
        let lifetime = Lifetime::default();
        let value = DynamicManagedRef::new(value, lifetime.borrow().unwrap());
        self.args.push(Value::Ref(value));
        self.lifetimes.push(lifetime);
        self
    }

    pub fn arg_ref_mut<T>(mut self, value: &mut T) -> Self {
        let lifetime = Lifetime::default();
        let value = DynamicManagedRefMut::new(value, lifetime.borrow_mut().unwrap());
        self.args.push(Value::RefMut(value));
        self.lifetimes.push(lifetime);
        self
    }

    pub fn arg_lazy<T>(mut self, value: &mut T) -> Self {
        let lifetime = Lifetime::default();
        let value = DynamicManagedLazy::new(value, lifetime.lazy());
        self.args.push(Value::Lazy(value));
        self.lifetimes.push(lifetime);
        self
    }

    pub fn arg_gc<T>(mut self, value: T) -> Self {
        let value = DynamicManagedGc::new(value);
        self.args.push(Value::Gc(value));
        self
    }

    pub fn call_return(mut self) -> DynamicManaged {
        let (context, registry) = self.host.context_and_registry();
        for arg in self.args.into_iter().rev() {
            match arg {
                Value::Owned(value) => context.stack().push(value),
                Value::Ref(value) => context.stack().push(value),
                Value::RefMut(value) => context.stack().push(value),
                Value::Lazy(value) => context.stack().push(value),
                Value::Gc(value) => context.stack().push(value),
            };
        }
        self.handle.invoke(context, registry);
        context.stack().pop::<DynamicManaged>().unwrap()
    }

    pub fn call_no_return(mut self) {
        let (context, registry) = self.host.context_and_registry();
        for arg in self.args.into_iter().rev() {
            match arg {
                Value::Owned(value) => context.stack().push(value),
                Value::Ref(value) => context.stack().push(value),
                Value::RefMut(value) => context.stack().push(value),
                Value::Lazy(value) => context.stack().push(value),
                Value::Gc(value) => context.stack().push(value),
            };
        }
        self.handle.invoke(context, registry);
    }
}

pub fn dynamic_extension(
    f: impl Fn(Arc<DynamicExtension>) -> ParserHandle + Send + Sync + 'static,
) -> ParserHandle {
    crate::shorthand::ext::<DynamicExtension>(f)
}

#[cfg(test)]
mod tests {
    use super::{DynamicExtensionBuilder, dynamic_extension};
    use crate::{
        ParserRegistry,
        shorthand::{map, number_float},
    };
    use intuicio_core::transformer::{DynamicManagedValueTransformer, ValueTransformer};
    use intuicio_derive::intuicio_function;

    #[intuicio_function(transformer = "DynamicManagedValueTransformer")]
    fn foo(value: String) -> f32 {
        value.parse().unwrap()
    }

    #[test]
    fn test_dynamic_extension() {
        let extension = DynamicExtensionBuilder::default()
            .with(foo::define_function)
            .build();
        let registry = ParserRegistry::default().with_extension(extension);
        let parser = dynamic_extension(|extension| {
            map::<String, f32>(number_float(), move |v| {
                extension
                    .call("foo")
                    .unwrap()
                    .arg_owned(v)
                    .call_return()
                    .consume()
                    .ok()
                    .unwrap()
            })
        });
        let (rest, result) = parser.parse(&registry, "42.0").unwrap();
        assert_eq!(rest, "");
        assert_eq!(result.consume::<f32>().ok().unwrap(), 42.0);
    }
}

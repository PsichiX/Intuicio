//! Grammar rules whose callbacks are Intuicio functions.
//!
//! The combinators that take a closure - map, inspect, the Pratt rules -
//! need Rust code, which a grammar loaded at run time cannot supply. The
//! parsers here take a function name instead and call it in a
//! [`DynamicExtension`], a small [`Host`] kept in the parser registry.
//!
//! This is what lets [`generator`](crate::generator) build a complete
//! frontend out of text: the grammar names the functions, and the host
//! holds them.
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
    managed::{
        DynamicManaged, DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut,
        gc::DynamicManagedGc,
    },
};
use std::sync::{Arc, RwLock, RwLockWriteGuard};

/// Short constructors for this module.
pub mod shorthand {
    use super::*;
    use crate::{
        pratt::{PrattParserAssociativity, PrattParserRule},
        shorthand::{inspect, map_err, omap, pratt},
    };

    /// [`inspect`](crate::inspect) that calls the named function with a
    /// reference to each output.
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

    /// [`omap`] that replaces each output with
    /// what the named function returns.
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

    /// [`map_err`] that replaces each error
    /// with what the named function returns.
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
    /// A [`PrattParserRule`] whose parts are
    /// named functions.
    ///
    /// The `Op` variants take the operator as plain text instead of a function,
    /// for the common case where recognising it is just a string comparison.
    pub enum DynamicPrattParserRule {
        /// An operator before its operand, recognised by a function.
        Prefix {
            operator_function_name: String,
            transformer_function_name: String,
        },
        /// An operator before its operand, recognised by its text.
        PrefixOp {
            operator: String,
            transformer_function_name: String,
        },
        /// An operator after its operand, recognised by a function.
        Postfix {
            operator_function_name: String,
            transformer_function_name: String,
        },
        /// An operator after its operand, recognised by its text.
        PostfixOp {
            operator: String,
            transformer_function_name: String,
        },
        /// An operator between two operands, recognised by a function.
        Infix {
            operator_function_name: String,
            transformer_function_name: String,
            associativity: PrattParserAssociativity,
        },
        /// An operator between two operands, recognised by its text.
        InfixOp {
            operator: String,
            transformer_function_name: String,
            associativity: PrattParserAssociativity,
        },
    }

    /// [`pratt`] with rules that call named
    /// functions. Each inner `Vec` is one precedence level, weakest first.
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
                                PrattParserRule::prefix_raw(
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
                                PrattParserRule::prefix_raw(
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

/// Collects the functions a grammar may call.
///
/// Starts with the managed box types already registered, since those are
/// what parser outputs travel as.
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
    /// [`DynamicExtensionBuilder::add`], builder style.
    pub fn with(mut self, f: impl FnOnce(&Registry) -> Function) -> Self {
        self.add(f);
        self
    }

    /// Adds a function, built against the registry as it stands.
    pub fn add(&mut self, f: impl FnOnce(&Registry) -> Function) {
        self.registry.add_function(f(&self.registry));
    }

    /// Wraps everything into a [`DynamicExtension`] with its own context.
    pub fn build(self) -> DynamicExtension {
        DynamicExtension {
            host: Arc::new(RwLock::new(Host::new(
                Context::new(10240, 10240),
                RegistryHandle::new(self.registry),
            ))),
        }
    }
}

/// A host that grammar callbacks are invoked in.
///
/// Add it to a [`ParserRegistry`](crate::ParserRegistry) with
/// `with_extension`, and the parsers in this module will find it. One lock
/// guards the host, so callbacks run one at a time.
pub struct DynamicExtension {
    host: Arc<RwLock<Host>>,
}

impl DynamicExtension {
    /// Starts a call to the function registered under `name`.
    ///
    /// Returns [`None`] when there is no such function, or while the host is
    /// busy with another call.
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

/// One argument of a call, in whichever box it travels as.
pub enum Value {
    /// The call takes the value.
    Owned(DynamicManaged),
    /// The call may read the value.
    Ref(DynamicManagedRef),
    /// The call may write to the value.
    RefMut(DynamicManagedRefMut),
    /// The call claims access only when it looks.
    Lazy(DynamicManagedLazy),
    /// The call shares ownership of the value.
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

/// A call being built up, argument by argument.
///
/// Holds the host locked until it is finished with `call_return` or
/// `call_no_return`.
pub struct DynamicExtensionCall<'a> {
    host: RwLockWriteGuard<'a, Host>,
    handle: FunctionHandle,
    args: Vec<Value>,
    lifetimes: Vec<Lifetime>,
}

impl DynamicExtensionCall<'_> {
    /// Appends an argument that is already in a box.
    pub fn arg(mut self, value: impl Into<Value>) -> Self {
        self.args.push(value.into());
        self
    }

    /// Appends `value`, giving it away.
    pub fn arg_owned<T>(mut self, value: T) -> Self {
        let value = DynamicManaged::new(value).ok().unwrap();
        self.args.push(Value::Owned(value));
        self
    }

    /// Appends a read-only reference to `value`, valid for this call only.
    pub fn arg_ref<T>(mut self, value: &T) -> Self {
        let lifetime = Lifetime::default();
        let value = DynamicManagedRef::new(value, lifetime.borrow().unwrap());
        self.args.push(Value::Ref(value));
        self.lifetimes.push(lifetime);
        self
    }

    /// Appends a writable reference to `value`, valid for this call only.
    pub fn arg_ref_mut<T>(mut self, value: &mut T) -> Self {
        let lifetime = Lifetime::default();
        let value = DynamicManagedRefMut::new(value, lifetime.borrow_mut().unwrap());
        self.args.push(Value::RefMut(value));
        self.lifetimes.push(lifetime);
        self
    }

    /// Appends a reference to `value` that claims nothing until it is used.
    pub fn arg_lazy<T>(mut self, value: &mut T) -> Self {
        let lifetime = Lifetime::default();
        let value = DynamicManagedLazy::new(value, lifetime.lazy());
        self.args.push(Value::Lazy(value));
        self.lifetimes.push(lifetime);
        self
    }

    /// Appends `value` as a shared, collected box.
    pub fn arg_gc<T>(mut self, value: T) -> Self {
        let value = DynamicManagedGc::new(value);
        self.args.push(Value::Gc(value));
        self
    }

    /// Runs the function and takes its result off the stack.
    ///
    /// # Panics
    ///
    /// Panics when the function left nothing behind.
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

    /// Runs the function and expects no result.
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

/// Builds a parser from the [`DynamicExtension`] in the registry.
///
/// [`ext`](crate::extension::shorthand::ext) fixed to this crate's
/// extension type.
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

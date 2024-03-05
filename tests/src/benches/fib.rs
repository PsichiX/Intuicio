use crate::{
    benches::{COMPARISON_FORMAT, DURATION},
    black_box, Benchmark,
};
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_frontend_vault::*;
use std::{error::Error, time::Duration};

const FIB_N: usize = 20;

pub fn bench() -> Result<(), Box<dyn Error>> {
    println!();
    println!("--- FIB | BENCHMARKS ---");

    // native
    let native_result = {
        fn fib(n: usize) -> usize {
            match n {
                0 => 0,
                1 => 1,
                n => fib(n - 1) + fib(n - 2),
            }
        }

        println!();
        println!("fib({}) = {}", FIB_N, fib(FIB_N));

        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "native fib",
            || {},
            |_| {
                fib(black_box(FIB_N));
            },
            |_| {},
        )
    };

    // shared lock pointer
    let shared_lock_result = {
        use std::sync::{Arc, RwLock};

        fn fib(n: Arc<RwLock<usize>>) -> Arc<RwLock<usize>> {
            let n = *n.read().unwrap();
            let result = match n {
                0 => 0,
                1 => 1,
                n => {
                    let a = *fib(Arc::new(RwLock::new(n - 1))).read().unwrap();
                    let b = *fib(Arc::new(RwLock::new(n - 2))).read().unwrap();
                    a + b
                }
            };
            Arc::new(RwLock::new(result))
        }

        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "shared lock pointer fib",
            || {},
            |_| {
                fib(Arc::new(RwLock::new(FIB_N)));
            },
            |_| {},
        )
    };

    // host
    let (host_result, host_indexed_result) = {
        fn fib(context: &mut Context, registry: &Registry) {
            let fib = registry
                .find_function(FunctionQuery {
                    name: Some("fib".into()),
                    ..Default::default()
                })
                .unwrap();
            let n = context.stack().pop::<usize>().unwrap();
            let result = match n {
                0 => 0,
                1 => 1,
                n => {
                    context.stack().push(n - 1);
                    fib.invoke(context, registry);
                    let a = context.stack().pop::<usize>().unwrap();
                    context.stack().push(n - 2);
                    fib.invoke(context, registry);
                    let b = context.stack().pop::<usize>().unwrap();
                    a + b
                }
            };
            context.stack().push(result);
        }

        let host_result = {
            println!();
            let mut registry = Registry::default().with_basic_types();
            registry.add_function(Function::new(
                function_signature! {
                    registry => fn fib(n: usize) -> (result: usize)
                },
                FunctionBody::pointer(fib),
            ));
            let mut context = Context::new(10240, 10240);
            Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
                "host fib",
                || {},
                |_| {
                    let fib = registry
                        .find_function(FunctionQuery {
                            name: Some("fib".into()),
                            ..Default::default()
                        })
                        .unwrap();
                    context.stack().push(black_box(FIB_N));
                    fib.invoke(&mut context, &registry);
                    context.stack().pop::<usize>();
                },
                |_| {},
            )
        };
        let host_indexed_result = {
            println!();
            let mut registry = Registry::default()
                .with_basic_types()
                .with_max_index_capacity();
            registry.add_function(Function::new(
                function_signature! {
                    registry => fn fib(n: usize) -> (result: usize)
                },
                FunctionBody::pointer(fib),
            ));
            let mut context = Context::new(10240, 10240);
            Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
                "host indexed fib",
                || {},
                |_| {
                    let fib = registry
                        .find_function(FunctionQuery {
                            name: Some("fib".into()),
                            ..Default::default()
                        })
                        .unwrap();
                    context.stack().push(black_box(FIB_N));
                    fib.invoke(&mut context, &registry);
                    context.stack().pop::<usize>();
                },
                |_| {},
            )
        };
        (host_result, host_indexed_result)
    };

    // vm
    let (vm_result, vm_indexed_result) = {
        fn fib(context: &mut Context, registry: &Registry) {
            let fib = registry
                .find_function(FunctionQuery {
                    name: Some("fib".into()),
                    ..Default::default()
                })
                .unwrap();
            let n = context.stack().pop::<usize>().unwrap();
            let result = match n {
                0 => 0,
                1 => 1,
                n => {
                    context.stack().push(n - 1);
                    fib.invoke(context, registry);
                    let a = context.stack().pop::<usize>().unwrap();
                    context.stack().push(n - 2);
                    fib.invoke(context, registry);
                    let b = context.stack().pop::<usize>().unwrap();
                    a + b
                }
            };
            context.stack().push(result);
        }

        let vm_result = {
            println!();
            let mut registry = Registry::default().with_basic_types();
            registry.add_function(Function::new(
                function_signature! {
                    registry => fn fib(n: usize) -> (result: usize)
                },
                FunctionBody::pointer(fib),
            ));
            let fib = registry.add_function(Function::new(
                function_signature! {
                    registry => fn fib_script(n: usize) -> (result: usize)
                },
                VmScope::<()>::generate_function_body(
                    ScriptBuilder::<()>::default()
                        .call_function(FunctionQuery {
                            name: Some("fib".into()),
                            ..Default::default()
                        })
                        .build(),
                    None,
                )
                .unwrap()
                .0,
            ));
            let mut context = Context::new(10240, 10240);
            Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
                "vm fib",
                || {},
                |_| {
                    fib.call::<(usize,), _>(&mut context, &registry, (black_box(FIB_N),), true);
                },
                |_| {},
            )
        };
        let vm_indexed_result = {
            println!();
            let mut registry = Registry::default()
                .with_basic_types()
                .with_max_index_capacity();
            registry.add_function(Function::new(
                function_signature! {
                    registry => fn fib(n: usize) -> (result: usize)
                },
                FunctionBody::pointer(fib),
            ));
            let fib = registry.add_function(Function::new(
                function_signature! {
                    registry => fn fib_script(n: usize) -> (result: usize)
                },
                VmScope::<()>::generate_function_body(
                    ScriptBuilder::<()>::default()
                        .call_function(FunctionQuery {
                            name: Some("fib".into()),
                            ..Default::default()
                        })
                        .build(),
                    None,
                )
                .unwrap()
                .0,
            ));
            let mut context = Context::new(10240, 10240);
            Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
                "vm indexed fib",
                || {},
                |_| {
                    fib.call::<(usize,), _>(&mut context, &registry, (black_box(FIB_N),), true);
                },
                |_| {},
            )
        };
        (vm_result, vm_indexed_result)
    };

    // script
    let script_result = {
        println!();
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn add(a: usize, b: usize) -> usize {
                a + b
            }
        });
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn sub(a: usize, b: usize) -> usize {
                a - b
            }
        });
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn less_than(a: usize, b: usize) -> bool {
                a < b
            }
        });
        registry.add_function(define_function! {
            registry => mod intrinsics struct (usize) fn clone(this: usize) -> (original: usize, clone: usize) {
                (this, this)
            }
        });
        let mut content_provider = FileContentProvider::new("vault", VaultContentParser);
        VaultPackage::new("../resources/package.vault", &mut content_provider)
            .unwrap()
            .compile()
            .install::<VmScope<VaultScriptExpression>>(&mut registry, None);
        let fib = registry
            .find_function(FunctionQuery {
                name: Some("fib".into()),
                module_name: Some("test".into()),
                ..Default::default()
            })
            .unwrap();
        let mut context = Context::new(10240, 10240);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "script fib",
            || {},
            |_| {
                fib.call::<(usize,), _>(&mut context, &registry, (black_box(FIB_N),), true);
            },
            |_| {},
        )
    };
    let script_indexed_result = {
        println!();
        let mut registry = Registry::default()
            .with_basic_types()
            .with_max_index_capacity();
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn add(a: usize, b: usize) -> usize {
                a + b
            }
        });
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn sub(a: usize, b: usize) -> usize {
                a - b
            }
        });
        registry.add_function(define_vault_function! {
            registry => mod intrinsics fn less_than(a: usize, b: usize) -> bool {
                a < b
            }
        });
        registry.add_function(define_function! {
            registry => mod intrinsics struct (usize) fn clone(this: usize) -> (original: usize, clone: usize) {
                (this, this)
            }
        });
        let mut content_provider = FileContentProvider::new("vault", VaultContentParser);
        VaultPackage::new("../resources/package.vault", &mut content_provider)
            .unwrap()
            .compile()
            .install::<VmScope<VaultScriptExpression>>(&mut registry, None);
        let fib = registry
            .find_function(FunctionQuery {
                name: Some("fib".into()),
                module_name: Some("test".into()),
                ..Default::default()
            })
            .unwrap();
        let mut context = Context::new(10240, 10240);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "script indexed fib",
            || {},
            |_| {
                fib.call::<(usize,), _>(&mut context, &registry, (black_box(FIB_N),), true);
            },
            |_| {},
        )
    };

    // rune
    let rune_result = {
        use rune::{
            termcolor::{ColorChoice, StandardStream},
            {Diagnostics, Vm},
        };
        use std::sync::Arc;

        println!();
        let context = rune_modules::default_context().unwrap();
        let mut sources = rune::sources!(
            entry => {
                pub fn rune_fib(n) {
                    match n {
                        0 => 0,
                        1 => 1,
                        n => rune_fib(n - 1) + rune_fib(n - 2),
                    }
                }
            }
        );
        let mut diagnostics = Diagnostics::new();
        let result = rune::prepare(&mut sources)
            .with_context(&context)
            .with_diagnostics(&mut diagnostics)
            .build()
            .unwrap();
        if !diagnostics.is_empty() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, &sources).unwrap();
        }
        let mut vm = Vm::new(Arc::new(context.runtime().unwrap()), Arc::new(result));
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "rune fib",
            || {},
            |_| {
                vm.execute(["rune_fib"], (black_box(FIB_N),))
                    .unwrap()
                    .complete()
                    .unwrap();
            },
            |_| {},
        )
    };

    // rhai
    let rhai_result = {
        use rhai::{Engine, Scope};

        println!();
        let engine = Engine::new();
        let ast = engine
            .compile(
                r#"fn rhai_fib(n) { if n < 2 { n } else { rhai_fib(n - 1) + rhai_fib(n - 2) } }"#,
            )
            .unwrap();
        let mut scope = Scope::new();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "rhai fib",
            || {},
            |_| {
                engine
                    .call_fn::<i64>(&mut scope, &ast, "rhai_fib", (black_box(FIB_N as i64),))
                    .unwrap();
            },
            |_| {},
        )
    };

    println!();
    println!("--- FIB | RESULTS ---");

    println!();
    println!("= Host vs Native:");
    host_result.print_comparison(&native_result, COMPARISON_FORMAT);

    println!();
    println!("= Host vs Shared Lock:");
    host_result.print_comparison(&shared_lock_result, COMPARISON_FORMAT);

    println!();
    println!("= Vm vs Host:");
    vm_result.print_comparison(&host_result, COMPARISON_FORMAT);

    println!();
    println!("= Script vs Vm:");
    script_result.print_comparison(&vm_result, COMPARISON_FORMAT);

    println!();
    println!("= Script vs Host:");
    script_result.print_comparison(&host_result, COMPARISON_FORMAT);

    println!();
    println!("= Script vs Rune:");
    script_result.print_comparison(&rune_result, COMPARISON_FORMAT);

    println!();
    println!("= Script vs Rhai:");
    script_result.print_comparison(&rhai_result, COMPARISON_FORMAT);

    println!();
    println!("= Host vs Host Indexed:");
    host_result.print_comparison(&host_indexed_result, COMPARISON_FORMAT);

    println!();
    println!("= Vm vs Vm Indexed:");
    vm_result.print_comparison(&vm_indexed_result, COMPARISON_FORMAT);

    println!();
    println!("= Script vs Script Indexed:");
    script_result.print_comparison(&script_indexed_result, COMPARISON_FORMAT);

    Ok(())
}

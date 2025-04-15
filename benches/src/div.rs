use crate::{Benchmark, COMPARISON_FORMAT, DURATION, black_box};
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_frontend_vault::*;
use std::{error::Error, time::Duration};

const DIV_A: f64 = 2.0;
const DIV_B: f64 = 40.0;

pub fn bench() -> Result<(), Box<dyn Error>> {
    println!();
    println!("--- DIV | BENCHMARKS ---");

    // native
    let native_result = {
        fn div(a: f64, b: f64) -> f64 {
            a / b
        }

        println!();
        println!("div({}, {}) = {}", DIV_A, DIV_B, div(DIV_A, DIV_B));

        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "native div",
            || {},
            |_| {
                div(black_box(DIV_A), black_box(DIV_B));
            },
            |_| {},
        )
    };

    // shared lock pointer
    let shared_lock_result = {
        use std::sync::{Arc, RwLock};

        fn div(a: Arc<RwLock<f64>>, b: Arc<RwLock<f64>>) -> Arc<RwLock<f64>> {
            let a = *a.read().unwrap();
            let b = *b.read().unwrap();
            Arc::new(RwLock::new(a / b))
        }

        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "shared lock pointer div",
            || {},
            |_| {
                div(Arc::new(RwLock::new(DIV_A)), Arc::new(RwLock::new(DIV_B)));
            },
            |_| {},
        )
    };

    // host
    let host_result = {
        println!();
        let mut registry = Registry::default().with_basic_types();
        let div = registry.add_function(define_function! {
            registry => fn div(a: f64, b: f64) -> (result: f64) {
                (a / b,)
            }
        });
        let mut context = Context::new(10240, 10240);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "host div",
            || {},
            |_| {
                context.stack().push(black_box(DIV_B));
                context.stack().push(black_box(DIV_A));
                div.invoke(&mut context, &registry);
                context.stack().pop::<f64>();
            },
            |_| {},
        )
    };

    // vm
    let vm_result = {
        println!();
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(define_function! {
            registry => fn div(a: f64, b: f64) -> (result: f64) {
                (a / b,)
            }
        });
        let div = registry.add_function(Function::new(
            function_signature! {
                registry => fn div_script(a: f64, b: f64) -> (result: f64)
            },
            VmScope::<()>::generate_function_body(
                ScriptBuilder::<()>::default()
                    .call_function(FunctionQuery {
                        name: Some("div".into()),
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
            "vm div",
            || {},
            |_| {
                div.call::<(f64,), _>(
                    &mut context,
                    &registry,
                    (black_box(DIV_A), black_box(DIV_B)),
                    true,
                );
            },
            |_| {},
        )
    };

    // script
    let script_result = {
        println!();
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(define_function! {
            registry => mod intrinsics fn div(a: f64, b: f64) -> (result: f64) {
                (a / b,)
            }
        });
        let mut content_provider = FileContentProvider::new("vault", VaultContentParser);
        VaultPackage::new("./resources/package.vault", &mut content_provider)
            .unwrap()
            .compile()
            .install::<VmScope<VaultScriptExpression>>(&mut registry, None);
        let div = registry
            .find_function(FunctionQuery {
                name: Some("div".into()),
                module_name: Some("test".into()),
                ..Default::default()
            })
            .unwrap();
        let mut context = Context::new(10240, 10240);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "script div",
            || {},
            |_| {
                div.call::<(f64,), _>(
                    &mut context,
                    &registry,
                    (black_box(DIV_A), black_box(DIV_B)),
                    true,
                );
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
                pub fn rune_div(a, b) {
                    a / b
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
            "rune div",
            || {},
            |_| {
                vm.execute(["rune_div"], (black_box(DIV_A), black_box(DIV_B)))
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
        let ast = engine.compile(r#"fn rhai_div(a, b) { a / b }"#).unwrap();
        let mut scope = Scope::new();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "rhai div",
            || {},
            |_| {
                engine
                    .call_fn::<f64>(
                        &mut scope,
                        &ast,
                        "rhai_div",
                        (black_box(DIV_A), black_box(DIV_B)),
                    )
                    .unwrap();
            },
            |_| {},
        )
    };

    println!();
    println!("--- DIV | RESULTS ---");

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
    println!("= Script vs Host:");
    script_result.print_comparison(&host_result, COMPARISON_FORMAT);

    println!();
    println!("= Script vs Rune:");
    script_result.print_comparison(&rune_result, COMPARISON_FORMAT);

    println!();
    println!("= Script vs Rhai:");
    script_result.print_comparison(&rhai_result, COMPARISON_FORMAT);

    Ok(())
}

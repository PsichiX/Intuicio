use crate::{
    benches::{COMPARISON_FORMAT, DURATION},
    Benchmark,
};
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use std::time::Duration;

const PROCESSING_N: usize = 10000;

pub fn bench() {
    println!();
    println!("--- PROCESSING | BENCHMARKS ---");

    // native
    let native_result = {
        fn processing(data: Vec<usize>) -> usize {
            let mut result = 0;
            for value in data {
                result += value;
            }
            result
        }

        println!();
        let data = (0..PROCESSING_N).collect::<Vec<_>>();
        println!(
            "processing({}) = {}",
            PROCESSING_N,
            processing(data.clone())
        );

        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "native processing",
            || {},
            |_| {
                processing(data.clone());
            },
            |_| {},
        )
    };

    // shared lock pointer
    let shared_lock_result = {
        use std::sync::{Arc, RwLock};

        fn processing(data: Arc<RwLock<Vec<Arc<RwLock<usize>>>>>) -> Arc<RwLock<usize>> {
            let mut result = 0;
            for value in data.read().unwrap().iter() {
                result += *value.read().unwrap();
            }
            Arc::new(RwLock::new(result))
        }

        println!();
        let data = Arc::new(RwLock::new(
            (0..PROCESSING_N)
                .map(|value| Arc::new(RwLock::new(value)))
                .collect::<Vec<_>>(),
        ));
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "shared lock pointer processing",
            || {},
            |_| {
                processing(data.clone());
            },
            |_| {},
        )
    };

    // host
    let host_result = {
        fn processing(context: &mut Context, _: &Registry) {
            let data = context.stack().pop::<Vec<usize>>().unwrap();
            let mut result = 0;
            for value in data {
                result += value;
            }
            context.stack().push(result);
        }

        println!();
        let mut registry = Registry::default()
            .with_basic_types()
            .with_struct(define_native_struct! { registry => struct (Vec<usize>) {} });
        let processing = registry.add_function(Function::new(
            function_signature! {
                registry => fn processing(data: Vec<usize>) -> (result: usize)
            },
            FunctionBody::pointer(processing),
        ));
        let mut context = Context::new(1024, 1024, 1024);
        let data = (0..PROCESSING_N).collect::<Vec<_>>();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "host processing",
            || {},
            |_| {
                context.stack().push(data.clone());
                processing.invoke(&mut context, &registry);
                context.stack().pop::<usize>();
            },
            |_| {},
        )
    };

    // vm
    let vm_result = {
        fn processing(context: &mut Context, _: &Registry) {
            let data = context.stack().pop::<Vec<usize>>().unwrap();
            let mut result = 0;
            for value in data {
                result += value;
            }
            context.stack().push(result);
        }

        println!();
        let mut registry = Registry::default()
            .with_basic_types()
            .with_struct(define_native_struct! { registry => struct (Vec<usize>) {} });
        registry.add_function(Function::new(
            function_signature! {
                registry => fn processing(data: Vec<usize>) -> (result: usize)
            },
            FunctionBody::pointer(processing),
        ));
        let processing = registry.add_function(Function::new(
            function_signature! {
                registry => fn processing_script(data: Vec<usize>) -> (result: usize)
            },
            VmScope::<()>::generate_function_body(
                ScriptBuilder::<()>::default()
                    .call_function(FunctionQuery {
                        name: Some("processing".into()),
                        ..Default::default()
                    })
                    .build(),
                None,
            )
            .unwrap()
            .0,
        ));
        let mut context = Context::new(1024, 1024, 1024);
        let data = (0..PROCESSING_N).collect::<Vec<_>>();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "vm processing",
            || {},
            |_| {
                processing.call::<(usize,), _>(&mut context, &registry, (data.clone(),), true);
            },
            |_| {},
        )
    };

    // // script
    // let script_result = {
    //     println!();
    //     let mut registry = Registry::default().with_basic_types();
    //     registry.add_function(define_vault_function! {
    //         registry => mod intrinsics fn add(a: usize, b: usize) -> usize {
    //             a + b
    //         }
    //     });
    //     registry.add_function(define_vault_function! {
    //         registry => mod intrinsics fn sub(a: usize, b: usize) -> usize {
    //             a - b
    //         }
    //     });
    //     registry.add_function(define_vault_function! {
    //         registry => mod intrinsics fn less_than(a: usize, b: usize) -> bool {
    //             a < b
    //         }
    //     });
    //     registry.add_function(define_function! {
    //         registry => mod intrinsics struct (usize) fn clone(this: usize) -> (original: usize, clone: usize) {
    //             (this, this)
    //         }
    //     });
    //     VaultPackage::new("../resources/package.vault", &mut FileContentProvider)
    //         .unwrap()
    //         .compile(&mut registry, None);
    //     let processing = registry
    //         .find_function(FunctionQuery {
    //             name: Some("processing".into()),
    //             module_name: Some("test".into()),
    //             ..Default::default()
    //         })
    //         .unwrap();
    //     let mut context = Context::new(1024, 1024, 1024);
    //     Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
    //         "script processing",
    //         || {},
    //         |_| {
    //             processing.call::<(usize,), _>(&mut context, &registry, (black_box(PROCESSING_N),), true);
    //         },
    //         |_| {},
    //     )
    // };

    // // rune
    // let rune_result = {
    //     use rune::{
    //         termcolor::{ColorChoice, StandardStream},
    //         {Diagnostics, Vm},
    //     };
    //     use std::sync::Arc;

    //     println!();
    //     let context = rune_modules::default_context().unwrap();
    //     let mut sources = rune::sources!(
    //         entry => {
    //             pub fn rune_fib(n) {
    //                 match n {
    //                     0 => 0,
    //                     1 => 1,
    //                     n => rune_fib(n - 1) + rune_fib(n - 2),
    //                 }
    //             }
    //         }
    //     );
    //     let mut diagnostics = Diagnostics::new();
    //     let result = rune::prepare(&mut sources)
    //         .with_context(&context)
    //         .with_diagnostics(&mut diagnostics)
    //         .build()
    //         .unwrap();
    //     if !diagnostics.is_empty() {
    //         let mut writer = StandardStream::stderr(ColorChoice::Always);
    //         diagnostics.emit(&mut writer, &sources).unwrap();
    //     }
    //     let mut vm = Vm::new(Arc::new(context.runtime()), Arc::new(result));
    //     Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
    //         "rune processing",
    //         || {},
    //         |_| {
    //             vm.execute(&["rune_fib"], (black_box(PROCESSING_N),))
    //                 .unwrap()
    //                 .complete()
    //                 .unwrap();
    //         },
    //         |_| {},
    //     )
    // };

    // // rhai
    // let rhai_result = {
    //     use rhai::{Engine, Scope};

    //     println!();
    //     let engine = Engine::new();
    //     let ast = engine
    //         .compile(
    //             r#"fn rhai_fib(n) { if n < 2 { n } else { rhai_fib(n - 1) + rhai_fib(n - 2) } }"#,
    //         )
    //         .unwrap();
    //     let mut scope = Scope::new();
    //     Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
    //         "rhai processing",
    //         || {},
    //         |_| {
    //             engine
    //                 .call_fn::<i64>(&mut scope, &ast, "rhai_fib", (black_box(PROCESSING_N as i64),))
    //                 .unwrap();
    //         },
    //         |_| {},
    //     )
    // };

    println!();
    println!("--- PROCESSING | RESULTS ---");

    println!();
    println!("= Host vs Native:");
    host_result.print_comparison(&native_result, COMPARISON_FORMAT);

    println!();
    println!("= Host vs Shared Lock:");
    host_result.print_comparison(&shared_lock_result, COMPARISON_FORMAT);

    println!();
    println!("= Vm vs Host:");
    vm_result.print_comparison(&host_result, COMPARISON_FORMAT);

    // println!();
    // println!("= Script vs Vm:");
    // script_result.print_comparison(&vm_result, COMPARISON_FORMAT);

    // println!();
    // println!("= Script vs Host:");
    // script_result.print_comparison(&host_result, COMPARISON_FORMAT);

    // println!();
    // println!("= Script vs Rune:");
    // script_result.print_comparison(&rune_result, COMPARISON_FORMAT);

    // println!();
    // println!("= Script vs Rhai:");
    // script_result.print_comparison(&rhai_result, COMPARISON_FORMAT);
}

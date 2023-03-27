use crate::{
    benches::{COMPARISON_FORMAT, DURATION},
    black_box, Benchmark,
};
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_frontend_vault::*;
use std::time::Duration;

const SQRT_N: f32 = 424242.424242;

pub fn bench() {
    println!();
    println!("=== SQRT | BENCHMARKS ===");

    // native
    let native_result = {
        fn sqrt(n: f32) -> f32 {
            n.sqrt()
        }

        println!();
        println!("sqrt({}) = {}", SQRT_N, sqrt(SQRT_N));

        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "native sqrt",
            || {},
            |_| {
                sqrt(black_box(SQRT_N));
            },
            |_| {},
        )
    };

    // shared lock pointer
    let shared_lock_result = {
        use std::sync::{Arc, RwLock};

        fn sqrt(n: Arc<RwLock<f32>>) -> Arc<RwLock<f32>> {
            let n = *n.read().unwrap();
            Arc::new(RwLock::new(n.sqrt()))
        }

        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "shared lock pointer sqrt",
            || {},
            |_| {
                sqrt(Arc::new(RwLock::new(SQRT_N)));
            },
            |_| {},
        )
    };

    // host
    let host_result = {
        println!();
        let mut registry = Registry::default().with_basic_types();
        let sqrt = registry.add_function(define_function! {
            registry => fn sqrt(n: f32) -> (result: f32) {
                (n.sqrt(),)
            }
        });
        let mut context = Context::new(1024, 1024, 1024);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "host sqrt",
            || {},
            |_| {
                context.stack().push(black_box(SQRT_N));
                sqrt.invoke(&mut context, &registry);
                context.stack().pop::<f32>();
            },
            |_| {},
        )
    };

    // vm
    let vm_result = {
        println!();
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(define_function! {
            registry => fn sqrt(n: f32) -> (result: f32) {
                (n.sqrt(),)
            }
        });
        let sqrt = registry.add_function(
            VmScope::<()>::generate_function(
                ScriptFunction {
                    signature: function_signature! {
                        registry => fn sqrt(n: f32) -> (result: f32)
                    },
                    script: ScriptBuilder::<()>::default()
                        .call_function(FunctionQuery {
                            name: Some("sqrt".into()),
                            ..Default::default()
                        })
                        .build(),
                },
                None,
            )
            .unwrap()
            .0,
        );
        let mut context = Context::new(1024, 1024, 1024);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "vm sqrt",
            || {},
            |_| {
                sqrt.call::<(f32,), _>(&mut context, &registry, (black_box(SQRT_N),), true);
            },
            |_| {},
        )
    };

    // script
    let script_result = {
        println!();
        let mut registry = Registry::default().with_basic_types();
        registry.add_function(define_function! {
            registry => mod intrinsics fn sqrt(n: f32) -> (result: f32) {
                (n.sqrt(),)
            }
        });
        VaultPackage::new("../resources/package.vault", &mut FileContentProvider)
            .unwrap()
            .compile::<VmScope<VaultScriptExpression>>(&mut registry, None);
        let sqrt = registry
            .find_function(FunctionQuery {
                name: Some("sqrt".into()),
                module_name: Some("test".into()),
                ..Default::default()
            })
            .unwrap();
        let mut context = Context::new(1024, 1024, 1024);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "script sqrt",
            || {},
            |_| {
                sqrt.call::<(f32,), _>(&mut context, &registry, (black_box(SQRT_N),), true);
            },
            |_| {},
        )
    };

    println!();
    println!("=== SQRT | RESULTS ===");

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
}

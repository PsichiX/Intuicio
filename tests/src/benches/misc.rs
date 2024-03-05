use crate::{benches::DURATION, Benchmark};
use intuicio_backend_vm::scope::VmScope;
use intuicio_core::{
    define_function, function::FunctionQuery, registry::Registry, script::FileContentProvider,
    struct_type::StructQuery,
};
use intuicio_frontend_vault::{
    define_vault_function, VaultContentParser, VaultPackage, VaultScriptExpression,
};
use std::time::Duration;

pub fn bench() {
    println!();
    println!("--- MISC | BENCHMARKS ---");

    // hashing function query
    {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "hashing function query",
            || {},
            |_| {
                FunctionQuery {
                    name: Some("function".to_owned().into()),
                    module_name: Some("module".to_owned().into()),
                    ..Default::default()
                }
                .as_hash()
            },
            |_| {},
        )
    };

    // hashing struct query
    {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "hashing struct query",
            || {},
            |_| {
                StructQuery {
                    name: Some("structure".to_owned().into()),
                    module_name: Some("module".to_owned().into()),
                    ..Default::default()
                }
                .as_hash()
            },
            |_| {},
        )
    };

    // querying struct
    {
        println!();
        let mut registry = Registry::default().with_basic_types();
        let mut content_provider = FileContentProvider::new("vault", VaultContentParser);
        VaultPackage::new("../resources/package.vault", &mut content_provider)
            .unwrap()
            .compile()
            .install::<VmScope<VaultScriptExpression>>(&mut registry, None);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "querying struct",
            || StructQuery {
                name: Some("usize".to_owned().into()),
                ..Default::default()
            },
            |query| {
                let _ = registry.find_struct(query);
            },
            |_| {},
        )
    };

    // querying function
    {
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
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "querying function",
            || FunctionQuery {
                name: Some("fib".to_owned().into()),
                ..Default::default()
            },
            |query| {
                let _ = registry.find_function(query);
            },
            |_| {},
        )
    };
}

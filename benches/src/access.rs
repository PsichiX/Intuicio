use crate::{Benchmark, COMPARISON_FORMAT, DURATION, black_box};
use intuicio_data::{managed::Managed, managed_gc::ManagedGc};
use std::time::Duration;

pub fn bench() {
    println!();
    println!("--- ACCESS | BENCHMARKS ---");

    // native
    let native_access_result = {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "native access",
            || Box::new(black_box(42u128)),
            |mut data| {
                *data += 42;
                data
            },
            |_| {},
        )
    };

    // managed
    let managed_access_result = {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "managed access",
            || Managed::new(black_box(42u128)),
            |mut data| {
                let value = *data.read().unwrap();
                *data.write().unwrap() = value + 42;
                data
            },
            |_| {},
        )
    };

    // managed gc
    let managed_gc_access_result = {
        println!();
        let _ = ManagedGc::new(true);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "managed gc access",
            || ManagedGc::new(black_box(42u128)),
            |mut data| {
                let value = *data.try_read().unwrap();
                *data.try_write().unwrap() = value + 42;
                data
            },
            |_| {},
        )
    };

    println!();
    println!("--- ACCESS | RESULTS ---");

    println!();
    println!("Managed vs Native:");
    managed_access_result.print_comparison(&native_access_result, COMPARISON_FORMAT);

    println!();
    println!("ManagedGc vs Native:");
    managed_gc_access_result.print_comparison(&native_access_result, COMPARISON_FORMAT);

    println!();
    println!("ManagedGc vs Managed:");
    managed_gc_access_result.print_comparison(&managed_access_result, COMPARISON_FORMAT);
}

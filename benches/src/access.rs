use crate::{black_box, Benchmark, COMPARISON_FORMAT, DURATION};
use intuicio_data::prelude::*;
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

    // managed box
    let managed_box_access_result = {
        println!();
        let _ = ManagedBox::new(true);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "managed box access",
            || ManagedBox::new(black_box(42u128)),
            |mut data| {
                let value = *data.read().unwrap();
                *data.write().unwrap() = value + 42;
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
    println!("ManagedBox vs Native:");
    managed_box_access_result.print_comparison(&native_access_result, COMPARISON_FORMAT);

    println!();
    println!("ManagedBox vs Managed:");
    managed_box_access_result.print_comparison(&managed_access_result, COMPARISON_FORMAT);
}

use crate::{
    benches::{COMPARISON_FORMAT, DURATION},
    black_box, Benchmark,
};
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
                *data = *data + 42;
                data
            },
            |data| drop(data),
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
            |data| drop(data),
        )
    };

    println!();
    println!("--- ACCESS | RESULTS ---");

    println!();
    println!("Managed vs Native:");
    managed_access_result.print_comparison(&native_access_result, COMPARISON_FORMAT);
}

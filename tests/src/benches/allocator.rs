use crate::{
    benches::{COMPARISON_FORMAT, DURATION},
    black_box, Benchmark,
};
use intuicio_data::prelude::*;
use std::time::Duration;

pub fn bench() {
    println!();
    println!("=== ALLOCATOR | BENCHMARKS ===");

    // native
    let native_alloc_result = {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "native heap alloc",
            || {},
            |_| Box::new(black_box(42u128)),
            |data| drop(data),
        )
    };

    // managed
    let managed_alloc_result = {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "managed heap alloc",
            || {},
            |_| Managed::new(black_box(42u128)),
            |data| drop(data),
        )
    };

    // data heap
    let data_heap_alloc_result = {
        println!();
        let mut heap = DataHeap::new(1024);
        heap.ensure_pages(1);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "data heap alloc",
            || {},
            |_| heap.alloc(42u128).unwrap(),
            |data| drop(data),
        )
    };

    // data stack
    let data_stack_alloc_result = {
        use std::{cell::RefCell, rc::Rc};

        println!();
        let stack = DataStack::new(1024, DataStackMode::All);
        let stack = Rc::new(RefCell::new(stack));
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "data stack alloc",
            || {},
            |_| {
                stack.borrow_mut().push(42u128);
            },
            |_| {
                stack.borrow_mut().pop::<u128>();
            },
        )
    };

    // native
    let native_dealloc_result = {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "native heap dealloc",
            || Box::new(black_box(42u128)),
            |data| drop(data),
            |_| {},
        )
    };

    // managed
    let managed_dealloc_result = {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "managed heap dealloc",
            || Managed::new(black_box(42u128)),
            |data| drop(data),
            |_| {},
        )
    };

    // data
    let data_heap_dealloc_result = {
        println!();
        let mut heap = DataHeap::new(1024);
        heap.ensure_pages(1);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "data heap dealloc",
            || heap.alloc(42u128).unwrap(),
            |data| drop(data),
            |_| {},
        )
    };

    // data stack
    let data_stack_dealloc_result = {
        use std::{cell::RefCell, rc::Rc};

        println!();
        let stack = DataStack::new(1024, DataStackMode::All);
        let stack = Rc::new(RefCell::new(stack));
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "data stack dealloc",
            || {
                stack.borrow_mut().push(42u128);
            },
            |_| {
                stack.borrow_mut().pop::<u128>();
            },
            |_| {},
        )
    };

    println!();
    println!("=== ALLOCATOR | RESULTS ===");

    println!();
    println!("Managed heap vs Native heap alloc:");
    managed_alloc_result.print_comparison(&native_alloc_result, COMPARISON_FORMAT);

    println!();
    println!("Data heap vs Native heap alloc:");
    data_heap_alloc_result.print_comparison(&native_alloc_result, COMPARISON_FORMAT);

    println!();
    println!("Data stack vs Native heap alloc:");
    data_stack_alloc_result.print_comparison(&native_alloc_result, COMPARISON_FORMAT);

    println!();
    println!("Managed heap vs Native heap dealloc:");
    managed_dealloc_result.print_comparison(&native_dealloc_result, COMPARISON_FORMAT);

    println!();
    println!("Data heap vs Native heap dealloc:");
    data_heap_dealloc_result.print_comparison(&native_dealloc_result, COMPARISON_FORMAT);

    println!();
    println!("Data stack vs Native heap dealloc:");
    data_stack_dealloc_result.print_comparison(&native_dealloc_result, COMPARISON_FORMAT);
}

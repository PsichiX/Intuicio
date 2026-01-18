use crate::{Benchmark, COMPARISON_FORMAT, DURATION, black_box};
use intuicio_data::{
    data_stack::{DataStack, DataStackMode},
    managed::{Managed, gc::ManagedGc},
};
use std::time::Duration;

pub fn bench() {
    println!();
    println!("--- ALLOCATOR | BENCHMARKS ---");

    // native
    let native_alloc_result = {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "native heap alloc",
            || {},
            |_| Box::new(black_box(42u128)),
            |_| {},
        )
    };

    // managed
    let managed_alloc_result = {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "managed alloc",
            || {},
            |_| Managed::new(black_box(42u128)),
            |_| {},
        )
    };

    // managed gc
    let managed_gc_alloc_result = {
        println!();
        let _ = ManagedGc::new(true);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "managed gc alloc",
            || {},
            |_| ManagedGc::new(black_box(42u128)),
            |_| {},
        )
    };

    // data stack
    let data_stack_alloc_result = {
        use std::{cell::RefCell, rc::Rc};

        println!();
        let stack = DataStack::new(10240, DataStackMode::Mixed);
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
            |_| {},
            |_| {},
        )
    };

    // managed
    let managed_dealloc_result = {
        println!();
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "managed dealloc",
            || Managed::new(black_box(42u128)),
            |_| {},
            |_| {},
        )
    };

    // managed gc
    let managed_gc_dealloc_result = {
        println!();
        let _ = ManagedGc::new(true);
        Benchmark::TimeDuration(Duration::from_secs(DURATION)).run(
            "managed gc dealloc",
            || ManagedGc::new(black_box(42u128)),
            |_| {},
            |_| {},
        )
    };

    // data stack
    let data_stack_dealloc_result = {
        use std::{cell::RefCell, rc::Rc};

        println!();
        let stack = DataStack::new(10240, DataStackMode::Mixed);
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
    println!("--- ALLOCATOR | RESULTS ---");

    println!();
    println!("Managed vs Native heap alloc:");
    managed_alloc_result.print_comparison(&native_alloc_result, COMPARISON_FORMAT);

    println!();
    println!("ManagedGc vs Native heap alloc:");
    managed_gc_alloc_result.print_comparison(&native_alloc_result, COMPARISON_FORMAT);

    println!();
    println!("ManagedGc vs Managed alloc:");
    managed_gc_alloc_result.print_comparison(&managed_alloc_result, COMPARISON_FORMAT);

    println!();
    println!("Data stack vs Native heap alloc:");
    data_stack_alloc_result.print_comparison(&native_alloc_result, COMPARISON_FORMAT);

    println!();
    println!("Managed vs Native heap dealloc:");
    managed_dealloc_result.print_comparison(&native_dealloc_result, COMPARISON_FORMAT);

    println!();
    println!("ManagedGc vs Native heap dealloc:");
    managed_gc_dealloc_result.print_comparison(&native_dealloc_result, COMPARISON_FORMAT);

    println!();
    println!("ManagedGc vs Managed dealloc:");
    managed_gc_dealloc_result.print_comparison(&managed_dealloc_result, COMPARISON_FORMAT);

    println!();
    println!("Data stack vs Native heap dealloc:");
    data_stack_dealloc_result.print_comparison(&native_dealloc_result, COMPARISON_FORMAT);
}

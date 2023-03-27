mod access;
mod allocator;
mod div;
mod fib;
mod processing;
mod sqrt;

use crate::ComparisonFormat;

const DURATION: u64 = 1;
const COMPARISON_FORMAT: ComparisonFormat = ComparisonFormat::Scale;

#[test]
fn bench() {
    access::bench();
    allocator::bench();
    div::bench();
    sqrt::bench();
    processing::bench();
    fib::bench();
}

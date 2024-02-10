mod access;
mod allocator;
mod div;
mod fib;
mod sqrt;

use crate::ComparisonFormat;

const DURATION: u64 = 1;
const COMPARISON_FORMAT: ComparisonFormat = ComparisonFormat::Scale;

#[test]
fn bench() {
    access::bench();
    allocator::bench();
    let _ = div::bench();
    sqrt::bench();
    let _ = fib::bench();
}

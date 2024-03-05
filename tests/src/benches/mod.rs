mod access;
mod allocator;
mod div;
mod fib;
mod misc;
mod sqrt;

use crate::ComparisonFormat;

const DURATION: u64 = 1;
const COMPARISON_FORMAT: ComparisonFormat = ComparisonFormat::Scale;

#[test]
pub fn bench() {
    access::bench();
    allocator::bench();
    let _ = div::bench();
    sqrt::bench();
    let _ = fib::bench();
    misc::bench();
}

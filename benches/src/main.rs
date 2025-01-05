#[cfg(feature = "bench_access")]
mod access;
#[cfg(feature = "bench_allocator")]
mod allocator;
#[cfg(feature = "bench_div")]
mod div;
#[cfg(feature = "bench_fib")]
mod fib;
#[cfg(feature = "bench_misc")]
mod misc;
#[cfg(feature = "bench_sqrt")]
mod sqrt;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

const DURATION: u64 = 1;
const COMPARISON_FORMAT: ComparisonFormat = ComparisonFormat::Scale;

pub fn main() {
    #[cfg(feature = "bench_access")]
    access::bench();
    #[cfg(feature = "bench_allocator")]
    allocator::bench();
    #[cfg(feature = "bench_div")]
    let _ = div::bench();
    #[cfg(feature = "bench_sqrt")]
    sqrt::bench();
    #[cfg(feature = "bench_fib")]
    let _ = fib::bench();
    #[cfg(feature = "bench_misc")]
    misc::bench();
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ComparisonFormat {
    Percentage,
    Scale,
}

impl ComparisonFormat {
    pub fn print(self, a: f64, b: f64) {
        match self {
            Self::Percentage => {
                if a.is_normal() && b.is_normal() {
                    print!("{:.4}%", 100.0 * a / b);
                } else {
                    print!("?");
                }
            }
            Self::Scale => {
                if a.is_normal() && b.is_normal() {
                    if a > b {
                        print!("{:.4} : 1", a / b);
                    } else if a < b {
                        print!("1 : {:.4}", b / a);
                    } else {
                        print!("1 : 1");
                    }
                } else {
                    print!("!");
                }
            }
        }
    }
}

#[derive(Default)]
pub struct BenchmarkResult {
    iterations: u32,
    total: Duration,
    min: Duration,
    max: Duration,
    average: Duration,
    least: Duration,
    most: Duration,
}

impl BenchmarkResult {
    pub fn print(&self) {
        println!("- iterations: {}", self.iterations);
        println!("- total: {:?}", self.total);
        println!("- min: {:?}", self.min);
        println!("- max: {:?}", self.max);
        println!("- average: {:?}", self.average);
        println!("- least: {:?}", self.least);
        println!("- most: {:?}", self.most);
    }

    pub fn print_comparison(&self, other: &Self, format: ComparisonFormat) {
        print!("- iterations = ");
        format.print(self.iterations as f64, other.iterations as f64);
        println!();
        print!("- total = ");
        format.print(self.total.as_secs_f64(), other.total.as_secs_f64());
        println!();
        print!("- min = ");
        format.print(self.min.as_secs_f64(), other.min.as_secs_f64());
        println!();
        print!("- max = ");
        format.print(self.max.as_secs_f64(), other.max.as_secs_f64());
        println!();
        print!("- average = ");
        format.print(self.average.as_secs_f64(), other.average.as_secs_f64());
        println!();
        print!("- least = ");
        format.print(self.least.as_secs_f64(), other.least.as_secs_f64());
        println!();
        print!("- most = ");
        format.print(self.most.as_secs_f64(), other.most.as_secs_f64());
        println!();
    }
}

pub enum Benchmark {
    Iterations(u32),
    TimeDuration(Duration),
}

impl Benchmark {
    pub fn run<I, O>(
        &self,
        label: &str,
        mut intro: impl FnMut() -> I,
        mut run: impl FnMut(I) -> O,
        mut outro: impl FnMut(O),
    ) -> BenchmarkResult {
        println!("* Benchmark for {}:", label);
        let (times, total) = match self {
            Self::Iterations(count) => {
                let timer = Instant::now();
                let times = (0..*count)
                    .map(|_| {
                        let payload = intro();
                        let t = Instant::now();
                        let payload = run(payload);
                        let result = t.elapsed();
                        outro(payload);
                        result
                    })
                    .collect::<Vec<_>>();
                let total = timer.elapsed();
                (times, total)
            }
            Self::TimeDuration(duration) => {
                let timer = Instant::now();
                let times = std::iter::from_fn(|| {
                    let payload = intro();
                    let t = Instant::now();
                    let payload = run(payload);
                    let time = t.elapsed();
                    outro(payload);
                    if timer.elapsed() <= *duration {
                        Some(time)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
                let total = timer.elapsed();
                (times, total)
            }
        };
        if times.is_empty() {
            println!("Failed! There was no iteration performed.");
            return Default::default();
        }
        let mut clusters = HashMap::<_, usize>::new();
        for time in &times {
            let entry = clusters.entry(*time).or_default();
            *entry += 1;
        }
        let result = BenchmarkResult {
            iterations: times.len() as _,
            min: *times.iter().min().unwrap(),
            max: *times.iter().max().unwrap(),
            average: times.iter().sum::<Duration>() / times.len() as _,
            least: *clusters.iter().min_by(|a, b| a.1.cmp(b.1)).unwrap().0,
            most: *clusters.iter().max_by(|a, b| a.1.cmp(b.1)).unwrap().0,
            total,
        };
        result.print();
        result
    }

    pub fn run_with_state<S, I, O>(
        &self,
        label: &str,
        state: &mut S,
        mut intro: impl FnMut(&mut S) -> I,
        mut run: impl FnMut(&mut S, I) -> O,
        mut outro: impl FnMut(&mut S, O),
    ) -> BenchmarkResult {
        println!("* Benchmark for {}:", label);
        let (times, total) = match self {
            Self::Iterations(count) => {
                let timer = Instant::now();
                let times = (0..*count)
                    .map(|_| {
                        let payload = intro(state);
                        let t = Instant::now();
                        let payload = run(state, payload);
                        let result = t.elapsed();
                        outro(state, payload);
                        result
                    })
                    .collect::<Vec<_>>();
                let total = timer.elapsed();
                (times, total)
            }
            Self::TimeDuration(duration) => {
                let timer = Instant::now();
                let times = std::iter::from_fn(|| {
                    let payload = intro(state);
                    let t = Instant::now();
                    let payload = run(state, payload);
                    let time = t.elapsed();
                    outro(state, payload);
                    if timer.elapsed() <= *duration {
                        Some(time)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
                let total = timer.elapsed();
                (times, total)
            }
        };
        if times.is_empty() {
            println!("Failed! There was no iteration performed.");
            return Default::default();
        }
        let mut clusters = HashMap::<_, usize>::new();
        for time in &times {
            let entry = clusters.entry(*time).or_default();
            *entry += 1;
        }
        let result = BenchmarkResult {
            iterations: times.len() as _,
            min: *times.iter().min().unwrap(),
            max: *times.iter().max().unwrap(),
            average: times.iter().sum::<Duration>() / times.len() as _,
            least: *clusters.iter().min_by(|a, b| a.1.cmp(b.1)).unwrap().0,
            most: *clusters.iter().max_by(|a, b| a.1.cmp(b.1)).unwrap().0,
            total,
        };
        result.print();
        result
    }
}

pub fn black_box<T: Copy>(dummy: T) -> T {
    unsafe { std::ptr::read_volatile(&dummy) }
}

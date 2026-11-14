//! Task 1.8 performance budget: the per-pixel workload in
//! `support/workload.rs` (4225 evaluations) must take under 2 ms in release
//! mode. Run with `cargo bench -p md-eval`.
//!
//! A std-only harness (`harness = false`): criterion's dependency graph pulls
//! `cc` (via `alloca`), which the workspace's cargo-deny policy bans. Under
//! `cargo test --benches` (no `--bench` flag) it runs one frame as a smoke
//! test.

#[path = "support/workload.rs"]
mod workload;

use std::hint::black_box;
use std::time::{Duration, Instant};
use workload::{Workload, EVALS};

const BUDGET: Duration = Duration::from_millis(2);
const WARMUP: Duration = Duration::from_millis(500);
const SAMPLES: usize = 300;

fn main() {
    let mut w = Workload::new();
    if !std::env::args().any(|a| a == "--bench") {
        black_box(w.frame());
        println!("per_pixel: smoke run ok (use `cargo bench` to measure)");
        return;
    }

    let start = Instant::now();
    while start.elapsed() < WARMUP {
        black_box(w.frame());
    }
    let mut times: Vec<Duration> = (0..SAMPLES)
        .map(|_| {
            let t = Instant::now();
            black_box(w.frame());
            t.elapsed()
        })
        .collect();
    times.sort();
    let mean = times.iter().sum::<Duration>() / SAMPLES as u32;
    let median = times[SAMPLES / 2];
    println!(
        "per_pixel_{EVALS}: min {:?}  median {:?}  mean {:?}  p95 {:?}  ({SAMPLES} samples; budget {:?})",
        times[0],
        median,
        mean,
        times[SAMPLES * 95 / 100],
        BUDGET
    );
    println!(
        "per evaluation: median {:.1} ns",
        median.as_nanos() as f64 / EVALS as f64
    );
    if median > BUDGET {
        eprintln!("per_pixel: OVER BUDGET (median {median:?} > {BUDGET:?})");
        std::process::exit(1);
    }
}

//! Coarse performance regression guard for the Task 1.8 budget, run by
//! plain `cargo test` in any profile.
//!
//! The real budget (under 2 ms per 4225 evaluations, release) is measured by
//! `cargo bench -p md-eval`. This guard only catches order-of-magnitude
//! regressions (per-run allocation, accidental quadratic work): it takes the
//! best of several frames, so scheduler noise and parallel tests cannot fail
//! it, and its limit is far above an unoptimised debug build's time.

#[path = "../benches/support/workload.rs"]
mod workload;

use std::time::{Duration, Instant};

/// ~40x a debug build's best frame (2.4 ms) on the CI machine class, ~200x
/// release (0.46 ms).
const LIMIT: Duration = Duration::from_millis(100);

#[test]
fn per_pixel_frame_is_not_catastrophically_slow() {
    let mut w = workload::Workload::new();
    let first = w.frame();
    let best = (0..7)
        .map(|_| {
            let t = Instant::now();
            assert_eq!(w.frame(), first, "frames are deterministic");
            t.elapsed()
        })
        .min()
        .unwrap();
    eprintln!("best of 7 frames ({} evals): {best:?}", workload::EVALS);
    assert!(best < LIMIT, "best frame {best:?} >= {LIMIT:?}");
}

//! Shared test helpers: PNG comparison, SSIM and golden-file blessing, the
//! preset corpus, a fixed clock, and the deterministic audio test signal.
#![forbid(unsafe_code)]

mod corpus;
mod fixed_clock;
mod image_diff;
mod signal;

pub use corpus::{corpus_presets, corpus_presets_from, presets_dir, EXTRA_PRESETS_ENV};
pub use fixed_clock::FixedClock;
pub use image_diff::{bless_enabled, compare_png, compare_png_with, ssim, DiffResult};
pub use signal::{test_signal, test_signal_sample, TEST_SIGNAL_RATE, TEST_SIGNAL_SECS};

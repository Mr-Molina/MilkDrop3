//! Shared test helpers: PNG comparison, SSIM and golden-file blessing.
#![forbid(unsafe_code)]

mod corpus;
mod fixed_clock;
mod image_diff;

pub use corpus::corpus_presets;
pub use fixed_clock::FixedClock;
pub use image_diff::{bless_enabled, compare_png, compare_png_with, ssim, DiffResult};

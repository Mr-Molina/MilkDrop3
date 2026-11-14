//! Audio analysis: FFT, band energies and attenuation (port of fft.cpp and AnalyzeNewSound).
#![forbid(unsafe_code)]

mod align;
pub mod analyzer;
pub mod fft;

pub use analyzer::{adjust_rate_to_fps, Analyzer, SoundData};
pub use fft::Fft;

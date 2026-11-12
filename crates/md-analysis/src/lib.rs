//! Audio analysis: FFT, band energies and attenuation (port of fft.cpp and AnalyzeNewSound).
#![forbid(unsafe_code)]

pub mod fft;

pub use fft::Fft;

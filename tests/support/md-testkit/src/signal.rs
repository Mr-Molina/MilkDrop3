//! The deterministic audio test signal stored at `tests/golden/test_signal.wav`.
//!
//! 20 s, 44.1 kHz, stereo (both channels identical), 16-bit:
//! a 55 Hz sine at 0.8 amplitude pulsing at 2 Hz plus a 4 kHz sine at 0.2
//! amplitude pulsing at 0.5 Hz. "Pulsing at f" is the raised-cosine envelope
//! `0.5 - 0.5*cos(2*pi*f*t)`, so the signal starts silent and peaks at 1.0.

use std::f64::consts::TAU;

pub const TEST_SIGNAL_RATE: u32 = 44_100;
pub const TEST_SIGNAL_SECS: u32 = 20;

fn envelope(freq: f64, t: f64) -> f64 {
    0.5 - 0.5 * (TAU * freq * t).cos()
}

/// Mono value of sample frame `i` in `[-1, 1]`.
pub fn test_signal_sample(i: u32) -> f64 {
    let t = f64::from(i) / f64::from(TEST_SIGNAL_RATE);
    0.8 * envelope(2.0, t) * (TAU * 55.0 * t).sin()
        + 0.2 * envelope(0.5, t) * (TAU * 4000.0 * t).sin()
}

/// The whole signal as interleaved stereo `i16` (L, R, L, R, ...).
pub fn test_signal() -> Vec<i16> {
    let frames = TEST_SIGNAL_RATE * TEST_SIGNAL_SECS;
    let mut out = Vec::with_capacity(frames as usize * 2);
    for i in 0..frames {
        let v = (test_signal_sample(i) * 32767.0)
            .round()
            .clamp(-32768.0, 32767.0) as i16;
        out.push(v);
        out.push(v);
    }
    out
}

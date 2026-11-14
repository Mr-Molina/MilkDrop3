//! Radix-2 magnitude FFT with the envelope and equalize tables of `fft.cpp@6f7cd5d`.
//!
//! Implemented from the formulas of `FFT::Init` / `FFT::time_to_frequency_domain`
//! (fft.cpp:61-318@6f7cd5d), keeping the C++ f32 operation order so the output
//! (including its un-normalised magnitude scaling) matches the original:
//! `NFREQ = 2 * samples_out` points; the first `samples_in` input samples (times the
//! envelope, when on) are placed in bit-reversed order and the rest zero-padded; an
//! in-place decimation-in-time radix-2 FFT runs with a per-stage twiddle recurrence;
//! the output is `|X[k]|` for `k < NFREQ/2`, times `equalize[k]` when on.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Magnitude FFT matching `FFT::time_to_frequency_domain`.
#[derive(Debug, Clone)]
pub struct Fft {
    samples_in: usize,
    nfreq: usize,
    bitrev: Vec<usize>,
    /// `(cos θ, sin θ)` with `θ = -2π / dftsize` for each stage, dftsize = 2, 4, .., NFREQ.
    twiddle: Vec<(f32, f32)>,
    envelope: Option<Vec<f32>>,
    equalize: Option<Vec<f32>>,
}

impl Fft {
    /// `samples_out` must be a power of two (NFREQ = 2 * samples_out).
    /// `envelope_power <= 0` disables the envelope, as in `FFT::Init` (fft.cpp:77-80@6f7cd5d).
    pub fn new(samples_in: usize, samples_out: usize, equalize: bool, envelope_power: f32) -> Fft {
        assert!(
            samples_out.is_power_of_two(),
            "samples_out must be a power of two"
        );
        let nfreq = samples_out * 2;
        let bits = nfreq.trailing_zeros();
        // Same permutation as InitBitRevTable's swap loop: index i holds bit-reverse(i).
        let bitrev = (0..nfreq)
            .map(|i| i.reverse_bits() >> (usize::BITS - bits))
            .collect();
        let mut twiddle = Vec::new();
        let mut dftsize = 2usize;
        while dftsize <= nfreq {
            let theta = -2.0f32 * PI / dftsize as f32;
            twiddle.push((theta.cos(), theta.sin()));
            dftsize <<= 1;
        }
        let envelope = (envelope_power > 0.0).then(|| envelope_table(samples_in, envelope_power));
        let equalize = equalize.then(|| equalize_table(nfreq / 2));
        Fft {
            samples_in,
            nfreq,
            bitrev,
            twiddle,
            envelope,
            equalize,
        }
    }

    /// The envelope table (`samples_in` entries), if the envelope is on.
    pub fn envelope(&self) -> Option<&[f32]> {
        self.envelope.as_deref()
    }

    /// The equalize table (`samples_out` entries), if equalization is on.
    pub fn equalize(&self) -> Option<&[f32]> {
        self.equalize.as_deref()
    }

    /// Writes `samples_out` magnitudes of `wave` (at least `samples_in` samples long)
    /// into `spectrum_out`.
    pub fn transform(&self, wave: &[f32], spectrum_out: &mut [f32]) {
        let n = self.nfreq;
        assert!(wave.len() >= self.samples_in.min(n), "wave too short");
        assert!(spectrum_out.len() >= n / 2, "spectrum_out too short");
        let mut real = vec![0f32; n];
        let mut imag = vec![0f32; n];
        for (r, &idx) in real.iter_mut().zip(&self.bitrev) {
            if idx < self.samples_in {
                *r = match &self.envelope {
                    Some(env) => wave[idx] * env[idx],
                    None => wave[idx],
                };
            }
        }
        let mut dftsize = 2usize;
        for &(wpr, wpi) in &self.twiddle {
            let (mut wr, mut wi) = (1.0f32, 0.0f32);
            let half = dftsize >> 1;
            for m in 0..half {
                for i in (m..n).step_by(dftsize) {
                    let j = i + half;
                    let tempr = wr * real[j] - wi * imag[j];
                    let tempi = wr * imag[j] + wi * real[j];
                    real[j] = real[i] - tempr;
                    imag[j] = imag[i] - tempi;
                    real[i] += tempr;
                    imag[i] += tempi;
                }
                let wtemp = wr;
                wr = wtemp * wpr - wi * wpi;
                wi = wi * wpr + wtemp * wpi;
            }
            dftsize <<= 1;
        }
        for (k, out) in spectrum_out[..n / 2].iter_mut().enumerate() {
            let mag = (real[k] * real[k] + imag[k] * imag[k]).sqrt();
            *out = match &self.equalize {
                Some(eq) => eq[k] * mag,
                None => mag,
            };
        }
    }
}

/// `InitEnvelopeTable` (fft.cpp:113-138@6f7cd5d): `0.5 + 0.5 sin(i * 2π/n - π/2)`,
/// raised to `power` unless it is exactly 1. The C++ literals 6.2831853f and
/// 1.5707963268f round to the same f32 as `TAU` and `FRAC_PI_2`.
fn envelope_table(samples_in: usize, power: f32) -> Vec<f32> {
    let mult = 1.0f32 / samples_in as f32 * TAU;
    (0..samples_in)
        .map(|i| {
            let e = 0.5f32 + 0.5f32 * (i as f32 * mult - FRAC_PI_2).sin();
            if power == 1.0 {
                e
            } else {
                e.powf(power)
            }
        })
        .collect()
}

/// `InitEqualizeTable` (fft.cpp:99-109@6f7cd5d): `-0.02 ln((half - i) / half)`.
fn equalize_table(half: usize) -> Vec<f32> {
    let inv_half = 1.0f32 / half as f32;
    (0..half)
        .map(|i| -0.02f32 * ((half - i) as f32 * inv_half).ln())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Fft;
    use std::f64::consts::PI;

    /// `amp * sin(2π k n / 1024)`: a 44100/1024*k Hz sine sampled at 44.1 kHz.
    fn sine(k: usize, amp: f64) -> Vec<f32> {
        (0..576)
            .map(|n| (amp * (2.0 * PI * k as f64 * n as f64 / 1024.0).sin()) as f32)
            .collect()
    }

    fn argmax(x: &[f32]) -> usize {
        let mut best = 0;
        for (i, v) in x.iter().enumerate() {
            if *v > x[best] {
                best = i;
            }
        }
        best
    }

    #[test]
    fn single_bin_sine_peaks_at_expected_bin() {
        let fft = Fft::new(576, 512, false, -1.0);
        // For k a multiple of 8, 2k*576/1024 is an integer, so the image term of the
        // zero-padded 1024-point DFT cancels exactly and |X[k]| = amp * 576 / 2 = 28800
        // (no normalisation in time_to_frequency_domain); see the report for the derivation.
        for k in [8usize, 32, 96, 200] {
            let mut out = [0f32; 512];
            fft.transform(&sine(k, 100.0), &mut out);
            assert_eq!(argmax(&out), k, "k={k}");
            assert!(
                (out[k] - 28800.0).abs() < 28800.0 * 1e-4,
                "k={k}: {}",
                out[k]
            );
        }
    }

    #[test]
    fn envelope_table_first_entry_is_zero_and_last_is_sin_squared() {
        // F65: env[i] = 0.5 + 0.5 sin(2πi/576 - π/2) = sin²(πi/576); env[575] = sin²(π/576).
        let fft = Fft::new(576, 512, false, 1.0);
        let env = fft.envelope().expect("envelope on");
        assert_eq!(env.len(), 576);
        assert!(env[0].abs() < 1e-6, "{}", env[0]);
        assert!(env[575] < 1e-4);
        assert!((env[575] - 2.974_75e-5).abs() < 1e-7, "{}", env[575]);
        assert!((env[288] - 1.0).abs() < 1e-6);
        let sq = Fft::new(576, 512, false, 2.0);
        let env2 = sq.envelope().unwrap();
        assert!((env2[100] - env[100] * env[100]).abs() < 1e-6);
        assert!(Fft::new(576, 512, false, -1.0).envelope().is_none());
        assert!(Fft::new(576, 512, false, 0.0).envelope().is_none());
    }

    #[test]
    fn equalize_table_first_entry_is_zero_and_last_is_positive() {
        let fft = Fft::new(576, 512, true, -1.0);
        let eq = fft.equalize().expect("equalize on");
        assert_eq!(eq.len(), 512);
        assert_eq!(eq[0], 0.0);
        // -0.02 ln(1/512) = 0.02 ln 512
        assert!((eq[511] - 0.124_766_49).abs() < 1e-6, "{}", eq[511]);
        assert!(eq.windows(2).all(|w| w[1] > w[0]));
        assert!(Fft::new(576, 512, false, -1.0).equalize().is_none());
    }

    #[test]
    fn equalize_multiplies_magnitude() {
        let plain = Fft::new(576, 512, false, -1.0);
        let eq = Fft::new(576, 512, true, -1.0);
        let w = sine(32, 100.0);
        let (mut a, mut b) = ([0f32; 512], [0f32; 512]);
        plain.transform(&w, &mut a);
        eq.transform(&w, &mut b);
        let table = eq.equalize().unwrap();
        for i in 0..512 {
            assert_eq!(b[i], table[i] * a[i]);
        }
        assert_eq!(b[0], 0.0);
    }

    #[test]
    fn dc_with_envelope_sums_the_envelope() {
        // X[0] of a constant 1.0 = Σ env[n] = Σ sin²(πn/576) = 288.
        let fft = Fft::new(576, 512, false, 1.0);
        let mut out = [0f32; 512];
        fft.transform(&[1.0; 576], &mut out);
        assert!((out[0] - 288.0).abs() < 1e-2, "{}", out[0]);
    }

    #[test]
    fn bit_reversal_matches_the_swap_loop() {
        // InitBitRevTable (fft.cpp:142-169@6f7cd5d), run literally.
        let n = 1024usize;
        let mut table: Vec<usize> = (0..n).collect();
        let mut j = 0usize;
        for i in 0..n {
            if j > i {
                table.swap(i, j);
            }
            let mut m = n >> 1;
            while m >= 1 && j >= m {
                j -= m;
                m >>= 1;
            }
            j += m;
        }
        assert_eq!(Fft::new(576, 512, true, 1.0).bitrev, table);
    }

    #[test]
    fn output_is_deterministic() {
        let fft = Fft::new(576, 512, true, 1.0);
        let w: Vec<f32> = (0..576).map(|i| ((i * 37 % 255) as f32) - 128.0).collect();
        let (mut a, mut b) = ([0f32; 512], [0f32; 512]);
        fft.transform(&w, &mut a);
        fft.transform(&w, &mut b);
        assert_eq!(a, b);
        let again = Fft::new(576, 512, true, 1.0);
        let mut c = [0f32; 512];
        again.transform(&w, &mut c);
        assert_eq!(a, c);
        assert!(a.iter().any(|v| *v > 0.0));
    }
}

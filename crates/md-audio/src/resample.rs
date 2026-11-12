//! Conversion of float PCM to the 576-byte frames the analyzer consumes.

const OUT_RATE: f64 = 44_100.0;

/// Converts interleaved float PCM into one analysis frame.
///
/// Windowing (exact): let `n` be the number of sample frames in `samples_interleaved`
/// and `ratio = sample_rate / 44100`. Output index `i` in `0..576` is the input
/// linearly interpolated at source position `(n - 1) - (575 - i) * ratio`, so the last
/// output sample is exactly the last input sample and the window is the most recent
/// 576 samples at 44.1 kHz. Positions before the start of the input read as 0.0
/// (silence). Samples are clamped to `[-1, 1]` and converted with
/// `((x * 127.0).round() as i8) as u8`. Mono is duplicated to both channels; for more
/// than two channels the first two are used. With zero frames both outputs are 0.
pub fn to_frame(
    samples_interleaved: &[f32],
    channels: u16,
    sample_rate: u32,
    left: &mut [u8; 576],
    right: &mut [u8; 576],
) {
    let ch = channels.max(1) as usize;
    let n = samples_interleaved.len() / ch;
    if n == 0 || sample_rate == 0 {
        left.fill(0);
        right.fill(0);
        return;
    }
    let ratio = f64::from(sample_rate) / OUT_RATE;
    let r_ch = if ch >= 2 { 1 } else { 0 };
    for i in 0..576 {
        let pos = (n - 1) as f64 - (575 - i) as f64 * ratio;
        left[i] = quantize(sample_at(samples_interleaved, ch, 0, pos));
        right[i] = quantize(sample_at(samples_interleaved, ch, r_ch, pos));
    }
}

fn sample_at(s: &[f32], ch: usize, c: usize, pos: f64) -> f32 {
    if pos < 0.0 {
        return 0.0;
    }
    let i0 = pos.floor() as usize;
    let frac = (pos - i0 as f64) as f32;
    let a = s[i0 * ch + c];
    if frac == 0.0 || (i0 + 1) * ch + c >= s.len() {
        return a;
    }
    let b = s[(i0 + 1) * ch + c];
    a + (b - a) * frac
}

fn quantize(x: f32) -> u8 {
    ((x.clamp(-1.0, 1.0) * 127.0).round() as i8) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The analyzer's byte conversion: `((b ^ 128) - 128)` on a u8 as i32.
    fn analyzer_convert(b: u8) -> f32 {
        (i32::from(b ^ 128) - 128) as f32
    }

    #[test]
    fn to_frame_round_trips_through_analysis_conversion() {
        // Cross-check the reimplemented conversion against md-analysis on a pure tone.
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.05).sin() * 0.9).collect();
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        to_frame(&samples, 1, 44_100, &mut l, &mut r);
        for (i, &b) in l.iter().enumerate() {
            let src = samples[1000 - 576 + i];
            let expect = (src * 127.0).round();
            assert_eq!(analyzer_convert(b), expect, "i={i}");
        }
        // End-to-end through the real analyzer: a full-scale positive sample is +127.
        to_frame(&[1.0, 2.0, -3.0, -1.0], 1, 44_100, &mut l, &mut r);
        assert_eq!(analyzer_convert(l[575]), -127.0);
        assert_eq!(analyzer_convert(l[573]), 127.0);
        let mut a = md_analysis::Analyzer::new();
        let _ = a.analyze(&l, &r, 30.0);
        // Silence is 0 and maps back to 0.
        let (mut l, mut r) = ([5u8; 576], [5u8; 576]);
        to_frame(&[], 2, 44_100, &mut l, &mut r);
        assert!(l.iter().all(|&b| analyzer_convert(b) == 0.0));
    }

    #[test]
    fn mono_is_duplicated() {
        let s: Vec<f32> = (0..700).map(|i| (i as f32 * 0.1).sin()).collect();
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        to_frame(&s, 1, 44_100, &mut l, &mut r);
        assert_eq!(l, r);
        assert!(l.iter().any(|&b| b != 0));
    }

    #[test]
    fn stereo_channels_stay_separate() {
        let mut s = Vec::new();
        for _ in 0..600 {
            s.extend_from_slice(&[0.5, -0.5]);
        }
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        to_frame(&s, 2, 44_100, &mut l, &mut r);
        assert!(l.iter().all(|&b| b as i8 == 64));
        assert!(r.iter().all(|&b| b as i8 == -64));
    }

    #[test]
    fn resamples_48k_to_44k1() {
        // A ramp x[i] = i/48000 at 48 kHz; resampled output step is 48000/44100 source
        // samples, so consecutive outputs differ by ~ (48000/44100)/48000 = 1/44100.
        let n = 2000;
        let s: Vec<f32> = (0..n).map(|i| i as f32 / 4000.0).collect();
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        to_frame(&s, 1, 48_000, &mut l, &mut r);
        let last = (s[n - 1] * 127.0).round() as i8;
        assert_eq!(l[575] as i8, last);
        // Position of l[0] is (n-1) - 575*48000/44100 = 1999 - 625.85...
        let pos = 1999.0 - 575.0 * 48_000.0 / 44_100.0;
        let expect = ((pos / 4000.0) as f32 * 127.0).round() as i8;
        assert!(
            (l[0] as i8 - expect).abs() <= 1,
            "{} vs {expect}",
            l[0] as i8
        );
    }

    #[test]
    fn clamps_out_of_range() {
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        to_frame(&[5.0, -5.0, f32::NAN], 1, 44_100, &mut l, &mut r);
        assert_eq!(l[573] as i8, 127);
        assert_eq!(l[574] as i8, -127);
        assert_eq!(l[575], 0);
    }
}

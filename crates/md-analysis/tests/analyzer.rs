//! Analyzer behaviour against DoCustomSoundAnalysis / AnalyzeNewSound / AlignWaves.
// Band loops index several parallel arrays (expected state and SoundData fields).
#![allow(clippy::needless_range_loop)]

use md_analysis::{adjust_rate_to_fps, Analyzer, Fft};
use std::f64::consts::PI;

const SILENCE: [u8; 576] = [0; 576];

/// Signed sample -> the byte pattern `((b ^ 128) - 128)` maps back to it.
fn to_byte(x: f64) -> u8 {
    (x.round().clamp(-128.0, 127.0) as i8) as u8
}

/// `amp * sin(2π k (n + phase) / 1024)` as bytes: bin k of the 1024-point FFT.
fn tone(k: f64, amp: f64, phase: usize) -> [u8; 576] {
    let mut b = [0u8; 576];
    for (n, v) in b.iter_mut().enumerate() {
        *v = to_byte(amp * (2.0 * PI * k * (n + phase) as f64 / 1024.0).sin());
    }
    b
}

fn converted(b: &[u8; 576]) -> [f32; 576] {
    let mut w = [0f32; 576];
    for i in 0..576 {
        w[i] = (((b[i] ^ 128) as i32) - 128) as f32;
    }
    w
}

#[test]
fn silence_gives_rel_values_of_one() {
    let mut a = Analyzer::new();
    for _ in 0..100 {
        a.analyze(&SILENCE, &SILENCE, 30.0);
    }
    let s = a.analyze(&SILENCE, &SILENCE, 30.0).clone();
    assert_eq!(s.imm_rel, [1.0; 3]);
    assert_eq!(s.avg_rel, [1.0; 3]);
    assert_eq!(s.imm, [0.0; 3]);
    assert_eq!(s.long_avg, [0.0; 3]);
    assert!(s.spectrum.iter().flatten().all(|v| *v == 0.0));
    assert!(s.wave.iter().flatten().all(|v| *v == 0.0));
}

#[test]
fn bass_tone_raises_band_zero_only() {
    // The preset FFT has the equalize table on, which strongly attenuates the lowest
    // bins (eq[k] = -0.02 ln(1 - k/512)), and AlignWaves zeroes samples 480.. after a
    // shift, which leaks a little energy into every band; hence a 5x margin.
    let mut a = Analyzer::new();
    assert_eq!(a.analyze(&SILENCE, &SILENCE, 30.0).imm, [0.0; 3]);
    for f in 0..10 {
        let t = tone(40.0, 100.0, f * 37);
        let s = a.analyze(&t, &t, 30.0);
        assert!(s.imm[0] > 50.0, "frame {f}: {:?}", s.imm);
        assert!(
            s.imm[0] > 5.0 * s.imm[1].max(s.imm[2]),
            "frame {f}: {:?}",
            s.imm
        );
    }
    // Control: a tone in the mid band (bins 85..170) raises band one instead.
    let mut a = Analyzer::new();
    let t = tone(120.0, 100.0, 0);
    let s = a.analyze(&t, &t, 30.0);
    assert!(s.imm[1] > 5.0 * s.imm[0].max(s.imm[2]), "{:?}", s.imm);
}

#[test]
fn long_avg_uses_0_9_for_first_50_frames() {
    let fps = 45.0;
    let early = adjust_rate_to_fps(0.9, 30.0, fps);
    let late = adjust_rate_to_fps(0.992, 30.0, fps);
    let mut a = Analyzer::new();
    let mut long = [0f32; 3];
    for frame in 0..80usize {
        let t = tone(24.0 + (frame % 5) as f64 * 40.0, 90.0, frame * 11);
        let s = a.analyze(&t, &SILENCE, fps);
        let rate = if frame < 50 { early } else { late };
        for i in 0..3 {
            long[i] = long[i] * rate + s.imm[i] * (1.0 - rate);
            assert_eq!(s.long_avg[i], long[i], "frame {frame} band {i}");
            let rel = if long[i].abs() < 0.001 {
                1.0
            } else {
                s.imm[i] / long[i]
            };
            assert_eq!(s.imm_rel[i], rel);
        }
    }
}

#[test]
fn avg_rises_at_0_2_and_falls_at_0_5_from_30_fps() {
    let fps = 60.0;
    let up = adjust_rate_to_fps(0.2, 30.0, fps);
    let down = adjust_rate_to_fps(0.5, 30.0, fps);
    let mut a = Analyzer::new();
    let mut avg = [0f32; 3];
    for frame in 0..40usize {
        let t = if frame % 7 < 3 {
            tone(40.0, 100.0, frame)
        } else {
            SILENCE
        };
        let s = a.analyze(&t, &t, fps);
        for i in 0..3 {
            let rate = if s.imm[i] > avg[i] { up } else { down };
            avg[i] = avg[i] * rate + s.imm[i] * (1.0 - rate);
            assert_eq!(s.avg[i], avg[i], "frame {frame} band {i}");
            let rel = if s.long_avg[i].abs() < 0.001 {
                1.0
            } else {
                avg[i] / s.long_avg[i]
            };
            assert_eq!(s.avg_rel[i], rel);
        }
    }
}

#[test]
fn adjust_rate_to_fps_identity_at_reference_fps() {
    for r in [0.2f32, 0.5, 0.9, 0.992] {
        assert!((adjust_rate_to_fps(r, 30.0, 30.0) - r).abs() < 1e-6, "{r}");
        assert!((adjust_rate_to_fps(r, 14.0, 14.0) - r).abs() < 1e-6, "{r}");
    }
    // Twice the fps: half the per-frame exponent.
    assert!((adjust_rate_to_fps(0.5, 30.0, 60.0) - 0.5f32.sqrt()).abs() < 1e-6);
    assert!((adjust_rate_to_fps(0.81, 30.0, 15.0) - 0.6561).abs() < 1e-5);
}

#[test]
fn bytes_convert_as_signed_8_bit() {
    // A constant wave has equal alignment error at every offset, so it is not shifted.
    let mut a = Analyzer::new();
    let s = a.analyze(&[200; 576], &[127; 576], 30.0);
    assert!(s.wave[0].iter().all(|v| *v == -56.0));
    assert!(s.wave[1].iter().all(|v| *v == 127.0));
    let s = a.analyze(&[128; 576], &[255; 576], 30.0);
    assert!(s.wave[0].iter().all(|v| *v == -128.0));
    assert!(s.wave[1].iter().all(|v| *v == -1.0));
}

#[test]
fn spectrum_is_the_shell_fft_of_the_damped_unaligned_input() {
    let fft = Fft::new(576, 512, true, 1.0);
    let mut a = Analyzer::new();
    for f in 0..4 {
        let (l, r) = (tone(30.0, 100.0, f * 13), tone(130.0, 60.0, f * 29));
        let s = a.analyze(&l, &r, 30.0);
        for (ch, bytes) in [l, r].iter().enumerate() {
            let w = converted(bytes);
            let mut damped = [0f32; 576];
            for i in 0..576 {
                damped[i] = 0.5 * (w[i] + w[i.saturating_sub(1)]);
            }
            let mut expect = [0f32; 512];
            fft.transform(&damped, &mut expect);
            assert_eq!(s.spectrum[ch], expect, "frame {f} ch {ch}");
        }
    }
}

#[test]
fn preset_bands_sum_the_undamped_aligned_left_fft() {
    // myfft.Init(576, 512, -1): the -1 lands in bEqualize, so equalize AND the
    // power-1 envelope are on; bands are plain sums over [0,85), [85,170), [170,256).
    let fft = Fft::new(576, 512, true, 1.0);
    let mut a = Analyzer::new();
    for f in 0..6 {
        let l = tone(20.0 + f as f64 * 31.0, 100.0, f * 17);
        let s = a.analyze(&l, &SILENCE, 30.0);
        let mut spec = [0f32; 512];
        fft.transform(&s.wave[0], &mut spec);
        let sum = |r: std::ops::Range<usize>| spec[r].iter().fold(0f32, |acc, v| acc + v);
        assert_eq!(
            s.imm,
            [sum(0..85), sum(85..170), sum(170..256)],
            "frame {f}"
        );
    }
}

fn smooth(phase: usize) -> [u8; 576] {
    let mut b = [0u8; 576];
    for (n, v) in b.iter_mut().enumerate() {
        let t = (n + phase) as f64;
        *v = to_byte(
            60.0 * (t * 0.031).sin()
                + 40.0 * (t * 0.0137 + 1.0).sin()
                + 20.0 * (t * 0.071 + 2.0).sin(),
        );
    }
    b
}

#[test]
fn a_repeated_frame_aligns_to_the_same_window() {
    // AlignWaves matches the new frame against the previous (unshifted) frame at the
    // previous offset. After s(n + 40), the frame s(n) is found 40 samples later, and
    // feeding s(n) again reproduces the same window. (White noise would not do this
    // reliably: its coarse octaves decorrelate and the C++ search, mirrored here, can
    // then settle elsewhere.)
    let mut a = Analyzer::new();
    let lead = a.analyze(&smooth(40), &SILENCE, 30.0).wave[0];
    assert_eq!(
        lead,
        converted(&smooth(40)),
        "this signal is not shifted against silence"
    );
    let first = a.analyze(&smooth(0), &SILENCE, 30.0).wave[0];
    assert_eq!(first[..480], lead[..480]);
    assert!(first[480..].iter().all(|v| *v == 0.0));
    let second = a.analyze(&smooth(0), &SILENCE, 30.0).wave[0];
    assert_eq!(first, second);
}

#[test]
fn med_avg_starts_at_one_and_decays_at_0_91_per_frame_at_14_fps() {
    let mut a = Analyzer::new();
    let r = adjust_rate_to_fps(0.91, 14.0, 14.0);
    let s = a.analyze(&SILENCE, &SILENCE, 14.0);
    assert_eq!(s.med_avg, [r; 3]);
    let s = a.analyze(&SILENCE, &SILENCE, 14.0);
    assert_eq!(s.med_avg, [r * r; 3]);
}

#[test]
fn analysis_is_deterministic() {
    let run = || {
        let mut a = Analyzer::new();
        let mut out = Vec::new();
        for f in 0..20 {
            let t = tone(10.0 + f as f64 * 7.0, 80.0, f * 3);
            out.push(a.analyze(&t, &tone(50.0, 40.0, f), 30.0).clone());
        }
        out
    };
    assert_eq!(run(), run());
}

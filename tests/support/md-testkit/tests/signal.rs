//! The deterministic test signal and the committed tests/golden/test_signal.wav.
use md_testkit::{test_signal, test_signal_sample, TEST_SIGNAL_RATE, TEST_SIGNAL_SECS};
use std::f64::consts::TAU;

#[test]
fn signal_has_the_specified_shape() {
    let s = test_signal();
    assert_eq!(TEST_SIGNAL_RATE, 44_100);
    assert_eq!(TEST_SIGNAL_SECS, 20);
    assert_eq!(s.len(), 20 * 44_100 * 2, "interleaved stereo");
    // Both channels carry the same signal.
    assert!(s.chunks(2).all(|f| f[0] == f[1]));
    // Peak stays inside i16 (0.8 + 0.2 = 1.0 full scale at most).
    let peak = s.iter().map(|v| (*v as i32).abs()).max().unwrap();
    assert!(peak <= 32_767 && peak > 25_000, "peak {peak}");
}

#[test]
fn sample_formula_matches_spec() {
    // t = 0: both envelopes are 0 -> silence.
    assert_eq!(test_signal_sample(0), 0.0);
    // Pick t where the 2 Hz envelope peaks (t = 0.25 s) and check against the formula.
    let i = 11_025; // 0.25 s
    let t = i as f64 / 44_100.0;
    let env2 = 0.5 - 0.5 * (TAU * 2.0 * t).cos();
    let env05 = 0.5 - 0.5 * (TAU * 0.5 * t).cos();
    let want = 0.8 * env2 * (TAU * 55.0 * t).sin() + 0.2 * env05 * (TAU * 4000.0 * t).sin();
    assert!((test_signal_sample(i) - want).abs() < 1e-12);
    assert!((env2 - 1.0).abs() < 1e-12);
}

#[test]
fn committed_wav_matches_generator() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/test_signal.wav");
    let mut r = hound::WavReader::open(&path).expect("tests/golden/test_signal.wav");
    let spec = r.spec();
    assert_eq!(spec.channels, 2);
    assert_eq!(spec.sample_rate, 44_100);
    assert_eq!(spec.bits_per_sample, 16);
    assert_eq!(spec.sample_format, hound::SampleFormat::Int);
    let got: Vec<i16> = r.samples::<i16>().map(Result::unwrap).collect();
    let want = test_signal();
    assert_eq!(got.len(), want.len());
    // libm sin/cos may differ by an ulp across platforms; allow 1 LSB.
    let worst = got
        .iter()
        .zip(&want)
        .map(|(a, b)| (*a as i32 - *b as i32).abs())
        .max()
        .unwrap();
    assert!(worst <= 1, "max sample difference {worst}");
}

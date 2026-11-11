//! Writes `tests/golden/test_signal.wav` (see `md_testkit::test_signal`).
//! Run: `cargo run -p md-testkit --example gen_signal [-- <out.wav>]`.
use md_testkit::{test_signal, TEST_SIGNAL_RATE};
use std::path::PathBuf;

fn main() -> Result<(), hound::Error> {
    let out = std::env::args_os().nth(1).map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../golden/test_signal.wav"),
        PathBuf::from,
    );
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: TEST_SIGNAL_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let samples = test_signal();
    let mut w = hound::WavWriter::create(&out, spec)?;
    for s in &samples {
        w.write_sample(*s)?;
    }
    w.finalize()?;
    println!(
        "wrote {} ({} frames, {} bytes)",
        out.display(),
        samples.len() / 2,
        std::fs::metadata(&out)?.len()
    );
    Ok(())
}

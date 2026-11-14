//! Task 6.9: the renderer golden suite. Every preset in
//! `tests/golden/manifest.toml` (the whole corpus under `tests/presets/`) is
//! rendered through `render_cmd::render_to_images` and each listed frame is
//! compared with its committed baseline
//! `tests/golden/rust/<preset path without .milk>/frame_<n>.png`.
//!
//! The baselines are PROVISIONAL: the entries say `driver = "static"` (the
//! Task 5.8 headless driver, no equations); Task 7.9 flips them to
//! `"visualizer"` and re-blesses. All failures are collected and reported in
//! one assertion, each with its DSSIM.
//!
//! GPU tests follow md-render's convention: without an adapter they print a
//! note and return, unless `MD_REQUIRE_GPU=1`, where they fail. Bless missing
//! or changed baselines with `MD_BLESS=1`.

use md_render::gpu_or_skip;
use milkdrop::render_cmd::{render_to_images, RenderArgs};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[derive(Deserialize)]
struct Manifest {
    defaults: Defaults,
    preset: Vec<Entry>,
}

#[derive(Deserialize)]
struct Defaults {
    width: u32,
    height: u32,
    seed: u64,
    frames: Vec<u32>,
    max_dssim: f64,
}

#[derive(Deserialize)]
struct Entry {
    path: String,
    covers: Vec<String>,
    driver: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    seed: Option<u64>,
    frames: Option<Vec<u32>>,
    max_dssim: Option<f64>,
}

fn manifest() -> Manifest {
    let text = std::fs::read_to_string(repo().join("tests/golden/manifest.toml")).expect("read");
    toml::from_str(&text).expect("parse manifest.toml")
}

fn baseline(path: &str, frame: u32) -> PathBuf {
    let stem = path.strip_suffix(".milk").unwrap_or(path);
    repo()
        .join("tests/golden/rust")
        .join(stem)
        .join(format!("frame_{frame}.png"))
}

/// The manifest is well-formed and lists every corpus preset exactly once
/// (no GPU needed).
#[test]
fn manifest_covers_the_whole_corpus() {
    let m = manifest();
    assert_eq!((m.defaults.width, m.defaults.height), (320, 240));
    let listed: Vec<&str> = m.preset.iter().map(|e| e.path.as_str()).collect();
    let unique: BTreeSet<&str> = listed.iter().copied().collect();
    assert_eq!(unique.len(), listed.len(), "duplicate manifest entries");
    let root = md_testkit::presets_dir();
    let corpus: BTreeSet<String> = md_testkit::corpus_presets()
        .iter()
        .map(|p| {
            p.strip_prefix(&root)
                .expect("under tests/presets")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    let missing: Vec<_> = corpus
        .iter()
        .filter(|p| !unique.contains(p.as_str()))
        .collect();
    let stale: Vec<_> = unique.iter().filter(|p| !corpus.contains(**p)).collect();
    assert!(
        missing.is_empty(),
        "corpus presets not in manifest: {missing:?}"
    );
    assert!(
        stale.is_empty(),
        "manifest presets not in corpus: {stale:?}"
    );
    for e in &m.preset {
        assert_eq!(
            e.driver.as_deref(),
            Some("static"),
            "{}: driver must be \"static\" until Task 7.9",
            e.path
        );
        assert!(!e.covers.is_empty(), "{}: empty covers", e.path);
    }
}

/// Ignored by default: 33 presets x 300 frames on lavapipe took ~270 s, over
/// the 180 s budget of the default gate (Task 6.9 ruling). Run explicitly with
/// `cargo test -p milkdrop --test golden_suite -- --include-ignored` (CI and the
/// hand-off gate do).
#[test]
#[ignore = "slow (~270 s on lavapipe); run with --include-ignored"]
fn golden_suite() {
    let Some(gpu) = gpu_or_skip("golden_suite") else {
        return;
    };
    let m = manifest();
    let wav = repo().join("tests/golden/test_signal.wav");
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for e in &m.preset {
        let frames = e
            .frames
            .clone()
            .unwrap_or_else(|| m.defaults.frames.clone());
        let max_dssim = e.max_dssim.unwrap_or(m.defaults.max_dssim);
        let args = RenderArgs {
            preset: repo().join("tests/presets").join(&e.path),
            wav: Some(wav.clone()),
            frames: frames.iter().copied().max().unwrap_or(1),
            size: (
                e.width.unwrap_or(m.defaults.width),
                e.height.unwrap_or(m.defaults.height),
            ),
            seed: e.seed.unwrap_or(m.defaults.seed),
            out: PathBuf::new(),
            every: 1,
        };
        let images = match render_to_images(&gpu, &args) {
            Ok(i) => i,
            Err(err) => {
                failures.push(format!("{}: render failed: {err}", e.path));
                continue;
            }
        };
        for &n in &frames {
            let Some((_, img)) = images.iter().find(|(k, _)| *k == n) else {
                failures.push(format!("{} frame {n}: not rendered", e.path));
                continue;
            };
            let golden = baseline(&e.path, n);
            let existed = golden.exists();
            let r = md_testkit::compare_png(img, &golden, max_dssim);
            checked += 1;
            if !r.passed {
                let why = if existed {
                    format!("dssim {:.5} > {max_dssim} (diff {:?})", r.dssim, r.diff_png)
                } else {
                    "missing baseline (bless with MD_BLESS=1)".to_string()
                };
                failures.push(format!("{} frame {n}: {why}", e.path));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} golden frames failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

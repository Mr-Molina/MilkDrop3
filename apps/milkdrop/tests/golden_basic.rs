//! Task 5.8: the first golden image, rendered through the library
//! (`render_cmd::render_to_images`, F55) on a headless device.
//!
//! GPU tests follow md-render's convention: without an adapter they print a
//! note and return, unless `MD_REQUIRE_GPU=1`, where they fail.
//! Bless a missing/changed golden with `MD_BLESS=1`.

use md_render::gpu_or_skip;
use milkdrop::cli::{parse_args, Command};
use milkdrop::render_cmd::{render_to_images, RenderArgs};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The golden preset: `ps2/shader_rings.milk`, a shader-only picture (rings,
/// spokes and a colour gradient drawn by its composite shader from `rad`,
/// `ang`, `uv`, `time` and the audio bands). The plan's default preset,
/// `upgraded_default.milk`, is black under the Task 5.8 driver (its image is
/// the built-in wave, drawn from Milestone 6 on), and so is every other
/// corpus preset except noise_grain, whose image is near-black grain.
const GOLDEN_PRESET: &str = "ps2/shader_rings.milk";

fn args(preset: &str, frames: u32, every: u32) -> RenderArgs {
    RenderArgs {
        preset: repo().join("tests/presets").join(preset),
        wav: Some(repo().join("tests/golden/test_signal.wav")),
        frames,
        size: (640, 480),
        seed: 7,
        out: PathBuf::new(),
        every,
    }
}

#[test]
fn golden_default_preset_frame_60() {
    let Some(gpu) = gpu_or_skip("golden_default_preset_frame_60") else {
        return;
    };
    let frames = render_to_images(&gpu, &args(GOLDEN_PRESET, 60, 60)).expect("render");
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].0, 60);
    let img = &frames[0].1;
    let lit = img
        .pixels()
        .filter(|p| p.0[..3].iter().any(|&c| c > 8))
        .count();
    assert!(lit > 1000, "frame 60 is (nearly) black: {lit} lit pixels");
    let golden = repo().join("tests/golden/rust/default_60.png");
    let r = md_testkit::compare_png(img, &golden, 0.02);
    assert!(r.passed, "dssim {} (diff {:?})", r.dssim, r.diff_png);
    // The golden discriminates: an all-black frame must fail against it.
    let black = md_render::RgbaImage::from_pixel(640, 480, image::Rgba([0, 0, 0, 255]));
    let golden_img = image::open(&golden).expect("golden").to_rgba8();
    let dssim = 1.0 / md_testkit::ssim(&black, &golden_img).max(1e-12) - 1.0;
    assert!(dssim > 0.02, "black passes: dssim {dssim}");
}

#[test]
fn rendering_twice_is_byte_identical() {
    let Some(gpu) = gpu_or_skip("rendering_twice_is_byte_identical") else {
        return;
    };
    let a = args(GOLDEN_PRESET, 20, 10);
    let one = render_to_images(&gpu, &a).expect("render 1");
    let two = render_to_images(&gpu, &a).expect("render 2");
    assert_eq!(one.len(), 2);
    for ((n1, i1), (n2, i2)) in one.iter().zip(&two) {
        assert_eq!(n1, n2);
        let (mut b1, mut b2) = (Vec::new(), Vec::new());
        let enc = |img: &md_render::RgbaImage, out: &mut Vec<u8>| {
            img.write_to(&mut std::io::Cursor::new(out), image::ImageFormat::Png)
                .expect("png")
        };
        enc(i1, &mut b1);
        enc(i2, &mut b2);
        assert_eq!(b1, b2, "frame {n1} differs between runs");
    }
}

#[test]
fn cli_parses_render_options() {
    let argv = [
        "render", "--preset", "p.milk", "--wav", "a.wav", "--frames", "120", "--size", "320x200",
        "--seed", "9", "--out", "o", "--every", "30",
    ]
    .map(String::from);
    let Command::Render(a) = parse_args(&argv).expect("parse") else {
        panic!("not render");
    };
    assert_eq!(a.preset, PathBuf::from("p.milk"));
    assert_eq!(a.wav, Some(PathBuf::from("a.wav")));
    assert_eq!(
        (a.frames, a.size, a.seed, a.every),
        (120, (320, 200), 9, 30)
    );
    assert_eq!(a.out, PathBuf::from("o"));
    // --every defaults to the last frame only; bad input is an error.
    let argv = ["render", "--preset", "p", "--frames", "5", "--out", "o"].map(String::from);
    let Command::Render(a) = parse_args(&argv).unwrap() else {
        panic!()
    };
    assert_eq!((a.every, a.size, a.seed, a.wav), (5, (640, 480), 0, None));
    assert!(parse_args(&["render".to_string(), "--size".into(), "10".into()]).is_err());
    assert!(parse_args(&["render".to_string()]).is_err());
}

#[test]
fn run_writes_the_requested_pngs() {
    let Some(_gpu) = gpu_or_skip("run_writes_the_requested_pngs") else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("milkdrop-run-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let argv: Vec<String> = [
        "render",
        "--preset",
        repo()
            .join("tests/presets")
            .join(GOLDEN_PRESET)
            .to_str()
            .unwrap(),
        "--wav",
        repo()
            .join("tests/golden/test_signal.wav")
            .to_str()
            .unwrap(),
        "--frames",
        "7",
        "--every",
        "3",
        "--size",
        "64x48",
        "--seed",
        "7",
        "--out",
        dir.to_str().unwrap(),
    ]
    .map(String::from)
    .to_vec();
    let Command::Render(a) = parse_args(&argv).expect("parse") else {
        panic!("not render");
    };
    let written = milkdrop::render_cmd::run(&a).expect("run");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("out dir")
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["frame_3.png", "frame_6.png"]);
    assert_eq!(written.len(), 2);
    for n in &names {
        let img = image::open(dir.join(n)).expect("decode");
        assert_eq!((img.width(), img.height()), (64, 48));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_numeric_flags_name_the_flag() {
    let e = parse_args(&["render".to_string(), "--frames".into(), "x".into()]).unwrap_err();
    assert!(e.contains("--frames"), "{e}");
    let e =
        parse_args(&["render".to_string(), "--every".into(), "99999999999".into()]).unwrap_err();
    assert!(e.contains("--every"), "{e}");
}

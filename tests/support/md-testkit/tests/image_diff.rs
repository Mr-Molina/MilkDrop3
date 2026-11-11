use image::{Rgba, RgbaImage};
use md_testkit::{compare_png_with, FixedClock};
use std::path::PathBuf;

/// Textured test image: low-frequency sinusoids (no noise), 64x64.
/// Smooth content, so a 1px shift is a small change. Adding +-3 hash noise
/// pushed dssim to ~0.057 (uncorrelated noise dominates flat windows), so the
/// calibration image is noise-free.
fn textured(w: u32, h: u32, shift: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        let x = x + shift;
        let (fx, fy) = (x as f64, y as f64);
        let base = 128.0
            + 60.0 * (fx * 0.11).sin() * (fy * 0.07).cos()
            + 30.0 * (fy * 0.05 + fx * 0.03).sin();
        let v = base.clamp(0.0, 255.0) as u8;
        Rgba([v, v.wrapping_add(20), 255 - v, 255])
    })
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("md-testkit-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn identical_images_have_zero_dssim() {
    let dir = tmp("ident");
    let g = dir.join("g.png");
    let img = textured(64, 64, 0);
    img.save(&g).unwrap();
    let r = compare_png_with(&img, &g, 0.0001, false);
    assert!(r.dssim.abs() < 1e-9, "dssim {}", r.dssim);
    assert!(r.passed);
    assert!(r.diff_png.is_none());
}

#[test]
fn shifted_by_one_pixel_is_under_0_01() {
    let dir = tmp("shift");
    let g = dir.join("g.png");
    textured(65, 64, 0).save(&g).unwrap();
    let actual = textured(65, 64, 1);
    let golden = image::open(&g).unwrap().to_rgba8();
    // crop both to 64 wide so only the content shift differs
    let a = image::imageops::crop_imm(&actual, 0, 0, 64, 64).to_image();
    let gg = image::imageops::crop_imm(&golden, 0, 0, 64, 64).to_image();
    gg.save(&g).unwrap();
    let r = compare_png_with(&a, &g, 0.01, false);
    assert!(r.dssim > 0.0, "shift should be detectable");
    eprintln!("shift dssim = {}", r.dssim);
    assert!(r.dssim < 0.01, "dssim {}", r.dssim);
    assert!(r.passed);
}

#[test]
fn different_images_fail_and_write_diff() {
    let dir = tmp("diff");
    let g = dir.join("g.png");
    textured(64, 64, 0).save(&g).unwrap();
    let other = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 255]));
    let r = compare_png_with(&other, &g, 0.01, false);
    assert!(!r.passed);
    assert!(r.diff_png.as_ref().is_some_and(|p| p.exists()));
}

#[test]
fn missing_golden_fails_unless_bless() {
    let dir = tmp("missing");
    let g = dir.join("new.png");
    let img = textured(32, 32, 0);
    let r = compare_png_with(&img, &g, 0.01, false);
    assert!(!r.passed);
    assert!(!g.exists());
    let r = compare_png_with(&img, &g, 0.01, true);
    assert!(r.passed);
    assert!(g.exists());
    let r = compare_png_with(&img, &g, 0.0001, false);
    assert!(r.passed);
}

#[test]
fn size_mismatch_fails() {
    let dir = tmp("size");
    let g = dir.join("g.png");
    textured(32, 32, 0).save(&g).unwrap();
    assert!(!compare_png_with(&textured(16, 16, 0), &g, 1.0, false).passed);
}

#[test]
fn fixed_clock_advances_by_frame() {
    let mut c = FixedClock::new(60.0);
    assert_eq!(c.now(), 0.0);
    c.advance();
    c.advance();
    assert!((c.now() - 2.0 / 60.0).abs() < 1e-12);
}

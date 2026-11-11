//! Golden-image comparison using a self-contained windowed SSIM
//! (Gaussian 11x11, sigma 1.5, K1=0.01, K2=0.03, L=255) over luma.
//! `dssim = 1/ssim - 1`, so identical images give 0.

use image::{Rgba, RgbaImage};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiffResult {
    pub dssim: f64,
    pub passed: bool,
    pub diff_png: Option<PathBuf>,
}

/// True when env `MD_BLESS=1`: missing/failing goldens are (re)written.
pub fn bless_enabled() -> bool {
    std::env::var("MD_BLESS").is_ok_and(|v| v == "1")
}

pub fn compare_png(actual: &RgbaImage, golden_path: &Path, max_dssim: f64) -> DiffResult {
    compare_png_with(actual, golden_path, max_dssim, bless_enabled())
}

/// Like [`compare_png`] with the bless flag explicit (no env access).
pub fn compare_png_with(
    actual: &RgbaImage,
    golden_path: &Path,
    max_dssim: f64,
    bless: bool,
) -> DiffResult {
    let write_golden = || {
        if let Some(parent) = golden_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        actual.save(golden_path).expect("write golden png");
    };
    let golden = match image::open(golden_path) {
        Ok(g) => g.to_rgba8(),
        Err(_) => {
            if bless {
                write_golden();
                return DiffResult {
                    dssim: 0.0,
                    passed: true,
                    diff_png: None,
                };
            }
            return DiffResult {
                dssim: f64::INFINITY,
                passed: false,
                diff_png: None,
            };
        }
    };
    let dssim = if golden.dimensions() == actual.dimensions() {
        let s = ssim(actual, &golden);
        1.0 / s.max(1e-12) - 1.0
    } else {
        f64::INFINITY
    };
    if dssim <= max_dssim {
        return DiffResult {
            dssim,
            passed: true,
            diff_png: None,
        };
    }
    if bless {
        write_golden();
        return DiffResult {
            dssim,
            passed: true,
            diff_png: None,
        };
    }
    let diff_path = golden_path.with_extension("diff.png");
    let diff = diff_image(actual, &golden);
    let diff_png = diff.save(&diff_path).ok().map(|_| diff_path);
    DiffResult {
        dssim,
        passed: false,
        diff_png,
    }
}

/// Per-pixel absolute difference, alpha forced opaque. Sized to the larger image.
fn diff_image(a: &RgbaImage, b: &RgbaImage) -> RgbaImage {
    let (w, h) = (a.width().max(b.width()), a.height().max(b.height()));
    RgbaImage::from_fn(w, h, |x, y| {
        match (a.get_pixel_checked(x, y), b.get_pixel_checked(x, y)) {
            (Some(p), Some(q)) => Rgba([
                p[0].abs_diff(q[0]),
                p[1].abs_diff(q[1]),
                p[2].abs_diff(q[2]),
                255,
            ]),
            _ => Rgba([255, 0, 255, 255]),
        }
    })
}

const RADIUS: usize = 5; // 11x11 window
const SIGMA: f64 = 1.5;

fn luma(img: &RgbaImage) -> Vec<f64> {
    img.pixels()
        .map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]))
        .collect()
}

fn kernel(radius: usize) -> Vec<f64> {
    let mut k: Vec<f64> = (0..=2 * radius)
        .map(|i| {
            let d = i as f64 - radius as f64;
            (-(d * d) / (2.0 * SIGMA * SIGMA)).exp()
        })
        .collect();
    let s: f64 = k.iter().sum();
    k.iter_mut().for_each(|v| *v /= s);
    k
}

/// Separable Gaussian filter, "valid" region only: output is (w-2r) x (h-2r).
fn blur(src: &[f64], w: usize, h: usize, k: &[f64], r: usize) -> Vec<f64> {
    let (ow, oh) = (w - 2 * r, h - 2 * r);
    let mut tmp = vec![0.0; ow * h];
    for y in 0..h {
        for x in 0..ow {
            tmp[y * ow + x] = k
                .iter()
                .enumerate()
                .map(|(i, kv)| kv * src[y * w + x + i])
                .sum();
        }
    }
    let mut out = vec![0.0; ow * oh];
    for y in 0..oh {
        for x in 0..ow {
            out[y * ow + x] = k
                .iter()
                .enumerate()
                .map(|(i, kv)| kv * tmp[(y + i) * ow + x])
                .sum();
        }
    }
    out
}

/// Mean SSIM (1.0 = identical) of two equally sized images, over luma.
/// Images smaller than the window use the largest odd window that fits.
pub fn ssim(a: &RgbaImage, b: &RgbaImage) -> f64 {
    assert_eq!(a.dimensions(), b.dimensions(), "ssim needs equal sizes");
    let (w, h) = (a.width() as usize, a.height() as usize);
    if w == 0 || h == 0 {
        return 1.0;
    }
    let r = RADIUS.min((w.min(h) - 1) / 2);
    let k = kernel(r);
    let (la, lb) = (luma(a), luma(b));
    let sq = |v: &[f64]| v.iter().map(|x| x * x).collect::<Vec<_>>();
    let prod: Vec<f64> = la.iter().zip(&lb).map(|(x, y)| x * y).collect();
    let mu_a = blur(&la, w, h, &k, r);
    let mu_b = blur(&lb, w, h, &k, r);
    let e_aa = blur(&sq(&la), w, h, &k, r);
    let e_bb = blur(&sq(&lb), w, h, &k, r);
    let e_ab = blur(&prod, w, h, &k, r);
    let c1 = (0.01_f64 * 255.0).powi(2);
    let c2 = (0.03_f64 * 255.0).powi(2);
    let n = mu_a.len();
    let total: f64 = (0..n)
        .map(|i| {
            let (ma, mb) = (mu_a[i], mu_b[i]);
            let va = e_aa[i] - ma * ma;
            let vb = e_bb[i] - mb * mb;
            let cov = e_ab[i] - ma * mb;
            ((2.0 * ma * mb + c1) * (2.0 * cov + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2))
        })
        .sum();
    total / n as f64
}

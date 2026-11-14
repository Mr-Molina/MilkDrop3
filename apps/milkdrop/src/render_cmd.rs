//! The `render` subcommand: drive the headless visualizer for N frames and
//! return or write PNGs.

use crate::headless::{Headless, HeadlessError};
use md_render::{Gpu, RgbaImage};
use std::path::PathBuf;

/// `milkdrop render` options.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderArgs {
    pub preset: PathBuf,
    /// Audio input; silence when `None`.
    pub wav: Option<PathBuf>,
    pub frames: u32,
    pub size: (u32, u32),
    pub seed: u64,
    pub out: PathBuf,
    /// Keep frame `n` when `n % every == 0`.
    pub every: u32,
}

/// Render errors.
#[derive(Debug, thiserror::Error)]
pub enum RenderCmdError {
    #[error(transparent)]
    Headless(#[from] HeadlessError),
    #[error("no GPU adapter available (set WGPU_BACKEND / WGPU_ADAPTER_NAME to choose one)")]
    NoGpu,
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Render `args.frames` frames on `gpu`; returns `(n, image)` for every kept
/// frame (`n` counts from 1, `n % args.every == 0`), in order.
pub fn render_to_images(
    gpu: &Gpu,
    args: &RenderArgs,
) -> Result<Vec<(u32, RgbaImage)>, RenderCmdError> {
    let mut driver = Headless::new(gpu, args)?;
    let every = args.every.max(1);
    let mut kept = Vec::new();
    for n in 1..=args.frames {
        let image = driver.render_frame();
        if n % every == 0 {
            kept.push((n, image));
        }
    }
    Ok(kept)
}

/// Open a headless device, render, and write `frame_<n>.png` files into
/// `args.out`. Returns the written paths.
pub fn run(args: &RenderArgs) -> Result<Vec<PathBuf>, RenderCmdError> {
    let gpu = md_render::headless_gpu().ok_or(RenderCmdError::NoGpu)?;
    let frames = render_to_images(&gpu, args)?;
    let werr =
        |path: &PathBuf, e: Box<dyn std::error::Error + Send + Sync>| RenderCmdError::Write {
            path: path.clone(),
            source: e,
        };
    std::fs::create_dir_all(&args.out).map_err(|e| werr(&args.out, e.into()))?;
    let mut written = Vec::new();
    for (n, image) in frames {
        let path = args.out.join(format!("frame_{n}.png"));
        image.save(&path).map_err(|e| werr(&path, e.into()))?;
        written.push(path);
    }
    Ok(written)
}

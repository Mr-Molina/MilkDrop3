//! Command-line parsing (hand-rolled: one subcommand, no extra dependency).

use crate::render_cmd::RenderArgs;
use std::path::PathBuf;

/// The usage text.
pub const USAGE: &str = "usage: milkdrop render --preset <file> [--wav <file>] --frames <n> \
[--size WxH] [--seed <u64>] --out <dir> [--every <k>]\n\
\n\
Renders <n> frames of the preset headlessly (audio from the WAV, silence without one) and \
writes every <k>-th as <dir>/frame_<n>.png (default: only the last frame).";

/// A parsed command line.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Render(RenderArgs),
    Help,
}

/// Parse the arguments after the program name.
pub fn parse_args(argv: &[String]) -> Result<Command, String> {
    let mut it = argv.iter();
    match it.next().map(String::as_str) {
        Some("render") => parse_render(&argv[1..]),
        Some("-h" | "--help" | "help") | None => Ok(Command::Help),
        Some(other) => Err(format!("unknown subcommand `{other}`")),
    }
}

fn parse_render(argv: &[String]) -> Result<Command, String> {
    let (mut preset, mut wav, mut frames, mut out) = (None, None, None, None);
    let (mut size, mut seed, mut every) = ((640, 480), 0u64, None);
    let mut it = argv.iter();
    while let Some(flag) = it.next() {
        if flag == "-h" || flag == "--help" {
            return Ok(Command::Help);
        }
        let mut value = || {
            it.next()
                .ok_or_else(|| format!("{flag} needs a value"))
                .map(String::as_str)
        };
        let num = |s: &str| s.parse::<u64>().map_err(|e| format!("{flag}: {e}"));
        match flag.as_str() {
            "--preset" => preset = Some(PathBuf::from(value()?)),
            "--wav" => wav = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--frames" => {
                frames = Some(u32::try_from(num(value()?)?).map_err(|e| format!("{flag}: {e}"))?)
            }
            "--every" => {
                every = Some(u32::try_from(num(value()?)?).map_err(|e| format!("{flag}: {e}"))?)
            }
            "--seed" => seed = num(value()?)?,
            "--size" => size = parse_size(value()?)?,
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    let frames = frames.ok_or("--frames is required")?;
    if frames == 0 {
        return Err("--frames must be at least 1".into());
    }
    let every = every.unwrap_or(frames);
    if every == 0 {
        return Err("--every must be at least 1".into());
    }
    Ok(Command::Render(RenderArgs {
        preset: preset.ok_or("--preset is required")?,
        wav,
        frames,
        size,
        seed,
        out: out.ok_or("--out is required")?,
        every,
    }))
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let bad = || format!("--size expects WxH (e.g. 640x480), got `{s}`");
    let (w, h) = s.split_once(['x', 'X']).ok_or_else(bad)?;
    let (w, h) = (
        w.parse::<u32>().map_err(|_| bad())?,
        h.parse::<u32>().map_err(|_| bad())?,
    );
    if w == 0 || h == 0 {
        return Err(bad());
    }
    Ok((w, h))
}

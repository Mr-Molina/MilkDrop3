//! The corpus translation loop shared by `tests/corpus.rs`
//! (`every_corpus_shader_translates_and_validates`) and
//! `examples/triage.rs`: every corpus preset (`md_testkit::corpus_presets`,
//! which includes `$MD_EXTRA_PRESETS`) with a non-empty warp or comp shader,
//! plus the shipped default shader bodies, through [`md_hlsl::translate`].
//!
//! Self-contained (no `super::` items) so the example can include it with
//! `#[path]`.
#![allow(dead_code)]

use md_hlsl::{translate, PsVersion, ShaderInput, ShaderKind};
use std::collections::BTreeMap;
use std::path::Path;

/// One shader the corpus run translates.
pub struct CorpusShader {
    /// Preset file name (or the default shader's asset name).
    pub preset: String,
    /// `"warp"` or `"comp"`.
    pub shader: &'static str,
    pub kind: ShaderKind,
    pub ps_version: PsVersion,
    pub body: String,
}

/// One failure: a preset that did not parse (`shader == "preset"`, line 0)
/// or a shader that did not translate.
pub struct Failure {
    pub preset: String,
    pub shader: &'static str,
    pub line: u32,
    pub col: u32,
    pub message: String,
}

/// The outcome of a corpus run.
pub struct Report {
    /// Shaders handed to `translate`.
    pub translated: usize,
    /// Presets read (each contributes up to two shaders).
    pub presets: usize,
    pub failures: Vec<Failure>,
}

pub fn asset_path(name: &str) -> String {
    format!("{}/../../assets/hlsl/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn read_asset(name: &str) -> String {
    let p = asset_path(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn hlsl_version(v: md_preset::PsVersion) -> PsVersion {
    match v {
        md_preset::PsVersion::None => PsVersion::None,
        md_preset::PsVersion::Ps2_0 => PsVersion::Ps2_0,
        md_preset::PsVersion::Ps2_X => PsVersion::Ps2_X,
        md_preset::PsVersion::Ps3_0 => PsVersion::Ps3_0,
        md_preset::PsVersion::Ps4_0 => PsVersion::Ps4_0,
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Every shader of the corpus: the shipped defaults first, then each
/// preset's non-empty warp and comp text (with its `PSVERSION_*`). Presets
/// that fail to parse are returned as failures.
pub fn corpus_shaders() -> (Vec<CorpusShader>, usize, Vec<Failure>) {
    let mut shaders = vec![
        CorpusShader {
            preset: "warp_ps.fx".into(),
            shader: "warp",
            kind: ShaderKind::Warp,
            ps_version: PsVersion::Ps2_0,
            body: read_asset("warp_ps.fx"),
        },
        CorpusShader {
            preset: "comp_ps.fx".into(),
            shader: "comp",
            kind: ShaderKind::Comp,
            ps_version: PsVersion::Ps2_0,
            body: read_asset("comp_ps.fx"),
        },
    ];
    let mut failures = Vec::new();
    let files = md_testkit::corpus_presets();
    for path in &files {
        let preset = display_name(path);
        let text = match std::fs::read(path) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => {
                failures.push(Failure {
                    preset,
                    shader: "preset",
                    line: 0,
                    col: 0,
                    message: format!("read: {e}"),
                });
                continue;
            }
        };
        let p = match md_preset::parse_milk(&text) {
            Ok(p) => p,
            Err(e) => {
                failures.push(Failure {
                    preset,
                    shader: "preset",
                    line: 0,
                    col: 0,
                    message: format!("parse_milk: {e}"),
                });
                continue;
            }
        };
        let (wv, cv) = (p.warp_ps_version(), p.comp_ps_version());
        let warp = (ShaderKind::Warp, "warp", wv, p.warp_shader);
        let comp = (ShaderKind::Comp, "comp", cv, p.comp_shader);
        for (kind, shader, v, body) in [warp, comp] {
            if body.trim().is_empty() {
                continue;
            }
            shaders.push(CorpusShader {
                preset: preset.clone(),
                shader,
                kind,
                ps_version: hlsl_version(v),
                body,
            });
        }
    }
    (shaders, files.len(), failures)
}

/// Translate the whole corpus against `assets/hlsl/include.fx`.
pub fn run_corpus() -> Report {
    let preamble = read_asset("include.fx");
    let (shaders, presets, mut failures) = corpus_shaders();
    for s in &shaders {
        let r = translate(&ShaderInput {
            kind: s.kind,
            body: &s.body,
            preamble: &preamble,
            ps_version: s.ps_version,
        });
        if let Err(e) = r {
            failures.push(Failure {
                preset: s.preset.clone(),
                shader: s.shader,
                line: e.line,
                col: e.col,
                message: e.message,
            });
        }
    }
    Report {
        translated: shaders.len(),
        presets,
        failures,
    }
}

/// `preset | shader | line:col | message` (first line of each message).
pub fn failure_table(failures: &[Failure]) -> String {
    let mut out = String::from("preset | shader | line:col | message\n");
    for f in failures {
        let msg = f.message.lines().next().unwrap_or("");
        out += &format!(
            "{} | {} | {}:{} | {msg}\n",
            f.preset, f.shader, f.line, f.col
        );
    }
    out
}

/// A message's grouping key: its first line with every quoted span
/// (`'...'` or `` `...` ``) replaced by `'_'` and every digit run by `#`,
/// so `unknown function 'foo'` and `unknown function 'bar'` group together.
pub fn message_prefix(message: &str) -> String {
    let line = message.lines().next().unwrap_or("");
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '`' => {
                if line.matches(c).count() >= 2 {
                    for d in chars.by_ref() {
                        if d == c {
                            break;
                        }
                    }
                    out.push(c);
                    out.push('_');
                    out.push(c);
                } else {
                    out.push(c);
                }
            }
            '0'..='9' => {
                while chars.peek().is_some_and(|d| d.is_ascii_digit()) {
                    chars.next();
                }
                out.push('#');
            }
            _ => out.push(c),
        }
    }
    out
}

/// Failures grouped by [`message_prefix`], most frequent first (ties by
/// prefix).
pub fn group_failures(failures: &[Failure]) -> Vec<(String, usize)> {
    let mut groups: BTreeMap<String, usize> = BTreeMap::new();
    for f in failures {
        *groups.entry(message_prefix(&f.message)).or_default() += 1;
    }
    let mut v: Vec<_> = groups.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

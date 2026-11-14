//! Corpus code-block extraction shared by the corpus tests.
//!
//! Blocks are assembled the way MilkDrop does it: `ReadCode`
//! (state.cpp:1071) reads `<prefix>1`, `<prefix>2`, ... until a key is
//! missing, strips one leading backtick, and `StripLinefeedCharsAndComments`
//! (state.cpp:1524) drops `//` and `\\` comments to end of line and joins the
//! lines with no separator. A minimal scan; real preset parsing is
//! md-preset's job.

#![allow(dead_code)]

use std::collections::HashMap;

pub fn prefixes() -> Vec<String> {
    let mut p: Vec<String> = ["per_frame_init_", "per_frame_", "per_pixel_"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for i in 0..4 {
        for kind in ["init", "per_frame", "per_point"] {
            p.push(format!("wave_{i}_{kind}"));
        }
        for kind in ["init", "per_frame"] {
            p.push(format!("shape_{i}_{kind}"));
        }
    }
    p
}

/// `key=value` (or `key value`) lines; the first occurrence of a key wins.
pub fn keys(text: &str) -> HashMap<&str, &str> {
    let mut map = HashMap::new();
    for line in text.lines() {
        if let Some(k) = line.find(['=', ' ']) {
            map.entry(&line[..k]).or_insert(&line[k + 1..]);
        }
    }
    map
}

pub fn strip_comment(line: &str) -> &str {
    let b = line.as_bytes();
    for i in 0..b.len().saturating_sub(1) {
        if (b[i] == b'/' && b[i + 1] == b'/') || (b[i] == b'\\' && b[i + 1] == b'\\') {
            return &line[..i];
        }
    }
    line
}

/// The code block for `prefix`, or `None` if `<prefix>1` is missing.
pub fn code_block(map: &HashMap<&str, &str>, prefix: &str) -> Option<String> {
    let mut code = String::new();
    let mut n = 1;
    while let Some(line) = map.get(format!("{prefix}{n}").as_str()) {
        let line = line.strip_prefix('`').unwrap_or(line);
        code.push_str(strip_comment(line));
        n += 1;
    }
    (n > 1).then_some(code)
}

/// One extracted code block of one corpus preset.
pub struct Block {
    pub path: std::path::PathBuf,
    pub prefix: String,
    pub code: String,
}

/// Every code block of every corpus preset.
pub fn corpus_blocks() -> Vec<Block> {
    let presets = md_testkit::corpus_presets();
    assert!(!presets.is_empty(), "corpus is empty");
    let mut out = Vec::new();
    for path in presets {
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let text = text.trim_start_matches('\u{feff}');
        let map = keys(text);
        for prefix in prefixes() {
            if let Some(code) = code_block(&map, &prefix) {
                out.push(Block {
                    path: path.clone(),
                    prefix,
                    code,
                });
            }
        }
    }
    out
}

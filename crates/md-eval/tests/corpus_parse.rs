//! Every EEL code block in the corpus must lex and parse.
//!
//! Code blocks are assembled the way MilkDrop does it: `ReadCode`
//! (state.cpp:1071) reads `<prefix>1`, `<prefix>2`, ... until a key is
//! missing, strips one leading backtick, and `StripLinefeedCharsAndComments`
//! (state.cpp:1524) drops `//` and `\\` comments to end of line and joins the
//! lines with no separator. A minimal scan; real preset parsing is
//! md-preset's job.

use std::collections::HashMap;

use md_eval::{parse, Lexer};

fn prefixes() -> Vec<String> {
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
fn keys(text: &str) -> HashMap<&str, &str> {
    let mut map = HashMap::new();
    for line in text.lines() {
        if let Some(k) = line.find(['=', ' ']) {
            map.entry(&line[..k]).or_insert(&line[k + 1..]);
        }
    }
    map
}

fn strip_comment(line: &str) -> &str {
    let b = line.as_bytes();
    for i in 0..b.len().saturating_sub(1) {
        if (b[i] == b'/' && b[i + 1] == b'/') || (b[i] == b'\\' && b[i + 1] == b'\\') {
            return &line[..i];
        }
    }
    line
}

/// The code block for `prefix`, or `None` if `<prefix>1` is missing.
fn code_block(map: &HashMap<&str, &str>, prefix: &str) -> Option<String> {
    let mut code = String::new();
    let mut n = 1;
    while let Some(line) = map.get(format!("{prefix}{n}").as_str()) {
        let line = line.strip_prefix('`').unwrap_or(line);
        code.push_str(strip_comment(line));
        n += 1;
    }
    (n > 1).then_some(code)
}

#[test]
fn corpus_code_blocks_parse() {
    let presets = md_testkit::corpus_presets();
    assert!(!presets.is_empty(), "corpus is empty");
    let mut blocks = 0;
    let mut failures = Vec::new();
    for path in presets {
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let text = text.trim_start_matches('\u{feff}');
        let map = keys(text);
        for prefix in prefixes() {
            let Some(code) = code_block(&map, &prefix) else {
                continue;
            };
            blocks += 1;
            if let Err(e) = Lexer::tokenize(&code).and_then(|t| parse(&t)) {
                failures.push(format!("{}: {prefix}: {e}\n    {code}", path.display()));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {blocks} blocks failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(blocks > 20, "only {blocks} code blocks found");
}

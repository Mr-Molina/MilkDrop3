//! Every per-frame / per-pixel / init code block in the corpus must lex.
//! Minimal `.milk` scanning only; real preset parsing is md-preset's job.

use std::collections::BTreeMap;

use md_eval::Lexer;

fn is_code_key(key: &str) -> bool {
    key.contains("per_frame")
        || key.contains("per_pixel")
        || key.contains("per_point")
        || key.ends_with("_init")
        || key.contains("_init_")
}

#[test]
fn corpus_code_blocks_lex() {
    let presets = md_testkit::corpus_presets();
    assert!(!presets.is_empty(), "corpus is empty");
    let mut blocks = 0;
    for path in presets {
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let text = text.trim_start_matches('\u{feff}');
        // group lines `<key>_<n>=code` by key, in file order
        let mut groups: BTreeMap<String, Vec<(u32, String)>> = BTreeMap::new();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let Some((stem, n)) = k.rsplit_once('_') else {
                continue;
            };
            let Ok(n) = n.parse::<u32>() else { continue };
            if is_code_key(stem) {
                groups
                    .entry(stem.to_string())
                    .or_default()
                    .push((n, v.to_string()));
            }
        }
        for (stem, mut lines) in groups {
            lines.sort_by_key(|(n, _)| *n);
            let code: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();
            let code = code.join("\n");
            if let Err(e) = Lexer::tokenize(&code) {
                panic!("{}: {stem}: {e}", path.display());
            }
            blocks += 1;
        }
    }
    assert!(blocks > 0, "no code blocks found");
}

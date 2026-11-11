//! Shared helpers for the corpus tests (`corpus.rs`, `write.rs`).

use std::path::PathBuf;

/// Every corpus preset (`md_testkit::corpus_presets`) with its text, read as
/// lossy UTF-8. Asserts the committed corpus is present.
pub fn corpus_texts() -> Vec<(PathBuf, String)> {
    let files = md_testkit::corpus_presets();
    assert!(files.len() >= 30, "only {} corpus presets", files.len());
    files
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let text = String::from_utf8_lossy(&bytes).into_owned();
            (path, text)
        })
        .collect()
}

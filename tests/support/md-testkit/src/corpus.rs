use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("milk"))
        {
            out.push(p);
        }
    }
}

/// Every `*.milk` under `<workspace>/tests/presets/`, sorted. Empty if the
/// directory does not exist (e.g. LFS corpus not checked out).
pub fn corpus_presets() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets");
    let mut out = Vec::new();
    walk(&root, &mut out);
    out.sort();
    out
}

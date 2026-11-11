use std::path::{Path, PathBuf};

/// Environment variable naming an extra directory of `.milk` files (for
/// example the Winamp/projectM preset pack) to add to the corpus.
pub const EXTRA_PRESETS_ENV: &str = "MD_EXTRA_PRESETS";

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

/// `<workspace>/tests/presets`, canonicalized when it exists.
pub fn presets_dir() -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets");
    std::fs::canonicalize(&p).unwrap_or(p)
}

/// Every `*.milk` (case-insensitive extension) under `root` and, if given,
/// under `extra`; canonicalized, sorted and deduplicated. Missing directories
/// contribute nothing.
pub fn corpus_presets_from(root: &Path, extra: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, &mut out);
    if let Some(extra) = extra {
        walk(extra, &mut out);
    }
    let mut out: Vec<PathBuf> = out
        .into_iter()
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Every `*.milk` under `<workspace>/tests/presets/`, plus every `*.milk`
/// under `$MD_EXTRA_PRESETS` when that variable names a directory; sorted and
/// deduplicated.
pub fn corpus_presets() -> Vec<PathBuf> {
    let extra = std::env::var_os(EXTRA_PRESETS_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    corpus_presets_from(&presets_dir(), extra.as_deref())
}

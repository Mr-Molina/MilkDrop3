//! Build-time translator fingerprint (ledger Task 5.5 ruling): SHA-256 of
//! every file of this crate's `src/`, its `Cargo.toml` and `assets/hlsl/`,
//! exported as `MD_HLSL_FINGERPRINT` for [`TRANSLATOR_FINGERPRINT`]. Pure
//! Rust (sha2 only): compiles no foreign code.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dirs = [root.join("src"), root.join("../../assets/hlsl")];
    let mut all = vec![root.join("Cargo.toml")];
    for d in &dirs {
        println!("cargo:rerun-if-changed={}", d.display());
        files(d, &mut all);
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    // Hash relative paths (stable across checkouts) in sorted order.
    let mut entries: Vec<(String, PathBuf)> = all
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            (rel, p)
        })
        .collect();
    entries.sort();
    let mut h = Sha256::new();
    for (rel, p) in &entries {
        println!("cargo:rerun-if-changed={}", p.display());
        let data = std::fs::read(p).unwrap_or_default();
        h.update((rel.len() as u64).to_le_bytes());
        h.update(rel.as_bytes());
        h.update((data.len() as u64).to_le_bytes());
        h.update(&data);
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    println!("cargo:rustc-env=MD_HLSL_FINGERPRINT={hex}");
}

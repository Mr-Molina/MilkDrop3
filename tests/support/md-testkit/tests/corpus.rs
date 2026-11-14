//! Corpus sanity: size, A.4 feature coverage, and the MD_EXTRA_PRESETS merge.
use md_testkit::{corpus_presets, corpus_presets_from, presets_dir};
use std::path::{Path, PathBuf};

fn read(p: &Path) -> String {
    let bytes = std::fs::read(p).unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn corpus_texts() -> Vec<(PathBuf, String)> {
    corpus_presets()
        .into_iter()
        .map(|p| {
            let t = read(&p);
            (p, t)
        })
        .collect()
}

fn line_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|l| {
        let l = l.trim_start_matches('\u{feff}');
        l.strip_prefix(key)
            .and_then(|r| r.strip_prefix('='))
            .map(|v| v.trim_end_matches('\r'))
    })
}

/// File-level PSVERSION as the C++ loader sees it: no MILKDROP_PRESET_VERSION
/// (or < 200) means a MilkDrop 1 preset, PS version 0.
fn ps_version(text: &str) -> i32 {
    let ver: i32 = line_value(text, "MILKDROP_PRESET_VERSION")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(100);
    if ver < 200 {
        return 0;
    }
    line_value(text, "PSVERSION")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(2)
}

#[test]
fn corpus_has_at_least_25_presets() {
    let n = corpus_presets().len();
    println!("corpus presets: {n}");
    assert!(n >= 25, "corpus has only {n} presets");
}

#[test]
fn corpus_covers_every_a4_feature() {
    let texts = corpus_texts();
    let any = |pred: &dyn Fn(&str) -> bool| texts.iter().any(|(_, t)| pred(t));
    let mut missing = Vec::new();
    let mut check = |name: &str, ok: bool| {
        if !ok {
            missing.push(name.to_string());
        }
    };

    for v in [0, 2, 3, 4] {
        check(&format!("PSVERSION {v}"), any(&|t| ps_version(t) == v));
    }
    for needle in [
        "tex3D(",
        "tex2Dbias(",
        "GetBlur1(",
        "GetBlur2(",
        "GetBlur3(",
        "sampler_noisevol_hq",
        "sampler_mytex",
        "megabuf(",
        "gmegabuf(",
        "loop(",
        "while(",
        "reg00",
        "if(",
        "equal(",
        "above(",
        "below(",
        "sqr(",
        "sqrt(",
        "sin(",
        "rand(",
    ] {
        check(needle, any(&|t| t.contains(needle)));
    }
    // sampler_randNN, optionally with a filter/wrap prefix (sampler_fc_randNN, ...).
    for i in 0..16 {
        let s = format!("sampler_rand{i:02}");
        let found = any(&|t| {
            ["", "fc_", "fw_", "pc_", "pw_"]
                .iter()
                .any(|pre| t.contains(&format!("sampler_{pre}rand{i:02}")))
        });
        check(&s, found);
    }
    for q in 1..=32 {
        let s = format!("q{q}");
        check(
            &s,
            any(&|t| {
                t.match_indices(&s).any(|(i, _)| {
                    let before = t[..i].chars().next_back();
                    let after = t[i + s.len()..].chars().next();
                    !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                        && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                })
            }),
        );
    }
    check(
        "custom wave enabled with per-point code",
        any(&|t| {
            (0..4).any(|i| {
                line_value(t, &format!("wavecode_{i}_enabled")) == Some("1")
                    && t.contains(&format!("wave_{i}_per_point1="))
            })
        }),
    );
    check(
        "custom shape enabled with per-frame code",
        any(&|t| {
            (0..4).any(|i| {
                line_value(t, &format!("shapecode_{i}_enabled")) == Some("1")
                    && t.contains(&format!("shape_{i}_per_frame1="))
            })
        }),
    );
    check(
        "user texture file textures/mytex.png",
        presets_dir().join("textures/mytex.png").is_file(),
    );

    // Review Focus #3: CRLF line endings + UTF-8 BOM + trailing junk after the section.
    let crlf_bom = corpus_presets().into_iter().any(|p| {
        let b = std::fs::read(&p).unwrap();
        let t = String::from_utf8_lossy(&b);
        b.starts_with(&[0xEF, 0xBB, 0xBF])
            && t.contains("\r\n")
            && !t.replace("\r\n", "").contains('\n')
            && t.trim_end()
                .lines()
                .last()
                .is_some_and(|l| !l.contains('='))
    });
    check("CRLF + BOM + trailing junk fixture", crlf_bom);

    assert!(missing.is_empty(), "corpus lacks features: {missing:?}");
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("md-testkit-corpus-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn extra_dir_presets_are_merged_sorted_and_deduped() {
    let root = scratch("root");
    let extra = scratch("extra");
    std::fs::write(root.join("b.milk"), "[preset00]\n").unwrap();
    std::fs::create_dir_all(extra.join("pack/sub")).unwrap();
    std::fs::write(extra.join("pack/sub/c.MILK"), "[preset00]\n").unwrap();
    std::fs::write(extra.join("a.milk"), "[preset00]\n").unwrap();
    std::fs::write(extra.join("notes.txt"), "x").unwrap();

    let got = corpus_presets_from(&root, Some(&extra));
    let names: Vec<_> = got
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 3, "{got:?}");
    let mut sorted = got.clone();
    sorted.sort();
    assert_eq!(got, sorted);
    for n in ["a.milk", "b.milk", "c.MILK"] {
        assert!(names.iter().any(|x| x == n), "{n} missing from {names:?}");
    }

    // Same directory given as root and extra (different spelling) => no duplicates.
    let again = corpus_presets_from(
        &root,
        Some(&root.join("../").join(root.file_name().unwrap())),
    );
    assert_eq!(again.len(), 1, "{again:?}");

    // No extra dir, or a missing one, yields just the root.
    assert_eq!(corpus_presets_from(&root, None).len(), 1);
    assert_eq!(
        corpus_presets_from(&root, Some(&root.join("nope"))).len(),
        1
    );
}

#[test]
fn default_corpus_root_is_tests_presets() {
    let d = presets_dir();
    assert!(d.ends_with("tests/presets"), "{}", d.display());
    let from_pure = corpus_presets_from(&d, None);
    assert!(!from_pure.is_empty());
}

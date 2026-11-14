//! Tests for the preset library and history (Task 7.4). Each test works in
//! its own directory under the system temp dir.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rand_core::SeedableRng;
use rand_pcg::Pcg32;

use crate::history::{History, PRESET_HIST_LEN};
use crate::library::PresetLibrary;
use md_preset::PsVersion;

const PS: PsVersion = PsVersion::Ps3_0;

struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "md-core-lib-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
    fn file(&self, name: &str, body: &str) {
        fs::write(self.0.join(name), body).unwrap();
    }
    fn milk(&self, name: &str, rating: f32) {
        self.file(
            name,
            &format!("MILKDROP_PRESET_VERSION=201\nPSVERSION=2\n[preset00]\nfRating={rating:.3}\nfDecay=0.5\n"),
        );
    }
    fn dir(&self, name: &str) {
        fs::create_dir_all(self.0.join(name)).unwrap();
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn names(lib: &PresetLibrary) -> Vec<&str> {
    lib.entries().iter().map(|e| e.name.as_str()).collect()
}

/// A library with one subdirectory, `hi.milk` (5) and `lo.milk` (1).
fn rated() -> (Tmp, PresetLibrary) {
    let t = Tmp::new();
    t.dir("sub");
    t.milk("hi.milk", 5.0);
    t.milk("lo.milk", 1.0);
    let lib = PresetLibrary::scan(&t.0, PS);
    (t, lib)
}

#[test]
fn dirs_sort_before_files_and_are_starred() {
    let t = Tmp::new();
    t.milk("b.milk", 3.0);
    t.milk("A.milk", 3.0);
    t.file("readme.txt", "x");
    t.dir("zeta");
    t.dir("alpha");
    let lib = PresetLibrary::scan(&t.0, PS);
    assert_eq!(names(&lib), ["*..", "*alpha", "*zeta", "A.milk", "b.milk"]);
    assert_eq!(lib.dir_count(), 3);
    assert!(lib.entries()[..3].iter().all(|e| e.is_dir));
    assert!(lib.entries()[3..].iter().all(|e| !e.is_dir));
}

#[test]
fn sort_is_case_insensitive() {
    // mystrcmpiW folds A-Z to a-z, so '_' (95) sorts before letters, and a
    // space maps to 255 and sorts after everything (plugin.cpp:556-572).
    let t = Tmp::new();
    for n in ["B.milk", "a.milk", "C.milk", "_x.milk", "a b.milk"] {
        t.milk(n, 3.0);
    }
    let lib = PresetLibrary::scan(&t.0, PS);
    assert_eq!(
        names(&lib)[1..],
        ["_x.milk", "a.milk", "a b.milk", "B.milk", "C.milk"]
    );
}

#[test]
fn only_milk_extension_any_case_hidden_included() {
    let t = Tmp::new();
    t.milk("up.MILK", 3.0);
    t.milk(".hidden.milk", 3.0);
    t.milk(".milk", 3.0);
    t.file("a.milk.bak", "x");
    t.file("b.txt", "x");
    t.file("milk", "x");
    let lib = PresetLibrary::scan(&t.0, PS);
    assert_eq!(names(&lib)[1..], [".hidden.milk", ".milk", "up.MILK"]);
}

#[test]
fn dotdot_only_when_not_at_root() {
    let t = Tmp::new();
    let lib = PresetLibrary::scan(&t.0, PS);
    assert_eq!(names(&lib), ["*.."]);
    let root = PresetLibrary::scan(Path::new("/"), PS);
    assert!(!names(&root).contains(&"*.."));
}

#[test]
fn rating_cdf_prefers_high_ratings() {
    let (_t, lib) = rated();
    // cum = [0 (..), 0 (sub), 5 (hi), 6 (lo)]: weight is the rating, 5:1.
    let cum: Vec<f32> = lib.entries().iter().map(|e| e.rating_cum).collect();
    assert_eq!(cum, [0.0, 0.0, 5.0, 6.0]);
    let mut rng = Pcg32::seed_from_u64(7);
    let (mut hi, mut lo) = (0u32, 0u32);
    for _ in 0..10_000 {
        match lib.entries()[lib.pick_random(&mut rng, true).unwrap()]
            .name
            .as_str()
        {
            "hi.milk" => hi += 1,
            "lo.milk" => lo += 1,
            other => panic!("picked {other}"),
        }
    }
    let ratio = f64::from(hi) / f64::from(lo);
    assert!((4.5..5.5).contains(&ratio), "ratio {ratio} ({hi}:{lo})");
}

#[test]
fn uniform_when_ratings_disabled() {
    let (_t, lib) = rated();
    let mut rng = Pcg32::seed_from_u64(7);
    let (mut hi, mut lo) = (0u32, 0u32);
    for _ in 0..10_000 {
        let i = lib.pick_random(&mut rng, false).unwrap();
        assert!(i >= lib.dir_count(), "directory picked");
        if lib.entries()[i].name == "hi.milk" {
            hi += 1;
        } else {
            lo += 1;
        }
    }
    assert!((4500..5500).contains(&hi), "{hi}:{lo}");
}

#[test]
fn uniform_when_cumulative_rating_below_threshold() {
    // all ratings 0.05 -> cum 0.05 < 0.1: uniform even with ratings on.
    let t = Tmp::new();
    t.dir("d");
    t.milk("a.milk", 0.05);
    t.milk("b.milk", 0.0);
    let lib = PresetLibrary::scan(&t.0, PS);
    let mut rng = Pcg32::seed_from_u64(3);
    let b = (0..2000)
        .filter(|_| lib.entries()[lib.pick_random(&mut rng, true).unwrap()].name == "b.milk")
        .count();
    assert!((800..1200).contains(&b), "{b}");
}

#[test]
fn pick_random_none_without_files() {
    let t = Tmp::new();
    t.dir("d");
    let lib = PresetLibrary::scan(&t.0, PS);
    assert_eq!(lib.pick_random(&mut Pcg32::seed_from_u64(1), true), None);
}

#[test]
fn rating_read_default_clamp_and_position() {
    let t = Tmp::new();
    t.file("none.milk", "[preset00]\nfDecay=0.9\n");
    t.milk("big.milk", 9.0);
    t.milk("neg.milk", -2.0);
    t.file(
        "late.milk",
        &format!("[preset00]\n{}fRating=4.5\n", "fX=1\n".repeat(2000)),
    );
    t.file("ci.milk", "[PRESET00]\nfrating = 2.5\n");
    let lib = PresetLibrary::scan(&t.0, PS);
    let r = |n: &str| lib.entries().iter().find(|e| e.name == n).unwrap().rating;
    assert_eq!(r("none.milk"), 3.0);
    assert_eq!(r("big.milk"), 5.0);
    assert_eq!(r("neg.milk"), 0.0);
    assert_eq!(r("late.milk"), 4.5);
    assert_eq!(r("ci.milk"), 2.5);
    assert_eq!(r("*.."), 0.0);
}

#[test]
fn sequential_wraps_past_dirs() {
    let (_t, lib) = rated(); // [*.., *sub, hi, lo]
    assert_eq!(lib.dir_count(), 2);
    assert_eq!(lib.next_sequential(Some(2)), Some(3));
    assert_eq!(
        lib.next_sequential(Some(3)),
        Some(2),
        "wraps to the first file, past dirs"
    );
    assert_eq!(lib.next_sequential(None), Some(2), "no current preset (-1)");
    assert_eq!(lib.prev_sequential(Some(3)), Some(2));
    assert_eq!(
        lib.prev_sequential(Some(2)),
        Some(3),
        "before the first file wraps to the last"
    );
    assert_eq!(lib.prev_sequential(None), Some(3));
}

#[test]
fn sequential_without_files_is_none() {
    let t = Tmp::new();
    let lib = PresetLibrary::scan(&t.0, PS);
    assert_eq!(lib.next_sequential(None), None);
    assert_eq!(lib.prev_sequential(Some(0)), None);
}

#[test]
fn ps_version_cap_skips_presets_above_it() {
    let t = Tmp::new();
    let hdr = |v: &str| format!("MILKDROP_PRESET_VERSION=201\n{v}\n[preset00]\nfRating=3\n");
    t.file("v0.milk", &hdr("PSVERSION=0"));
    t.file("v2.milk", &hdr("PSVERSION=2"));
    t.file("v3.milk", &hdr("PSVERSION=3"));
    t.file("v4.milk", &hdr("PSVERSION=4"));
    t.file("warp_line2.milk", &hdr("PSVERSION_WARP=4")); // unreadable number: 2
    t.file(
        "missing.milk",
        "MILKDROP_PRESET_VERSION=201\n[preset00]\nfRating=3\n",
    );
    t.file("md1.milk", "[preset00]\nPSVERSION=9\nfRating=3\n"); // no header: runs
    let list = |cap| {
        let lib = PresetLibrary::scan(&t.0, cap);
        names(&lib)[1..]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        list(PsVersion::Ps2_0),
        [
            "md1.milk",
            "missing.milk",
            "v0.milk",
            "v2.milk",
            "warp_line2.milk"
        ]
    );
    assert_eq!(list(PsVersion::Ps2_X).len(), 6);
    assert_eq!(list(PsVersion::Ps3_0).len(), 7);
    assert_eq!(list(PsVersion::None).len(), 3 /* md1, missing, v0 */);
}

#[cfg(unix)]
#[test]
fn unopenable_milk_is_skipped() {
    // A dangling symlink is listed by read_dir but cannot be opened (this
    // also holds when the tests run as root, where mode 000 would not).
    let t = Tmp::new();
    t.milk("ok.milk", 3.0);
    std::os::unix::fs::symlink(t.0.join("missing-target"), t.0.join("dangling.milk")).unwrap();
    let lib = PresetLibrary::scan(&t.0, PS);
    assert_eq!(names(&lib)[1..], ["ok.milk"]);
}

#[test]
fn relative_dir_gets_dotdot_without_empty_path() {
    // "." is relative: it is not a root, and its parent entry must not be "".
    let lib = PresetLibrary::scan(Path::new("."), PS);
    assert_eq!(lib.entries()[0].name, "*..");
    assert_eq!(lib.path_of(0), Some(PathBuf::from("./..")));
}

#[test]
fn find_prefix_is_case_insensitive_and_skips_dirs() {
    let t = Tmp::new();
    t.dir("alpha");
    for n in ["Apple.milk", "apricot.milk", "banana.milk"] {
        t.milk(n, 3.0);
    }
    let lib = PresetLibrary::scan(&t.0, PS); // [.., alpha, Apple, apricot, banana]
    let at = |i: usize| lib.entries()[i].name.as_str();
    let i = lib.find_prefix("a", 0).unwrap();
    assert_eq!(at(i), "Apple.milk", "the directory *alpha is not a match");
    assert_eq!(at(lib.find_prefix("AP", 0).unwrap()), "Apple.milk");
    assert_eq!(at(lib.find_prefix("apr", 0).unwrap()), "apricot.milk");
    assert_eq!(at(lib.find_prefix("a", i + 1).unwrap()), "apricot.milk");
    assert_eq!(at(lib.find_prefix("b", 0).unwrap()), "banana.milk");
    assert_eq!(lib.find_prefix("z", 0), None);
    assert_eq!(lib.find_prefix("a", 99), None);
}

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[test]
fn history_back_then_forward_returns_same() {
    let mut h = History::new();
    assert_eq!(h.back(), None);
    assert_eq!(h.forward(), None);
    for n in ["a", "b", "c"] {
        h.push(&p(n)); // c is current
    }
    assert_eq!(h.back(), Some(p("b").as_path()));
    assert_eq!(h.back(), Some(p("a").as_path()));
    assert_eq!(h.back(), None, "at the back fence");
    assert_eq!(h.forward(), Some(p("b").as_path()));
    assert_eq!(h.forward(), Some(p("c").as_path()));
    assert_eq!(h.forward(), None, "at the frontier: pick a new one");
    h.push(&p("d"));
    assert_eq!(h.back(), Some(p("c").as_path()));
}

#[test]
fn history_ring_keeps_65_entries_and_stores_duplicates() {
    let mut h = History::new();
    for i in 0..200 {
        h.push(&p(&format!("p{i}")));
    }
    let mut steps = 0;
    while h.back().is_some() {
        steps += 1;
    }
    assert_eq!(steps, PRESET_HIST_LEN - 2);
    let mut d = History::new();
    d.push(&p("x"));
    d.push(&p("x"));
    assert_eq!(d.back(), Some(p("x").as_path()), "no duplicate suppression");
}

#[test]
fn history_menu_load_mid_history_is_not_recorded_and_reset_reseeds() {
    let mut h = History::new();
    for n in ["a", "b", "c"] {
        h.push(&p(n));
    }
    h.back(); // at b
    h.push_user_load(&p("menu")); // pos -> c's slot, not the frontier: not stored
    assert_eq!(h.forward(), None, "frontier reached");
    h.reset(&p("cur"));
    assert_eq!(h.back(), None);
    assert_eq!(h.forward(), None);
    h.push(&p("n1"));
    assert_eq!(h.back(), Some(p("cur").as_path()));
}

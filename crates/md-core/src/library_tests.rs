//! Tests for the preset library and history (Task 7.4). Each test works in
//! its own directory under the system temp dir.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use rand_core::SeedableRng;
use rand_pcg::Pcg32;

use crate::history::{History, PRESET_HIST_LEN};
use crate::library::PresetLibrary;

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
    let lib = PresetLibrary::scan(&t.0, true);
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
    let lib = PresetLibrary::scan(&t.0, true);
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
    let lib = PresetLibrary::scan(&t.0, true);
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
    let lib = PresetLibrary::scan(&t.0, true);
    assert_eq!(names(&lib)[1..], [".hidden.milk", ".milk", "up.MILK"]);
}

#[test]
fn dotdot_only_when_not_at_root() {
    let t = Tmp::new();
    let lib = PresetLibrary::scan(&t.0, true);
    assert_eq!(names(&lib), ["*.."]);
    let root = PresetLibrary::scan(Path::new("/"), true);
    assert!(!names(&root).contains(&"*.."));
}

#[test]
fn rating_cdf_prefers_high_ratings() {
    // cum = [0 (..), 0 (sub), 5 (hi), 6 (lo)]. The weight is the rating, so
    // the ideal ratio is 5:1; but the C++ draws `rand() % 14345` from a
    // 15-bit rand(), and 32768 % 14345 = 4078 makes the low residues 3/2
    // likelier. P(hi) = (3*4078 + 2*(11955-4078)) / 32768 = 0.8541, so the
    // C++ ratio is 5.85:1 (computed analytically), not 5:1.
    let (_t, lib) = rated();
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
    assert!((5.3..6.4).contains(&ratio), "ratio {ratio} ({hi}:{lo})");
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
    let lib = PresetLibrary::scan(&t.0, true);
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
    let lib = PresetLibrary::scan(&t.0, true);
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
    let lib = PresetLibrary::scan(&t.0, true);
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
    assert_eq!(lib.next_sequential(2), 3);
    assert_eq!(
        lib.next_sequential(3),
        2,
        "wraps to the first file, past dirs"
    );
    assert_eq!(lib.next_sequential(usize::MAX), 2, "no current preset (-1)");
    assert_eq!(lib.prev_sequential(3), 2);
    assert_eq!(
        lib.prev_sequential(2),
        3,
        "before the first file wraps to the last"
    );
    assert_eq!(lib.prev_sequential(usize::MAX), 3);
}

#[test]
fn read_more_ratings_progresses_n_per_call() {
    let t = Tmp::new();
    for (n, r) in [("a", 1.0), ("b", 2.0), ("c", 4.0), ("d", 5.0)] {
        t.milk(&format!("{n}.milk"), r);
    }
    let mut lib = PresetLibrary::scan(&t.0, false);
    assert!(lib.ratings_pending());
    let read = |lib: &PresetLibrary| lib.entries().iter().map(|e| e.rating).collect::<Vec<_>>();
    assert_eq!(read(&lib), [0.0; 5]);
    lib.read_more_ratings(2);
    assert_eq!(read(&lib), [0.0, 1.0, 2.0, 0.0, 0.0]);
    let cum: Vec<f32> = lib.entries().iter().map(|e| e.rating_cum).collect();
    assert_eq!(cum, [0.0, 1.0, 3.0, 3.0, 3.0]);
    lib.read_more_ratings(1);
    assert_eq!(read(&lib), [0.0, 1.0, 2.0, 4.0, 0.0]);
    assert!(lib.ratings_pending());
    lib.read_more_ratings(10);
    assert_eq!(read(&lib), [0.0, 1.0, 2.0, 4.0, 5.0]);
    assert!(!lib.ratings_pending());
    lib.read_more_ratings(10); // no-op
    assert_eq!(lib.entries()[4].rating_cum, 12.0);
}

#[test]
fn find_prefix_is_case_insensitive_and_skips_dirs() {
    let t = Tmp::new();
    t.dir("alpha");
    for n in ["Apple.milk", "apricot.milk", "banana.milk"] {
        t.milk(n, 3.0);
    }
    let lib = PresetLibrary::scan(&t.0, true); // [.., alpha, Apple, apricot, banana]
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

//! The preset list: scan, sort, ratings and selection (`UpdatePresetList`,
//! `LoadRandomPreset`, `NextPreset`/`PrevPreset`, `SeekToPreset`).
//!
//! Line citations are into `code/vis_milk2/plugin.cpp`. Departures from the
//! C++ are listed in the task report; the ones that change behaviour are
//! marked `Departure:` below.

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use md_preset::{Ini, PsVersion};
use rand_core::Rng;

/// `GetPrivateProfileFloatW(L"preset00", L"fRating", 3.0f, ...)`
/// (plugin.cpp:7609).
const DEFAULT_RATING: f32 = 3.0;
/// The cumulative rating must reach this for the CDF to be used
/// (plugin.cpp:6990).
const MIN_CUM_RATING: f32 = 0.1;
/// Bytes read for the cheap rating probe before falling back to the whole
/// file (the C++ probes `PRESET_HEADER_SCAN_BYTES` = 160, plugin.cpp:7544).
const HEADER_PROBE_BYTES: usize = 2048;

/// One row of the preset list (`PresetInfo`: `szFilename`, `fRatingThis`,
/// `fRatingCum`).
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// File name; directories carry the `*` prefix the C++ displays
    /// (plugin.cpp:7521), and the parent entry is `*..`.
    pub name: String,
    pub is_dir: bool,
    /// `fRatingThis`, in `[0, 5]`; `0` for directories.
    pub rating: f32,
    /// `fRatingCum`: running sum of `rating` (see [`PresetLibrary::scan`]).
    pub rating_cum: f32,
}

/// The sorted contents of one preset directory.
#[derive(Clone, Debug)]
pub struct PresetLibrary {
    dir: PathBuf,
    entries: Vec<Entry>,
    dirs: usize,
}

impl PresetLibrary {
    /// Scan `dir` (`__UpdatePresetList`, plugin.cpp:7418-7732).
    ///
    /// Entries are directories (starred, plus `*..` unless `dir` is a
    /// filesystem root) and files whose name ends in `.milk` (any ASCII case,
    /// length >= 5, so `.milk` itself qualifies; hidden files are not
    /// filtered; plugin.cpp:7527-7528). Directories sort first, then each
    /// group by [`cmp_names`] (plugin.cpp:7807-7875). Every file's `fRating`
    /// is read now (plugin.cpp:7539-7610, the live code).
    ///
    /// A `.milk` that cannot be opened is skipped (plugin.cpp:7540-7542). A
    /// file whose first line starts with `MILKDROP_PRESET_VERSION` and whose
    /// second line starts with `PSVERSION` is skipped when that version
    /// exceeds `max_ps` (plugin.cpp:7566-7580; the cap is
    /// `m_nMaxPSVersion`, plugin.cpp:1562-1570). The number is read at byte 10
    /// of the line as `%d`, defaulting to 2 when unreadable; a missing
    /// `PSVERSION` line never skips, and a file without the version header
    /// is a MilkDrop 1 preset and always runs (plugin.cpp:7582-7586).
    ///
    /// `rating_cum[i]` is `rating_cum[i-1] + rating[i]` with
    /// `rating_cum[0] = 0` (plugin.cpp:7700-7702). That formula leaves out
    /// entry 0's own rating, so at a filesystem root with no subdirectory the
    /// first file's rating never counts. It is reproduced as is: below the
    /// root entry 0 is always the `*..` directory, whose rating is 0.
    ///
    /// Departure: a missing or unreadable directory gives an empty library
    /// (the C++ falls back to `FindValidPresetDir`, plugin.cpp:7440, 7478,
    /// 7689).
    pub fn scan(dir: &Path, max_ps: PsVersion) -> PresetLibrary {
        let max_ps = match max_ps {
            PsVersion::None => 0,
            PsVersion::Ps2_0 => 2,
            PsVersion::Ps2_X => 3,
            PsVersion::Ps3_0 => 4,
            PsVersion::Ps4_0 => 5,
        };
        let mut entries = Vec::new();
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let path = e.path();
                if path.is_dir() {
                    // follows links, like FindFirstFile
                    entries.push(entry(format!("*{name}"), true, 0.0));
                } else if is_milk_name(&name) {
                    if let Some(rating) = read_file_info(&path, max_ps) {
                        entries.push(entry(name, false, rating));
                    }
                }
            }
            // FindFirstFile reports ".." in every directory but a drive root.
            let abs = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
            if abs.parent().is_some() {
                entries.push(entry("*..".to_string(), true, 0.0));
            }
        }
        // `MergeSortPresets` is a stable merge sort; so is `sort_by`.
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => cmp_names(&a.name, &b.name),
        });
        let dirs = entries.iter().filter(|e| e.is_dir).count();
        let mut lib = PresetLibrary {
            dir: dir.to_path_buf(),
            entries,
            dirs,
        };
        lib.rebuild_cum();
        lib
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// `m_nDirs`: the first file is `entries()[dir_count()]`.
    pub fn dir_count(&self) -> usize {
        self.dirs
    }

    /// The scanned directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Full path of entry `index` (`m_szPresetDir` + name; plugin.cpp:7033-
    /// 7035). A directory entry has no leading star; `*..` is the parent.
    pub fn path_of(&self, index: usize) -> Option<PathBuf> {
        let e = self.entries.get(index)?;
        Some(match (e.is_dir, e.name.strip_prefix('*')) {
            (true, Some("..")) => match self.dir.parent() {
                Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                _ => self.dir.join(".."), // relative dir: no empty path
            },
            (true, Some(n)) => self.dir.join(n),
            _ => self.dir.join(&e.name),
        })
    }

    fn rebuild_cum(&mut self) {
        let mut prev = 0.0;
        for (i, e) in self.entries.iter_mut().enumerate() {
            e.rating_cum = if i == 0 { 0.0 } else { prev + e.rating };
            prev = e.rating_cum;
        }
    }

    /// Random pick (`LoadRandomPreset`, plugin.cpp:6990-7030). Never returns
    /// a directory; may return the current preset again (the C++ does not
    /// exclude it); `None` when there is no file.
    ///
    /// Uniform unless `use_ratings` and the last `rating_cum` is at least 0.1
    /// (plugin.cpp:6990). Otherwise `cdf_pos` is a uniform draw in
    /// `[0, total)` (plugin.cpp:6996) and the entry is the one whose
    /// cumulative interval holds it: first file if `cdf_pos < cum[first
    /// file]`, else the binary search at plugin.cpp:7016-7028. A file is
    /// weighted by its rating, not a power.
    ///
    /// Randomness comes from the caller's generator (A.4); the CRT's
    /// `rand() % 14345` / `rand() % n` shapes are not reproduced.
    pub fn pick_random(&self, rng: &mut impl Rng, use_ratings: bool) -> Option<usize> {
        let n = self.entries.len();
        let files = n - self.dirs;
        if files == 0 {
            return None;
        }
        let total = self.entries[n - 1].rating_cum;
        if !use_ratings || total < MIN_CUM_RATING {
            return Some(self.dirs + below(rng, files as u64) as usize);
        }
        let unit = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64; // [0, 1)
        let cdf_pos = (unit * f64::from(total)) as f32;
        if cdf_pos < self.entries[self.dirs].rating_cum {
            return Some(self.dirs);
        }
        let (mut lo, mut hi) = (self.dirs, n);
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if self.entries[mid].rating_cum > cdf_pos {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        // Departure: float rounding could leave hi == n; the C++ would read
        // past the list there.
        Some(hi.min(n - 1))
    }

    /// Sequential "next" (`LoadRandomPreset` with `m_bSequentialPresetOrder`,
    /// plugin.cpp:6981-6986): `current + 1`, wrapping to the first file when
    /// it falls on a directory or past the end. `current` is `None` when
    /// there is no current preset (the C++'s -1, set when the directory
    /// changes, plugin.cpp:6167). `None` result: the library has no file.
    pub fn next_sequential(&self, current: Option<usize>) -> Option<usize> {
        if self.entries.len() == self.dirs {
            return None;
        }
        let next = current.map_or(0, |c| c.saturating_add(1));
        Some(if next < self.dirs || next >= self.entries.len() {
            self.dirs
        } else {
            next
        })
    }

    /// Sequential "previous" (`PrevPreset`, plugin.cpp:6898-6904):
    /// `current - 1`, wrapping to the last entry when it falls on a
    /// directory (`None` current counts as -1, so it wraps too). `None`
    /// result: the library has no file.
    pub fn prev_sequential(&self, current: Option<usize>) -> Option<usize> {
        if self.entries.len() == self.dirs {
            return None;
        }
        let n = self.entries.len();
        Some(match current.and_then(|c| c.checked_sub(1)) {
            Some(p) if p >= n => self.dirs, // "just in case" (plugin.cpp:6903)
            Some(p) if p >= self.dirs => p,
            _ => n - 1,
        })
    }

    /// Load-menu type-to-find (`SeekToPreset`, plugin.cpp:7359-7375): the
    /// first file at or after `from` whose name starts with `typed`, ignoring
    /// ASCII case. The C++ takes one typed letter and always searches from
    /// the first file (`from` = 0 gives that); directories never match.
    pub fn find_prefix(&self, typed: &str, from: usize) -> Option<usize> {
        let t = typed.as_bytes();
        (from.max(self.dirs)..self.entries.len()).find(|&i| {
            self.entries[i]
                .name
                .as_bytes()
                .get(..t.len())
                .is_some_and(|h| h.eq_ignore_ascii_case(t))
        })
    }
}

fn entry(name: String, is_dir: bool, rating: f32) -> Entry {
    Entry {
        name,
        is_dir,
        rating,
        rating_cum: 0.0,
    }
}

/// `len >= 5 && wcsicmp(name + len - 5, ".milk") == 0` (plugin.cpp:7527).
fn is_milk_name(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 5 && b[b.len() - 5..].eq_ignore_ascii_case(b".milk")
}

/// `mystrcmpiW` (plugin.cpp:714-737), the sort comparator. Its `LC2UC` table
/// (plugin.cpp:556-572), despite the name, maps `A-Z` to `a-z`, space to 255
/// and everything else to itself; characters are compared by that value, a
/// shorter prefix first. UTF-16 code units there, Unicode scalars here.
pub fn cmp_names(a: &str, b: &str) -> Ordering {
    fn fold(c: char) -> u32 {
        match c {
            ' ' => 255,
            'A'..='Z' => c as u32 + 32,
            _ => c as u32,
        }
    }
    a.chars().map(fold).cmp(b.chars().map(fold))
}

/// Uniform integer in `[0, n)` without modulo bias (rejection sampling).
fn below(rng: &mut impl Rng, n: u64) -> u64 {
    let zone = u64::MAX - (u64::MAX % n + 1) % n;
    loop {
        let v = rng.next_u64();
        if v <= zone {
            return v % n;
        }
    }
}

/// Header probe of one `.milk` (plugin.cpp:7539-7610): `None` skips the file
/// (unopenable, plugin.cpp:7540-7542, or `PSVERSION` above `max_ps`),
/// otherwise its `fRating` as `GetPrivateProfileFloatW(preset00, fRating,
/// 3.0)` clamped to `[0, 5]` (plugin.cpp:7609-7610). The first 2 KiB serve
/// both the version check and the rating (the C++ probes 160 bytes); when the
/// rating is not in the probe the whole file is parsed, which is exact
/// because the first occurrence of a key wins.
fn read_file_info(path: &Path, max_ps: i32) -> Option<f32> {
    let mut f = File::open(path).ok()?;
    let mut buf = vec![0u8; HEADER_PROBE_BYTES];
    let mut got = 0;
    while got < buf.len() {
        match f.read(&mut buf[got..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => got += k,
        }
    }
    buf.truncate(got);
    let probe_full = got == HEADER_PROBE_BYTES;
    let head = String::from_utf8_lossy(&buf).into_owned();
    if ps_version_exceeds(&head, max_ps) {
        return None;
    }
    let get = |text: &str| {
        let ini = Ini::parse(text);
        ini.get("preset00", "fRating")
            .map(|_| ini.get_f32("preset00", "fRating", DEFAULT_RATING))
    };
    let mut rating = None;
    if probe_full {
        // keep whole lines only
        if let Some(cut) = head.rfind('\n') {
            rating = get(&head[..cut]);
        }
        if rating.is_none() {
            let _ = f.read_to_end(&mut buf);
            rating = get(&String::from_utf8_lossy(&buf));
        }
    } else {
        rating = get(&head);
    }
    let r = rating.unwrap_or(DEFAULT_RATING);
    Some(if r.is_nan() { 0.0 } else { r.clamp(0.0, 5.0) })
}

/// plugin.cpp:7566-7580: line 1 starts with `MILKDROP_PRESET_VERSION`, line 2
/// with `PSVERSION`, and the `%d` at byte 10 of line 2 (default 2) is above
/// the cap.
fn ps_version_exceeds(head: &str, max_ps: i32) -> bool {
    let mut lines = head.split('\n').map(|l| l.trim_end_matches('\r'));
    if !lines
        .next()
        .is_some_and(|l| l.starts_with("MILKDROP_PRESET_VERSION"))
    {
        return false;
    }
    let Some(l2) = lines.next().filter(|l| l.starts_with("PSVERSION")) else {
        return false;
    };
    let v = scan_int(l2.get(10..).unwrap_or("")).unwrap_or(2);
    v > max_ps
}

/// `sscanf("%d")`: optional leading whitespace, sign, digits.
fn scan_int(s: &str) -> Option<i32> {
    let s = s.trim_start();
    let b = s.as_bytes();
    let sign = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let digits = b[sign..].iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    s[..sign + digits].parse().ok()
}

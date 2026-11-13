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

use md_preset::Ini;
use rand_core::Rng;

/// `GetPrivateProfileFloatW(L"preset00", L"fRating", 3.0f, ...)`
/// (plugin.cpp:7609).
const DEFAULT_RATING: f32 = 3.0;
/// The cumulative rating must reach this for the CDF to be used
/// (plugin.cpp:6990).
const MIN_CUM_RATING: f32 = 0.1;
/// `rand() % 14345` granularity of the CDF draw (plugin.cpp:6996).
const CDF_STEPS: u32 = 14345;
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
    /// `fRatingThis`, in `[0, 5]`; `0` for directories and for files whose
    /// rating has not been read yet.
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
    /// Index of the next entry whose rating `read_more_ratings` will read.
    progress: usize,
}

impl PresetLibrary {
    /// Scan `dir` (`__UpdatePresetList`, plugin.cpp:7418-7732).
    ///
    /// Entries are directories (starred, plus `*..` when `dir` has a parent)
    /// and files whose name ends in `.milk` (any ASCII case, length >= 5, so
    /// `.milk` itself qualifies; hidden files are not filtered; plugin.cpp:
    /// 7527-7528). Directories sort first, then each group by [`cmp_names`]
    /// (plugin.cpp:7807-7875). With `insta_scan_ratings` every file's
    /// `fRating` is read now; otherwise ratings stay 0 until
    /// [`read_more_ratings`](Self::read_more_ratings) reads them.
    ///
    /// `rating_cum[i]` is `rating_cum[i-1] + rating[i]` with
    /// `rating_cum[0] = 0` (plugin.cpp:7700-7702). That formula leaves out
    /// entry 0's own rating, so at a filesystem root with no subdirectory the
    /// first file's rating never counts. It is reproduced as is: below the
    /// root entry 0 is always the `*..` directory, whose rating is 0.
    ///
    /// Departure: a missing or unreadable directory gives an empty library
    /// (the C++ falls back to `FindValidPresetDir`, plugin.cpp:7440, 7478, 7689);
    /// there is no `PSVERSION` filter (`nMaxPSVersion`, plugin.cpp:7573).
    pub fn scan(dir: &Path, insta_scan_ratings: bool) -> PresetLibrary {
        let mut entries = Vec::new();
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let is_dir = e.path().is_dir(); // follows links, like FindFirstFile
                if is_dir {
                    entries.push(entry(format!("*{name}"), true));
                } else if is_milk_name(&name) {
                    entries.push(entry(name, false));
                }
            }
            // FindFirstFile reports ".." in every directory but a drive root.
            if dir.parent().is_some() {
                entries.push(entry("*..".to_string(), true));
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
            progress: dirs,
        };
        if insta_scan_ratings {
            lib.read_more_ratings(usize::MAX);
        }
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
            (true, Some("..")) => self.dir.parent()?.to_path_buf(),
            (true, Some(n)) => self.dir.join(n),
            _ => self.dir.join(&e.name),
        })
    }

    /// True while some file's rating is still unread.
    pub fn ratings_pending(&self) -> bool {
        self.progress < self.entries.len()
    }

    /// Read the ratings of the next `n` unread files (directories are
    /// skipped without counting) and rebuild the cumulative sums. This is the
    /// progressive reader of the disabled `UpdatePresetRatings`
    /// (plugin.cpp:8268-8307, `presets_per_frame` = 1 or 4096 for
    /// `m_bInstaScan`).
    ///
    /// Departure: the C++ live code reads every rating during the scan
    /// (plugin.cpp:7539-7610) and `UpdatePresetRatings` is commented out; and
    /// that dead code spends its first call on the directories and returns
    /// unless insta-scan is on. Here directories cost nothing and the first
    /// call already reads `n` files.
    pub fn read_more_ratings(&mut self, n: usize) {
        let end = self.progress.saturating_add(n).min(self.entries.len());
        for i in self.progress.min(end)..end {
            if !self.entries[i].is_dir {
                let path = self.dir.join(&self.entries[i].name);
                self.entries[i].rating = read_rating(&path);
            }
        }
        self.progress = self.progress.max(end);
        self.rebuild_cum();
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
    /// (plugin.cpp:6990). Otherwise `cdf_pos = (rand % 14345) / 14345 *
    /// total` and the entry is the one whose cumulative interval holds it:
    /// first file if `cdf_pos < cum[first file]`, else the binary search at
    /// plugin.cpp:7016-7028. A file is weighted by its rating, not a power.
    ///
    /// Departure: `rand()` is MSVC's 15-bit value; the CDF draw uses 15 bits
    /// of the generator to keep that granularity, but the uniform pick uses
    /// all 32 bits so libraries over 32768 files stay reachable.
    pub fn pick_random(&self, rng: &mut impl Rng, use_ratings: bool) -> Option<usize> {
        let n = self.entries.len();
        let files = n - self.dirs;
        if files == 0 {
            return None;
        }
        let total = self.entries[n - 1].rating_cum;
        if !use_ratings || total < MIN_CUM_RATING {
            return Some(self.dirs + (rng.next_u32() as usize % files));
        }
        let draw = (rng.next_u32() >> 17) % CDF_STEPS;
        let cdf_pos = draw as f32 / CDF_STEPS as f32 * total;
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
    /// it falls on a directory or past the end. `current` may be
    /// `usize::MAX` for the C++'s -1 ("no current preset", set when the
    /// directory changes, plugin.cpp:6167).
    pub fn next_sequential(&self, current: usize) -> usize {
        let next = (current as isize).wrapping_add(1);
        if next < self.dirs as isize || next >= self.entries.len() as isize {
            self.dirs
        } else {
            next as usize
        }
    }

    /// Sequential "previous" (`PrevPreset`, plugin.cpp:6898-6904):
    /// `current - 1`, wrapping to the last entry when it falls on a
    /// directory. Callers need at least one file.
    pub fn prev_sequential(&self, current: usize) -> usize {
        let prev = (current as isize).wrapping_sub(1);
        if prev < self.dirs as isize {
            self.entries.len().saturating_sub(1)
        } else if prev >= self.entries.len() as isize {
            self.dirs // "just in case" (plugin.cpp:6903)
        } else {
            prev as usize
        }
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

fn entry(name: String, is_dir: bool) -> Entry {
    Entry {
        name,
        is_dir,
        rating: 0.0,
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

/// A file's `fRating` as `GetPrivateProfileFloatW(preset00, fRating, 3.0)`
/// clamped to `[0, 5]` (plugin.cpp:7609-7610); unreadable files give the
/// default. Probes the first bytes before reading the whole file, which is
/// exact because the first occurrence of a key wins.
fn read_rating(path: &Path) -> f32 {
    let get = |text: &str| {
        let ini = Ini::parse(text);
        ini.get("preset00", "fRating")
            .map(|_| ini.get_f32("preset00", "fRating", DEFAULT_RATING))
    };
    let Ok(mut f) = File::open(path) else {
        return DEFAULT_RATING;
    };
    let mut buf = vec![0u8; HEADER_PROBE_BYTES];
    let mut got = 0;
    while got < buf.len() {
        match f.read(&mut buf[got..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => got += k,
        }
    }
    buf.truncate(got);
    let mut rating = None;
    if got == HEADER_PROBE_BYTES {
        // keep whole lines only
        if let Some(cut) = buf.iter().rposition(|&c| c == b'\n') {
            rating = get(&String::from_utf8_lossy(&buf[..cut]));
        }
        if rating.is_none() {
            let _ = f.read_to_end(&mut buf);
        }
    }
    let rating = rating.or_else(|| get(&String::from_utf8_lossy(&buf)));
    let r = rating.unwrap_or(DEFAULT_RATING);
    if r.is_nan() {
        0.0
    } else {
        r.clamp(0.0, 5.0)
    }
}

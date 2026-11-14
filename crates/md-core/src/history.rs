//! The preset history ring (`m_presetHistory`, plugin.h:431-432).
//!
//! Mirrors the C++ ring of `PRESET_HIST_LEN` (66) entries with a position and
//! two fences; the fences make the usable depth 65 entries.

use std::path::{Path, PathBuf};

/// `PRESET_HIST_LEN` = `64 + 2` (plugin.h:431).
pub const PRESET_HIST_LEN: usize = 66;

/// Circular history of loaded preset files.
#[derive(Clone, Debug)]
pub struct History {
    ring: Vec<PathBuf>,
    pos: usize,
    back_fence: usize,
    fwd_fence: usize,
}

impl Default for History {
    fn default() -> Self {
        History::new()
    }
}

impl History {
    /// Empty history (plugin.cpp:1066-1070).
    pub fn new() -> History {
        History {
            ring: vec![PathBuf::new(); PRESET_HIST_LEN],
            pos: 0,
            back_fence: 0,
            fwd_fence: 0,
        }
    }

    /// `bHistoryEmpty` (plugin.cpp:6948).
    pub fn is_empty(&self) -> bool {
        self.fwd_fence == self.back_fence
    }

    /// Erase the history and seed it with `current`, as toggling the preset
    /// order does (plugin.cpp:6343-6346).
    pub fn reset(&mut self, current: &Path) {
        self.ring[0] = current.to_path_buf();
        self.pos = 0;
        self.fwd_fence = 1;
        self.back_fence = 0;
    }

    /// `PrevPreset`, random order (plugin.cpp:6914-6919): step back one entry
    /// unless at the back fence, returning the file to load.
    pub fn back(&mut self) -> Option<&Path> {
        if self.pos == self.back_fence {
            return None;
        }
        self.pos = (self.pos + PRESET_HIST_LEN - 1) % PRESET_HIST_LEN;
        Some(&self.ring[self.pos])
    }

    /// First half of `LoadRandomPreset` (plugin.cpp:6948-6959): when there is
    /// history ahead of the position, step onto it and return the file to
    /// load. `None` means the caller must pick a new preset and `push` it.
    pub fn forward(&mut self) -> Option<&Path> {
        let next = (self.pos + 1) % PRESET_HIST_LEN;
        if next != self.fwd_fence && !self.is_empty() {
            self.pos = next;
            Some(&self.ring[self.pos])
        } else {
            None
        }
    }

    /// Record a newly picked preset: the tail of `LoadRandomPreset`
    /// (plugin.cpp:7037-7038, advance unless the history is empty) followed by
    /// the record step of `LoadPreset` (plugin.cpp:7221-7233). There is no
    /// duplicate check: picking the same file twice stores it twice.
    pub fn push(&mut self, path: &Path) {
        if !self.is_empty() {
            self.pos = (self.pos + 1) % PRESET_HIST_LEN;
        }
        self.record(path);
    }

    /// A preset loaded from the Load menu (plugin.cpp:6180): the position
    /// advances unconditionally, then `LoadPreset` records it only if that
    /// lands on the forward fence (so a menu load in the middle of the
    /// history is not recorded, exactly as in the C++).
    pub fn push_user_load(&mut self, path: &Path) {
        self.pos = (self.pos + 1) % PRESET_HIST_LEN;
        self.record(path);
    }

    /// `LoadPreset`'s record step (plugin.cpp:7221-7233).
    fn record(&mut self, path: &Path) {
        if self.fwd_fence == self.pos {
            self.ring[self.pos] = path.to_path_buf();
            self.fwd_fence = (self.fwd_fence + 1) % PRESET_HIST_LEN;
            // don't let the two fences touch
            if self.back_fence == self.fwd_fence {
                self.back_fence = (self.back_fence + 1) % PRESET_HIST_LEN;
            }
        }
    }
}

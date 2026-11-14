//! The preset mash-up screen (`UI_MASHUP`): five bins, each taking one part
//! of a preset file, picked from the preset list (plugin.cpp:4705-4933,
//! 5901-6034, 6449-6472). Reached from the menu item "do a preset mash-up".

use md_render::md_text::Rect;
use md_render::{FontId, TextQueue, DARK_TRANSLUCENT_BOX};

use super::load_screen::draw_entries;
use super::mode::UiMode;
use super::overlays::*;
use super::strings;
use super::{ErrCategory, Ui, UiAction, UiCtx};
use crate::commands::Command;

/// `MASH_SLOTS` (plugin.h:392): general, motion, waves and shapes, warp
/// shader, composite shader.
pub const MASH_SLOTS: usize = 5;
/// `MASH_APPLY_DELAY_FRAMES` (plugin.h:393).
pub const MASH_APPLY_DELAY_FRAMES: i64 = 1;

/// The mash-up screen's state.
#[derive(Clone, Debug, Default)]
pub struct Mashup {
    /// `m_nMashSlot`: the bin being changed.
    pub slot: usize,
    /// `m_nMashPreset`: the list row each bin takes from.
    pub preset: [usize; MASH_SLOTS],
    /// `m_nLastMashChangeFrame`: a bin is applied when
    /// `last_change + MASH_APPLY_DELAY_FRAMES + 1 == frame`.
    pub last_change: [i64; MASH_SLOTS],
    /// PageUp pressed.
    pub paged_up: bool,
    /// PageDown pressed.
    pub paged_down: bool,
}

impl Ui {
    /// A bin was given a file by the host (after
    /// [`UiAction::RandomizeMashup`]); it applies one frame sooner than a
    /// browsed change (plugin.cpp:6456-6457).
    pub fn set_mash_preset(&mut self, slot: usize, index: usize, frame: u64) {
        if slot < MASH_SLOTS {
            self.mash.preset[slot] = index;
            self.mash.last_change[slot] = frame as i64 + MASH_APPLY_DELAY_FRAMES;
        }
    }

    /// The keys of the screen (plugin.cpp:5901-6034, 6130-6134).
    pub(super) fn mash_key(&mut self, cmd: Command, ctx: &UiCtx) -> bool {
        let n = ctx.library.entries().len();
        let dirs = ctx.library.dir_count();
        let frame = ctx.frame as i64;
        let slot = self.mash.slot.min(MASH_SLOTS - 1);
        let touched = |m: &mut Mashup| m.last_change[slot] = frame;
        match cmd {
            Command::Left(_) => self.mash.slot = slot.saturating_sub(1),
            Command::Right(_) => self.mash.slot = (slot + 1).min(MASH_SLOTS - 1),
            Command::Up(_) => {
                self.mash.preset[slot] = self.mash.preset[slot].saturating_sub(1).max(dirs);
                touched(&mut self.mash);
            }
            Command::Down(_) => {
                self.mash.preset[slot] = (self.mash.preset[slot] + 1).min(n.saturating_sub(1));
                touched(&mut self.mash);
            }
            Command::Home(_) => {
                self.mash.preset[slot] = dirs;
                touched(&mut self.mash);
            }
            Command::End(_) => {
                self.mash.preset[slot] = n.saturating_sub(1);
                touched(&mut self.mash);
            }
            Command::PageUp(_) => {
                self.mash.paged_up = true;
                touched(&mut self.mash);
            }
            Command::PageDown(_) => {
                self.mash.paged_down = true;
                touched(&mut self.mash);
            }
            Command::Enter(_) => self.mash.last_change[slot] = frame + MASH_APPLY_DELAY_FRAMES,
            _ => return false,
        }
        true
    }

    /// The screen (plugin.cpp:4705-4933).
    pub(super) fn draw_mashup(
        &mut self,
        q: &mut TextQueue,
        lay: &ScreenLayout,
        ctx: &UiCtx,
        c: &mut Corners,
        out: &mut Vec<UiAction>,
    ) {
        let lib = ctx.library;
        let (n, dirs) = (lib.entries().len(), lib.dir_count());
        if n == dirs {
            let msg = strings::no_preset_file_found_in(&super::load_screen::dir_string(lib.dir()));
            self.add_error(&msg, 6.0, ErrCategory::Misc, true, ctx.now);
            self.mode = UiMode::Regular;
            return;
        }
        for m in 0..MASH_SLOTS {
            self.mash.preset[m] = self.mash.preset[m].clamp(dirs, n - 1);
            if self.mash.last_change[m] + MASH_APPLY_DELAY_FRAMES + 1 == ctx.frame as i64 {
                if let Some(path) = lib.path_of(self.mash.preset[m]) {
                    out.push(UiAction::ApplyMashup { slot: m, path });
                }
            }
        }
        for t in strings::MASHUP_TEXT {
            text_out(
                q,
                lay,
                c,
                FontId::Simple,
                t,
                Corner::UpperLeft,
                Some(0xFF000000),
            );
        }
        c.ul += PLAYLIST_INNER_MARGIN;

        let fh = lay.font_height(FontId::Simple);
        let mut rect = Rect::new(
            c.xl + PLAYLIST_INNER_MARGIN,
            c.ul + PLAYLIST_INNER_MARGIN,
            c.xr - PLAYLIST_INNER_MARGIN,
            c.ll - PLAYLIST_INNER_MARGIN,
        );
        let mut lines =
            (rect.bottom - rect.top - PLAYLIST_INNER_MARGIN * 2) / fh.max(1) - MASH_SLOTS as i32;
        if lines < 10 {
            rect.bottom = rect.top + fh * 10 + 1;
            lines = 10;
        }
        let lines = lines.min(16) as usize;
        let slot = self.mash.slot.min(MASH_SLOTS - 1);
        if self.mash.paged_down {
            self.mash.preset[slot] = (self.mash.preset[slot] + lines).min(n - 1);
            self.mash.paged_down = false;
        }
        if self.mash.paged_up {
            self.mash.preset[slot] = self.mash.preset[slot].saturating_sub(lines).max(dirs);
            self.mash.paged_up = false;
        }
        let cur = self.mash.preset[slot];
        let first = cur - cur % lines;
        let last = (first + lines).min(n);
        if ctx.show_menu_tooltips {
            draw_tooltip(
                q,
                &strings::page_of(cur / lines + 1, n.div_ceil(lines)),
                c.xr,
                c.lr,
            );
        }

        // the five bins, in a box
        let mut orig = rect;
        for pass in 0..2 {
            let mut boxr = orig;
            let (mut w, mut h) = (0, 0);
            for m in 0..MASH_SLOTS {
                let s = format!(
                    "{}{}",
                    strings::MASHUP_NAMES[m],
                    lib.entries()[self.mash.preset[m]].name
                );
                let r2 = Rect::new(orig.left, orig.top + h, orig.right, orig.bottom);
                let color = if m == slot {
                    PLAYLIST_COLOR_HILITE_TRACK
                } else {
                    PLAYLIST_COLOR_NORMAL
                };
                let t = if pass == 0 {
                    q.calc_rect(FontId::Simple, &s, r2, single_line())
                } else {
                    q.draw(FontId::Simple, &s, r2, single_line(), argb(color), None)
                };
                h += t.height();
                w = w.max(t.width());
            }
            if pass == 0 {
                boxr.right = boxr.left + w;
                boxr.bottom = boxr.top + h;
                q.draw_box(boxr, DARK_TRANSLUCENT_BOX);
            } else {
                orig.top += h;
            }
        }
        orig.top += fh + PLAYLIST_INNER_MARGIN;

        // the list, after it. The C++ compares the bin's file name (with its
        // extension) to the row's name without one, so no row is "running".
        let bin_name = lib.entries()[cur].name.clone();
        draw_entries(
            q,
            ctx,
            orig,
            first..last,
            cur,
            &|_, stripped| bin_name == stripped,
            c,
        );
    }
}

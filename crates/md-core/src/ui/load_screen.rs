//! The preset list (`UI_LOAD`): cursor, paging, type-to-seek, and the screen
//! (plugin.cpp:4935-5088, 5943-6034, 6100-6183, 7359-7375). The list itself is the
//! [`PresetLibrary`] of Task 7.4.

use std::path::Path;

use md_render::md_text::Rect;
use md_render::{FontId, TextQueue, DARK_TRANSLUCENT_BOX};

use super::mode::UiMode;
use super::overlays::*;
use super::save_screen::path_max_len;
use super::strings;
use super::{ErrCategory, Ui, UiAction, UiCtx};
use crate::library::PresetLibrary;

/// `MAX_PRESETS_PER_PAGE` (defines.h:216): the most rows a page shows,
/// however tall the window.
pub const MAX_PRESETS_PER_PAGE: usize = 128;

/// The list's own state.
#[derive(Clone, Debug, Default)]
pub struct LoadScreen {
    /// `m_nPresetListCurPos`: the highlighted row (not yet loaded).
    pub pos: usize,
    /// `m_bUserPagedUp`: PageUp pressed; the next draw moves a page.
    pub paged_up: bool,
    /// `m_bUserPagedDown`.
    pub paged_down: bool,
    /// Rows per page as of the last draw (`lines_available`).
    pub lines: usize,
}

impl LoadScreen {
    /// `SeekToPreset` (plugin.cpp:7359-7375): jump to the first file whose
    /// name starts with the letter `ch`, ignoring case; no match, no move.
    pub fn seek(&mut self, ch: char, lib: &PresetLibrary) {
        if let Some(i) = lib.find_prefix(&ch.to_string(), 0) {
            self.pos = i;
        }
    }

    /// The row to highlight after a rescan (plugin.cpp:7708-7725): the
    /// current file when it is in the list, else the first row.
    pub fn reselect(&mut self, lib: &PresetLibrary, current_file: Option<&str>) {
        self.pos = current_file
            .and_then(|f| {
                (lib.dir_count()..lib.entries().len()).find(|&i| lib.entries()[i].name == f)
            })
            .unwrap_or(0);
    }

    /// After a delete: the same row, or the new last one
    /// (`DeletePresetFile`, plugin.cpp:8199-8201).
    pub fn clamp(&mut self, lib: &PresetLibrary) {
        self.pos = self.pos.min(lib.entries().len().saturating_sub(1));
    }
}

/// `RemoveExtension`: the name up to its last `.`.
pub fn strip_ext(name: &str) -> &str {
    name.rfind('.').map_or(name, |i| &name[..i])
}

impl Ui {
    /// The list was rescanned with the "re-select the current preset" flag
    /// (`UpdatePresetList(.., bTryReselectCurrentPreset = true)`, the default,
    /// plugin.cpp:7708-7725): put the highlight on `current_file`, else row 0.
    /// Use this after a save or any rescan that is not one of the file
    /// operations below. After a delete call [`Ui::end_delete`], after a
    /// rename [`Ui::end_rename`], after a directory change from the list
    /// [`Ui::dir_changed`]; each reproduces what the C++ does there.
    pub fn library_changed(&mut self, lib: &PresetLibrary, current_file: Option<&str>) {
        self.load.reselect(lib, current_file);
    }

    /// Up/Down/Home/End/PageUp/PageDown of the list (plugin.cpp:5943-6034).
    pub(super) fn load_key_nav(&mut self, cmd: crate::Command, n: usize) -> bool {
        use crate::Command as C;
        match cmd {
            C::Up(_) => self.load.pos = self.load.pos.saturating_sub(1),
            C::Down(_) => {
                if self.load.pos + 1 < n {
                    self.load.pos += 1;
                }
            }
            C::Home(_) => self.load.pos = 0,
            C::End(_) => self.load.pos = n.saturating_sub(1),
            C::PageUp(_) => self.load.paged_up = true,
            C::PageDown(_) => self.load.paged_down = true,
            _ => return false,
        }
        true
    }

    /// `HitEnterFromLoadMenu` (plugin.cpp:6135-6183).
    pub(super) fn hit_enter(&mut self, soft: bool, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        let lib = ctx.library;
        let Some(e) = lib.entries().get(self.load.pos) else {
            return;
        };
        let Some(path) = lib.path_of(self.load.pos) else {
            return;
        };
        if e.is_dir {
            out.push(UiAction::ChangeDir(path));
        } else {
            out.push(UiAction::LoadPreset {
                index: self.load.pos,
                path,
                soft,
            });
        }
    }

    /// Delete (plugin.cpp:6038-6044): not for directories.
    pub(super) fn begin_delete(&mut self, ctx: &UiCtx) {
        if ctx
            .library
            .entries()
            .get(self.load.pos)
            .is_some_and(|e| !e.is_dir)
        {
            self.mode = UiMode::LoadDel;
        }
    }

    /// Insert (plugin.cpp:6100-6126): rename a preset file.
    pub(super) fn begin_rename(&mut self, ctx: &UiCtx) {
        let Some(e) = ctx.library.entries().get(self.load.pos) else {
            return;
        };
        if e.is_dir {
            return;
        }
        let text = strip_ext(&e.name).to_owned();
        self.mode = UiMode::LoadRename;
        self.edit = super::WaitString {
            active: true,
            filter_bad_chars: true,
            max_len: path_max_len(ctx.library.dir()),
            prompt: strings::enter_the_new_name_for(&text),
            ..Default::default()
        };
        self.edit.set_text(&text);
    }

    /// The load screen (plugin.cpp:4935-5088).
    pub(super) fn draw_load(
        &mut self,
        q: &mut TextQueue,
        lay: &ScreenLayout,
        ctx: &UiCtx,
        c: &mut Corners,
    ) {
        let lib = ctx.library;
        let n = lib.entries().len();
        if n == 0 {
            let dir = dir_string(lib.dir());
            let msg = strings::no_preset_file_found_in(&dir);
            self.add_error(&msg, 6.0, ErrCategory::Misc, true, ctx.now);
            self.mode = UiMode::Regular;
            return;
        }
        let fh = lay.font_height(FontId::Simple);
        text_out(
            q,
            lay,
            c,
            FontId::Simple,
            strings::LOAD_WHICH_PRESET,
            Corner::UpperLeft,
            Some(0xFF000000),
        );
        text_out(
            q,
            lay,
            c,
            FontId::Simple,
            &strings::directory_of(&dir_string(lib.dir())),
            Corner::UpperLeft,
            Some(0xFF000000),
        );
        c.ul += fh / 2;

        let mut rect = Rect::new(
            c.xl + PLAYLIST_INNER_MARGIN,
            c.ul + PLAYLIST_INNER_MARGIN,
            c.xr - PLAYLIST_INNER_MARGIN,
            c.ll - PLAYLIST_INNER_MARGIN,
        );
        let mut lines = ((rect.bottom - rect.top - PLAYLIST_INNER_MARGIN * 2) / fh.max(1)).max(0);
        if lines < 1 {
            rect.bottom = rect.top + fh + 1;
            lines = 1;
        }
        let lines = (lines as usize).min(MAX_PRESETS_PER_PAGE);
        self.load.lines = lines;
        if self.load.paged_down {
            self.load.pos = (self.load.pos + lines).min(n - 1);
            self.load.paged_down = false;
        }
        if self.load.paged_up {
            self.load.pos = self.load.pos.saturating_sub(lines);
            self.load.paged_up = false;
        }
        self.load.pos = self.load.pos.min(n - 1);
        let first = self.load.pos - self.load.pos % lines;
        let last = (first + lines).min(n);
        if ctx.show_menu_tooltips {
            let s = strings::page_of(self.load.pos / lines + 1, n.div_ceil(lines));
            draw_tooltip(q, &s, c.xr, c.lr);
        }

        draw_entries(
            q,
            ctx,
            rect,
            first..last,
            self.load.pos,
            &|i, _| Some(i) == ctx.current_preset,
            c,
        );
    }
}

/// The two-pass drawing of a page of list rows (plugin.cpp:5021-5087; the
/// mash-up screen's list is the same, 4863-4932): pass 0 measures the rows
/// for the dark box, pass 1 draws them. `running(i, stripped_name)` says
/// whether row `i` is the loaded preset.
pub(super) fn draw_entries(
    q: &mut TextQueue,
    ctx: &UiCtx,
    rect: Rect,
    rows: std::ops::Range<usize>,
    selected: usize,
    running: &dyn Fn(usize, &str) -> bool,
    c: &mut Corners,
) {
    let lib = ctx.library;
    let mut boxr = Rect::new(rect.left, rect.top, rect.left, rect.top);
    for pass in 0..2 {
        let mut r = rect;
        for i in rows.clone() {
            let e = &lib.entries()[i];
            let stripped = strip_ext(&e.name);
            let is_running = !e.is_dir && running(i, stripped);
            let is_selected = i == selected;
            let mut s = if e.is_dir {
                let n = e.name.trim_start_matches('*');
                if n == ".." {
                    format!(" [ {n} ] ({}) ", strings::PARENT_DIRECTORY)
                } else {
                    format!(" [ {n} ] ")
                }
            } else {
                format!(" {stripped} ")
            };
            if is_running && ctx.locked_by_user {
                s.push_str(strings::LOCKED);
            }
            let mut color = if e.is_dir {
                DIR_COLOR
            } else {
                PLAYLIST_COLOR_NORMAL
            };
            if is_running {
                color = if is_selected {
                    PLAYLIST_COLOR_BOTH
                } else {
                    PLAYLIST_COLOR_PLAYING_TRACK
                };
            } else if is_selected {
                color = PLAYLIST_COLOR_HILITE_TRACK;
            }
            if pass == 0 {
                let t = q.calc_rect(FontId::Simple, &s, r, single_line());
                boxr.right = boxr.right.max(boxr.left + t.width());
                boxr.bottom += t.height();
                r.top += t.height();
            } else {
                let t = q.draw(FontId::Simple, &s, r, single_line(), argb(color), None);
                r.top += t.height();
            }
        }
        if pass == 0 {
            boxr.top -= PLAYLIST_INNER_MARGIN;
            boxr.left -= PLAYLIST_INNER_MARGIN;
            boxr.right += PLAYLIST_INNER_MARGIN;
            boxr.bottom += PLAYLIST_INNER_MARGIN;
            q.draw_box(boxr, DARK_TRANSLUCENT_BOX);
            c.ul = boxr.bottom + PLAYLIST_INNER_MARGIN;
        }
    }
}

/// `m_szPresetDir`: the directory with its trailing separator.
pub fn dir_string(dir: &Path) -> String {
    let s = dir.display().to_string();
    if s.ends_with(std::path::MAIN_SEPARATOR) {
        s
    } else {
        format!("{s}{}", std::path::MAIN_SEPARATOR)
    }
}

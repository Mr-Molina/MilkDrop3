//! The string prompts and the `[y/N]` questions: save as, overwrite, rename,
//! delete, change directory, wave and shape import/export, the shader
//! upgrade, and the key handling of the string editor
//! (plugin.cpp:5287-5391, 5492-5870, 6038-6126, 6528-6546).
//!
//! The UI only decides and asks ([`UiAction`]); the file is written, moved or
//! deleted by the host. Where the C++ continues according to what the file
//! system said (rename, change directory, import), the host reports back with
//! [`Ui::end_rename`], [`Ui::change_dir_result`] and [`Ui::file_op_failed`].

use std::path::{Path, PathBuf};

use super::edit::TypeError;
use super::menu_tree::ItemKind;
use super::mode::UiMode;
use super::strings;
use super::{ErrCategory, FieldValue, FileOp, Ui, UiAction, UiCtx, WaitString};
use crate::commands::Command;
use crate::library::PresetLibrary;

/// `MAX_PATH` (Win32).
const MAX_PATH: usize = 260;
/// `sizeof(m_waitstring.szText) / sizeof(wchar_t)` (plugin.h:74).
const WAITSTRING_CAPACITY: usize = 48000;

/// `min(sizeof(szText)-1, MAX_PATH - wcslen(GetPresetDir()) - 6)`: the longest
/// name that still fits `dir + name + ".milk"` (plugin.cpp:6111, 6540).
pub fn path_max_len(dir: &Path) -> usize {
    let dir_len = super::load_screen::dir_string(dir).chars().count();
    (WAITSTRING_CAPACITY - 1).min(MAX_PATH.saturating_sub(dir_len).saturating_sub(6))
}

impl Ui {
    /// A typing error becomes the toast of plugin.cpp:5196-5201.
    pub(super) fn toast_type_error(&mut self, r: Result<(), TypeError>, ctx: &UiCtx) {
        let msg = match r {
            Ok(()) => return,
            Err(TypeError::Illegal) => strings::ILLEGAL_CHARACTER,
            Err(TypeError::TooLong) => strings::STRING_TOO_LONG,
        };
        self.add_error(msg, 2.5, ErrCategory::Misc, true, ctx.now);
    }

    /// `s` / `S` in regular mode (plugin.cpp:6528-6546).
    pub(super) fn begin_save_as(&mut self, ctx: &UiCtx) {
        self.mode = UiMode::SaveAs;
        self.edit = WaitString {
            active: true,
            filter_bad_chars: true,
            max_len: path_max_len(ctx.library.dir()),
            prompt: strings::SAVE_AS.into(),
            ..Default::default()
        };
        self.edit.set_text(ctx.preset_name);
    }

    /// `UI_CHANGEDIR`'s entry (the F8 key is commented out in the C++,
    /// plugin.cpp:5449-5468): ask for a directory, starting from `dir`.
    pub fn begin_change_dir(&mut self, dir: &Path) {
        self.mode = UiMode::ChangeDir;
        let mut s = dir.display().to_string();
        // "for subtle beauty - remove the trailing '\' (if not just "x:\")"
        if s.chars().count() > 3 && s.ends_with(std::path::MAIN_SEPARATOR) {
            s.pop();
        }
        self.edit = WaitString {
            active: true,
            max_len: MAX_PATH - 1,
            prompt: strings::DIRECTORY_TO_JUMP_TO.into(),
            ..Default::default()
        };
        self.edit.set_text(&s);
    }

    /// The host's answer to [`UiAction::ChangeDir`] from the directory
    /// prompt (plugin.cpp:5782-5832): on success go to the load list; else
    /// say so and stay in the prompt.
    pub fn change_dir_result(&mut self, ok: bool, now: f32) {
        if ok {
            self.edit.active = false;
            self.mode = UiMode::Load;
            self.clear_errors(Some(ErrCategory::Misc));
        } else {
            self.add_error(strings::INVALID_PATH, 3.5, ErrCategory::Misc, true, now);
        }
    }

    /// The host's answer to [`UiAction::RenamePreset`] once it has moved the
    /// file and rescanned `lib` (`RenamePresetFile`, plugin.cpp:8205-8263):
    /// back to the load list, with the highlight on the renamed file
    /// (`renamed` = the new file name with its `.milk`; 8238-8256 look the
    /// name up, from row `m_nDirs`, and fall back to row 0). Pass `None` when
    /// the move failed: the C++ still leaves the prompt (8261-8263) but does
    /// not re-select. When the name is already taken the host raises
    /// [`super::host_toasts::RENAME_EXISTS`] and does *not* call this: the prompt
    /// stays (8209-8216).
    pub fn end_rename(&mut self, lib: &PresetLibrary, renamed: Option<&str>) {
        self.edit.active = false;
        self.mode = UiMode::Load;
        if let Some(name) = renamed {
            self.load.reselect(lib, Some(name));
        }
    }

    /// The host's answer to [`UiAction::DeletePreset`] once the file is gone
    /// and `lib` rescanned (`DeletePresetFile`, plugin.cpp:8199-8201): the
    /// highlight stays on the same row, or the new last one. Do not call
    /// [`Ui::library_changed`] for this (it would jump to the playing preset).
    pub fn end_delete(&mut self, lib: &PresetLibrary) {
        self.load.clamp(lib);
    }

    /// The host rescanned after [`UiAction::ChangeDir`] from the load list
    /// (plugin.cpp:6164, `UpdatePresetList(false, true, false)`: no
    /// re-select): the highlight goes to row 0.
    pub fn dir_changed(&mut self) {
        self.load.pos = 0;
    }

    /// An import or export failed (plugin.cpp:5710-5718). The C++ builds a
    /// message for the file error and then shows `IDS_STRING_TOO_LONG`
    /// instead; that is reproduced.
    pub fn file_op_failed(&mut self, now: f32) {
        self.add_error(strings::STRING_TOO_LONG, 2.5, ErrCategory::Misc, true, now);
    }

    /// `[y/N]` for a delete (plugin.cpp:5287-5300).
    pub(super) fn answer_delete(&mut self, ch: char, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        if matches!(ch, 'y' | 'Y') {
            if let Some(path) = ctx.library.path_of(self.load.pos) {
                out.push(UiAction::DeletePreset {
                    index: self.load.pos,
                    path,
                });
            }
        }
        self.mode = UiMode::Load;
    }

    /// `[y/N]` for the shader upgrade (plugin.cpp:5303-5368).
    pub(super) fn answer_upgrade(&mut self, ch: char, _ctx: &UiCtx, out: &mut Vec<UiAction>) {
        if matches!(ch, 'y' | 'Y') {
            out.push(UiAction::UpgradePixelShader);
        }
        if ch != '\r' {
            self.mode = UiMode::Menu;
        }
    }

    /// `[y/N]` for an overwrite (plugin.cpp:5369-5391): `y` saves; another
    /// printable key or Escape goes back to typing the name.
    pub(super) fn answer_overwrite(&mut self, ch: char, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        if matches!(ch, 'y' | 'Y') {
            self.save_now(ctx, out);
        } else if (' '..='z').contains(&ch) || ch == '\u{1b}' {
            self.mode = UiMode::SaveAs;
            self.edit.active = true;
        }
    }

    /// `SavePresetAs` of the typed name, leaving the editor.
    fn save_now(&mut self, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        let name = self.edit.as_string();
        let path = ctx.library.dir().join(format!("{name}.milk"));
        out.push(UiAction::SavePreset { path, name });
        self.mode = UiMode::Regular;
        self.edit.active = false;
    }

    /// A key while the string editor is active (plugin.cpp:5492-5870).
    pub(super) fn edit_key(&mut self, cmd: Command, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        use Command as C;
        match cmd {
            C::Left(m) | C::Right(m) | C::Home(m) | C::End(m) | C::Up(m) | C::Down(m) => {
                self.edit.begin_move(m.shift);
                match cmd {
                    C::Left(_) if m.ctrl => self.edit.seek_left_word(),
                    C::Right(_) if m.ctrl => self.edit.seek_right_word(),
                    C::Home(_) if m.ctrl => self.edit.home_of_text(),
                    C::End(_) if m.ctrl => self.edit.end_of_text(),
                    _ if m.ctrl => {}
                    C::Left(_) => self.edit.left(),
                    C::Right(_) => self.edit.right(),
                    C::Home(_) => self.edit.home(),
                    C::End(_) => self.edit.end(),
                    C::Up(_) => self.edit.up(),
                    C::Down(_) => self.edit.down(),
                    _ => {}
                }
            }
            C::Insert => self.edit.overtype = !self.edit.overtype,
            C::Copy => {
                if let Some(s) = self.edit.copy() {
                    out.push(UiAction::Copy(s));
                }
            }
            C::Cut => {
                if let Some(s) = self.edit.cut() {
                    out.push(UiAction::Copy(s));
                }
            }
            C::Paste => {
                let r = self.edit.paste(ctx.clipboard);
                self.toast_type_error(r, ctx);
            }
            C::PrevPreset => self.edit.backspace(),
            C::Delete(m) if !m.ctrl => self.edit.delete(),
            C::Enter(m) if m.ctrl => {
                if self.edit.display_as_code {
                    self.accept_menu_string(out);
                }
            }
            C::Enter(_) => self.edit_enter(ctx, out),
            C::Escape => self.edit_escape(),
            _ => {}
        }
    }

    /// `VK_RETURN` without Ctrl (plugin.cpp:5674-5832).
    fn edit_enter(&mut self, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        let dir = ctx.library.dir();
        match self.mode {
            UiMode::LoadRename => {
                let Some(from) = ctx.library.path_of(self.load.pos) else {
                    return;
                };
                let name = self.edit.as_string();
                out.push(UiAction::RenamePreset {
                    index: self.load.pos,
                    from,
                    to: dir.join(format!("{name}.milk")),
                    name,
                });
            }
            UiMode::ImportWave | UiMode::ExportWave | UiMode::ImportShape | UiMode::ExportShape => {
                let (op, slot) = match self.mode {
                    UiMode::ImportWave => (FileOp::ImportWave, self.menu_slot()),
                    UiMode::ExportWave => (FileOp::ExportWave, self.menu_slot()),
                    UiMode::ImportShape => (FileOp::ImportShape, self.menu_slot()),
                    _ => (FileOp::ExportShape, self.menu_slot()),
                };
                out.push(UiAction::File {
                    op,
                    slot,
                    path: PathBuf::from(self.edit.as_string()),
                });
                self.edit.active = false;
                self.mode = UiMode::Menu;
            }
            UiMode::SaveAs => {
                let name = self.edit.as_string();
                let path = dir.join(format!("{name}.milk"));
                if (ctx.file_exists)(&path) {
                    self.edit.active = false;
                    self.mode = UiMode::SaveOverwrite;
                } else {
                    self.save_now(ctx, out);
                }
            }
            UiMode::EditMenuString => {
                if self.edit.display_as_code {
                    let r = self.edit.insert_line_break();
                    self.toast_type_error(r, ctx);
                } else {
                    self.accept_menu_string(out);
                }
            }
            UiMode::ChangeDir => {
                let mut p = self.edit.as_string();
                if !p.is_empty() && !p.ends_with(std::path::MAIN_SEPARATOR) {
                    p.push(std::path::MAIN_SEPARATOR);
                }
                out.push(UiAction::ChangeDir(PathBuf::from(p)));
            }
            _ => {}
        }
    }

    /// `OnWaitStringAccept` (menu.cpp:396-420).
    fn accept_menu_string(&mut self, out: &mut Vec<UiAction>) {
        let text = self.edit.as_string();
        let cursor = self.edit.cursor;
        let cur = self.menu.cur;
        let menu = &mut self.menu.menus[cur];
        menu.editing = false;
        let field = menu.cur_field();
        let n = menu.children.len();
        if let Some(item) = menu
            .cur_sel
            .checked_sub(n)
            .and_then(|i| menu.items.get_mut(i))
        {
            if item.def.kind == ItemKind::Text {
                item.last_cursor = cursor;
                if let Some(f) = field {
                    out.push(UiAction::SetValue(f, FieldValue::Text(text)));
                }
                if item.def.on_edit != super::Recompile::None {
                    out.push(UiAction::Recompile(item.def.on_edit));
                }
            }
        }
        self.edit.active = false;
        self.mode = UiMode::Menu;
    }

    /// `VK_ESCAPE` in the editor (plugin.cpp:5834-5868).
    fn edit_escape(&mut self) {
        self.edit.active = false;
        self.mode = match self.mode {
            UiMode::LoadRename => UiMode::Load,
            UiMode::EditMenuString if self.edit.display_as_code => UiMode::Menu,
            _ => UiMode::Regular,
        };
    }

    /// The custom wave or shape the menu is on (the item's `lParam`).
    pub(super) fn menu_slot(&self) -> usize {
        usize::from(self.menu.current().slot)
    }
}

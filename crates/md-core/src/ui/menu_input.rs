//! The keys of the menu and of the other modes: `CMilkMenu::HandleKeydown`
//! (menu.cpp:423-749) and the `WM_KEYDOWN` cases of `MyWindowProc`
//! (plugin.cpp:5881-6228).

use super::edit::WaitString;
use super::field::FieldValue;
use super::menu::{field_of, original_of, restore_of, step_value};
use super::menu_tree::ItemKind;
use super::mode::UiMode;
use super::save_screen::path_max_len;
use super::strings;
use super::{Ui, UiAction, UiCtx};
use crate::commands::Command;

/// `min(sizeof(szText) - 16, ...)` of a code string (menu.cpp:561): 96000
/// bytes of `wchar_t` text.
const CODE_BUFFER_LIMIT: usize = 96000 - 16;

impl Ui {
    /// `CMilkMenu::HandleKeydown`; `true` when the menu took the key.
    pub(super) fn menu_key(&mut self, cmd: Command, ctx: &UiCtx, out: &mut Vec<UiAction>) -> bool {
        let cur = self.menu.cur;
        if !self.menu.menus[cur].editing {
            match cmd {
                Command::Up(_) => self.menu.up(),
                Command::Down(_) => self.menu.down(),
                Command::Home(_) => self.menu.menus[cur].cur_sel = 0,
                Command::End(_) => {
                    let n = self.menu.menus[cur].len();
                    self.menu.menus[cur].cur_sel = n.saturating_sub(1);
                }
                Command::Escape => self.mode = UiMode::Regular,
                Command::PrevPreset | Command::Left(_) => match self.menu.menus[cur].parent {
                    Some(p) => self.menu.cur = p,
                    None => self.mode = UiMode::Regular,
                },
                Command::Enter(_) | Command::Right(_) | Command::NextPresetSoft => {
                    self.menu_select(ctx, out)
                }
                _ => return false,
            }
            return true;
        }
        self.menu_edit_key(cmd, ctx, out)
    }

    /// Enter on a row: open the sub-menu or begin editing the item
    /// (menu.cpp:485-585).
    fn menu_select(&mut self, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        let cur = self.menu.cur;
        let menu = &self.menu.menus[cur];
        let n = menu.children.len();
        if menu.cur_sel < n {
            self.menu.cur = menu.children[menu.cur_sel];
            return;
        }
        let Some(item) = menu.cur_item() else { return };
        let def = item.def;
        let last_cursor = item.last_cursor;
        let field = field_of(def.target, menu.slot);
        let value = field.map(|f| f.get(ctx.preset));
        match def.kind {
            ItemKind::Mode => {
                let mode = def.mode.unwrap_or(UiMode::Regular);
                self.mode = mode;
                if matches!(
                    mode,
                    UiMode::ImportWave
                        | UiMode::ExportWave
                        | UiMode::ImportShape
                        | UiMode::ExportShape
                ) {
                    let import = matches!(mode, UiMode::ImportWave | UiMode::ImportShape);
                    self.edit = WaitString {
                        active: true,
                        max_len: path_max_len(ctx.library.dir()),
                        prompt: if import {
                            strings::LOAD_FROM_FILE
                        } else {
                            strings::SAVE_TO_FILE
                        }
                        .into(),
                        ..Default::default()
                    };
                    let dir = super::load_screen::dir_string(ctx.library.dir());
                    self.edit.set_text(&format!("{dir}file.dat"));
                }
            }
            ItemKind::Bool => {
                if let (Some(f), Some(v)) = (field, value) {
                    out.push(UiAction::SetValue(f, FieldValue::Bool(!v.as_bool())));
                }
            }
            ItemKind::Text => {
                self.mode = UiMode::EditMenuString;
                self.edit = WaitString {
                    active: true,
                    display_as_code: true,
                    max_len: def.max_len.min(CODE_BUFFER_LIMIT),
                    prompt: strings::ENTER_THE_NEW_STRING.into(),
                    tooltip: def.tooltip.into(),
                    ..Default::default()
                };
                self.edit
                    .set_text(&value.map(|v| v.as_text()).unwrap_or_default());
                self.edit.cursor = self.edit.cursor.min(last_cursor);
            }
            kind => {
                if let Some(v) = value {
                    let orig = original_of(kind, &v);
                    let menu = &mut self.menu.menus[cur];
                    menu.editing = true;
                    let i = menu.cur_sel - menu.children.len();
                    menu.items[i].original = orig;
                }
            }
        }
    }

    /// The menu keys while a value is edited (menu.cpp:586-725).
    fn menu_edit_key(&mut self, cmd: Command, ctx: &UiCtx, out: &mut Vec<UiAction>) -> bool {
        let cur = self.menu.cur;
        let menu = &self.menu.menus[cur];
        let Some(item) = menu.cur_item() else {
            return false;
        };
        let (def, original) = (item.def, item.original);
        let Some(field) = field_of(def.target, menu.slot) else {
            return false;
        };
        match cmd {
            Command::Escape => {
                if matches!(
                    def.kind,
                    ItemKind::Int
                        | ItemKind::Float
                        | ItemKind::LogFloat
                        | ItemKind::Blendable
                        | ItemKind::LogBlendable
                ) {
                    self.menu.menus[cur].editing = false;
                    out.push(UiAction::SetValue(field, restore_of(def.kind, original)));
                }
            }
            Command::Enter(_) => self.menu.menus[cur].editing = false,
            Command::PageUp(_) | Command::PageDown(_) | Command::Up(_) | Command::Down(_) => {
                let mut mult = 1.0f32;
                if matches!(cmd, Command::PageUp(_) | Command::PageDown(_)) {
                    mult *= 10.0;
                }
                if matches!(cmd, Command::Up(m) | Command::Down(m) if m.shift) {
                    mult *= 0.1;
                }
                let dec = matches!(cmd, Command::Down(_) | Command::PageDown(_));
                let v = field.get(ctx.preset).as_f32();
                let nv = step_value(def.kind, v, def.min, def.max, dec, mult);
                let value = if def.kind == ItemKind::Int {
                    FieldValue::Int(nv as i32)
                } else {
                    FieldValue::Float(nv)
                };
                out.push(UiAction::SetValue(field, value));
            }
            _ => return false,
        }
        true
    }

    /// The `WM_KEYDOWN` cases after the editor and the menu
    /// (plugin.cpp:5881-6228); `true` when one took the key.
    pub(super) fn mode_key(&mut self, cmd: Command, ctx: &UiCtx, out: &mut Vec<UiAction>) -> bool {
        use Command as C;
        let n = ctx.library.entries().len();
        match cmd {
            C::Left(_) | C::Right(_) => match self.mode {
                UiMode::Load => {
                    // "it's annoying when the music skips ... so we exit the
                    // menu" (5885)
                    if matches!(cmd, C::Left(_)) {
                        self.mode = UiMode::Regular;
                    }
                    true
                }
                UiMode::UpgradePixelShader => {
                    self.mode = UiMode::Menu;
                    true
                }
                UiMode::Mashup => self.mash_key(cmd, ctx),
                _ => false,
            },
            C::Escape => match self.mode {
                UiMode::Load | UiMode::Menu | UiMode::Mashup => {
                    self.mode = UiMode::Regular;
                    true
                }
                UiMode::LoadDel => {
                    self.mode = UiMode::Load;
                    true
                }
                UiMode::UpgradePixelShader => {
                    self.mode = UiMode::Menu;
                    true
                }
                UiMode::SaveOverwrite => {
                    // back to the editor, parameters as they were
                    self.mode = UiMode::SaveAs;
                    self.edit.active = true;
                    true
                }
                _ => false,
            },
            C::Up(_) | C::Down(_) | C::Home(_) | C::End(_) | C::PageUp(_) | C::PageDown(_) => {
                match self.mode {
                    UiMode::Mashup => self.mash_key(cmd, ctx),
                    UiMode::Load => self.load_key_nav(cmd, n),
                    _ => false,
                }
            }
            C::NextPresetSoft => match self.mode {
                UiMode::Load => {
                    self.hit_enter(true, ctx, out);
                    true
                }
                UiMode::Regular => {
                    out.push(UiAction::Forward(cmd));
                    true
                }
                // m_bPresetLockedByCode: any other mode locks the preset
                // (plugin.cpp:3922, 5986)
                _ => false,
            },
            C::Enter(_) => match self.mode {
                UiMode::Mashup => self.mash_key(cmd, ctx),
                UiMode::Load => {
                    self.hit_enter(false, ctx, out);
                    true
                }
                _ => false,
            },
            C::Delete(_) => {
                if self.mode == UiMode::Load {
                    self.begin_delete(ctx);
                } else {
                    // stop the custom message / kill sprites: the visualizer
                    // knows its numeric input mode (6047-6095)
                    out.push(UiAction::Forward(cmd));
                }
                true
            }
            C::Insert if self.mode == UiMode::Load => {
                self.begin_rename(ctx);
                true
            }
            C::PrevPreset | C::KillSprites | C::KillTexts => {
                out.push(UiAction::Forward(cmd));
                true
            }
            _ => false,
        }
    }
}

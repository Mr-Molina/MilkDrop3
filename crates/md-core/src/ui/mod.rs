//! The UI state machine (Task 7.7): the fifteen modes of `m_UI_mode`
//! (plugin.h:48), the preset menu, the load list, the string prompts and the
//! overlays the C++ draws (`MyRenderUI`, plugin.cpp:4107-5120, and
//! `RenderBuiltInTextMsgs`, pluginshell.cpp:1857-1900).
//!
//! # Input
//!
//! [`Ui::handle`] takes the [`Command`]s of Task 7.6 under its input contract
//! (see [`crate::keymap`]): every typed character arrives as
//! [`Command::TypeChar`] and is read per mode, exactly as the C++ `WM_CHAR`
//! handlers do (plugin.cpp:5171-5395): as text while a string is edited, as
//! the answer of a `[y/N]` prompt, as a seek in the load list, and in every
//! other mode through [`crate::lookup_char`] as a regular hotkey. Keys
//! arrive as their own commands and are read as `WM_KEYDOWN`
//! (plugin.cpp:5400-6229) reads them: the F-keys first, then the string
//! editor, then the menu, then the per-mode keys.
//!
//! What the UI asks of the rest of the program is a list of [`UiAction`]s:
//! it never touches a file, a preset or the config. A command the UI does not
//! own (a hotkey such as `NextPresetHard` or `RateUp`) comes back as
//! [`UiAction::Forward`] for Task 7.9.
//!
//! # Output
//!
//! [`Ui::draw`] queues the overlays on a [`TextQueue`](md_render::TextQueue).
//! Like `MyRenderUI` it also does the per-frame bookkeeping the C++ does
//! while drawing (paging the load list, expiring toasts, applying a
//! mash-up), so it takes `&mut self` and returns the actions that causes.

pub mod edit;
pub mod field;
pub mod field_tables;
pub mod host_toasts;
pub mod load_screen;
pub mod mashup;
pub mod menu;
pub mod menu_custom;
pub mod menu_draw;
pub mod menu_input;
pub mod menu_tips;
pub mod menu_tree;
pub mod mode;
pub mod overlays;
pub mod prompt_draw;
pub mod save_screen;
pub mod strings;

use std::path::{Path, PathBuf};

use md_preset::Preset;

use crate::commands::Command;
use crate::keymap::lookup_char;
use crate::library::PresetLibrary;

pub use edit::WaitString;
pub use field::{FieldId, FieldValue};
pub use field_tables::{PresetField, ShapeField, WaveField};
pub use load_screen::{LoadScreen, MAX_PRESETS_PER_PAGE};
pub use mashup::{Mashup, MASH_SLOTS};
pub use menu::{MenuTree, ROOT};
pub use menu_tree::Recompile;
pub use mode::UiMode;
pub use overlays::ScreenLayout;

/// `ERR_*` (plugin.h:453-459): what a message is about. A notification
/// replaces the earlier ones (`AddError`, plugin.cpp:4082).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrCategory {
    /// `ERR_INIT`.
    Init,
    /// `ERR_PRESET`.
    Preset,
    /// `ERR_MISC`.
    Misc,
    /// `ERR_NOTIFY`: "shuffle is now ON" and the like.
    Notify,
    /// `ERR_SCANNING_PRESETS`.
    Scanning,
}

/// One timed message in the upper-right corner (`ErrorMsg`, plugin.h:182-189).
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    /// `msg`.
    pub msg: String,
    /// `birthTime`.
    pub birth: f32,
    /// `expireTime`: shown while `birth <= now < expire`.
    pub expire: f32,
    /// `category`.
    pub category: ErrCategory,
    /// `bBold`: the box fades from red to black over the message's life.
    pub bold: bool,
}

/// The flags the host keeps in its config (`WriteRealtimeConfig`,
/// plugin.cpp:6873-6881) or acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiFlag {
    /// `m_bShowFPS` (F5).
    ShowFps,
    /// `m_bShowPresetInfo` (F4).
    ShowPresetName,
    /// `m_bShowRating` (F6).
    ShowRating,
    /// `m_bShowDebugInfo` (`n`).
    ShowDebug,
    /// `m_show_help` (F1).
    ShowHelp,
    /// `m_bShowShaderHelp` (F9).
    ShowShaderHelp,
    /// F10 pause (the Task 7.6 ruling: the host freezes the clock).
    Paused,
}

/// The wave and shape file operations of the prompts (plugin.cpp:5688-5723).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileOp {
    /// `m_wave[i].Import`.
    ImportWave,
    /// `m_wave[i].Export`.
    ExportWave,
    /// `m_shape[i].Import`.
    ImportShape,
    /// `m_shape[i].Export`.
    ExportShape,
}

/// What the UI asks the visualizer or the app to do.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    /// A hotkey the UI does not own: the visualizer runs it.
    Forward(Command),
    /// Load (and blend to) preset `index` of the library
    /// (`HitEnterFromLoadMenu`, plugin.cpp:6169-6182): the host sets the
    /// current preset to `index` and steps the history. `soft` (Space) blends
    /// by `m_fBlendTimeUser`, Enter cuts.
    LoadPreset {
        /// `m_nCurrentPreset`.
        index: usize,
        /// The file.
        path: PathBuf,
        /// Blend (Space) or hard cut (Enter).
        soft: bool,
    },
    /// `SavePresetAs(szNewFile)`: write the preset there, then rescan, and
    /// make `name` its `m_szDesc`.
    SavePreset {
        /// The file, `dir + name + ".milk"`.
        path: PathBuf,
        /// The typed name.
        name: String,
    },
    /// Make `dir` the preset directory and rescan (plugin.cpp:6139-6168 and
    /// `UI_CHANGEDIR`, 5782-5832). From the load list the host then calls
    /// [`Ui::dir_changed`]. From the `UI_CHANGEDIR` prompt it answers with
    /// [`Ui::change_dir_result`]: success goes to the load list, an invalid
    /// directory raises [`strings::INVALID_PATH`] and the prompt stays.
    ChangeDir(PathBuf),
    /// `DeletePresetFile`: delete the file, then rescan and select
    /// `index` again.
    DeletePreset {
        /// The list row.
        index: usize,
        /// The file.
        path: PathBuf,
    },
    /// `RenamePresetFile` (plugin.cpp:5675-5687).
    RenamePreset {
        /// The list row.
        index: usize,
        /// The file now.
        from: PathBuf,
        /// `dir + typed + ".milk"`.
        to: PathBuf,
        /// The typed name.
        name: String,
    },
    /// A menu edited a value.
    SetValue(FieldId, FieldValue),
    /// The text of a code item was accepted: recompile (the C++ callback).
    Recompile(Recompile),
    /// Import or export of a custom wave or shape at `path`
    /// (plugin.cpp:5688-5723). After an import the host recompiles the preset
    /// code; a failure is answered with a toast.
    File {
        /// Which of the four.
        op: FileOp,
        /// The custom wave or shape.
        slot: usize,
        /// The typed path.
        path: PathBuf,
    },
    /// `y` at the upgrade prompt (plugin.cpp:5303-5368).
    UpgradePixelShader,
    /// A mash-up bin changed: import that part of the preset file
    /// (`Import(szFile, time, m_pState, ApplyFlags)`, plugin.cpp:4729-4756).
    ApplyMashup {
        /// The bin, `0..MASH_SLOTS` (general, motion, waves, warp, comp).
        slot: usize,
        /// The file.
        path: PathBuf,
    },
    /// `h` / `H` in the mash-up screen: pick a random file for the bin (or,
    /// `all`, for every bin) and call [`Ui::set_mash_preset`]
    /// (plugin.cpp:6449-6472).
    RandomizeMashup {
        /// `H`: every bin.
        all: bool,
    },
    /// A flag changed.
    FlagChanged(UiFlag, bool),
    /// Text copied out of the string editor.
    Copy(String),
}

/// What the UI reads of the program's state, and what the host decides for it.
/// Fields besides `library` and `preset` default to the C++ defaults.
pub struct UiCtx<'a> {
    /// The preset list (`m_presets`, `m_nDirs`).
    pub library: &'a PresetLibrary,
    /// `m_nCurrentPreset`.
    pub current_preset: Option<usize>,
    /// `m_pState`.
    pub preset: &'a Preset,
    /// `m_pState->m_szDesc`.
    pub preset_name: &'a str,
    /// `m_pNewState->m_szDesc` while a load is in progress.
    pub loading_name: Option<&'a str>,
    /// `GetTime()`.
    pub now: f32,
    /// `GetFrame()`.
    pub frame: u64,
    /// `GetFps()`.
    pub fps: f32,
    /// `*m_pState->var_pf_monitor`.
    pub monitor: f32,
    /// `m_bPresetLockedByUser`.
    pub locked_by_user: bool,
    /// `m_bEnableRating`.
    pub rating_enabled: bool,
    /// `m_fShowRatingUntilThisTime` (plugin.cpp:8350).
    pub show_rating_until: f32,
    /// `m_bWarpShaderLock`.
    pub warp_shader_lock: bool,
    /// `m_bCompShaderLock`.
    pub comp_shader_lock: bool,
    /// `m_bShowMenuToolTips`: hardwired true (plugin.cpp:959).
    pub show_menu_tooltips: bool,
    /// `m_bWarningsDisabled2`: no toasts.
    pub warnings_disabled: bool,
    /// `m_nMaxPSVersion`: the highest pixel shader version of the renderer as a
    /// `MD2_PS_*` number (`0` none, `2`, `3`, `4`; plugin.h:94-98).
    pub max_ps: i32,
    /// The clipboard, for Ctrl+V.
    pub clipboard: &'a str,
    /// `m_time`: seconds since the plugin started, for "Press F1".
    pub time_since_start: f32,
    /// `GetFileAttributesW(path) != -1`: whether a file exists, for the
    /// overwrite question. A query only; the UI changes no file.
    pub file_exists: fn(&Path) -> bool,
}

impl<'a> UiCtx<'a> {
    /// A context over `library` and `preset` with the C++ defaults.
    pub fn new(library: &'a PresetLibrary, preset: &'a Preset) -> UiCtx<'a> {
        UiCtx {
            library,
            current_preset: None,
            preset,
            preset_name: "",
            loading_name: None,
            now: 0.0,
            frame: 0,
            fps: 0.0,
            monitor: 0.0,
            locked_by_user: false,
            rating_enabled: true,
            show_rating_until: -1.0,
            warp_shader_lock: false,
            comp_shader_lock: false,
            show_menu_tooltips: true,
            warnings_disabled: false,
            max_ps: 4,
            clipboard: "",
            time_since_start: 0.0,
            file_exists: |p| p.exists(),
        }
    }
}

/// The UI state (`m_UI_mode` and everything it owns).
pub struct Ui {
    /// `m_UI_mode`.
    pub mode: UiMode,
    /// `m_menuPreset` and its sub-menus, `m_pCurMenu`.
    pub menu: MenuTree,
    /// The preset list screen.
    pub load: LoadScreen,
    /// `m_errors`.
    pub toasts: Vec<Toast>,
    /// `m_bShowFPS`.
    pub show_fps: bool,
    /// `m_bShowPresetInfo`.
    pub show_preset_name: bool,
    /// `m_bShowRating`.
    pub show_rating: bool,
    /// `m_show_help`.
    pub show_help: bool,
    /// `m_bShowDebugInfo`.
    pub show_debug: bool,
    /// `m_bShowShaderHelp`.
    pub show_shader_help: bool,
    /// F10 (the Task 7.6 ruling): the host has frozen the visualizer.
    pub paused: bool,
    /// `m_show_press_f1_msg`: the "Press F1 for Help" message at the start
    /// (the plugin's default is 0, plugin.cpp:887).
    pub show_press_f1: bool,
    /// `m_waitstring`.
    pub edit: WaitString,
    /// The preset mash-up screen.
    pub mash: Mashup,
}

impl Default for Ui {
    fn default() -> Ui {
        Ui::new()
    }
}

impl Ui {
    /// A fresh UI in `UI_REGULAR` (plugin.cpp:1023-1081).
    pub fn new() -> Ui {
        Ui {
            mode: UiMode::Regular,
            menu: MenuTree::new(),
            load: LoadScreen::default(),
            toasts: Vec::new(),
            show_fps: false,
            show_preset_name: false,
            show_rating: false,
            show_help: false,
            show_debug: false,
            show_shader_help: false,
            paused: false,
            show_press_f1: false,
            edit: WaitString::default(),
            mash: Mashup::default(),
        }
    }

    /// `AddError` (plugin.cpp:4080): a notification first removes the
    /// notifications before it.
    pub fn add_error(
        &mut self,
        msg: &str,
        duration: f32,
        category: ErrCategory,
        bold: bool,
        now: f32,
    ) {
        if category == ErrCategory::Notify {
            self.clear_errors(Some(ErrCategory::Notify));
        }
        self.toasts.push(Toast {
            msg: msg.to_owned(),
            birth: now,
            expire: now + duration,
            category,
            bold,
        });
    }

    /// `ClearErrors` (plugin.cpp:4095): one category, or all.
    pub fn clear_errors(&mut self, category: Option<ErrCategory>) {
        self.toasts
            .retain(|t| category.is_some_and(|c| t.category != c));
    }

    /// Drop the messages that are over (the erase in plugin.cpp:5111-5117).
    pub fn prune_toasts(&mut self, now: f32) {
        self.toasts.retain(|t| now < t.expire);
    }

    /// One command. See the module docs for how it is read.
    pub fn handle(&mut self, cmd: Command, ctx: &UiCtx) -> Vec<UiAction> {
        let mut out = Vec::new();
        self.prune_toasts(ctx.now);
        match cmd {
            Command::TypeChar(ch) => self.type_char(ch, ctx, &mut out),
            // Ctrl+Backspace is Backspace to everything but the editor
            Command::CtrlBackspace if !self.edit.active => {
                self.key(Command::PrevPreset, ctx, &mut out)
            }
            other => self.key(other, ctx, &mut out),
        }
        out
    }

    /// `WM_CHAR` (plugin.cpp:5171-5395).
    fn type_char(&mut self, ch: char, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        if self.edit.active {
            let r = self.edit.type_char(ch);
            self.toast_type_error(r, ctx);
        } else if self.mode == UiMode::LoadDel {
            self.answer_delete(ch, ctx, out);
        } else if self.mode == UiMode::UpgradePixelShader {
            self.answer_upgrade(ch, ctx, out);
        } else if self.mode == UiMode::SaveOverwrite {
            self.answer_overwrite(ch, ctx, out);
        } else {
            self.regular_char(ch, ctx, out);
        }
    }

    /// `HandleRegularKey` (plugin.cpp:6245-6608) for one character.
    fn regular_char(&mut self, ch: char, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        if self.mode == UiMode::Load && ch.is_ascii_alphabetic() {
            self.load.seek(ch, ctx.library);
            return;
        }
        if self.mode == UiMode::Mashup && ('1'..='5').contains(&ch) {
            self.mash.slot = (ch as u8 - b'1') as usize;
            return;
        }
        if let Some(cmd) = lookup_char(ch) {
            self.regular_command(cmd, ch == 'H', ctx, out);
        }
    }

    /// What a regular-mode hotkey does. `all` is `H` (a capital) in the
    /// mash-up screen.
    fn regular_command(&mut self, cmd: Command, all: bool, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        match cmd {
            Command::NextPresetHard if self.mode == UiMode::Mashup => {
                out.push(UiAction::RandomizeMashup { all });
            }
            Command::ToggleDebugInfo => self.flip(UiFlag::ShowDebug, out),
            Command::OpenLoadMenu => {
                // 'l' in the load list is a seek (6256), so only the regular
                // and menu modes get here; the close branch (6556) is dead.
                if matches!(self.mode, UiMode::Regular | UiMode::Menu) {
                    self.mode = UiMode::Load;
                    self.load.paged_up = false;
                    self.load.paged_down = false;
                }
            }
            Command::ToggleMenu => match self.mode {
                UiMode::Menu => self.mode = UiMode::Regular,
                UiMode::Regular | UiMode::Load => self.mode = UiMode::Menu,
                _ => {}
            },
            Command::SaveAs => {
                if self.mode == UiMode::Regular {
                    self.begin_save_as(ctx);
                }
            }
            other => out.push(UiAction::Forward(other)),
        }
    }

    /// Flip a flag and report it.
    pub(crate) fn flip(&mut self, flag: UiFlag, out: &mut Vec<UiAction>) {
        let slot = match flag {
            UiFlag::ShowFps => &mut self.show_fps,
            UiFlag::ShowPresetName => &mut self.show_preset_name,
            UiFlag::ShowRating => &mut self.show_rating,
            UiFlag::ShowDebug => &mut self.show_debug,
            UiFlag::ShowHelp => &mut self.show_help,
            UiFlag::ShowShaderHelp => &mut self.show_shader_help,
            UiFlag::Paused => &mut self.paused,
        };
        *slot = !*slot;
        out.push(UiAction::FlagChanged(flag, *slot));
    }

    /// `WM_KEYDOWN` (plugin.cpp:5400-6229).
    fn key(&mut self, cmd: Command, ctx: &UiCtx, out: &mut Vec<UiAction>) {
        // 1. keys read in every mode, even while a string is edited
        //    (5409-5480).
        match cmd {
            Command::ToggleFps => return self.flip(UiFlag::ShowFps, out),
            Command::TogglePresetName => return self.flip(UiFlag::ShowPresetName, out),
            Command::ToggleRating => return self.flip(UiFlag::ShowRating, out),
            Command::ToggleShaderHelp => return self.flip(UiFlag::ShowShaderHelp, out),
            Command::TogglePause => return self.flip(UiFlag::Paused, out),
            Command::CycleMaxFps
            | Command::CycleTimeBetweenPresets
            | Command::ToggleAlwaysOnTop
            | Command::ToggleLock => return out.push(UiAction::Forward(cmd)),
            _ => {}
        }
        // 2. the string editor absorbs every other key (5482-5868).
        if self.edit.active {
            return self.edit_key(cmd, ctx, out);
        }
        // 3. the menu (5871-5877).
        if self.mode == UiMode::Menu && self.menu_key(cmd, ctx, out) {
            return;
        }
        // 4. the keys of the other modes (5880-6226).
        if self.mode_key(cmd, ctx, out) {
            return;
        }
        // 5. not read by the plugin: the shell's own keys
        //    (pluginshell.cpp:2284-2294), then whatever the visualizer does
        //    with a key.
        match cmd {
            Command::ToggleHelp => {
                // F1 also ends the "Press F1" message (pluginshell.cpp:2287)
                self.show_press_f1 = false;
                self.flip(UiFlag::ShowHelp, out)
            }
            Command::Escape if self.show_help => self.flip(UiFlag::ShowHelp, out),
            Command::ToggleFullscreen | Command::ToggleStretch => out.push(UiAction::Forward(cmd)),
            // keys no mode reads here (the C++ shell absorbs them)
            Command::Escape
            | Command::Up(_)
            | Command::Down(_)
            | Command::Left(_)
            | Command::Right(_)
            | Command::Home(_)
            | Command::End(_)
            | Command::PageUp(_)
            | Command::PageDown(_)
            | Command::Enter(_)
            | Command::Delete(_)
            | Command::Insert
            | Command::Copy
            | Command::Cut
            | Command::Paste
            | Command::NextPresetSoft
            | Command::PrevPreset
            | Command::TypeChar(_) => {}
            other => self.regular_command(other, false, ctx, out),
        }
    }
}

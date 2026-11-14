//! Tests of the UI state machine (Task 7.7): modes, menu, prompts, the load
//! list. Each test works in its own directory under the system temp dir.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use md_preset::{Preset, PsVersion};

use crate::commands::{Command, Mods};
use crate::library::PresetLibrary;
use crate::ui::menu::{self, MenuTree, POST, ROOT};
use crate::ui::menu_tree::ItemKind;
use crate::ui::{FieldId, FieldValue, PresetField, Recompile, Ui, UiAction, UiMode, WaveField};

pub(crate) struct Tmp(pub PathBuf);

impl Tmp {
    pub fn new() -> Tmp {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "md-core-ui-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A context over the harness's fields (a macro, so a draw can borrow
/// `h.ui` mutably next to it).
macro_rules! ctx {
    ($h:expr) => {{
        let mut c = $crate::ui::UiCtx::new(&$h.lib, &$h.preset);
        c.now = $h.now;
        c.frame = $h.frame;
        c.preset_name = &$h.name;
        c.clipboard = &$h.clipboard;
        c
    }};
}
pub(crate) use ctx;

/// A UI over a temp preset directory, applying `SetValue` as the host would.
pub(crate) struct Harness {
    pub ui: Ui,
    pub lib: PresetLibrary,
    pub preset: Preset,
    pub tmp: Tmp,
    pub now: f32,
    pub frame: u64,
    pub name: String,
    pub clipboard: String,
}

impl Harness {
    /// A directory with `sub/` and the given preset files.
    pub fn with_files(names: &[&str]) -> Harness {
        let tmp = Tmp::new();
        fs::create_dir_all(tmp.0.join("sub")).unwrap();
        for n in names {
            fs::write(
                tmp.0.join(n),
                "MILKDROP_PRESET_VERSION=201\nPSVERSION=2\n[preset00]\nfRating=3.0\n",
            )
            .unwrap();
        }
        let lib = PresetLibrary::scan(&tmp.0, PsVersion::Ps3_0);
        Harness {
            ui: Ui::new(),
            lib,
            preset: Preset::defaults(),
            tmp,
            now: 100.0,
            frame: 1000,
            name: "current preset".into(),
            clipboard: String::new(),
        }
    }

    pub fn new() -> Harness {
        Harness::with_files(&[
            "alpha.milk",
            "beta.milk",
            "gamma.milk",
            "Gamma2.milk",
            "rho.milk",
        ])
    }

    /// Send one command; `SetValue`s are applied to the preset.
    pub fn send(&mut self, cmd: Command) -> Vec<UiAction> {
        let ctx = ctx!(self);
        let acts = self.ui.handle(cmd, &ctx);
        for a in &acts {
            if let UiAction::SetValue(f, v) = a {
                f.set(&mut self.preset, v);
            }
        }
        acts
    }

    pub fn ch(&mut self, c: char) -> Vec<UiAction> {
        self.send(Command::TypeChar(c))
    }

    pub fn type_str(&mut self, s: &str) {
        for c in s.chars() {
            self.ch(c);
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.tmp.0.join(name)
    }

    /// Row of the library by name.
    pub fn row(&self, name: &str) -> usize {
        self.lib
            .entries()
            .iter()
            .position(|e| e.name == name)
            .unwrap()
    }
}

pub(crate) fn up() -> Command {
    Command::Up(Mods::NONE)
}
pub(crate) fn down() -> Command {
    Command::Down(Mods::NONE)
}
pub(crate) fn enter() -> Command {
    Command::Enter(Mods::NONE)
}

/// Put the menu on `label` of menu `m` (not through the keys).
pub(crate) fn point_menu_at(ui: &mut Ui, m: usize, label: &str) {
    ui.mode = UiMode::Menu;
    ui.menu.cur = m;
    let n = ui.menu.menus[m].children.len();
    let i = ui.menu.menus[m]
        .items
        .iter()
        .position(|i| i.def.label == label)
        .unwrap();
    ui.menu.menus[m].cur_sel = n + i;
}

#[test]
fn escape_from_menu_returns_to_regular() {
    let mut h = Harness::new();
    assert_eq!(h.ui.mode, UiMode::Regular);
    h.ch('m');
    assert_eq!(h.ui.mode, UiMode::Menu);
    h.send(Command::Escape);
    assert_eq!(h.ui.mode, UiMode::Regular);

    // from inside a sub-menu too (menu.cpp:473-476 goes straight out)
    h.ch('m');
    h.send(enter()); // into "--MOTION"
    assert_eq!(h.ui.menu.cur, menu::MOTION);
    h.send(Command::Escape);
    assert_eq!(h.ui.mode, UiMode::Regular);

    // 'm' again closes
    h.ch('m');
    h.ch('m');
    assert_eq!(h.ui.mode, UiMode::Regular);
}

#[test]
fn menu_back_goes_up_then_out() {
    let mut h = Harness::new();
    h.ch('m');
    h.send(enter());
    assert_eq!(h.ui.menu.cur, menu::MOTION);
    h.send(Command::Left(Mods::NONE));
    assert_eq!(h.ui.menu.cur, ROOT);
    h.send(Command::Left(Mods::NONE));
    assert_eq!(h.ui.mode, UiMode::Regular);
}

#[test]
fn load_screen_search_jumps_to_prefix() {
    let mut h = Harness::new();
    h.ch('l');
    assert_eq!(h.ui.mode, UiMode::Load);
    let acts = h.ch('g');
    assert!(
        acts.is_empty(),
        "a letter seeks, it is not a hotkey: {acts:?}"
    );
    assert_eq!(h.ui.load.pos, h.row("gamma.milk"), "first g, ignoring case");
    h.ch('R');
    assert_eq!(h.ui.load.pos, h.row("rho.milk"));
    // no file starts with z: stay
    h.ch('z');
    assert_eq!(h.ui.load.pos, h.row("rho.milk"));
    // 'l' and 'm' seek too, they do not close the list or open the menu
    h.ch('l');
    h.ch('m');
    assert_eq!(h.ui.mode, UiMode::Load);
    // directories are never found: 's' of "sub" matches no file
    h.ch('s');
    assert_eq!(h.ui.load.pos, h.row("rho.milk"));
}

#[test]
fn typechar_is_a_hotkey_in_regular_mode_and_text_or_seek_elsewhere() {
    let mut h = Harness::new();
    // regular: through lookup_char
    assert_eq!(
        h.ch('r'),
        vec![UiAction::Forward(Command::ToggleRandomOrder)]
    );
    assert_eq!(h.ch('q'), vec![UiAction::Forward(Command::EchoZoomDown)]);
    assert_eq!(h.ch('~'), vec![UiAction::Forward(Command::ToggleLock)]);
    assert!(
        h.ch('\u{7}').is_empty(),
        "an unbound character does nothing"
    );
    // load list: a letter seeks instead of being a hotkey...
    h.ch('l');
    assert!(h.ch('r').is_empty());
    assert_eq!(h.ui.load.pos, h.row("rho.milk"));
    // ...but a digit still falls through to the hotkeys (plugin.cpp:6256-6299)
    assert_eq!(h.ch('5'), vec![UiAction::Forward(Command::Digit(5))]);
    h.send(Command::Escape);
    // text entry: typed
    h.ch('s');
    assert_eq!(h.ui.mode, UiMode::SaveAs);
    assert!(h.ch('r').is_empty());
    h.ch('q');
    assert_eq!(h.ui.edit.as_string(), "current presetrq");
}

#[test]
fn menu_float_item_increments_by_step() {
    let mut h = Harness::new();
    point_menu_at(&mut h.ui, POST, "blur1: edge darken amount");
    let def = h.ui.menu.current().cur_item().unwrap().def;
    assert_eq!((def.kind, def.min, def.max), (ItemKind::Float, 0.0, 1.0));
    h.preset.blur1_edge_darken.set(0.5);
    h.send(enter());
    assert!(h.ui.menu.current().editing);

    // (max - min) * 0.01 per Up, menu.cpp:668
    let acts = h.send(up());
    let field = FieldId::Preset(PresetField::Blur1EdgeDarken);
    assert_eq!(
        acts,
        vec![UiAction::SetValue(field, FieldValue::Float(0.51))]
    );
    h.send(up());
    assert!((h.preset.blur1_edge_darken.value() - 0.52).abs() < 1e-6);
    h.send(down());
    assert!((h.preset.blur1_edge_darken.value() - 0.51).abs() < 1e-6);
    // Shift: a tenth of the step; PageUp: ten steps (menu.cpp:637-649)
    h.send(Command::Up(Mods::SHIFT));
    assert!((h.preset.blur1_edge_darken.value() - 0.511).abs() < 1e-6);
    h.send(Command::PageUp(Mods::NONE));
    assert!((h.preset.blur1_edge_darken.value() - 0.611).abs() < 1e-5);
    // clamped to max
    for _ in 0..10 {
        h.send(Command::PageUp(Mods::NONE));
    }
    assert_eq!(h.preset.blur1_edge_darken.value(), 1.0);
    // Escape puts the original value back and stops editing
    h.send(Command::Escape);
    assert!(!h.ui.menu.current().editing);
    assert_eq!(h.preset.blur1_edge_darken.value(), 0.5);
    assert_eq!(
        h.ui.mode,
        UiMode::Menu,
        "Escape ended the edit, not the menu"
    );
    // Enter keeps the value
    h.send(enter());
    h.send(up());
    h.send(enter());
    assert!(!h.ui.menu.current().editing);
    assert!((h.preset.blur1_edge_darken.value() - 0.51).abs() < 1e-6);
}

#[test]
fn menu_item_kinds_step_like_the_c_plus_plus() {
    use menu::step_value as s;
    // int: whole steps, never below 1 per press, clamped (menu.cpp:656-662)
    assert_eq!(s(ItemKind::Int, 3.0, 0.0, 7.0, false, 0.1), 4.0);
    assert_eq!(s(ItemKind::Int, 3.0, 0.0, 7.0, true, 10.0), 0.0);
    assert_eq!(s(ItemKind::Int, 5.0, 0.0, 7.0, false, 10.0), 7.0);
    // log: by factors of 1.01 (menu.cpp:674-679)
    assert!((s(ItemKind::LogFloat, 1.0, 0.01, 100.0, false, 1.0) - 1.01).abs() < 1e-6);
    assert!((s(ItemKind::LogFloat, 1.0, 0.01, 100.0, true, 1.0) - 1.0 / 1.01).abs() < 1e-6);
    assert!(
        (s(ItemKind::LogBlendable, 1.0, 0.01, 100.0, false, 10.0) - 1.01f32.powi(10)).abs() < 1e-5
    );
    assert_eq!(s(ItemKind::LogFloat, 99.9, 0.01, 100.0, false, 10.0), 100.0);
    // a log item with no bounds gets 0.01..100 (menu.cpp:228-233)
    let tree = MenuTree::new();
    let zoom = tree.menus[menu::MOTION]
        .items
        .iter()
        .find(|i| i.def.label == "zoom amount")
        .unwrap();
    assert_eq!((zoom.def.min, zoom.def.max), (0.01, 100.0));
    // blendable: like float; setting cancels a blend in progress
    assert!((s(ItemKind::Blendable, 0.5, 0.0, 2.0, false, 1.0) - 0.52).abs() < 1e-6);
}

#[test]
fn menu_int_and_log_items_edit_through_set_value() {
    let mut h = Harness::new();
    point_menu_at(&mut h.ui, menu::WAVEFORM, "wave type");
    h.preset.wave_mode = 2;
    h.send(enter());
    h.send(down());
    assert_eq!(h.preset.wave_mode, 1);
    h.send(Command::PageDown(Mods::NONE));
    assert_eq!(h.preset.wave_mode, 0);
    h.send(Command::Escape);
    assert_eq!(h.preset.wave_mode, 2, "restored");

    point_menu_at(&mut h.ui, menu::MOTION, "zoom amount");
    h.preset.zoom.set(1.0);
    h.send(enter());
    h.send(up());
    assert!((h.preset.zoom.value() - 1.01).abs() < 1e-6);
}

#[test]
fn menu_bool_item_toggles_on_enter() {
    let mut h = Harness::new();
    point_menu_at(&mut h.ui, menu::WAVEFORM, "use dots");
    assert!(!h.preset.wave_dots);
    let acts = h.send(enter());
    assert_eq!(
        acts,
        vec![UiAction::SetValue(
            FieldId::Preset(PresetField::WaveDots),
            FieldValue::Bool(true)
        )]
    );
    assert!(h.preset.wave_dots);
    h.send(Command::NextPresetSoft); // Space selects in a menu (menu.cpp:487)
    assert!(!h.preset.wave_dots);
}

#[test]
fn menu_custom_wave_slot_reaches_the_right_wave() {
    let mut h = Harness::new();
    h.ch('m');
    // root: Motion, shapes, waves, ... ; into "custom waves" then "custom wave 3"
    h.send(down());
    h.send(down());
    h.send(enter());
    assert_eq!(h.ui.menu.cur, menu::CUSTOM_WAVES);
    h.send(down());
    h.send(down());
    h.send(enter());
    assert_eq!(h.ui.menu.cur, menu::WAVECODE0 + 2);
    let (before, others) = (h.preset.waves[2].enabled, h.preset.waves[1].enabled);
    let acts = h.send(enter()); // item 0: "enabled"
    assert_eq!(
        acts,
        vec![UiAction::SetValue(
            FieldId::Wave(2, WaveField::Enabled),
            FieldValue::Bool(before == 0)
        )]
    );
    assert_ne!(h.preset.waves[2].enabled, before);
    assert_eq!(h.preset.waves[1].enabled, others);
}

#[test]
fn menu_tree_item_count_matches_cpp() {
    // Counted from the AddItem calls of CPlugin::BuildMenus (plugin.cpp:6702-6868):
    // preset 7, motion 12, simple waveform 17, borders/motion vectors 19,
    // post-processing 19 (stereo 3D at 6802 is commented out); each of the
    // 4 custom waves 18 and each of the 4 custom shapes 28.
    let tree = MenuTree::new();
    let n = |m: usize| tree.menus[m].items.len();
    assert_eq!(
        [
            n(ROOT),
            n(menu::MOTION),
            n(menu::WAVEFORM),
            n(menu::AUGMENT),
            n(POST)
        ],
        [7, 12, 17, 19, 19]
    );
    assert_eq!(n(menu::CUSTOM_SHAPES) + n(menu::CUSTOM_WAVES), 0);
    for i in 0..4 {
        assert_eq!(n(menu::WAVECODE0 + i), 18);
        assert_eq!(n(menu::SHAPECODE0 + i), 28);
    }
    assert_eq!(tree.item_count(), 7 + 12 + 17 + 19 + 19 + 4 * 18 + 4 * 28);
    assert_eq!(tree.item_count(), 258);
    // sub-menus: 6 under the preset menu, 4 under each of shapes and waves
    assert_eq!(tree.menus[menu::CUSTOM_SHAPES].children.len(), 4);
    assert_eq!(tree.menus[menu::CUSTOM_WAVES].children.len(), 4);
    // every sub-menu knows its parent (AddChildMenu -> SetParentPointer)
    for (i, m) in tree.menus.iter().enumerate() {
        for &k in &m.children {
            assert_eq!(tree.menus[k].parent, Some(i));
        }
    }
    // sample rows pinned to plugin.cpp:6794, 6743, 6781, 6843 and plugin.rc
    let row = |m: usize, label: &str| {
        tree.menus[m]
            .items
            .iter()
            .find(|i| i.def.label == label)
            .unwrap()
            .def
    };
    let d = row(POST, "sustain level");
    assert_eq!((d.kind, d.min, d.max), (ItemKind::Blendable, 0.5, 1.0));
    assert_eq!(
        d.tooltip,
        "controls the eventual fade to black; 1=no fade, 0.9=strong fade; 0.98=recommended."
    );
    let d = row(menu::WAVEFORM, "wave type");
    assert_eq!((d.kind, d.min, d.max), (ItemKind::Int, 0.0, 7.0));
    assert_eq!(
        d.tooltip,
        "each value represents a different way of drawing the waveform"
    );
    let d = row(menu::MOTION, "zoom amount");
    assert_eq!(
        (d.kind, d.min, d.max),
        (ItemKind::LogBlendable, 0.01, 100.0)
    );
    assert_eq!(
        d.tooltip,
        "controls inward/outward motion.  0.9=zoom out, 1.0=no zoom, 1.1=zoom in"
    );
    let d = row(menu::SHAPECODE0 + 3, "number of sides");
    assert_eq!((d.kind, d.min, d.max), (ItemKind::Int, 3.0, 100.0));
    assert_eq!(
        d.tooltip,
        "the default number of sides that make up the polygonal shape"
    );
    assert_eq!(row(menu::WAVECODE0, "number of samples").max, 512.0);
    // each row carries a tooltip from the .rc table
    assert!(tree
        .menus
        .iter()
        .flat_map(|m| &m.items)
        .all(|i| !i.def.tooltip.is_empty()));
}

#[test]
fn disabled_menu_rows_are_skipped_by_navigation() {
    let mut tree = MenuTree::new();
    // a preset with no pixel shaders (SetMenusForPresetVersion(0, 0))
    tree.set_for_preset_version(0, 0);
    let row = |t: &MenuTree, label: &str| {
        t.menus[ROOT].children.len()
            + t.menus[ROOT]
                .items
                .iter()
                .position(|i| i.def.label == label)
                .unwrap()
    };
    assert!(
        !tree.menus[ROOT]
            .items
            .iter()
            .find(|i| i.def.label == "[ edit warp shader ]")
            .unwrap()
            .enabled
    );
    // move from "edit per_vertex equations" down: skips the two shader rows
    tree.menus[ROOT].cur_sel = row(&tree, "[ edit per_vertex equations ]");
    tree.down();
    assert_eq!(
        tree.menus[ROOT].cur_sel,
        row(&tree, "upgrade preset's pixel shader version")
    );
    tree.up();
    assert_eq!(
        tree.menus[ROOT].cur_sel,
        row(&tree, "[ edit per_vertex equations ]")
    );
    // blur items are off with no shaders at all, gamma and echo on
    let post = |l: &str| {
        tree.menus[POST]
            .items
            .iter()
            .find(|i| i.def.label == l)
            .unwrap()
            .enabled
    };
    assert!(!post("blur1: min color value"));
    assert!(post("gamma adjustment"));
    tree.set_for_preset_version(4, 4);
    let post = |l: &str| {
        tree.menus[POST]
            .items
            .iter()
            .find(|i| i.def.label == l)
            .unwrap()
            .enabled
    };
    assert!(post("blur1: min color value"));
    assert!(!post("gamma adjustment"), "the comp shader replaces it");
}

#[test]
fn string_item_opens_the_editor_and_ctrl_enter_applies() {
    let mut h = Harness::new();
    h.preset.per_frame = "zoom=1;".into();
    point_menu_at(&mut h.ui, ROOT, "[ edit per_frame equations ]");
    h.send(enter());
    assert_eq!(h.ui.mode, UiMode::EditMenuString);
    assert!(h.ui.edit.active && h.ui.edit.display_as_code);
    assert_eq!(h.ui.edit.as_string(), "zoom=1;");
    // the first time the cursor is where the item last left it: 0
    // (m_nLastCursorPos, menu.cpp:54, 418)
    assert_eq!(h.ui.edit.cursor, 0);
    h.send(Command::End(Mods::NONE));
    assert_eq!(h.ui.edit.cursor, 7);
    // typing goes into the text
    h.type_str("rot=2;");
    // Enter in code is a line break, not an accept
    let acts = h.send(enter());
    assert!(acts.is_empty());
    h.type_str("x=1;");
    assert_eq!(h.ui.edit.as_string(), "zoom=1;rot=2;\nx=1;");
    // Ctrl+Enter applies
    let acts = h.send(Command::Enter(Mods::CTRL));
    assert_eq!(
        acts,
        vec![
            UiAction::SetValue(
                FieldId::Preset(PresetField::PerFrame),
                FieldValue::Text("zoom=1;rot=2;\nx=1;".into())
            ),
            UiAction::Recompile(Recompile::PresetCode),
        ]
    );
    assert_eq!(h.ui.mode, UiMode::Menu);
    assert!(!h.ui.edit.active);
    // the cursor is remembered for the next time (menu.cpp:418)
    h.send(enter());
    assert_eq!(h.ui.edit.cursor, 18);
}

#[test]
fn escape_in_code_editor_returns_to_the_menu() {
    let mut h = Harness::new();
    point_menu_at(&mut h.ui, ROOT, "[ edit per_frame equations ]");
    h.send(enter());
    h.send(Command::Escape);
    assert_eq!((h.ui.mode, h.ui.edit.active), (UiMode::Menu, false));
    assert_eq!(h.preset.per_frame, "", "nothing was applied");
}

#[test]
fn import_export_items_open_a_path_prompt() {
    let mut h = Harness::new();
    h.ui.menu.cur = menu::WAVECODE0 + 1;
    h.ui.mode = UiMode::Menu;
    h.ui.menu.menus[menu::WAVECODE0 + 1].cur_sel = 13; // "--export to file"
    assert_eq!(
        h.ui.menu.current().cur_item().unwrap().def.label,
        "--export to file"
    );
    h.send(enter());
    assert_eq!(h.ui.mode, UiMode::ExportWave);
    assert!(h.ui.edit.active);
    let dir = h.tmp.0.display().to_string();
    assert!(h.ui.edit.as_string().ends_with("file.dat"));
    assert!(h.ui.edit.as_string().starts_with(&dir));
    let acts = h.send(enter());
    match &acts[..] {
        [UiAction::File {
            op: crate::ui::FileOp::ExportWave,
            slot: 1,
            path,
        }] => {
            assert!(path.ends_with("file.dat"));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(h.ui.mode, UiMode::Menu);
}

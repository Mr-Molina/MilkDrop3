//! Tests of the UI's prompts, the load list and the string editor
//! (Task 7.7): save and overwrite, delete, rename, change of mode keys,
//! toasts, the mash-up screen and the field accessors.

use md_preset::Preset;

use crate::commands::{Command, Mods};
use crate::ui::menu::{self, MenuTree, ROOT};
use crate::ui::menu_tree::ItemKind;
use crate::ui::{FieldId, PresetField, UiAction, UiFlag, UiMode, WaveField};
use crate::ui_tests::{down, enter, point_menu_at, up, Harness};

#[test]
fn save_overwrite_requires_confirmation() {
    let mut h = Harness::new();
    h.ch('s');
    assert_eq!(h.ui.mode, UiMode::SaveAs);
    assert_eq!(
        h.ui.edit.as_string(),
        "current preset",
        "starts from m_szDesc"
    );
    // type over it: select the name away with backspaces
    for _ in 0..14 {
        h.send(Command::PrevPreset);
    }
    h.type_str("alpha");
    let acts = h.send(enter());
    assert!(
        acts.is_empty(),
        "the file exists: no save yet, a question first"
    );
    assert_eq!(h.ui.mode, UiMode::SaveOverwrite);
    assert!(!h.ui.edit.active);
    // anything but y goes back to the name
    assert!(h.ch('n').is_empty());
    assert_eq!(h.ui.mode, UiMode::SaveAs);
    assert!(h.ui.edit.active);
    assert_eq!(h.ui.edit.as_string(), "alpha");
    // Escape from the question also goes back (plugin.cpp:5927-5932)
    h.send(enter());
    assert_eq!(h.ui.mode, UiMode::SaveOverwrite);
    h.send(Command::Escape);
    assert_eq!((h.ui.mode, h.ui.edit.active), (UiMode::SaveAs, true));
    // y saves
    h.send(enter());
    let acts = h.ch('y');
    assert_eq!(
        acts,
        vec![UiAction::SavePreset {
            path: h.path("alpha.milk"),
            name: "alpha".into()
        }]
    );
    assert_eq!((h.ui.mode, h.ui.edit.active), (UiMode::Regular, false));
}

#[test]
fn save_of_a_new_name_goes_straight_through() {
    let mut h = Harness::new();
    h.ch('S');
    for _ in 0..14 {
        h.send(Command::PrevPreset);
    }
    h.type_str("fresh");
    let acts = h.send(enter());
    assert_eq!(
        acts,
        vec![UiAction::SavePreset {
            path: h.path("fresh.milk"),
            name: "fresh".into()
        }]
    );
    assert_eq!(h.ui.mode, UiMode::Regular);
    // s only opens the prompt in regular mode (plugin.cpp:6528-6530)
    h.ch('m');
    h.ch('s');
    assert_eq!(h.ui.mode, UiMode::Menu);
}

#[test]
fn save_prompt_filters_file_name_characters() {
    let mut h = Harness::new();
    h.ch('s');
    let before = h.ui.edit.as_string();
    for bad in ['"', '\\', '/', ':', '*', '?', '|', '<', '>', '&'] {
        h.ch(bad);
    }
    // '|' and '~' are outside the accepted range anyway; the rest are refused
    assert_eq!(h.ui.edit.as_string(), before);
    assert!(h
        .ui
        .toasts
        .iter()
        .any(|t| t.msg == "(illegal character)" && (t.expire - t.birth - 2.5).abs() < 1e-6));
    h.ch('é');
    assert_eq!(h.ui.edit.as_string(), before, "outside ' '..='z'");
    h.ch('{');
    assert!(h.ui.edit.as_string().ends_with('{'));
}

#[test]
fn delete_prompt_flow() {
    let mut h = Harness::new();
    h.ch('l');
    h.ui.load.pos = h.row("beta.milk");
    h.send(Command::Delete(Mods::NONE));
    assert_eq!(h.ui.mode, UiMode::LoadDel);
    // Space does not load a random preset in a prompt (locked by code)
    assert!(h.send(Command::NextPresetSoft).is_empty());
    // any other character answers no
    assert!(h.ch('n').is_empty());
    assert_eq!(h.ui.mode, UiMode::Load);
    // Escape answers no
    h.send(Command::Delete(Mods::NONE));
    h.send(Command::Escape);
    assert_eq!(h.ui.mode, UiMode::Load);
    // y deletes
    h.send(Command::Delete(Mods::NONE));
    let acts = h.ch('Y');
    assert_eq!(
        acts,
        vec![UiAction::DeletePreset {
            index: h.row("beta.milk"),
            path: h.path("beta.milk")
        }]
    );
    assert_eq!(h.ui.mode, UiMode::Load);
    // a directory cannot be deleted
    h.ui.load.pos = h.row("*sub");
    h.send(Command::Delete(Mods::NONE));
    assert_eq!(h.ui.mode, UiMode::Load);
}

#[test]
fn rename_prompt_flow() {
    let mut h = Harness::new();
    h.ch('l');
    h.ui.load.pos = h.row("beta.milk");
    h.send(Command::Insert);
    assert_eq!(h.ui.mode, UiMode::LoadRename);
    assert_eq!(
        h.ui.edit.as_string(),
        "beta",
        "the name without its extension"
    );
    assert_eq!(h.ui.edit.prompt, "Enter the new name for \"beta\":");
    h.type_str("2");
    h.ch('/'); // illegal in a file name
    assert_eq!(h.ui.edit.as_string(), "beta2");
    let acts = h.send(enter());
    assert_eq!(
        acts,
        vec![UiAction::RenamePreset {
            index: h.row("beta.milk"),
            from: h.path("beta.milk"),
            to: h.path("beta2.milk"),
            name: "beta2".into(),
        }]
    );
    assert_eq!(
        h.ui.mode,
        UiMode::LoadRename,
        "stays until the host says it worked"
    );
    h.ui.end_rename(&h.lib, None);
    assert_eq!(h.ui.mode, UiMode::Load);
    // Escape leaves the prompt for the list
    h.send(Command::Insert);
    h.send(Command::Escape);
    assert_eq!((h.ui.mode, h.ui.edit.active), (UiMode::Load, false));
    // a directory cannot be renamed
    h.ui.load.pos = h.row("*sub");
    h.send(Command::Insert);
    assert_eq!(h.ui.mode, UiMode::Load);
}

#[test]
fn load_list_enter_loads_and_dirs_change_directory() {
    let mut h = Harness::new();
    h.ch('l');
    h.ui.load.pos = h.row("gamma.milk");
    let acts = h.send(enter());
    assert_eq!(
        acts,
        vec![UiAction::LoadPreset {
            index: h.row("gamma.milk"),
            path: h.path("gamma.milk"),
            soft: false
        }]
    );
    let acts = h.send(Command::NextPresetSoft);
    assert!(matches!(
        &acts[..],
        [UiAction::LoadPreset { soft: true, .. }]
    ));
    h.ui.load.pos = h.row("*sub");
    assert_eq!(h.send(enter()), vec![UiAction::ChangeDir(h.path("sub"))]);
    h.ui.load.pos = h.row("*..");
    assert!(matches!(&h.send(enter())[..], [UiAction::ChangeDir(_)]));
}

#[test]
fn load_list_navigation_and_exit() {
    let mut h = Harness::new();
    h.ch('L');
    h.send(Command::Home(Mods::NONE));
    assert_eq!(h.ui.load.pos, 0);
    h.send(down());
    h.send(down());
    assert_eq!(h.ui.load.pos, 2);
    h.send(up());
    assert_eq!(h.ui.load.pos, 1);
    h.send(Command::End(Mods::NONE));
    assert_eq!(h.ui.load.pos, h.lib.entries().len() - 1);
    h.send(down());
    assert_eq!(h.ui.load.pos, h.lib.entries().len() - 1, "no wrap");
    // Left leaves, as does Escape (plugin.cpp:5888)
    h.send(Command::Left(Mods::NONE));
    assert_eq!(h.ui.mode, UiMode::Regular);
    h.ch('l');
    h.send(Command::Escape);
    assert_eq!(h.ui.mode, UiMode::Regular);
    // the menu key opens the list from the menu and the menu from the list
    h.ch('m');
    h.ch('l');
    assert_eq!(h.ui.mode, UiMode::Load);
}

#[test]
fn upgrade_prompt_answers() {
    let mut h = Harness::new();
    point_menu_at(&mut h.ui, ROOT, "upgrade preset's pixel shader version");
    h.send(enter());
    assert_eq!(h.ui.mode, UiMode::UpgradePixelShader);
    assert!(h.ch('\r').is_empty());
    assert_eq!(
        h.ui.mode,
        UiMode::UpgradePixelShader,
        "Enter does not answer (plugin.cpp:5365)"
    );
    assert_eq!(h.ch('y'), vec![UiAction::UpgradePixelShader]);
    assert_eq!(h.ui.mode, UiMode::Menu);
    h.send(enter());
    assert!(h.ch('n').is_empty());
    assert_eq!(h.ui.mode, UiMode::Menu);
    h.send(enter());
    h.send(Command::Escape);
    assert_eq!(h.ui.mode, UiMode::Menu);
}

#[test]
fn function_keys_work_in_every_mode() {
    let mut h = Harness::new();
    assert_eq!(
        h.send(Command::ToggleFps),
        vec![UiAction::FlagChanged(UiFlag::ShowFps, true)]
    );
    h.ch('s'); // while a string is edited
    assert_eq!(
        h.send(Command::ToggleFps),
        vec![UiAction::FlagChanged(UiFlag::ShowFps, false)]
    );
    h.send(Command::TogglePresetName);
    h.send(Command::ToggleRating);
    assert!(h.ui.show_preset_name && h.ui.show_rating);
    // F1 is not read by the editor (it absorbs it), but is everywhere else
    h.send(Command::ToggleHelp);
    assert!(!h.ui.show_help);
    h.send(Command::Escape);
    h.send(Command::ToggleHelp);
    assert!(h.ui.show_help);
    // Escape closes the help in regular mode
    h.send(Command::Escape);
    assert!(!h.ui.show_help);
    // 7.6 ruling: pause only flips and reports
    assert_eq!(
        h.send(Command::TogglePause),
        vec![UiAction::FlagChanged(UiFlag::Paused, true)]
    );
    assert!(h.ui.paused);
    assert_eq!(
        h.send(Command::CycleMaxFps),
        vec![UiAction::Forward(Command::CycleMaxFps)]
    );
    h.send(Command::Escape);
    assert_eq!(
        h.ch('n'),
        vec![UiAction::FlagChanged(UiFlag::ShowDebug, true)]
    );
}

#[test]
fn space_loads_a_random_preset_only_in_regular_mode() {
    let mut h = Harness::new();
    assert_eq!(
        h.send(Command::NextPresetSoft),
        vec![UiAction::Forward(Command::NextPresetSoft)]
    );
    h.ch('m');
    // in the menu Space selects; the menu never forwards it
    assert!(!h
        .send(Command::NextPresetSoft)
        .iter()
        .any(|a| matches!(a, UiAction::Forward(_))));
    // Backspace goes back a preset in regular mode, up a menu in the menu
    h.send(Command::Escape);
    assert_eq!(
        h.send(Command::PrevPreset),
        vec![UiAction::Forward(Command::PrevPreset)]
    );
}

#[test]
fn text_editor_selection_clipboard_and_limits() {
    let mut h = Harness::new();
    h.ch('s');
    for _ in 0..14 {
        h.send(Command::PrevPreset);
    }
    h.type_str("hello world");
    // select "world" with Shift+Left
    for _ in 0..5 {
        h.send(Command::Left(Mods::SHIFT));
    }
    assert_eq!(h.ui.edit.anchor, Some(11));
    assert_eq!(h.send(Command::Copy), vec![UiAction::Copy("world".into())]);
    assert_eq!(h.send(Command::Cut), vec![UiAction::Copy("world".into())]);
    assert_eq!(h.ui.edit.as_string(), "hello ");
    h.clipboard = "there".into();
    h.send(Command::Paste);
    assert_eq!(h.ui.edit.as_string(), "hello there");
    // Ctrl+Left jumps by word
    h.send(Command::Left(Mods::CTRL));
    assert_eq!(h.ui.edit.cursor, 6);
    h.send(Command::Home(Mods::NONE));
    assert_eq!(h.ui.edit.cursor, 0);
    h.send(Command::End(Mods::NONE));
    assert_eq!(h.ui.edit.cursor, 11);
    // overtype
    h.send(Command::Home(Mods::NONE));
    h.send(Command::Insert);
    h.ch('J');
    assert_eq!(h.ui.edit.as_string(), "Jello there");
    // delete right, backspace
    h.send(Command::Delete(Mods::NONE));
    assert_eq!(h.ui.edit.as_string(), "Jllo there");
    // too long: the limit is nMaxLen - 1 characters
    let max = h.ui.edit.max_len;
    h.ui.edit.set_text(&"x".repeat(max - 1));
    h.ch('y');
    assert_eq!(h.ui.edit.text.len(), max - 1);
    assert!(h.ui.toasts.iter().any(|t| t.msg == "(string too long)"));
}

#[test]
fn code_editor_moves_by_lines() {
    let mut h = Harness::new();
    h.preset.per_pixel = "a=1;\nbbbbbb=2;\nc=3;".into();
    point_menu_at(&mut h.ui, ROOT, "[ edit per_vertex equations ]");
    h.send(enter());
    h.send(Command::Home(Mods::CTRL));
    h.send(down());
    assert_eq!(h.ui.edit.cursor, 5);
    h.send(Command::End(Mods::NONE));
    assert_eq!(h.ui.edit.cursor, 5 + 9);
    h.send(down());
    assert_eq!(
        h.ui.edit.cursor,
        15 + 4,
        "column clipped to the short last line"
    );
    h.send(up());
    assert_eq!(h.ui.edit.cursor, 5 + 4);
    h.send(Command::Home(Mods::NONE));
    assert_eq!(h.ui.edit.cursor, 5);
}

#[test]
fn toast_durations_and_stacking() {
    let mut h = Harness::new();
    // two errors stack; a notification replaces the previous notification only
    h.ui.add_error("first", 3.0, crate::ui::ErrCategory::Preset, true, h.now);
    h.ui.add_error("note1", 3.0, crate::ui::ErrCategory::Notify, false, h.now);
    h.ui.add_error("note2", 3.0, crate::ui::ErrCategory::Notify, false, h.now);
    let msgs: Vec<_> = h.ui.toasts.iter().map(|t| t.msg.as_str()).collect();
    assert_eq!(msgs, ["first", "note2"]);
    // shown while birth <= now < expire; pruned after
    h.now += 2.9;
    h.send(Command::Escape);
    assert_eq!(h.ui.toasts.len(), 2);
    h.now += 0.1;
    h.send(Command::Escape);
    assert!(h.ui.toasts.is_empty());
    // clear by category
    h.ui.add_error("a", 6.0, crate::ui::ErrCategory::Misc, true, h.now);
    h.ui.add_error("b", 6.0, crate::ui::ErrCategory::Preset, true, h.now);
    h.ui.clear_errors(Some(crate::ui::ErrCategory::Preset));
    assert_eq!(h.ui.toasts.len(), 1);
    h.ui.clear_errors(None);
    assert!(h.ui.toasts.is_empty());
}

#[test]
fn mashup_screen_keys_and_delayed_apply() {
    let mut h = Harness::new();
    point_menu_at(&mut h.ui, ROOT, "do a preset mash-up");
    h.send(enter());
    assert_eq!(h.ui.mode, UiMode::Mashup);
    h.ch('3');
    assert_eq!(h.ui.mash.slot, 2);
    h.send(Command::Left(Mods::NONE));
    h.send(Command::Left(Mods::NONE));
    h.send(Command::Left(Mods::NONE));
    assert_eq!(h.ui.mash.slot, 0);
    h.send(Command::Right(Mods::NONE));
    assert_eq!(h.ui.mash.slot, 1);
    // browsing never goes onto a directory (max(.., m_nDirs))
    let dirs = h.lib.dir_count();
    h.send(Command::Home(Mods::NONE));
    assert_eq!(h.ui.mash.preset[1], dirs);
    h.send(up());
    assert_eq!(h.ui.mash.preset[1], dirs);
    h.send(down());
    assert_eq!(h.ui.mash.preset[1], dirs + 1);
    assert_eq!(h.ch('h'), vec![UiAction::RandomizeMashup { all: false }]);
    assert_eq!(h.ch('H'), vec![UiAction::RandomizeMashup { all: true }]);
    h.ui.set_mash_preset(1, dirs + 2, h.frame);
    assert_eq!(h.ui.mash.preset[1], dirs + 2);
    h.send(Command::Escape);
    assert_eq!(h.ui.mode, UiMode::Regular);
}

#[test]
fn field_values_roundtrip_through_the_preset() {
    let mut p = Preset::defaults();
    for f in PresetField::ALL {
        let id = FieldId::Preset(f);
        let v = id.get(&p);
        id.set(&mut p, &v);
        assert_eq!(id.get(&p), v, "{f:?}");
    }
    for slot in 0..4u8 {
        for f in WaveField::ALL {
            let id = FieldId::Wave(slot, f);
            let v = id.get(&p);
            id.set(&mut p, &v);
            assert_eq!(id.get(&p), v, "{f:?}");
        }
        for f in crate::ui::ShapeField::ALL {
            let id = FieldId::Shape(slot, f);
            let v = id.get(&p);
            id.set(&mut p, &v);
            assert_eq!(id.get(&p), v, "{f:?}");
        }
    }
    // every field a table row names exists in the field enums, and text
    // fields are exactly the Text items
    let tree = MenuTree::new();
    for m in &tree.menus {
        for i in &m.items {
            if let Some(f) = menu::field_of(i.def.target, m.slot) {
                assert_eq!(f.is_text(), i.def.kind == ItemKind::Text, "{}", i.def.label);
            }
        }
    }
}

/// Scan the harness directory again, as the host does after a file operation.
fn rescan(h: &mut Harness) {
    h.lib = crate::library::PresetLibrary::scan(&h.tmp.0, md_preset::PsVersion::Ps3_0);
}

#[test]
fn end_delete_keeps_the_row_and_clamps() {
    let mut h = Harness::new();
    h.ch('l');
    h.ui.load.pos = h.row("beta.milk");
    h.send(Command::Delete(Mods::NONE));
    h.ch('y');
    std::fs::remove_file(h.path("beta.milk")).unwrap();
    rescan(&mut h);
    h.ui.end_delete(&h.lib);
    // the same row: now the next file (plugin.cpp:8199-8201)
    assert_eq!(h.ui.load.pos, h.row("gamma.milk"));
    assert_eq!(h.lib.entries()[h.ui.load.pos].name, "gamma.milk");
    // deleting the last row selects the new last one
    let last = h.lib.entries().len() - 1;
    h.ui.load.pos = last;
    std::fs::remove_file(h.path("rho.milk")).unwrap();
    rescan(&mut h);
    h.ui.end_delete(&h.lib);
    assert_eq!(h.ui.load.pos, h.lib.entries().len() - 1);
    // library_changed would have jumped to the playing file instead
    h.ui.load.pos = 3;
    h.ui.library_changed(&h.lib, Some("alpha.milk"));
    assert_eq!(h.ui.load.pos, h.row("alpha.milk"));
}

#[test]
fn end_rename_highlights_the_renamed_file() {
    let mut h = Harness::new();
    h.ch('l');
    h.ui.load.pos = h.row("beta.milk");
    h.send(Command::Insert);
    h.type_str("zz");
    h.send(enter());
    std::fs::rename(h.path("beta.milk"), h.path("betazz.milk")).unwrap();
    rescan(&mut h);
    h.ui.end_rename(&h.lib, Some("betazz.milk"));
    assert_eq!((h.ui.mode, h.ui.edit.active), (UiMode::Load, false));
    assert_eq!(h.lib.entries()[h.ui.load.pos].name, "betazz.milk");
    // a failed move leaves the prompt and the row alone
    h.ui.load.pos = 4;
    h.send(Command::Insert);
    h.ui.end_rename(&h.lib, None);
    assert_eq!((h.ui.mode, h.ui.load.pos), (UiMode::Load, 4));
}

#[test]
fn dir_changed_goes_to_row_zero() {
    let mut h = Harness::new();
    h.ch('l');
    h.ui.load.pos = 3;
    h.ui.dir_changed();
    assert_eq!(h.ui.load.pos, 0);
}

#[test]
fn f1_ends_the_press_f1_message_and_ctrl_backspace_is_not_an_edit() {
    let mut h = Harness::new();
    h.ui.show_press_f1 = true;
    h.send(Command::ToggleHelp);
    assert!(!h.ui.show_press_f1 && h.ui.show_help);
    // Ctrl+Backspace is its own command and PrevPreset outside the editor
    let chord = crate::keymap::KeyChord::ctrl(crate::keymap::Key::Backspace);
    assert_eq!(crate::keymap::lookup(chord), Some(Command::CtrlBackspace));
    assert_eq!(
        h.send(Command::CtrlBackspace),
        vec![UiAction::Forward(Command::PrevPreset)]
    );
    h.ch('s');
    let before = h.ui.edit.as_string();
    assert!(h.send(Command::CtrlBackspace).is_empty());
    assert_eq!(h.ui.edit.as_string(), before);
    h.send(Command::PrevPreset);
    assert_eq!(h.ui.edit.as_string().len(), before.len() - 1);
}

#[test]
fn host_toasts_carry_the_cpp_durations() {
    use crate::ui::host_toasts as t;
    use crate::ui::ErrCategory::{Misc, Notify, Preset};
    let table = [
        (t::SAVE_FAILED, 6.0, Preset, true),
        (t::SAVE_OK, 3.0, Notify, false),
        (t::DELETE_FAILED, 6.0, Misc, true),
        (t::DELETE_OK, 3.0, Notify, false),
        (t::RENAME_EXISTS, 6.0, Preset, true),
        (t::RENAME_FAILED, 6.0, Misc, true),
        (t::RENAME_OK, 3.0, Notify, false),
    ];
    for (s, d, c, b) in table {
        assert_eq!((s.duration, s.category, s.bold), (d, c, b));
    }
    let mut h = Harness::new();
    h.ui.raise(
        t::DELETE_OK,
        Some(&crate::ui::strings::preset_deleted("a.milk")),
        h.now,
    );
    assert_eq!(h.ui.toasts[0].msg, "[preset \"a.milk\" deleted]");
    assert_eq!(h.ui.toasts[0].expire - h.ui.toasts[0].birth, 3.0);
}

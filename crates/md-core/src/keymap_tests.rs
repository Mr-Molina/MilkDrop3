use crate::commands::{Command, Mods};
use crate::keymap::{accepts_text_char, lookup, Key, KeyChord, BINDINGS};
use std::collections::HashSet;

const HELP: &str = include_str!("../../../assets/help.txt");

/// The key chords `assets/help.txt` names, counted by hand from the file:
/// ESC, ALT+ENTER, ALT+SHIFT, SPACE, H, BACKSPACE, `, ~, R, L, M, S, A, D,
/// F1, F2, F3, F4, F5, F6, F7, F10 (22). Letters are the character the key
/// types unshifted; the C++ binds the lowercase `r` only (plugin.cpp:6333).
fn help_chords() -> Vec<(&'static str, KeyChord, Command)> {
    vec![
        ("ESC", KeyChord::plain(Key::Escape), Command::Escape),
        (
            "ALT+ENTER",
            KeyChord::alt(Key::Enter),
            Command::ToggleFullscreen,
        ),
        (
            "ALT+SHIFT",
            KeyChord::alt(Key::Shift),
            Command::ToggleStretch,
        ),
        (
            "SPACE",
            KeyChord::plain(Key::Space),
            Command::NextPresetSoft,
        ),
        ("H", KeyChord::ch('h'), Command::NextPresetHard),
        (
            "BACKSPACE",
            KeyChord::plain(Key::Backspace),
            Command::PrevPreset,
        ),
        ("`", KeyChord::ch('`'), Command::ToggleLock),
        ("~", KeyChord::ch('~'), Command::ToggleLock),
        ("R", KeyChord::ch('r'), Command::ToggleRandomOrder),
        ("L", KeyChord::ch('l'), Command::OpenLoadMenu),
        ("M", KeyChord::ch('m'), Command::ToggleMenu),
        ("S", KeyChord::ch('s'), Command::SaveAs),
        ("A", KeyChord::ch('a'), Command::RandomMashup),
        ("D", KeyChord::ch('d'), Command::ToggleShaderLocks),
        ("F1", KeyChord::plain(Key::F(1)), Command::ToggleHelp),
        ("F2", KeyChord::plain(Key::F(2)), Command::CycleMaxFps),
        (
            "F3",
            KeyChord::plain(Key::F(3)),
            Command::CycleTimeBetweenPresets,
        ),
        ("F4", KeyChord::plain(Key::F(4)), Command::TogglePresetName),
        ("F5", KeyChord::plain(Key::F(5)), Command::ToggleFps),
        ("F6", KeyChord::plain(Key::F(6)), Command::ToggleRating),
        ("F7", KeyChord::plain(Key::F(7)), Command::ToggleAlwaysOnTop),
        ("F10", KeyChord::plain(Key::F(10)), Command::TogglePause),
    ]
}

#[test]
fn every_help_text_key_has_a_binding() {
    let chords = help_chords();
    assert_eq!(chords.len(), 22);
    for (name, chord, command) in chords {
        assert!(
            BINDINGS.contains(&(chord, command)),
            "{name} is named in help.txt but {chord:?} -> {command:?} is not in BINDINGS"
        );
        assert_eq!(lookup(chord), Some(command), "{name}");
    }
}

#[test]
fn help_text_names_every_key_in_the_list() {
    // The hand list above must not drift from the file.
    for needle in [
        "ESC ",
        "ALT+ENTER",
        "ALT+SHIFT",
        "SPACE / H",
        "BACKSPACE",
        "` / ~",
        "R ",
        "L ",
        "M ",
        "S ",
        "A ",
        "D ",
        "F1 ",
        "F2 ",
        "F3 ",
        "F4 ",
        "F5 ",
        "F6 ",
        "F7 ",
        "F10 ",
    ] {
        assert!(HELP.contains(needle), "help.txt lost {needle:?}");
    }
}

#[test]
fn digits_feed_numeric_input() {
    for d in 0..=9u8 {
        let c = char::from(b'0' + d);
        assert_eq!(lookup(KeyChord::ch(c)), Some(Command::Digit(d)), "{c}");
    }
}

#[test]
fn y_k_and_shift_k_pick_the_numeric_input_mode() {
    assert_eq!(
        lookup(KeyChord::ch('y')),
        Some(Command::BeginCustomMessageInput)
    );
    assert_eq!(
        lookup(KeyChord::ch('Y')),
        Some(Command::BeginCustomMessageInput)
    );
    assert_eq!(lookup(KeyChord::ch('k')), Some(Command::BeginSpriteInput));
    assert_eq!(
        lookup(KeyChord::ch('K')),
        Some(Command::BeginSpriteKillInput)
    );
    assert_eq!(lookup(KeyChord::ch('*')), Some(Command::ClearNumericInput));
}

#[test]
fn kill_chords() {
    assert_eq!(
        lookup(KeyChord::ctrl(Key::Char('k'))),
        Some(Command::KillSprites)
    );
    assert_eq!(
        lookup(KeyChord::ctrl(Key::Char('t'))),
        Some(Command::KillTexts)
    );
    assert_eq!(
        lookup(KeyChord::ctrl(Key::Char('y'))),
        Some(Command::KillTexts)
    );
    // The app may send the shifted letter with Ctrl held.
    let c = KeyChord {
        key: Key::Char('K'),
        shift: true,
        ctrl: true,
        alt: false,
    };
    assert_eq!(lookup(c), Some(Command::KillSprites));
}

#[test]
fn bindings_are_unique() {
    let mut seen = HashSet::new();
    for (chord, command) in BINDINGS {
        assert!(
            seen.insert(*chord),
            "{chord:?} is bound twice (second: {command:?})"
        );
    }
}

#[test]
fn every_regular_char_case_of_the_cpp_is_bound() {
    // HandleRegularKey, plugin.cpp:6245-6608, one entry per `case`.
    let cases = "0123456789qQwWeEIinNrtToO!@aAdDhHfFgGjJkK[]{}<>sS`~lLmM-+*yY";
    for c in cases.chars() {
        assert!(
            lookup(KeyChord::ch(c)).is_some(),
            "{c:?} has a case in HandleRegularKey"
        );
    }
    // `R` and the Winamp keys (u, U, x, c, v, b) have none.
    for c in "RuUxcvbB".chars() {
        assert_eq!(lookup(KeyChord::ch(c)), None, "{c:?}");
    }
}

#[test]
fn navigation_keys_carry_their_modifiers() {
    let up = KeyChord::plain(Key::Up);
    assert_eq!(lookup(up), Some(Command::Up(Mods::NONE)));
    let shift_up = KeyChord { shift: true, ..up };
    assert_eq!(lookup(shift_up), Some(Command::Up(Mods::SHIFT)));
    let ctrl_left = KeyChord::ctrl(Key::Left);
    assert_eq!(lookup(ctrl_left), Some(Command::Left(Mods::CTRL)));
    let ctrl_enter = KeyChord::ctrl(Key::Enter);
    assert_eq!(lookup(ctrl_enter), Some(Command::Enter(Mods::CTRL)));
    let cs_del = KeyChord {
        shift: true,
        ..KeyChord::ctrl(Key::Delete)
    };
    assert_eq!(lookup(cs_del), Some(Command::Delete(Mods::CTRL_SHIFT)));
    // Plain Enter is not Alt+Enter.
    assert_eq!(
        lookup(KeyChord::plain(Key::Enter)),
        Some(Command::Enter(Mods::NONE))
    );
    assert_eq!(lookup(KeyChord::alt(Key::Up)), None);
}

#[test]
fn copy_cut_paste_and_insert() {
    assert_eq!(lookup(KeyChord::ctrl(Key::Char('c'))), Some(Command::Copy));
    assert_eq!(lookup(KeyChord::ctrl(Key::Insert)), Some(Command::Copy));
    assert_eq!(lookup(KeyChord::ctrl(Key::Char('x'))), Some(Command::Cut));
    assert_eq!(lookup(KeyChord::ctrl(Key::Char('v'))), Some(Command::Paste));
    assert_eq!(lookup(KeyChord::plain(Key::Insert)), Some(Command::Insert));
}

#[test]
fn modifiers_do_not_change_a_virtual_key_but_alt_does() {
    // VK_F2 ignores Shift and Ctrl (plugin.cpp:5411); Alt makes it a
    // WM_SYSKEYDOWN, which the plugin never reads.
    let f2 = KeyChord::plain(Key::F(2));
    assert_eq!(
        lookup(KeyChord { shift: true, ..f2 }),
        Some(Command::CycleMaxFps)
    );
    assert_eq!(
        lookup(KeyChord { ctrl: true, ..f2 }),
        Some(Command::CycleMaxFps)
    );
    assert_eq!(lookup(KeyChord { alt: true, ..f2 }), None);
    assert_eq!(
        lookup(KeyChord {
            ctrl: true,
            ..KeyChord::plain(Key::Space)
        }),
        Some(Command::NextPresetSoft)
    );
}

#[test]
fn text_chars_are_the_cpp_range() {
    for c in [' ', 'a', 'z', 'A', '0', '/', '{', '}', '`', '_'] {
        assert!(accepts_text_char(c), "{c:?}");
    }
    for c in ['|', '~', '\n', '\u{7f}', 'é'] {
        assert!(!accepts_text_char(c), "{c:?}");
    }
}

#[test]
fn no_media_or_desktop_commands_are_bound() {
    // F47 and A.4: media keys (Ctrl+arrows, the shell's Left/Right seek) and
    // Winamp keys are not ported. Ctrl+arrows are text-cursor words only.
    assert_eq!(
        lookup(KeyChord::ctrl(Key::Left)),
        Some(Command::Left(Mods::CTRL))
    );
    assert!(BINDINGS.iter().all(|(c, _)| c.key != Key::Char('u')));
}

#[test]
fn help_txt_has_lf_endings_and_matches_the_original() {
    let bytes = HELP.as_bytes();
    assert!(
        !bytes.contains(&b'\r'),
        "assets/help.txt must not contain CR"
    );
    assert!(
        bytes.is_ascii(),
        "text1.bin is plain ASCII and so is the copy"
    );
    assert!(HELP.ends_with("F10   Pause\n"));
    // FNV-1a of the file, so the copy cannot drift once the original is
    // deleted (Task 9.1).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h = (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
    }
    assert_eq!((h, bytes.len()), (0x701e_d215_e13b_5249, 794));
    // While the original exists: the same bytes apart from CR.
    let src =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../code/vis_milk2/text1.bin");
    if let Ok(orig) = std::fs::read(src) {
        let lf: Vec<u8> = orig.into_iter().filter(|b| *b != b'\r').collect();
        assert_eq!(lf, bytes);
    }
}

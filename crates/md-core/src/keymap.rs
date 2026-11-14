//! Keys and the regular-mode key bindings.
//!
//! The C++ splits input in two: `WM_CHAR` carries the character the keyboard
//! produced (letters, digits, punctuation), `WM_KEYDOWN` a virtual key
//! (F-keys, arrows, Space, Enter, Backspace, ..., plugin.cpp:5400). A mode
//! reads whichever it needs: regular mode reads characters in
//! `HandleRegularKey` (plugin.cpp:6245), text editing types characters and
//! absorbs every key (5171-5285, 5869), the load list seeks on every letter
//! (6256), the delete / overwrite / upgrade prompts answer on any character
//! (5287-5391).
//!
//! # Input contract (Task 7.6 ruling)
//!
//! The app does not know md-core's UI mode, so it never chooses by mode:
//!
//! - EVERY character goes to `Visualizer::text_input(ch)` =
//!   `command(`[`Command::TypeChar`]`(ch))`. md-core resolves it by mode: in
//!   regular mode through [`lookup_char`], elsewhere as text, seek or answer
//!   (7.7 owns that dispatch).
//! - Everything that is not a character goes through [`lookup`] and then
//!   `command`: non-character keys, and chords with Ctrl or Alt.
//! - A key that also types (Space; Enter, Backspace and Escape produce control
//!   characters md-core ignores) is sent both ways, key first, as Windows
//!   delivers `WM_KEYDOWN` then `WM_CHAR`.
//!
//! Classification, by what winit reports:
//!
//! | press | app sends |
//! |---|---|
//! | `q`, `Q` (Shift+q), `r`, `?` | `TypeChar('q')`, `TypeChar('Q')`, ... |
//! | `5`, Shift+`1` (`!`) | `TypeChar('5')`, `TypeChar('!')` |
//! | Ctrl+`K`, Ctrl+`T`, Ctrl+`C` | `lookup(KeyChord{ key: Char('k'), ctrl, .. })` (no `WM_CHAR` in C++) |
//! | Alt+Enter, Alt+Shift | `lookup(..)` (`WM_SYSKEYDOWN`) |
//! | F1..F10, Up, Home, Delete, Insert | `lookup(..)` |
//! | Space | `lookup(Space)`, then `TypeChar(' ')` |
//!
//! [`BINDINGS`] lists the regular-mode meaning of every chord, plus the
//! navigation keys with no modifiers. [`lookup`] resolves a chord the way the
//! C++ does: modifiers other than Alt do not change which non-character key
//! it is. [`lookup_char`] is the regular-mode resolver for a typed character.

use crate::commands::{Command, Mods};

/// A platform-neutral key. The app maps winit's logical keys onto it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A character key, as typed (`WM_CHAR`): case applied, so `Q` and `q`
    /// differ and Shift is already in the character. With Ctrl held it is the
    /// lowercase letter (the C++ sees the virtual key, plugin.cpp:6196).
    Char(char),
    /// `VK_SPACE` (plugin.cpp:5983).
    Space,
    /// `VK_RETURN`.
    Enter,
    /// `VK_ESCAPE`.
    Escape,
    /// `VK_BACK`.
    Backspace,
    /// `VK_INSERT`.
    Insert,
    /// `VK_DELETE`.
    Delete,
    /// `VK_HOME`.
    Home,
    /// `VK_END`.
    End,
    /// `VK_PRIOR`.
    PageUp,
    /// `VK_NEXT`.
    PageDown,
    /// `VK_UP`.
    Up,
    /// `VK_DOWN`.
    Down,
    /// `VK_LEFT`.
    Left,
    /// `VK_RIGHT`.
    Right,
    /// `VK_F1`..`VK_F12`, as the number.
    F(u8),
    /// `VK_SCROLL` (plugin.cpp:5476).
    ScrollLock,
    /// The Shift key itself: with Alt it is the stretch toggle
    /// (Milkdrop2PcmVisualizer.cpp:267). Pressing it sets no Shift flag.
    Shift,
}

/// A key with the modifiers held while it went down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    /// The key.
    pub key: Key,
    /// Shift held.
    pub shift: bool,
    /// Ctrl held.
    pub ctrl: bool,
    /// Alt held.
    pub alt: bool,
}

impl KeyChord {
    /// `key` with no modifiers.
    pub const fn plain(key: Key) -> KeyChord {
        KeyChord {
            key,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }

    /// The character `c` as typed.
    pub const fn ch(c: char) -> KeyChord {
        KeyChord::plain(Key::Char(c))
    }

    /// `key` with Ctrl held.
    pub const fn ctrl(key: Key) -> KeyChord {
        KeyChord {
            key,
            shift: false,
            ctrl: true,
            alt: false,
        }
    }

    /// `key` with Alt held.
    pub const fn alt(key: Key) -> KeyChord {
        KeyChord {
            key,
            shift: false,
            ctrl: false,
            alt: true,
        }
    }
}

/// True for the characters the C++ takes into an edited string
/// (plugin.cpp:5175: `' '..='z'`, `{`, `}`; not `|` or `~`).
pub fn accepts_text_char(c: char) -> bool {
    (' '..='z').contains(&c) || c == '{' || c == '}'
}

const fn c(ch: char) -> KeyChord {
    KeyChord::ch(ch)
}

const fn k(key: Key) -> KeyChord {
    KeyChord::plain(key)
}

/// Regular-mode bindings, one row per chord. Rows are grouped by the C++
/// handler they come from; each command's doc comment cites the line.
///
/// Navigation keys appear once with [`Mods::NONE`]; [`lookup`] adds the
/// modifiers held. A chord is bound once, so a key that means different
/// things in different modes (Space, Backspace, Enter, the arrows, letters in
/// the load list) has the regular-mode meaning here and 7.7 re-reads it.
pub const BINDINGS: &[(KeyChord, Command)] = &[
    // ---- the shell: pluginshell.cpp:2286-2296, Milkdrop2PcmVisualizer.cpp:263-275
    (k(Key::F(1)), Command::ToggleHelp),
    (k(Key::Escape), Command::Escape),
    (KeyChord::alt(Key::Enter), Command::ToggleFullscreen),
    (KeyChord::alt(Key::Shift), Command::ToggleStretch),
    // ---- WM_KEYDOWN, plugin.cpp:5411-5476
    (k(Key::F(2)), Command::CycleMaxFps),
    (k(Key::F(3)), Command::CycleTimeBetweenPresets),
    (k(Key::F(4)), Command::TogglePresetName),
    (k(Key::F(5)), Command::ToggleFps),
    (k(Key::F(6)), Command::ToggleRating),
    (k(Key::F(7)), Command::ToggleAlwaysOnTop),
    (k(Key::F(9)), Command::ToggleShaderHelp),
    (k(Key::F(10)), Command::TogglePause),
    (k(Key::ScrollLock), Command::ToggleLock),
    (k(Key::Space), Command::NextPresetSoft),
    (k(Key::Backspace), Command::PrevPreset),
    (KeyChord::ctrl(Key::Char('t')), Command::KillTexts),
    (KeyChord::ctrl(Key::Char('y')), Command::KillTexts),
    (KeyChord::ctrl(Key::Char('k')), Command::KillSprites),
    (KeyChord::ctrl(Key::Char('c')), Command::Copy),
    (KeyChord::ctrl(Key::Insert), Command::Copy),
    (KeyChord::ctrl(Key::Char('x')), Command::Cut),
    (KeyChord::ctrl(Key::Char('v')), Command::Paste),
    // ---- navigation, plugin.cpp:5888-6128 and menu.cpp:423-750
    (k(Key::Up), Command::Up(Mods::NONE)),
    (k(Key::Down), Command::Down(Mods::NONE)),
    (k(Key::Left), Command::Left(Mods::NONE)),
    (k(Key::Right), Command::Right(Mods::NONE)),
    (k(Key::Home), Command::Home(Mods::NONE)),
    (k(Key::End), Command::End(Mods::NONE)),
    (k(Key::PageUp), Command::PageUp(Mods::NONE)),
    (k(Key::PageDown), Command::PageDown(Mods::NONE)),
    (k(Key::Enter), Command::Enter(Mods::NONE)),
    (k(Key::Delete), Command::Delete(Mods::NONE)),
    (k(Key::Insert), Command::Insert),
    // ---- WM_CHAR: HandleRegularKey, plugin.cpp:6267-6604
    (c('0'), Command::Digit(0)),
    (c('1'), Command::Digit(1)),
    (c('2'), Command::Digit(2)),
    (c('3'), Command::Digit(3)),
    (c('4'), Command::Digit(4)),
    (c('5'), Command::Digit(5)),
    (c('6'), Command::Digit(6)),
    (c('7'), Command::Digit(7)),
    (c('8'), Command::Digit(8)),
    (c('9'), Command::Digit(9)),
    (c('q'), Command::EchoZoomDown),
    (c('Q'), Command::EchoZoomUp),
    (c('w'), Command::WaveModeNext),
    (c('W'), Command::WaveModePrev),
    (c('e'), Command::WaveAlphaDown),
    (c('E'), Command::WaveAlphaUp),
    (c('I'), Command::ZoomDown),
    (c('i'), Command::ZoomUp),
    (c('n'), Command::ToggleDebugInfo),
    (c('N'), Command::ToggleDebugInfo),
    (c('r'), Command::ToggleRandomOrder),
    (c('t'), Command::LaunchSongTitle),
    (c('T'), Command::LaunchSongTitle),
    (c('o'), Command::WarpAmountDown),
    (c('O'), Command::WarpAmountUp),
    (c('!'), Command::RandomizeWarpShader),
    (c('@'), Command::RandomizeCompShader),
    (c('a'), Command::RandomMashup),
    (c('A'), Command::RandomMashup),
    (c('d'), Command::ToggleShaderLocks),
    (c('D'), Command::ToggleShaderLocks),
    (c('h'), Command::NextPresetHard),
    (c('H'), Command::NextPresetHard),
    (c('f'), Command::EchoOrientNext),
    (c('F'), Command::EchoOrientNext),
    (c('g'), Command::GammaDown),
    (c('G'), Command::GammaUp),
    (c('j'), Command::WaveScaleDown),
    (c('J'), Command::WaveScaleUp),
    (c('k'), Command::BeginSpriteInput),
    (c('K'), Command::BeginSpriteKillInput),
    (c('['), Command::XPushDown),
    (c(']'), Command::XPushUp),
    (c('{'), Command::YPushDown),
    (c('}'), Command::YPushUp),
    (c('<'), Command::RotPlus),
    (c('>'), Command::RotMinus),
    (c('s'), Command::SaveAs),
    (c('S'), Command::SaveAs),
    (c('`'), Command::ToggleLock),
    (c('~'), Command::ToggleLock),
    (c('l'), Command::OpenLoadMenu),
    (c('L'), Command::OpenLoadMenu),
    (c('m'), Command::ToggleMenu),
    (c('M'), Command::ToggleMenu),
    (c('-'), Command::RateDown),
    (c('+'), Command::RateUp),
    (c('*'), Command::ClearNumericInput),
    (c('y'), Command::BeginCustomMessageInput),
    (c('Y'), Command::BeginCustomMessageInput),
];

/// The chord as the table spells it: a character is already cased, so Shift
/// adds nothing; with Ctrl the letter is lowercase (the C++ sees the virtual
/// key there). The Shift key itself carries no Shift flag.
fn normalize(chord: KeyChord) -> KeyChord {
    match chord.key {
        Key::Char(ch) if chord.ctrl => KeyChord {
            key: Key::Char(ch.to_ascii_lowercase()),
            shift: false,
            ..chord
        },
        Key::Char(_) | Key::Shift => KeyChord {
            shift: false,
            ..chord
        },
        _ => chord,
    }
}

fn exact(chord: KeyChord) -> Option<Command> {
    BINDINGS
        .iter()
        .find(|(bound, _)| *bound == chord)
        .map(|(_, command)| *command)
}

/// The command for `chord`, or `None`. Like the C++ virtual-key switch, a
/// non-character key keeps its meaning under Shift and Ctrl (`VK_F2` is F2
/// whatever is held; the navigation commands take the modifiers along); Alt
/// makes it a different message the plugin never reads, so only the chords
/// listed with Alt match.
pub fn lookup(chord: KeyChord) -> Option<Command> {
    // The host toggles on any `WM_SYSKEYDOWN` of Enter or Shift, whatever
    // else is held (Milkdrop2PcmVisualizer.cpp:264-271).
    if chord.alt {
        match chord.key {
            Key::Enter => return Some(Command::ToggleFullscreen),
            Key::Shift => return Some(Command::ToggleStretch),
            _ => {}
        }
    }
    if chord.ctrl && !chord.alt && chord.key == Key::Backspace {
        return Some(Command::CtrlBackspace);
    }
    let chord = normalize(chord);
    if let Some(command) = exact(chord) {
        return Some(command);
    }
    if chord.alt || matches!(chord.key, Key::Char(_)) {
        return None;
    }
    let base = exact(KeyChord::plain(chord.key))?;
    Some(base.with_mods(Mods {
        shift: chord.shift,
        ctrl: chord.ctrl,
    }))
}

/// The regular-mode command for a typed character (`HandleRegularKey`,
/// plugin.cpp:6245-6608), or `None` when the C++ has no `case` for it (`R`,
/// `u`, most punctuation). Other modes read the character themselves.
pub fn lookup_char(ch: char) -> Option<Command> {
    lookup(KeyChord::ch(ch))
}

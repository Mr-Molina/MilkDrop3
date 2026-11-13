//! The platform-neutral command layer: what a key press asks the visualizer
//! to do. The app maps winit keys onto [`crate::KeyChord`] (or onto
//! [`Command::TypeChar`] while text is being typed) and forwards the result
//! to `Visualizer::command`.
//!
//! One variant per `case` of the C++ key handlers: `HandleRegularKey`
//! (WM_CHAR, plugin.cpp:6245-6608), `MyWindowProc` WM_KEYDOWN
//! (plugin.cpp:5400-6229), `CMilkMenu::HandleKeydown` (menu.cpp:423-750) and
//! the shell's `PluginShellWindowProc` (pluginshell.cpp:2073-2307). Doc
//! comments quote `assets/help.txt` (the F1 screen, `text1.bin`) where it
//! names the key and the C++ comment or code otherwise. Not ported (A.4, F47)
//! and listed in the task report: Winamp IPC and playlist keys, media keys,
//! desktop mode, `Alt+K`, `Alt+F4`.
//!
//! A command names what the key does in REGULAR mode, or its generic role
//! for navigation keys. Modes (menu, load list, text editing, ...) read the
//! same command differently; that dispatch is Task 7.7's, and each variant
//! says which C++ case it stands for in the other modes.

/// Shift and Ctrl as the C++ reads them with `GetKeyState` next to a virtual
/// key. Navigation commands carry them because `Visualizer::command` takes no
/// chord: Shift extends the selection while editing text (plugin.cpp:5505)
/// and scales a menu step by 0.1 (menu.cpp:646-649), Ctrl jumps by word or line
/// (plugin.cpp:5533-5545).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    /// Shift held.
    pub shift: bool,
    /// Ctrl held.
    pub ctrl: bool,
}

impl Mods {
    /// Neither held.
    pub const NONE: Mods = Mods {
        shift: false,
        ctrl: false,
    };
    /// Shift only.
    pub const SHIFT: Mods = Mods {
        shift: true,
        ctrl: false,
    };
    /// Ctrl only.
    pub const CTRL: Mods = Mods {
        shift: false,
        ctrl: true,
    };
    /// Ctrl and Shift.
    pub const CTRL_SHIFT: Mods = Mods {
        shift: true,
        ctrl: true,
    };
}

/// One thing a key can ask for. Window commands stay here (F46): the app
/// handles them before forwarding, `md-core` owns no window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    // ---- window (handled by the app, F46) ----
    /// Help text: "ALT+ENTER       toggle fullscreen (or double-click on mouse!)"
    /// (Milkdrop2PcmVisualizer.cpp:271; the double click is `WM_LBUTTONDBLCLK`,
    /// :214, and is the app's mouse handling).
    ToggleFullscreen,
    /// Help text: "ALT+SHIFT       toggle multiple monitor stretching"
    /// (Milkdrop2PcmVisualizer.cpp:267, `WM_SYSKEYDOWN` with `VK_SHIFT`).
    ToggleStretch,
    /// Help text: "F7    Always on top" (plugin.cpp:5438).
    ToggleAlwaysOnTop,

    // ---- help and overlays ----
    /// Help text: "F1    help" (pluginshell.cpp:2286, `ToggleHelp`).
    ToggleHelp,
    /// Help text: "F4    Show preset name" (plugin.cpp:5433,
    /// `m_bShowPresetInfo`).
    TogglePresetName,
    /// Help text: "F5    Show FPS" (plugin.cpp:5434, `m_bShowFPS`).
    ToggleFps,
    /// Help text: "F6    Show preset rating" (plugin.cpp:5435,
    /// `m_bShowRating`).
    ToggleRating,
    /// `case 'n': case 'N': m_bShowDebugInfo = !m_bShowDebugInfo;`
    /// (plugin.cpp:6328).
    ToggleDebugInfo,
    /// `case VK_F9: m_bShowShaderHelp = !m_bShowShaderHelp;` (plugin.cpp:5472;
    /// not in the help text).
    ToggleShaderHelp,
    /// Help text: "F10   Pause". The help screen names the key, but the C++
    /// has no handler for it (`VK_F10` arrives as `WM_SYSKEYDOWN`, which the
    /// shell does not read, pluginshell.cpp:2073-2307); bound per the help
    /// text, behaviour is 7.9's.
    TogglePause,
    /// Help text: "F2    change max FPS" (plugin.cpp:5411: cycles 30, 60, 90,
    /// 120, 144, 240, 360 and unlimited, announcing it as a song title).
    CycleMaxFps,
    /// Help text: "F3    change time between preset" (plugin.cpp:5423:
    /// cycles 15, 30, 60, 120, 180 sec and unlimited, which also locks).
    CycleTimeBetweenPresets,

    // ---- presets ----
    /// Help text: "SPACE / H       soft / hard cut to next preset" (soft cut:
    /// `case VK_SPACE`, blend `m_fBlendTimeUser`, plugin.cpp:5983). In the
    /// load list Space is Enter (5985); in a menu it selects (menu.cpp:487);
    /// while typing text Space is a character, not this command.
    NextPresetSoft,
    /// Help text: "SPACE / H       soft / hard cut to next preset" (hard cut:
    /// `case 'h': case 'H': NextPreset(0)`, plugin.cpp:6449). In the mash-up
    /// screen `h` randomizes the slot and `H` every slot (6451-6472).
    NextPresetHard,
    /// Help text: "BACKSPACE       go back to previous preset"
    /// (plugin.cpp:6187, `PrevPreset(0)`). In a menu Backspace is "back"
    /// (menu.cpp:477); while editing text it deletes left (plugin.cpp:5609).
    PrevPreset,
    /// Help text: "` / ~           [un]lock current preset" (plugin.cpp:6549).
    /// Scroll Lock does the same in the C++ (plugin.cpp:5476, which sets the
    /// lock from the key's toggle state; here it toggles).
    ToggleLock,
    /// Help text: "R               toggle random/sequential preset order"
    /// (plugin.cpp:6333; also erases the history).
    ToggleRandomOrder,
    /// Help text: "L               load specific preset" (plugin.cpp:6554;
    /// pressed in the load list it closes it). In the load list the other
    /// letters seek (6258, `SeekToPreset`); the app sends those as
    /// [`Command::TypeChar`] while that list is up.
    OpenLoadMenu,
    /// Help text: "M               show/hide preset-editing Menu"
    /// (plugin.cpp:6575).
    ToggleMenu,
    /// Help text: "S               save new preset" (plugin.cpp:6528; only in
    /// regular mode).
    SaveAs,
    /// Help text: "A               do random mini-mash-up" (plugin.cpp:6396:
    /// a random preset, then a random warp shader, then a random comp
    /// shader).
    RandomMashup,
    /// Help text: "D               toggle warp & comp shader locks"
    /// (plugin.cpp:6413: cycles comp, warp, both, neither).
    ToggleShaderLocks,
    /// `case '!':` "randomize warp shader" (plugin.cpp:6369).
    RandomizeWarpShader,
    /// `case '@':` "randomize comp shader" (plugin.cpp:6382).
    RandomizeCompShader,
    /// `case '+': SetCurrentPresetRating(m_fRating + 1.0f)` (plugin.cpp:6588).
    RateUp,
    /// `case '-': SetCurrentPresetRating(m_fRating - 1.0f)` (plugin.cpp:6585).
    RateDown,

    // ---- live tweaks of the current preset (not in the help text) ----
    /// `case 'q': m_fVideoEchoZoom /= 1.05f` (plugin.cpp:6302).
    EchoZoomDown,
    /// `case 'Q': m_fVideoEchoZoom *= 1.05f` (plugin.cpp:6305).
    EchoZoomUp,
    /// `case 'w': m_nWaveMode++` (wraps, plugin.cpp:6308).
    WaveModeNext,
    /// `case 'W': m_nWaveMode--` (wraps, plugin.cpp:6312).
    WaveModePrev,
    /// `case 'e': m_fWaveAlpha -= 0.1f` (floor 0, plugin.cpp:6316).
    WaveAlphaDown,
    /// `case 'E': m_fWaveAlpha += 0.1f` (plugin.cpp:6320).
    WaveAlphaUp,
    /// `case 'I': m_fZoom -= 0.01f` (plugin.cpp:6325).
    ZoomDown,
    /// `case 'i': m_fZoom += 0.01f` (plugin.cpp:6326).
    ZoomUp,
    /// `case 'o': m_fWarpAmount /= 1.1f` (plugin.cpp:6366).
    WarpAmountDown,
    /// `case 'O': m_fWarpAmount *= 1.1f` (plugin.cpp:6367).
    WarpAmountUp,
    /// `case 'f': case 'F': m_nVideoEchoOrientation = (+1) % 4`
    /// (plugin.cpp:6474).
    EchoOrientNext,
    /// `case 'g': m_fGammaAdj -= 0.1f` (floor 0, plugin.cpp:6478).
    GammaDown,
    /// `case 'G': m_fGammaAdj += 0.1f` (plugin.cpp:6482).
    GammaUp,
    /// `case 'j': m_fWaveScale *= 0.9f` (plugin.cpp:6486).
    WaveScaleDown,
    /// `case 'J': m_fWaveScale /= 0.9f` (plugin.cpp:6489).
    WaveScaleUp,
    /// `case '[': m_fXPush -= 0.005f` (plugin.cpp:6509).
    XPushDown,
    /// `case ']': m_fXPush += 0.005f` (plugin.cpp:6512).
    XPushUp,
    /// `case '{': m_fYPush -= 0.005f` (plugin.cpp:6515).
    YPushDown,
    /// `case '}': m_fYPush += 0.005f` (plugin.cpp:6518).
    YPushUp,
    /// `case '<': m_fRot += 0.02f` (plugin.cpp:6521).
    RotPlus,
    /// `case '>': m_fRot -= 0.02f` (plugin.cpp:6524).
    RotMinus,

    // ---- song titles, custom messages and sprites ----
    /// `case 't': case 'T': LaunchSongTitleAnim()` (plugin.cpp:6362).
    LaunchSongTitle,
    /// `case '0'..'9'`: two digits make a number, then `LaunchCustomMessage`,
    /// `LaunchSprite` or the sprite kill, by the mode `y`, `k` or `K` set
    /// (plugin.cpp:6267-6299). In the mash-up screen `1`..`5` pick the slot
    /// instead (plugin.cpp:6261-6264). While text is typed the digit is
    /// [`Command::TypeChar`].
    Digit(u8),
    /// `case 'y'` / `'Y'`: numeric input launches custom messages
    /// (plugin.cpp:6599-6604, the C++ default, 1108). The C++ compares the
    /// key against `keyMappings[3..=4]`, which nothing ever fills, so the
    /// branch is dead there; bound to `y` as its comment intends.
    BeginCustomMessageInput,
    /// `case 'k'` (unshifted): numeric input launches sprites
    /// (plugin.cpp:6492-6503).
    BeginSpriteInput,
    /// `case 'K'` (shifted): numeric input kills sprites by number
    /// (plugin.cpp:6493-6503).
    BeginSpriteKillInput,
    /// `case '*': m_nNumericInputDigits = 0; m_nNumericInputNum = 0;`
    /// (plugin.cpp:6592): forget a half-typed number.
    ClearNumericInput,
    /// "CTRL+K kills all running sprites" (plugin.cpp:6204, 137).
    KillSprites,
    /// "CTRL+T kills current song title anim" and "CTRL+Y kills current
    /// custom message" (plugin.cpp:6196, 6221, 138-139); both set
    /// `m_supertext.fStartTime = -1`, so one command.
    KillTexts,

    // ---- generic navigation (modes decide the meaning, 7.7) ----
    /// `VK_UP`: menu up (menu.cpp:437), load list up (plugin.cpp:5943),
    /// mash-up preset up (5947), text cursor up (5599). Menu value up while
    /// editing one, Shift = x0.1 (menu.cpp:646-649).
    Up(Mods),
    /// `VK_DOWN`: the mirror of [`Command::Up`] (menu.cpp:451,
    /// plugin.cpp:5963, 5604).
    Down(Mods),
    /// `VK_LEFT`: menu back (menu.cpp:478), leave the load list
    /// (plugin.cpp:5888), previous mash-up slot (5904), text cursor left
    /// (5569); Ctrl = by word (5533).
    Left(Mods),
    /// `VK_RIGHT`: menu select (menu.cpp:486), next mash-up slot (5906), text
    /// cursor right (5575); Ctrl = by word (5534).
    Right(Mods),
    /// `VK_HOME`: first menu item (menu.cpp:465), first preset (plugin.cpp:6011),
    /// start of line (5591); Ctrl = start of text (5535).
    Home(Mods),
    /// `VK_END`: the mirror of [`Command::Home`] (menu.cpp:469,
    /// plugin.cpp:6024, 5595, 5536).
    End(Mods),
    /// `VK_PRIOR`: page up the load list or mash-up (plugin.cpp:5993); while
    /// editing a menu value, x10 (menu.cpp:637).
    PageUp(Mods),
    /// `VK_NEXT`: page down (plugin.cpp:6002, menu.cpp:637).
    PageDown(Mods),
    /// `VK_RETURN`: load the highlighted preset (plugin.cpp:6128), apply a
    /// mash-up (6132), accept edited text (5674); Ctrl+Enter accepts code
    /// (5546); a menu selects (menu.cpp:485-486). Does nothing in regular mode.
    Enter(Mods),
    /// `VK_ESCAPE`: help text "ESC             exit help / menus"
    /// (pluginshell.cpp:2291); also leaves the load list and mash-up
    /// (plugin.cpp:5911), cancels typing (5834) and a menu value edit
    /// (menu.cpp:599).
    Escape,
    /// `VK_DELETE`: delete a preset file in the load list (plugin.cpp:6038);
    /// delete right while typing (5651); in regular mode stop the custom
    /// message, or with sprite input kill the newest sprite, Shift the oldest,
    /// Ctrl+Shift all (6047-6095).
    Delete(Mods),
    /// `VK_INSERT`: rename a preset in the load list (plugin.cpp:6100) or
    /// toggle overtype while typing (5565).
    Insert,
    /// `Ctrl+C` or `Ctrl+Insert` while typing: `WaitString_Copy`
    /// (plugin.cpp:5520-5524).
    Copy,
    /// `Ctrl+X` while typing: `WaitString_Cut` (plugin.cpp:5525).
    Cut,
    /// `Ctrl+V` while typing: `WaitString_Paste` (plugin.cpp:5529).
    Paste,

    // ---- text input ----
    /// A character typed while a string is being edited (`WM_CHAR`,
    /// plugin.cpp:5171-5285: only `' '..='z'`, `{` and `}` are taken, see
    /// [`crate::accepts_text_char`]), or a letter pressed in the load list
    /// (6258). `Visualizer::text_input(ch)` is `command(TypeChar(ch))` (F46).
    TypeChar(char),
}

impl Command {
    /// The same command with `mods` in place of its modifiers, when it takes
    /// any; everything else comes back unchanged.
    pub fn with_mods(self, mods: Mods) -> Command {
        match self {
            Command::Up(_) => Command::Up(mods),
            Command::Down(_) => Command::Down(mods),
            Command::Left(_) => Command::Left(mods),
            Command::Right(_) => Command::Right(mods),
            Command::Home(_) => Command::Home(mods),
            Command::End(_) => Command::End(mods),
            Command::PageUp(_) => Command::PageUp(mods),
            Command::PageDown(_) => Command::PageDown(mods),
            Command::Enter(_) => Command::Enter(mods),
            Command::Delete(_) => Command::Delete(mods),
            other => other,
        }
    }
}

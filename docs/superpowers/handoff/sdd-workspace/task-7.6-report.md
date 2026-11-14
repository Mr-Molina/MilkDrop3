# Task 7.6 report: commands and key bindings

Status: DONE_WITH_CONCERNS. Commit caf78bc `feat(core): command enum and key bindings` (base 7eb3769, not pushed).

## What was implemented
- `crates/md-core/src/commands.rs` (300 lines): `Command` (every variant has a doc comment quoting the help.txt line or the C++ code/comment, with a line cite) and `Mods { shift, ctrl }`.
- `crates/md-core/src/keymap.rs` (281 lines): `Key`, `KeyChord { key, shift, ctrl, alt }`, `BINDINGS: &[(KeyChord, Command)]`, `lookup(chord)`, `accepts_text_char(ch)`. Re-exported from `lib.rs`.
- `crates/md-core/src/keymap_tests.rs`: 13 tests.
- `assets/help.txt` (F45): copy of `code/vis_milk2/text1.bin`. The source is 7-bit ASCII (no BOM, no non-ASCII bytes, no NUL), CRLF, 28 lines, 822 bytes, ending in `F10   Pause\r\n`. The copy is the same bytes with CR removed: 794 bytes, ASCII kept, trailing newline kept. The help screen is loaded as an ANSI resource (IDR_TEXT1, plugin.cpp:933); IDR_TEXT2 (unicode) has no file in the tree. Its first line says "BEATDROP2077:", kept verbatim.
- `.gitattributes`: `assets/help.txt text eol=lf` so autocrlf checkouts cannot reintroduce CR (small addition, not in the brief).

## Key-vs-char split (matches the C++, F46)
- `Key::Char(c)` = WM_CHAR (HandleRegularKey, plugin.cpp:6245): letters, digits, punctuation, cased as typed (`Q` != `q`; Shift adds nothing). Every other `Key` = WM_KEYDOWN virtual key (F-keys, arrows, Space, Enter, Backspace, Esc, Insert/Delete/Home/End/PgUp/PgDn, ScrollLock). With Ctrl the char is lowercase (C++ sees the VK, plugin.cpp:6196).
- While a string is edited the C++ absorbs every WM_KEYDOWN (plugin.cpp:5869) and types only WM_CHARs in `' '..='z'`, `{`, `}` (plugin.cpp:5175; not `|` `~`). `accepts_text_char` encodes that. In a typing mode the app sends `TypeChar(ch)` for characters (Space included, though outside typing Space is a key) and `lookup(chord)` for non-character keys. `text_input(ch)` = `command(TypeChar(ch))`.
- Other modes that take raw characters: load list (letters A-Za-z -> SeekToPreset, plugin.cpp:6256-6258, delivered as `TypeChar`), confirm prompts (`y`), mash-up (digits 1..5 pick a slot, via `Digit(n)`).

## How mode-specific keys are represented
- `BINDINGS` holds ONE command per chord: the regular-mode meaning, or the generic navigation role. 7.7 reinterprets per mode; each variant's doc lists the other-mode C++ cases (menu.cpp / plugin.cpp cites). Examples: Space = `NextPresetSoft` (= Enter in load list, select in menu); Backspace = `PrevPreset` (= back in menu, delete-left in text); `h` = `NextPresetHard` (randomize slot in mash-up).
- Because `Visualizer::command` takes no chord, modifiers the C++ reads next to a virtual key travel in the command: `Up/Down/Left/Right/Home/End/PageUp/PageDown/Enter/Delete(Mods)`. Shift = select / x0.1 menu step / oldest sprite; Ctrl = word/line jump / Ctrl+Enter accept code; Ctrl+Shift+Delete = all sprites. `lookup` resolves like the C++ VK switch: Shift/Ctrl do not change which non-char key it is (F2 with Ctrl is still F2; the nav commands take the flags); Alt turns it into a different message the plugin never reads, so only listed Alt chords match (Alt+Enter, Alt+Shift). Alt+Shift is `KeyChord{key: Key::Shift, alt: true}` (the Shift key itself sets no Shift flag).

## Changes to the brief's Command list (authority: C++ handlers)
Kept (same name): ToggleHelp, ToggleFullscreen, ToggleStretch, ToggleAlwaysOnTop, NextPresetSoft, NextPresetHard, PrevPreset, ToggleLock, ToggleRandomOrder, OpenLoadMenu, ToggleMenu, SaveAs, RandomMashup, ToggleShaderLocks, CycleMaxFps, CycleTimeBetweenPresets, TogglePresetName, ToggleFps, ToggleRating, TogglePause, ToggleDebugInfo, RateUp, RateDown, LaunchSongTitle, KillSprites, KillTexts, TypeChar.
Renamed: `MenuEscape` -> `Escape` (Esc also closes help, cancels typing: pluginshell.cpp:2291, plugin.cpp:5834); `ShowShaderHelp` -> `ToggleShaderHelp` (F9 toggles, plugin.cpp:5472); `MenuUp/Down/Left/Right/Enter` -> `Up/Down/Left/Right/Enter(Mods)`, `PageUp/PageDown/Home/End/Delete` -> take `Mods`; `Backspace` merged into `PrevPreset` (same chord, plugin.cpp:6187; 7.7 maps it to delete-left/back by mode).
Removed: `ToggleSongTitle` (no key in C++; `m_bShowSongTitle` is a config option), `LaunchCustomMessage(u8)`, `LaunchSprite(u8)`, `MashupSlot(u8)` (the C++ has no such keys: two digits accumulate into a number whose use depends on the numeric-input mode, plugin.cpp:6267-6299, mash-up slot is the same digit key at 6261-6264), `IncreaseValue/DecreaseValue` (menu value edit is Up/Down/PageUp/PageDown, menu.cpp:628-700; covered by the nav commands with Mods). `Backspace`, `MenuEscape` etc. as above.
Added: `Digit(u8)`, `BeginCustomMessageInput` (y/Y), `BeginSpriteInput` (k), `BeginSpriteKillInput` (K), `ClearNumericInput` (*), `Insert`, `Copy`, `Cut`, `Paste` (Ctrl+C/Ctrl+Insert, Ctrl+X, Ctrl+V while typing, plugin.cpp:5520-5530), `RandomizeWarpShader` (!), `RandomizeCompShader` (@), the live tweaks `EchoZoomDown/Up` (q/Q), `WaveModeNext/Prev` (w/W), `WaveAlphaDown/Up` (e/E), `ZoomDown/Up` (I/i), `WarpAmountDown/Up` (o/O), `EchoOrientNext` (f/F), `GammaDown/Up` (g/G), `WaveScaleDown/Up` (j/J), `XPushDown/Up` ([ ]), `YPushDown/Up` ({ }), `RotPlus/RotMinus` (< >): each is a `case` in HandleRegularKey (plugin.cpp:6302-6524).
Mapped to the same command: Ctrl+T and Ctrl+Y both -> `KillTexts` (both set `m_supertext.fStartTime = -1`, plugin.cpp:6196, 6221); Ctrl+K -> `KillSprites`; Scroll Lock -> `ToggleLock` (C++ copies the key's toggle state, plugin.cpp:5476; here a toggle).

## Dropped keys (A.4 / F47), not bound
- Winamp/media: `x c v b s r` playback stubs in the shell's WM_CHAR (pluginshell.cpp:2191-2216; `s`/`r` are taken earlier by the plugin), `u/U` ("shuffle is now on", plugin.cpp:6351), playlist keys Up/Down/Home/End/PgUp/PgDn/Enter while `m_show_playlist` (pluginshell.cpp:2233-2279), Left/Right = rewind/ff 5 s (pluginshell.cpp:2296-2301), the commented-out `playback->` F1-F8 block (Milkdrop2PcmVisualizer.cpp:231-258). Ctrl+arrows are text-cursor word jumps only, no media meaning.
- Desktop mode / musikcube / Explorer menus: no key handlers found in the key paths beyond the desktop wndproc forwarding to the same procs (pluginshell.cpp:2327-2340); nothing bound.
- `Alt+K` (pluginshell.cpp:2169-2173, "exit vis before running config panel", Winamp config) and `Alt+F4` (Milkdrop2PcmVisualizer.cpp:264, quit, OS-level in winit). Double-click fullscreen (WM_LBUTTONDBLCLK, :214) is a mouse event for the app.
- `VK_F8` change-dir (commented out, plugin.cpp:5449) not bound.

## Test list
Brief's four, adapted to real C++: `every_help_text_key_has_a_binding` (hand list of the REAL 22 chords: ESC, ALT+ENTER, ALT+SHIFT, SPACE, H, BACKSPACE, `, ~, R, L, M, S, A, D, F1-F7, F10; asserts both `BINDINGS` membership and `lookup`), `digits_feed_numeric_input` (replaces `digits_launch_custom_messages`), `y_k_and_shift_k_pick_the_numeric_input_mode` (replaces `shift_digits_launch_sprites`: no such C++ chord), `bindings_are_unique`. Extra: `help_text_names_every_key_in_the_list`, `kill_chords`, `every_regular_char_case_of_the_cpp_is_bound`, `navigation_keys_carry_their_modifiers`, `copy_cut_paste_and_insert`, `modifiers_do_not_change_a_virtual_key_but_alt_does`, `text_chars_are_the_cpp_range`, `no_media_or_desktop_commands_are_bound`, `help_txt_has_lf_endings_and_matches_the_original` (no CR, ASCII, FNV-1a + length pin so it survives 9.1 deleting `text1.bin`, byte compare to the original modulo CR while it exists).

## TDD evidence
RED (stub `BINDINGS = &[]`, `lookup` -> None), `cargo test -p md-core keymap`: `test result: FAILED. 4 passed; 9 failed` (failures: copy_cut_paste_and_insert, every_help_text_key_has_a_binding, digits_feed_numeric_input, every_regular_char_case_of_the_cpp_is_bound, modifiers_do_not_change_a_virtual_key_but_alt_does, kill_chords, no_media_or_desktop_commands_are_bound, y_k_and_shift_k_pick_the_numeric_input_mode, navigation_keys_carry_their_modifiers). Full output: `task-7.6-red.txt` in this directory (first block RED, second GREEN).
GREEN: `test result: ok. 13 passed; 0 failed`. Gate: `cargo fmt --check` clean, `cargo build --workspace` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace` all ok (md-core lib 90 passed).

## Citations
Every plugin.cpp / menu.cpp / pluginshell.cpp / Milkdrop2PcmVisualizer.cpp line cited in commands.rs, keymap.rs and keymap_tests.rs was checked with `sed -n 'Np'` against the C++ before the commit. Eleven menu.cpp cites and a handful of plugin.cpp cites were wrong on the first pass and corrected (e.g. menu.cpp nav cases are 437-487, not 427-476; function ends are plugin.cpp:6608, menu.cpp:750, pluginshell.cpp:2307; Ctrl+K note is line 137).

## Concerns
1. `y`/`Y` (custom-message input) and Ctrl+Y are DEAD in the C++: they compare against `keyMappings[2..4]`, a `char[8]` declared at Milkdrop2PcmVisualizer.cpp:40 and never written, so the compare is against 0 (plugin.cpp:6221, 6599). I bound them as the comments intend (plugin.cpp:139); without them custom-message numeric input is unreachable after pressing `k` (the C++ default mode is custom message, :1108). If strict parity is wanted, delete those three rows.
2. F10 "Pause" is in the help text but the C++ has no handler (F10 is a WM_SYSKEYDOWN the shell ignores). `TogglePause` is bound per the help text; 7.9/8.x must decide what it does.
3. Help says "R" but the C++ binds lowercase `r` only (plugin.cpp:6333); so does BINDINGS (Caps Lock/Shift+R do nothing, as in C++).
4. `KeyChord` for Alt+Shift uses `Key::Shift`; the app must emit it on a Shift press while Alt is held.
5. `help.txt` still says "BEATDROP2077:" in its heading (verbatim, per F45).

## Fix round 1 (review findings)
Changes (commit `fix(core): TypeChar input contract; keymap fixes`):
- Input contract per the Task 7.6 ruling: keymap.rs module doc rewritten (app sends EVERY character as `TypeChar`; only non-character keys and Ctrl/Alt chords go through `lookup`; Space is sent both ways, key first; classification table with letters, Shift+letter, digits, Shift+1, Ctrl+letters, Alt chords, F-keys, arrows). New `lookup_char(ch) -> Option<Command>` (regular-mode resolver for 7.7), re-exported. `TypeChar` doc states how md-core reads it by mode (regular via `lookup_char`; edit text per `accepts_text_char`; load-list seek 6256-6259; prompts 5287-5391).
- `OpenLoadMenu` doc: the "l closes the list" branch (6556-6559) and `m`'s UI_LOAD branch (6580) are dead because SeekToPreset catches every letter first (6256-6259); documented as do-not-port.
- `BeginCustomMessageInput` cites plugin.rc:1087 `IDS_KEY_MAPPINGS "yYYyYzZ"`.
- k/K: accepted deviation, documented (C++ uses GetKeyState(VK_SHIFT), plugin.cpp:6496-6501; Rust uses the character's case, differs only under Caps Lock since TypeChar carries no Shift state).
- Alt edge cases: `lookup` now maps Alt+Enter to ToggleFullscreen and Alt+Shift key to ToggleStretch whatever other modifiers are held (host acts on any WM_SYSKEYDOWN of VK_RETURN/VK_SHIFT, Milkdrop2PcmVisualizer.cpp:264-271).
- TogglePause doc states the ruling (freeze of preset clock, frame counter, blend/preset timers, auto-advance; last frame re-presented; audio read but not analysed; UI works).
- Tests: `help_text_names_every_key_in_the_list` now matches line-anchored key + description for all 20 help lines; `no_media_or_desktop_commands_are_bound` checks u/U/x/X/c/C/v/V/b/B through `lookup_char` and BINDINGS; new `lookup_char_resolves_regular_mode_characters` (bound resolves, unbound R/u/x/b/?/z/space/é give None) and `alt_enter_and_alt_shift_ignore_other_modifiers`.
Citations touched (6256-6259, 6556-6559, 6580, 6496-6501, plugin.rc:1087, Milkdrop2PcmVisualizer.cpp:264-271, 5287-5391) were checked with `sed -n`.
Evidence: `cargo test -p md-core keymap`: 15 passed; `commands`: 1 passed; fmt, build, clippy -D warnings, `cargo test --workspace` all clean.

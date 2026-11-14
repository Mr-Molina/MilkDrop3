### Task 7.6: Commands and key bindings

**Files:**
- Create: `crates/md-core/src/{commands.rs,keymap.rs}`

**Interfaces:**
- Produces: `pub enum Command { ToggleHelp, ToggleFullscreen, ToggleStretch, NextPresetSoft, NextPresetHard, PrevPreset, ToggleLock, ToggleRandomOrder, OpenLoadMenu, ToggleMenu, SaveAs, RandomMashup, ToggleShaderLocks, CycleMaxFps, CycleTimeBetweenPresets, TogglePresetName, ToggleFps, ToggleRating, ToggleAlwaysOnTop, TogglePause, ToggleSongTitle, ToggleDebugInfo, ShowShaderHelp, RateUp, RateDown, MenuUp, MenuDown, MenuLeft, MenuRight, MenuEscape, MenuEnter, PageUp, PageDown, Home, End, Delete, Backspace, LaunchCustomMessage(u8), LaunchSongTitle, KillSprites, KillTexts, LaunchSprite(u8), MashupSlot(u8), IncreaseValue, DecreaseValue, TypeChar(char) }` (one variant per key case found in `plugin.cpp` and `pluginshell.cpp` key handlers; the implementer maps each `case` to a variant and lists the mapping in `keymap.rs` as `pub const BINDINGS: &[(KeyChord, Command)]`), `pub struct KeyChord { key: Key, shift: bool, ctrl: bool, alt: bool }` where `Key` is a platform-neutral enum in `md-core` (winit mapping lives in the app).
- Every `Command` has a doc comment quoting the help text line from `assets/help.txt` or the C++ comment.

- [ ] **Step 1:** Write failing tests `every_help_text_key_has_a_binding` (a hand-written list in the test of the 24 key chords named in `assets/help.txt`, each asserted to appear in `BINDINGS`), `digits_launch_custom_messages`, `shift_digits_launch_sprites`, `bindings_are_unique`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): command enum and key bindings`


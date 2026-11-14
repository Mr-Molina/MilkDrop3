### Finding Verdicts
1. **Load-list highlight after file ops** — ADDRESSED. `end_delete` (save_screen.rs, diff) clamps `pos` and never reselects, as plugin.cpp:8199-8201. `end_rename(lib, Option<&str>)` leaves the prompt and reselects from `dir_count()` with row 0 fallback (load_screen.rs:45-51 matches 8238-8256); `None` skips the reselect (8261-8263). `dir_changed` sets row 0 (6164). Tests `end_delete_keeps_the_row_and_clamps`, `end_rename_highlights_the_renamed_file`, `dir_changed_goes_to_row_zero` assert the C++ rows. The `library_changed` doc names which call goes with which operation, and the "after every rescan" note is withdrawn. Name-taken correctly does not call `end_rename` (8209-8216).
2. **F1 clears "Press F1"** — ADDRESSED. mod.rs ToggleHelp arm sets `show_press_f1 = false` (pluginshell.cpp:2287). Tested.
3. **Ctrl+Backspace ignored in the editor** — ADDRESSED. In the editor `handle` sends `CtrlBackspace` to `key()`, which the editor absorbs. The test asserts empty actions and an unchanged text. The plain `PrevPreset` still deletes.
4. **Ctrl+Backspace via a new `Command`, judged against the 7.6 contract** — ADDRESSED, consistent. 7.6 sends Ctrl chords through `lookup`, and `lookup` now returns `CtrlBackspace` for Ctrl without Alt (keymap.rs). Ctrl+Alt+Backspace falls through to the old path. `PrevPreset` carries no mods, so a new command was needed. C++ check outside the editor: plugin.cpp:6187 `case VK_BACK: PrevPreset(0)` has no Ctrl test (the Ctrl tests at 6197-6223 belong to other keys). menu.cpp:477 treats VK_BACK as back with no Ctrl test. The Rust maps `CtrlBackspace` to `PrevPreset` when no editor is active, so the menu "back" path is the same. Verified.
5. **`set_mash_preset` doc** — ADDRESSED. It now says apply at frame+3, one frame later than a browsed change.
6. **`ChangeDir` doc** — ADDRESSED. It matches `change_dir_result`: the prompt stays on failure, and `dir_changed` follows from the list.
7. **`SaveScreen` removed** — ADDRESSED. Struct, field and the write are gone. The only uses were the removed write and `Ui::new`.
8. **`handle` takes `&UiCtx`** — ADDRESSED. `handle` and `key` now take `&UiCtx`, and the harness is updated.
9. **`host_toasts` and `Ui::raise`** — ADDRESSED. The values match plugin.cpp 8165 (6.0 Preset bold), 8170 (3.0 Notify), 8189 (6.0 Misc bold), 8213 (6.0 Preset bold), 8223 (6.0 Misc bold) and 8228 (3.0 Notify). 8196 (delete OK, 3.0 Notify) was confirmed in the earlier read. The test checks the table and the delete override message.
10. **Strengthened asserts and sample-row pin** — ADDRESSED. `~` asserts `Forward(ToggleLock)`. The page count is exactly 40. Five sample rows are pinned. I spot-checked zoom amount (LOGBLENDABLE, 6781) and number of sides (INT 3..100, 6843) against the C++. The other rows were covered by the earlier script diff.

### New Breakage in the Fix Diff
None Critical or Important.
- Minor: `CtrlBackspace` in the editor is dropped silently. Some terminals or winit deliver Ctrl+Backspace as TypeChar('\x7f'). The editor ignores it because it accepts only ' '..'z', as in the C++ WM_CHAR path (5171-5395), so that matches. It is an app-side note for 8.2, not a defect.
- Minor: `host_toasts::DELETE_OK` has an empty `msg`, so it must be raised with `Some(msg)`. This is documented, and `raise` with `None` would show an empty toast. Not blocking.
- Tests: the fix report names the covering tests and shows gate output (md-core 142 passed, workspace 834 passed, clippy and fmt clean). Not re-run.

### Out-of-Scope Observations
None.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage.

### Spec Compliance
✅ (a) Menu tree: a script diffed all 120 `AddItem` rows (258 once the slots unroll) of plugin.cpp:6702-6868 against menu_tree.rs/menu_custom.rs, resolving every label and `_TT` id through plugin.rc. Labels, kinds, min/max (the log 0.01..100 default, menu.cpp:228-233) and tooltip texts: 0 mismatches. Targets checked by hand (120/120, including inner border's reuse of the `*_OUTER` labels, wave/shape export `lParam` = slot, and the callbacks). There are 15 menus. Menu names match plugin.rc:444-452. Post-processing has 19 items because 6802 is commented out.
✅ (b) Increments are menu.cpp:637-698 exactly: `(max-min)*0.01*mult`, log `1.01^±mult`, ints in whole steps with mult≥1, PageUp/PageDown ×10, Shift ×0.1 on Up/Down only. Original value is `(int)(v*10000)` and Escape restores `*0.0001` (534-627). The value view is " %d " / " %5.3f " (menu.cpp:363-389).
✅ (c) WM_CHAR order (editor → LOAD_DEL → UPGRADE `!= 13` → SAVE_OVERWRITE `' '..'z'`/27 → HandleRegularKey) matches plugin.cpp:5171-5395. The editor accepts `' '..'z'{}`, the filter set and `len+1>=max`. Load list: letters seek from m_nDirs, digits fall through to hotkeys, mash-up `1`..`5` pick the bin. WM_KEYDOWN order (F-keys → editor absorbs → menu → case 3 → shell F1/Esc) is right; checked Load/Mashup/Upgrade/SaveOverwrite/LoadDel, Delete/Insert on dirs, Space/Enter blend choice, Backspace → PrevPreset, Ctrl branch of the editor. The paired key+char deliveries of the 7.6 contract (Space, Enter, Esc, BS) also come out as in the C++.
✅ (d) Overlays: upper-right name (decorative, lock if user lock or mode≠Regular, matching 3922/4161), rating, fps, debug (simple), each with a shadow. Waitstring tooltip lower right. Upper-left screens in the C++ else-if order. Help box geometry and `+h+3m` (pluginshell.cpp:1863-1890). Press F1 `dx=160(t/3)^10`. Toasts last, red fade `200-199·age⁴` when bold. `draw_box(DARK_TRANSLUCENT_BOX)` under menus, lists and tooltips (6.6 note). F43 slots are right.
✅ Strings: a script compared 61 single-id strings in strings.rs with plugin.rc: all equal. QREF 615-635 and the mash-up text/names were checked by hand.
✅ (f) md-render change is additive: `Item` made pub and re-exported as `QueuedMessage`, plus `TextQueue::messages()`. Only the draw tests use it.
✅ Citations: sed-checked about 95 of the 141 distinct citations (first and last line of each range): all point at the cited code. That includes 553 (IsAlphanumericChar with `.`), md_defines.h:51/53, state.h:52/60 and plugin.h:94-98.
❌ (e) The 7.9 callbacks do not restore the load-list highlight the way the C++ does after delete and rename (see Important 1).
⚠️ The API departures (draw returns actions, LoadPreset{index,path,soft}, SavePreset{path,name}, no `step`, name upper right) follow the C++ and are coherent. The ledger should record them as rulings.

### Strengths
- The table is complete to the character. Increment rules and navigation mirror menu.cpp, including the disabled-row loops (437-464, 116-120).
- The per-mode input matches the C++ under the 7.6 TypeChar contract, including C++ quirks (dead `l` close branch, Enter not answering the upgrade prompt, mash-up running-row compare, IDS_STRING_TOO_LONG on import failure).
- The tests mostly assert C++-derived values: step 0.51/0.511/0.611, int clamp, the 128-row cap with page 1/2/3 and clamp at 301, right edge = W-10, rows at 10+i·h, shadow +1,+1, bold toast colour, Ctrl+Enter vs Enter in code.

### Issues
#### Critical
(none)
#### Important
1. Delete and rename leave the load-list highlight on the wrong row. The C++ keeps the deleted row index (DeletePresetFile, plugin.cpp:8199-8201: `newPos` clamped after the rescan) and highlights the renamed file (8238-8256: rescan without reselect, then a search for the new name). In the Rust, `Ui::end_rename()` (save_screen.rs:99) takes neither the library nor the new name. `library_changed` (load_screen.rs:67) always selects the current preset's file, and the 7.9 note (report "Concerns"; ledger "Task 7.7: Note") says to call it after every rescan. Following that contract puts the cursor on the playing preset after a delete or rename, and `LoadScreen::clamp` can no longer recover the old row because `pos` was overwritten. The load-list ChangeDir case has the same problem: the C++ rescans with bTryReselect=false (6164), so pos=0, while `library_changed` may reselect. Fix: an `end_delete(lib, index)`, `end_rename(lib, new_name)` and a no-reselect variant for directory changes, each with a test, and correct the 7.9 note.
#### Minor
2. F1 does not clear `show_press_f1` (pluginshell.cpp:2287 `m_show_press_f1_msg = 0`), at mod.rs:508.
3. Ctrl+Backspace deletes a character in the editor (save_screen.rs:190). In the C++ the Ctrl branch has no VK_BACK (5516-5560), so the key is absorbed.
4. The `set_mash_preset` doc (mashup.rs:38-41) says the bin applies "one frame sooner". With `last = frame+1` and apply at `last+2`, it applies one frame later than a browsed change (frame+3 vs frame+2). The code mirrors the C++; only the doc is wrong.
5. The `UiAction::ChangeDir` doc (mod.rs:152-159) says that on an invalid UI_CHANGEDIR directory the host "goes on in the load list". `change_dir_result(false)` (save_screen.rs:85-94) stays in the prompt, as the C++ does (5807-5814). The doc contradicts the code.
6. `SaveScreen::name` is written (save_screen.rs:238) but never read: the overwrite prompt draws `edit.as_string()`. This is dead state.
7. `Ui::handle` takes `&mut UiCtx` and never mutates it (acknowledged in the report).
8. Durations and categories of host-raised toasts are not exported for 7.9: save, delete (6.0 MISC / 3.0 NOTIFY), rename (6.0 PRESET / 3.0 NOTIFY, 8213-8228). Only the strings are.
9. Weak asserts: `h.ch('~').len() == 1` (ui_tests.rs:225), and the short-window page count is checked only as `30 < lines < 128` instead of the exact value. `menu_tree_item_count_matches_cpp` checks only that tooltips are non-empty; pin a sample of (label, kind, min, max, tooltip) tuples taken from plugin.cpp/plugin.rc.

### Assessment
**Task quality:** Needs fixes. **Reasoning:** The menu table, the increment rules, the per-mode input and the overlays match the C++ (table and strings verified by script, about 95 citations checked). One Important gap remains: the re-selection after delete, rename and directory changes that the 7.9 callbacks must reproduce is missing or contradicted by the documented contract.

### Finding Verdicts
1. Input contract — ADDRESSED. keymap.rs:1-40 states the contract (every char -> TypeChar; non-char keys and Ctrl/Alt chords -> lookup) with the classification table; commands.rs TypeChar doc gives the per-mode reading; `lookup_char` (keymap.rs, end) is the regular-mode resolver, re-exported lib.rs:41. Checks: (a) Space is WM_KEYDOWN-only in the C++ (plugin.cpp:5983, no `' '` case in HandleRegularKey); Rust binds Key::Space (keymap.rs:175) but no Key::Char(' ') row, and `lookup_char(' ')` is None (keymap_tests lookup_char test), so key-first-then-TypeChar(' ') cannot fire NextPresetSoft twice in regular mode. (b) Every Char row in BINDINGS (0-9, letters, punctuation, y/Y) is `c(..)` = plain, so reachable via lookup_char (normalize strips Shift for Char). (c) No double fire: `lookup` returns no plain Char binding for non-Char keys (the `matches!(Key::Char)` early None) and Char bindings with Ctrl (t,y,k,c,x,v) are only reachable via the chord path, never via TypeChar; the sets are disjoint by construction.
2. Folded items — ADDRESSED. OpenLoadMenu doc marks 6556-6559 and m's 6580 branch dead, do-not-port (commands.rs); BeginCustomMessageInput cites plugin.rc:1087; TogglePause doc states the ruling (matches ledger line 470); Alt+Enter/Alt+Shift match for any other modifiers (keymap.rs lookup, tested by alt_enter_and_alt_shift_ignore_other_modifiers); tests strengthened (line-anchored help check over all 20 lines, u/U/x/X/c/C/v/V/b/B through lookup_char and BINDINGS, new lookup_char test). k/K Caps Lock deviation: documented; acceptable. C++ 6496-6501 reads GetKeyState(VK_SHIFT) while TypeChar carries only the char; Caps Lock makes the char case and Shift disagree, a rare edge, and the contract forbids Shift state on TypeChar (cost to change is an app/core contract change); the doc names it.

### New Breakage in the Fix Diff
None. Minor note: the `lookup_char` doc says "most punctuation" has no case; accurate. Report evidence names cargo test results (keymap 15, commands 1, workspace clean).

### Out-of-Scope Observations
None.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage.

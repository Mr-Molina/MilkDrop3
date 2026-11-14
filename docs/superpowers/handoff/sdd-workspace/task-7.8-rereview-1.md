### Finding Verdicts
1. Sprite code-error toasts (one when exactly one fails, none when both) - ADDRESSED. sprites.rs: `init_failed != code_failed` gate; texmgr.cpp:138/144 OR the two bits, plugin.cpp:8674-8686 matches only 0x10/0x20. `code_errors_warn_but_keep_the_sprite` covers init-only, per-frame-only, both (no toast), and checks the sprite still ticks.
2. Per-sprite Vm departure documented - ADDRESSED (sprites.rs module doc, cites texmgr:76-85).
3. SuperTextSlot::is_active documented unreliable - ADDRESSED (supertext.rs doc names md-render as authoritative, mfs:1127-1128, 945-946).
4. Fade stored once - ADDRESSED. `writes_fade: bool`; `start` takes `l.st.fade_time` when true, keeps old value when false; same semantics as the old Option (messages.rs sets true with st.fade_time = m.fade; song_title false). Residual (not new): a wholesale copy of `launch.st` by 7.9 would still clobber for song titles.
5. Image path concatenation - ADDRESSED. C++ is verbatim `swprintf("%s%s", m_szMilkdrop2Path, img)` (plugin.cpp:8589-8594); it does NOT drop a leading separator. Rust drops it, which matches Windows because the base path ends in `\` and doubled separators collapse; the doc comment's rationale is fair. `img[1]==':'` branch unchanged. Test `image_path_is_the_base_dir_and_the_ini_text` covers `\pics\a.png`.
6. sprites.rs split - ADDRESSED (515 lines; Refs moved to sprite_vars.rs, 114 lines, content identical, `pub(crate)`, private mod).
7. Test citations - ADDRESSED. `sel` is plugin.cpp:8471 (sed-verified; my earlier 8470 was wrong), `if (img[1] != L':')` is 8589.

### New Breakage in the Fix Diff
None. Reported test output (27 passed, full gate clean) names the covering command.

### Out-of-Scope Observations
None.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage.

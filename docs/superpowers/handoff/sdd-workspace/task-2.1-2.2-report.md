# Tasks 2.1 + 2.2 report (md-preset: INI reader, Blendable)

Commits: f1c3609 `feat(preset): INI reader`; fcbb1a3 `feat(preset): blendable floats`.
Files: crates/md-preset/src/{ini.rs,blend.rs,lib.rs}. No new dependencies.

## Task 2.1 - INI reader
Implemented `md_preset::ini::Ini` (pub, general purpose; re-exported with `strip_code` from lib.rs).
C++ (`_GetLineByName`, state.cpp:61-165) is the authority and differs from the brief, so `Ini` has two lookup models over one parse:
- Flat (preset) model, `flat_get/flat_f32/flat_i32/flat_bool` and `code_block(prefix)`: mirrors the C++ fast reader. Section-blind (the fixture has MILKDROP_PRESET_VERSION etc. BEFORE `[preset00]`), keys case-SENSITIVE, key ends at first `=` or space (so `b = y` gives value `= y`), value is the UNTRIMMED rest of the line to `\r`/`\n`, first occurrence wins, lone `\r` ends a line, lines without delimiter skipped, numeric reads use sscanf `%d`/`%f` prefix rules.
- Sectioned model (`get/get_f32/get_i32/get_bool`, per brief and for md-platform's milk2.ini, F49): case-insensitive section and key, trimmed, `;` comment lines, first wins, `""` section for keys before any header.
- `code_block(prefix)` mirrors ReadCode (state.cpp:1071): lines `<prefix>1..` until the first MISSING key (an empty value is present, not a gap), one leading backtick stripped, joined with '\n', stops when the 32768-byte buffer would overflow (C++ `len >= MAX-1-char_pos-1`).
- `strip_code(text)` mirrors StripLinefeedCharsAndComments (state.cpp:1525): drops '\n' with no replacement, removes `//` and `\\` comments to end of their own line (a comment does not swallow the next line).
- BOM stripped (intentional divergence, ledger); CRLF and trailing junk accepted.

Deviations from the brief (follow C++): `code_block` takes only `prefix` (flat, section-blind) instead of `(section, prefix)`; values are not trimmed in the flat model; flat keys are case-sensitive; `;` lines are only comments in the sectioned model (in C++ a `;foo=1` line becomes key `;foo`, which is never looked up, so preset behaviour is identical). Task 2.3 should use the `flat_*` API.

C++ quirks NOT mirrored (noted): the C++ lookup keeps a per-FILE* "current line" hint, so with duplicate keys read in file order the hint can pick a later duplicate; Rust always uses the first. Keys longer than ~125 chars are truncated in C++. Value reads are capped by buffer sizes in C++.

TDD: RED = lookups/eval stubbed to return None/0.0: `cargo test -p md-preset` -> `26 failed; 3 passed` (all behavioural tests failed). GREEN: `29 passed; 0 failed` (18 ini tests at the time of the ini commit).
Tests pinned: first_duplicate_key_wins, value_may_contain_equals, code_block_stops_at_gap, backtick_prefix_is_stripped, parses_crlf_bom_and_trailing_junk (uses tests/presets/edge/crlf_bom_junk.milk via include_str!), flat_keys_are_case_sensitive_but_sectioned_are_not, flat_lookup_ignores_sections, flat_value_is_not_trimmed_and_space_delimits_key, lines_without_delimiter_and_blank_lines_are_skipped, lone_cr_ends_a_line, semicolon_lines_are_comments_in_sectioned_model, numeric_getters_follow_sscanf, empty_code_line_is_present_not_a_gap, code_block_stops_when_buffer_would_overflow, strip_code_{joins_without_separator, comment_does_not_swallow_next_line, single_slash_and_backslash_are_kept, preserves_non_ascii}.
md-eval's test-only helper was not touched.

## Task 2.2 - Blendable
`md_preset::blend::Blendable` (Clone, Copy, Debug, PartialEq, Default): `new`, `set` (clears blend, as operator=), `value`, `is_blending`, `blend_to(from,to,start,end)`, `eval(&self,time)`.
Semantics per CBlendableFloat (state.cpp:1912-1975), F17: `eval` returns the TARGET for time<0, before the window, after it, or when no blend started; inside `[start, start+dur]` (inclusive both ends) returns `from*(1-mix)+val*mix`. `blend_to` sets target = `to`; if `end-start < 0.001` (StartBlendFrom early return) no blend starts and any existing blend state is left untouched.
Brief's `eval_before_start_returns_start` replaced by `eval_before_start_returns_target_not_start` (+ `negative_time_returns_target`). Other tests: eval_after_end_returns_end, window_edges_are_inclusive, eval_midway_is_linear, zero_duration_returns_end, duration_below_minimum_starts_no_blend_but_minimum_does, short_blend_leaves_existing_blend_running, set_cancels_blend, never_started_returns_value, blend_at_time_zero_start.
Divergences: C++ `eval` mutates `m_bBlending=false` when time leaves the window; Rust `eval` is `&self` and pure (matters only if time re-enters the window; anim time is monotonic). NaN time returns the target (C++ would compute a NaN mix). The C++ compound operators (*=, /=, -=, +=, which also clear blending) are not provided; add if 2.5 needs them.
TDD: same RED run as above (blend tests all failed with eval stubbed), GREEN 11 blend tests pass.

## Verification
cargo fmt --check, cargo build --workspace, cargo clippy --workspace --all-targets -- -D warnings: clean. cargo test --workspace: all pass (md-preset 29 passed). Initial clippy flagged `!(a > b)` on floats (neg_cmp_op_on_partial_ord); rewrote as `<=`/`>=`.
Trailers used are those in implementer-rules.md (Claude Opus 5.5), not the session system-reminder's Sonnet line. Not pushed.

# Tasks 2.4 + 2.5 report (md-preset: writer, round trip, corpus, blending)

Base 8e97d42. Commits (not pushed):
- 47d0dcb `feat(preset): writer with round-trip tests` (Task 2.4)
- eb328fa `feat(preset): blending and corpus parse test` (Task 2.5)

## Task 2.4: writer and round trip

### Files
- New: crates/md-preset/src/write.rs (`Preset::to_milk`), crates/md-preset/tests/write.rs, crates/md-preset/tests/fixtures/all_keys_exact.milk
- Modified: crates/md-preset/src/lib.rs (`mod write`)

### What the C++ does (verified against state.cpp@6f7cd5d) and what the writer does
- **Header** (state.cpp:884-890): written only when `m_nMaxPSVersion > 0`, where `m_nMaxPSVersion = max(warp, comp)` (state.cpp:1448). It is placed BEFORE `[preset00]` and has 4 lines: `MILKDROP_PRESET_VERSION=201` (`CUR_MILKDROP_PRESET_VERSION`, state.h:236), `PSVERSION=<max>`, `PSVERSION_WARP`, `PSVERSION_COMP`. On re-read, 201 selects the split keys, so the versions round-trip. A version-0 preset gets no header, so it reads back as 100, which gives (0,0).
- **Key order and formats**: exactly as in Export, CWave::Export and CShape::Export. A `// %.3f`, `// %.5f` or `// %d` comment sits on every key.
  - `%.5f` keys: `fZoomExponent`, `zoom`, `rot`, `dx`, `dy`, `warp`, `sx`, `sy`, wave `scaling` and `smoothing`, shape `rad`, `ang`, `tex_ang` and `tex_zoom`.
  - `%.3f` keys: every other float, including `cx`/`cy`, `fRating` and `fWarpAnimSpeed`.
  - `%d` keys: ints and bools.
  - This follows F18. The brief's `%.6f` list is wrong.
- **Float printing**: `c_fixed` formats `f64::from(v)` with Rust's `{:.N}`. I checked this against glibc printf with a scratch C program: ties-to-even on the exact binary value (0.125→"0.12", 2.5→"2"), and "-0.000" for negative values that round to zero, give identical output. Only NaN/inf are spelled differently in Rust, so `c_fixed` spells them glibc-style.
- **Code blocks** (`WriteCode`, state.cpp:851-876; call sites :977-983):
  - The writer splits the stored text on `'\n'` and writes `<prefix><n>=<line>`.
  - The C++ prepends a backtick to **every** warp/comp line (`bPrependApostrophe=true`), not only lines starting with `[` or `;`. It writes EEL lines (per_frame_init, per_frame, per_pixel, wave and shape code) raw. The writer does the same.
  - Shader text is written only when `warp_ps >= MD2_PS_2_0` (and the same for `comp_ps`), as at state.cpp:980 and 982.
  - WriteCode has no 32768 limit; only ReadCode has one.
- **Empty text**: the C++ buffer ends every line with a terminator, but the Rust text joins lines with '\n'. So `""` can mean either no lines or one empty line.
  - EEL blocks write nothing, which matches an empty C++ buffer.
  - Shader blocks write one empty line (`warp_1=` followed by a backtick). After Import, a C++ shader buffer is never empty: it is generated when no line was read. So an empty Rust shader text can only come from a lone `warp_1=`. Writing nothing instead would make the next read generate a shader.

### Tests (tests/write.rs, 7)
- `round_trip_preserves_every_field`: `parse(p.to_milk()) == p` for `all_keys_exact.milk`. This is the all-keys fixture with every 4-decimal value cut to 3 decimals, which `sed` generated; the values are still distinct. Because those values are exactly representable in the written precision, exact equality holds. The test also checks that the fixture really sets every field: all 53 blendables differ from the defaults, every wave and shape differs, the code blocks are non-empty, and the versions are (3,4).
- `code_line_starting_with_bracket_gets_backtick`: shader lines `[x]`, `;y` and `z` are all written with a backtick. EEL lines `[a` and `;b` are written raw, and everything reads back intact. The name comes from the brief; the assertion follows the C++ rule.
- `header_and_key_order_follow_export`: compares the full key sequence (header, `[preset00]`, the 69 scalars, the 4×13 wave keys, the 4×24 shape keys, then the code) with Export's order, and checks the exact header text.
- `float_formats_match_export`: spot checks of `%.3f` and `%.5f` output, `-0.00000`, and bool/int output.
- `version_gates_header_and_shader_text`: version 0 gets no header and no shaders. warp=1 with comp=2 gets a header and comp text but no warp text.
- `empty_shader_text_is_one_empty_line`.
- `corpus_round_trip_is_stable`. For each corpus preset, with `p = parse(x)` and `q = parse(write(p))`, it asserts:
  1. `q` equals `p` once floats that agree within the format's rounding are snapped. The snap allows a difference up to 0.0005 + 1e-6·|v|, and also covers the fields Export does not write: shader text below version 2, and versions that are both ≤ 0.
  2. `parse(write(q)) == q`: one round trip reaches an exact fixed point.
  3. `write(q) == write(p)`: the F18 form.

  Property choice: the brief's `parse(write(parse(x))) == parse(x)` is false in general, because `%.3f`/`%.5f` drop digits; real packs carry values like 1.285751. It holds only for files that already use ≤ 3 decimals. The dispatch's suggested `parse(write(parse(x))) == parse(write(parse(write(parse(x)))))` is property 2 (a Preset fixed point). Property 2 is stronger than F18's string form, which follows from it. Property 1 adds the bounded-loss analogue of the brief's form. All three hold for the corpus.

### TDD evidence
- RED: `to_milk` stubbed to return `String::new()`. `cargo test -p md-preset --test write` gave `test result: FAILED. 0 passed; 7 failed`.
- GREEN: `cargo test -p md-preset --test write` gave `test result: ok. 7 passed; 0 failed`.
- Before the commit, these were all clean: `cargo fmt --check`, `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.

## Task 2.5: corpus parse test and blending

### Files
- New: crates/md-preset/src/transition.rs (`Transition`, `Preset::start_blend`, plus unit tests), crates/md-preset/tests/common/mod.rs (`corpus_texts()`, shared corpus loader)
- Renamed and reworked: tests/corpus_code.rs became tests/corpus.rs. It has `every_corpus_preset_parses_without_error` and `corpus_presets_code_compiles`, the compile check moved here from 2.3. Both tests and tests/write.rs use `common::corpus_texts()`, so the extraction logic is not duplicated.
- Modified:
  - blend.rs: new `Blendable::start_blend_from` and its test.
  - preset.rs: new field `transition: Transition`.
  - defaults.rs: `transition` default, with line refs.
  - lib.rs: export of `Transition`.
  - tests/parse.rs: the redundant `blendables().len() == 53` assert was removed. Two Default spot checks for the transition were added (state.cpp:533 and :566). The file is now 595 lines, so I did not split it.

### StartBlendFrom semantics (state.cpp:704-849@6f7cd5d), mirrored
1. Instant switch: `s_to` keeps all of its own bools, ints, strings and code. The C++ code that copies them over is commented out. Only `m_fVideoEchoAlphaOld = from.fVideoEchoAlpha.eval(-1)`, `m_nVideoEchoOrientationOld` and `m_nOldWaveMode` are copied from `from`. The renderer uses these in milkdropfs.cpp:2771 and :4069ff.
2. `m_bBlending = true`, `m_fBlendStartTime` and `m_fBlendDuration` are set unconditionally, even for a duration under 0.001.
3. Each of the 53 CBlendableFloats runs `StartBlendFrom`. The target is kept; the blend starts at `from`'s value **at `now`**, `f_from->eval(t)`, so an old preset still mid-blend contributes its blended value. A duration under 0.001 skips the field.
   - I added `Blendable::start_blend_from(&from, time, duration)` because the brief's `blend_to(from, to, now, now+duration)` recomputes the duration as `(now+duration)-now`. That is not bit-exact for large `now`, while the C++ stores the duration as given. `blend_to` is unchanged.
4. Motion-vector special case. When `from.mv_a ≥ 0.001` and `self.mv_a < 0.001`, the C++ runs `s_from->X = s_to->X.eval(t); s_to->X = s_from->X.eval(t)` for mv_x..mv_b. The two `if`s test the same condition. On `s_to` this works out to exactly `X.set(X.eval(now))`: the field takes the start-of-blend value, which is the old preset's, with no blend, so only mv_a fades. The reverse case (transparent before) is not handled in the C++, despite its comment, and it is not handled here either.
- New pub type `Transition { blending, start_time, duration, video_echo_alpha_old, video_echo_orientation_old, old_wave_mode }`, held as `Preset::transition`. These are CState members (state.h:267-276 and 382-385). The default has `old_wave_mode = -1` (state.cpp:566). `m_fBlendProgress` is per-frame renderer state and is not stored.

### Tests
- tests/corpus.rs:
  - `every_corpus_preset_parses_without_error`: asserts that all 32 corpus presets parse, which is 100%, and that each has non-empty warp/comp text. On failure it reports a count.
  - `corpus_presets_code_compiles` (unchanged logic).
- transition.rs unit tests:
  - `blend_midpoint_is_average_for_blendable_fields` (F62): for EVERY field `blendables()` lists, from != to, the field is blending, eval(start) == from, eval(mid) == (from+to)/2 exactly, and eval(end) == to.
  - `non_blendable_fields_switch_immediately`: after `start_blend`, the preset with its blendables and transition restored equals the pre-blend preset, while `from` differs in the int, bool, code, wave/shape and version fields. The test also checks every `Transition` field.
  - `tiny_duration_flags_preset_but_blends_no_field`.
  - `blend_starts_from_old_presets_current_value`.
  - `motion_vectors_fading_out_keep_old_grid`, which also covers the reverse case: everything morphs.
- blend.rs: `start_blend_from_keeps_target_and_reads_from_at_time`.

### TDD evidence
- RED: `start_blend` was a stub (`let _ = (from, now, duration);`). `cargo test -p md-preset --no-fail-fast` gave lib `FAILED. 38 passed; 5 failed`; all 5 transition tests failed.
  - The corpus parse test and `start_blend_from_keeps_target...` passed at RED. Parsing was already implemented in 2.3, and `start_blend_from` was written together with its test.
- GREEN: lib `43 passed`, corpus `2 passed`, parse `9 passed`, write `7 passed`.
- Clean: `cargo fmt --check`, `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`. `cargo test --workspace` had no failing suite.

## Self-review / concerns
1. **Brief vs C++ on backticks**: the brief says lines starting with `[` or `;` get a backtick. The C++ backticks every shader line and no EEL line, and I mirrored the C++, as the dispatch asked. One C++ loss is mirrored too: an EEL line whose stored text starts with a backtick (the file had two) loses it on the next read.
2. **Brief vs F18 on formats**: I used the state.cpp formats (`%.5f` for zoom, rot, dx, dy, warp, sx, sy and fZoomExponent; `%.3f` for cx/cy), not the brief's `%.6f`.
3. **Known non-fixed-point edges**, which the corpus does not hit:
   - The EEL doubled-backtick case above.
   - A shader block at the ReadCode 32768-byte limit whose source lines had no backtick: writing adds one byte per line, so a re-read can stop one line earlier, as in the C++.
   - `warp_ps`/`comp_ps` both ≤ 0 but not 0, which re-read as (0,0).
   In each case one more round trip is stable.
4. **start_blend divergence**: the C++ also mutates `s_from`. It freezes from's mv_x..mv_b in the fade-out case, and `eval` clears finished blend flags. `from` is `&Preset` per A.3, so these writes are not mirrored. The result differs only when the old preset's mv fields are themselves mid-blend, or when the blend is shorter than 0.001 s. This may need a ledger note.
5. **New pub API beyond the brief**: `Transition` / `Preset::transition`, which M5/M7 consume for the old-wave-mode and echo-orientation snaps, and `Blendable::start_blend_from`.
6. The corpus tests cover only the 32 self-authored presets unless `MD_EXTRA_PRESETS` is set. The round-trip test is written so that it should also hold on real packs.

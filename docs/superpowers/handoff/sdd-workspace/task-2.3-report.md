# Task 2.3 report (md-preset: model, defaults, parser)

Commit: a16f567 `feat(preset): model, defaults, and parser` (base 21d9086). Not pushed.

## Files
- New: crates/md-preset/src/{preset.rs, wave.rs, shape.rs, defaults.rs, parse.rs, gen_shader.rs}
- New: crates/md-preset/tests/{parse.rs, corpus_code.rs}, crates/md-preset/tests/fixtures/all_keys.milk (crate-local fixture; documented in the tests/parse.rs header)
- Modified: crates/md-preset/src/{ini.rs, lib.rs}, crates/md-preset/Cargo.toml (dev-deps: md-eval and md-testkit, both path crates), Cargo.lock. No new external crates; cargo deny is clean.

## What was implemented
- `Preset` (Clone, Debug, PartialEq): every key in the milestone list is a typed field. Field types follow `CState`: the 53 `CBlendableFloat` members are `Blendable`; plain float, int and bool members are `f32`, `i32` and `bool`. `warp_ps` and `comp_ps` are `i32`, as in `m_nWarpPSVersion`/`m_nCompPSVersion`, so the writer can echo them. Code blocks are `per_frame_init`, `per_frame`, `per_pixel`, `warp_shader` and `comp_shader`. Waves and shapes are `[WaveData; 4]` and `[ShapeData; 4]`. Wave and shape int fields stay `i32` because C++ stores them as int.
- Blendable list: the `for_each_blendable!` macro in preset.rs lists exactly the `CBlendableFloat` members of state.h:264-351, in declaration order, excluding the commented-out `m_fPlate*` block. A comment cites state.h. The macro generates `blendables()` and `blendables_mut()`, which return `(name, &Blendable)` pairs for Task 2.5. Per F62 there is no count test; `defaults_match_cstate_default` checks `blendables().len() == 53` only as a sanity line.
- `PsVersion { None, Ps2_0, Ps2_X, Ps3_0, Ps4_0 }` uses the A.3 names (with `allow(non_camel_case_types)`). `PsVersion::from_file_value` maps: values <= 0 to None, 2/3/4/5 to the four models per plugin.h:94-98. Values C++ does not handle (1, and 6 or more) hit `assert(0)` and a `ps_0_0` profile in C++; here 1 maps to Ps2_0 and 6+ maps to Ps4_0. The raw ints are kept. `Preset::warp_ps_version()` and `comp_ps_version()` are provided.
- `Preset::defaults()` lives in defaults.rs. Every value carries a `state.cpp:<line>@6f7cd5d` comment (F61 form; 6f7cd5d is the last commit that touched code/).
- `parse_milk` is a port of `CState::Import` with `STATE_ALL`:
  - Import calls `Default()` first and then reads every key with the current value as the fallback. The Rust code mirrors this: it starts from `defaults()` and reads in the C++ section order (general, wave, motion, warp, comp).
  - All lookups use the flat model (`flat_*`, `code_block`).
  - Version precedence follows state.cpp:1309-1327 exactly: `MILKDROP_PRESET_VERSION` (default 100) below 200 gives (0,0). Exactly 200 uses `PSVERSION` (default 2) for both shaders. Above 200 uses `PSVERSION_WARP` and `PSVERSION_COMP` (each default 2), and **`PSVERSION` is ignored**. This differs from the brief's wording "PSVERSION applies to both unless WARP/COMP present"; the binding ruling says to follow Import.
  - Mirrored C++ quirks that change what a parsed preset contains:
    1. `wave_r`, `wave_g`, `wave_b`, `wave_x` and `wave_y` fall back to `m_fRot.eval(-1)` (state.cpp:1368-1372). `rot` is read later, so a missing `wave_r` parses as 0.0, not 1.0.
    2. `mv_a` falls back to `bMotionVectorsOn != 0 ? 1 : 0` (state.cpp:1381), which reads the legacy key. A file with neither key gets `mv_a = 0.0`.
  - `ParseError` is a struct with a `message` field. `parse_milk` never returns it, because C++ Import fails only when the file cannot be opened.
- Code text: the Preset stores the text as ReadCode builds it: lines joined with '\n', one leading backtick stripped, comments kept, so it round-trips through 2.4. I checked the C++: `StripLinefeedCharsAndComments` runs only in `RecompileExpressions` (state.cpp:1665ff), at compile time, never at load. Accessors `*_eel()` on `Preset`, `WaveData` and `ShapeData` return the `strip_code` form for md-eval.
- F19: `gen_warp_shader(decay, wrap)` and `gen_comp_shader(&CompShaderParams{...})` are ported from plugin.cpp:8782-8850.
  - The comp function takes a params struct instead of 9 arguments, which avoids clippy's `too_many_arguments`.
  - Output is in code_block form (no trailing separator). The kept prefix before the last `{` of warp_ps.fx/comp_ps.fx is always `"shader_body\n{"`, so it is a constant.
  - Float formats match C `%.2f`/`%.3f`. I checked Rust rounding against C-double rounding, including ties and 0.005.
  - The fill condition mirrors state.cpp:1432-1434 and 1442-1444: generate when ReadCode read no line, **regardless of the PS version**. The dispatch asked me to check for "PSVERSION>0"; Import has no such check, so version-0 presets also get the generated text, as in C++.
  - Detecting "no line read" needs `Ini::has_code(prefix)` (new). A lone empty `warp_1=` line is non-empty in C++, which stores its terminator, but gives "" from `code_block`.

## Folded-in 2.1 minors
- (a) The ini.rs module doc and the parse comment now call lone-`\r` handling intentional leniency (C++ `fgets` splits only on `\n`). The test is renamed `lone_cr_ends_a_line_as_intentional_leniency` and explains the C++ behaviour.
- (b) New test `flat_lookup_matches_whole_keys_only`: `per_frame_1` does not match `per_frame_10=` or `per_frame_1x=`, `zoom` does not match `zoomexp=` or `zoom2=`, and code_block and has_code agree with this.
- Also new: `has_code` plus the test `has_code_counts_lines_not_text`.

## Tests
- gen_shader unit tests (6): `warp_text_is_exact`, `warp_without_wrap_uses_clamped_sampler`, `comp_plain_text_is_exact`, `comp_echo_text_is_exact`, `comp_echo_orientations` (including negative orientation and the 0.001 threshold), `comp_hue_and_flags_in_order`.
- tests/parse.rs (9):
  - `defaults_match_cstate_default`: 21 spot checks with line numbers.
  - `parses_all_scalar_keys`: 242 assertions. Every scalar key, all wave/shape keys and every code block in all_keys.milk get distinct, non-round values; bools take their non-default value. PSVERSION=2 is present and asserted to be ignored (warp=3, comp=4).
  - `parses_four_waves_and_shapes`: corpus four_waves and four_shapes, plus a no-aliasing check across indices.
  - `ps_version_precedence`: unit fixtures for every branch (100/missing, 199, 200 with and without PSVERSION and with split keys present, 201, 250), the rule that versions below 200 win over explicit shader text, corpus `version200.milk` → (Ps2_0, Ps2_0), `mixed_versions.milk` → (2,4)/(Ps2_0, Ps3_0), a classic preset → (0,0), and the enum mapping.
  - `parses_crlf_bom_and_trailing_junk`: all_keys with BOM, CRLF, `;` junk, a prose line and ^Z gives a Preset equal to the LF version. The corpus edge file equals its LF/no-BOM/no-junk normalisation.
  - `missing_keys_keep_defaults`: an empty preset equals defaults plus the two quirks plus the generated shaders. Also covers partial files and unparsable values.
  - `legacy_motion_vectors_on_seeds_mv_a`.
  - `empty_shader_sections_are_generated_from_parsed_values`:
    - upgraded_default.milk shader text == gen(0.97, wrap) and gen(gamma 1.56).
    - Removing the warp_/comp_ lines from upgraded_default.milk and from upgraded_echo.milk regenerates identical Presets. upgraded_echo covers decay 0.95, no wrap, echo 0.5 / 1.33 / orientation 1, gamma 1.9.
    - A classic version-0 preset gets text.
    - `warp_1=` alone is not regenerated.
  - `code_is_stored_as_read_and_stripped_for_eel`.
- tests/corpus_code.rs: `corpus_presets_parse_and_code_compiles`. Every `md_testkit::corpus_presets()` file parses. Every non-empty `strip_code`'d block compiles with `md_eval::Program::compile`; empty blocks are skipped, as RecompileExpressions skips them. The test also asserts that warp and comp text are non-empty for every preset.

### TDD evidence
RED: I wrote all tests and the model, then stubbed `parse_milk` to return `Preset::defaults()` and both `gen_*` functions to return `""`.
- `cargo test -p md-preset` → `test result: FAILED. 31 passed; 6 failed` (all 6 gen_shader tests failed).
- `cargo test -p md-preset --no-fail-fast --test parse` → `1 passed; 8 failed`. Only `defaults_match_cstate_default` passed, because `defaults()` was not stubbed.

GREEN, after restoring the implementation:
- `cargo test -p md-preset --no-fail-fast` → lib `37 passed; 0 failed`, parse `9 passed; 0 failed`.
- `--test corpus_code` → `1 passed`.

Verification:
- `cargo fmt --check` OK.
- `cargo build --workspace` OK.
- `cargo clippy --workspace --all-targets -- -D warnings` clean, after replacing the generated `assert_eq!(x, true)` with `assert!`.
- `cargo test --workspace`: all pass.
- `cargo deny check`: advisories, bans, licenses and sources ok.

## Self-review / concerns
1. **Deliberate divergence in GenWarpPShaderText.** The C++ passes `bWrap ? L"" : L"_fc"` (wide literals) to a narrow `%s`. With `bTexWrap=0` it actually emits `sampler__main` (only `_` is printed), a sampler that does not exist, so that shader fails to compile in the C++. I emit the intended `sampler_fc_main` (the pre-wide-string 2.25 behaviour), which the corpus' upgraded_echo.milk already assumes. This needs a ledger ruling.
2. **Brief vs C++ on PSVERSION.** The brief says PSVERSION applies unless the split keys are present; C++ ignores PSVERSION entirely for versions above 200 (defaults 2). I followed C++ as the ruling requires. All corpus files carry both keys, so no corpus file is affected.
3. **Generated shader text is filled even for PSVERSION 0**, as Import does. Consumers must gate on `warp_ps_version() != None`. The writer (2.4) must emit warp_/comp_ only when the version is 2 or higher, as Export does; parse then regenerates the same text.
4. **`PsVersion::from_file_value` for values 1 and 6 or more** clamps instead of failing as C++ does. Raw values are kept.
5. **Two observable C++ quirks are mirrored** (missing `wave_r..y` → 0.0, missing `mv_a` → `bMotionVectorsOn`). So `parse_milk("")` is not equal to `Preset::defaults()`. Task 2.4's round trip is unaffected because the writer always emits these keys.
6. tests/parse.rs is 594 lines, near the ~600 guideline; it is mostly the generated assertion table.

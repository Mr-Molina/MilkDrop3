# Pre-flight consistency scan — Rust conversion plan

Plan: `docs/superpowers/plans/2026-11-11-rust-conversion.md` (wins over spec).
Spot-checks against `code/` are cited as `file:line`. Findings are `F1..F65`; rulings are in section 4.
Severity: **H** = blocks a task or CI as written, **M** = wrong result or rework later, **L** = wording or hygiene.

## 1. Cross-task pairs (shared files or interfaces)

| Tasks | Producer → consumer | Finding |
|---|---|---|
| 0.1 → 3.2, 3.3, 4.6, 5.1, 5.2, 7.1, 8.1 | 0.1 creates every `lib.rs`; later tasks say "Create" the same `lib.rs` | conflict F9 (Create should be Modify) |
| 0.1 → 0.2 | 0.1 creates `md-testkit/src/lib.rs`; 0.2 "Create" again | conflict F9 |
| 0.1 → 5.8 | 0.1 creates `apps/milkdrop/src/main.rs`; 5.8 "Create" again | conflict F9 |
| 0.1 → all | pinned dep list omits deps later tasks use (libpulse-simple-binding, bytemuck, pollster, proptest, criterion, blake3, zbus, objc2, windows) | conflict F10 |
| 0.1 lints → 5.2 | `forbid(unsafe_code)` vs `bytemuck::Pod` derive on `MdUniforms`/`Vertex` | conflict F11 (PLAUSIBLE) |
| 0.1 lints → 8.5 | `forbid(unsafe_code)` in app vs objc2 / WinRT media bindings | conflict F47 |
| 0.2 → 5.8, 6.9, 5.6 | `compare_png`, `bless_enabled`, DSSIM | OK on API; license conflict F1 |
| 0.2 → 1.9, 2.4, 2.5, 4.9, 7.4 | `corpus_presets()` | OK; needs LFS checkout F8 |
| 0.3 → 3.4 | CI vs PulseAudio link needs `libpulse-dev` | conflict F8 |
| 0.3 → 5.2+ | CI never sets `MD_REQUIRE_GPU=1`; GPU tests skip and pass | conflict F8 |
| 0.3 → 9.1 | script extension list vs 9.1 `find` list | conflict F60 |
| 0.4 → 6.9 | README cites `tests/golden/manifest.toml` that 6.9 creates; frames 1/60/300/600 vs 1/60/300 | OK (forward ref; superset) |
| 1.1 → 1.2 | `Tok` ops → grammar | conflict F14 (`^=`, `[ ]` missing in both) |
| 1.2 → 1.4 | `Expr` → `compile_ast` | OK |
| 1.3 → 1.4 | `Vm`, `VarRef`, `MegaBuf` → `Load/Store/LoadMem` | OK |
| 1.4 → 1.5 | `Op::Call1(Fn1)/Call2(Fn2)` uses enums defined in 1.5; no `Call3` for `Fn3` | conflict F15 |
| 1.4 → 1.6 | 1.4 tests loop cap; 1.6 implements cap | conflict F15 |
| 1.5 ↔ Global "Pcg32 only" | MT19937 in `GlobalMem` | conflict F12 |
| 1.7 → 1.6 | 100 ms run budget vs 1,048,576-iteration loop cap | conflict F16 |
| 1.7, 4.10 ↔ Global | cargo-fuzz (`libfuzzer-sys` builds C++ with `cc`) | conflict F3 |
| 2.1 → 2.3, 1.9 | `Ini::code_block` → code fields | OK |
| 2.1 → 8.1, 7.8 | INI reader → `milk2.ini` import, `milk2_msg.ini`/`milk2_img.ini` | conflict F49 (md-platform would duplicate it) |
| 2.2 → 2.5 | `Blendable::blend_to/eval` → `start_blend` | OK on API; semantics conflict F17 |
| 2.3 → 2.4 | `Preset` fields → writer | OK; formats conflict F18 |
| 2.3 → 4.6/5.5 | `PsVersion` → `ShaderInput.ps_version` | conflict F23 (no md-hlsl→md-preset edge) |
| 2.3 → 5.5 | empty shader with PSVERSION>0 | conflict F19 (no generated-shader rule) |
| 3.1 → 3.2 | `Fft` → `Analyzer` | OK |
| 3.2 → 5.x, 7.9 | `SoundData` → `FrameInput.sound`, `Visualizer` | OK; appendix deps F58 |
| 3.3 → 3.6 | `AudioSource` → `TimeoutSource<S>` | OK |
| 3.3 → 5.8, 7.9 | `WavSource` → headless render | conflict F51 (advance per frame undefined) |
| 3.4 → 8.4 | `default_loopback`, `list_devices` → `select_source` with `--audio-device` | conflict F50 (no open-by-name API) |
| 3.4 → 8.4 | 3.4 manual check runs `--list-audio-devices`, built in 8.4 | conflict F50 |
| 3.6 → 8.4 | `TimeoutSource` 50 ms | OK |
| 4.1 → 4.2 | `Token` → parser | OK |
| 4.1/4.2 → 4.6 | tests read `include.fx`; 4.6 copies it | conflict F22 |
| 4.2 → 4.3 | `Expr` lacks `Splat` that 4.3 emits | conflict F26 |
| 4.2 ↔ include.fx | `sampler_state { ... }` initializers not in grammar | conflict F21 |
| 4.3 → 4.4 | intrinsic table vs emission rules (texCUBE, tex2Dgrad, tex2Dproj, tex3Dlod, modf, frexp, ldexp) | conflict F26 |
| 4.5 → 4.4 | `Bindings` consumed by 4.4 before 4.5 creates it | conflict F25 |
| 4.6 → 4.4 | 4.4 naga test "default warp shader" needs 4.6 wrapper | conflict F25 |
| 4.4 → 5.2 | `MD_UNIFORMS_SIZE`, offsets → `MdUniforms` | conflict F24 (float4x3 mapping), F29 (duplicate layout) |
| 4.6 → 4.7 | `translate` pipeline → `apply_fixups` before lexing | conflict F28 (wrap.rs not in 4.7 files; redundant fixups) |
| 4.6 → 5.5, 9.1 | default `warp_ps.fx`/`comp_ps.fx` bodies | conflict F20 (never copied to assets) |
| 4.8 → 5.2 | "three vertex layouts" vs one `Vertex` | conflict F30 |
| 4.8 → 5.3 | built-in WGSL → `PipelineCache` | OK |
| M5 intro → 4.8 | `D3D9_HALF_TEXEL` in a Rust `layout.rs` used by hand-written WGSL | conflict F29 |
| 4.9 → M2 | corpus test needs `.milk` parsing | conflict F23/F58 |
| 5.1 → 6.6, 6.7 | `Atlas`, `layout` → `TextQueue`, title texture | OK; font naming F43 |
| 5.2 → 5.3–6.8 | `Gpu<'a>` borrowed device → `Renderer` → `Visualizer` → app | conflict F6 |
| 5.3 → 6.2–6.5 | `DrawBatch`, `FixedMode`, `BlendMode`, `Topo` | OK |
| 5.4 → 5.5 | `TextureStore::get` | OK; ext order F32 |
| 5.4 → 6.8 | no colorkey, no `TextureId` | conflict F34 |
| 5.5 → 7.5 | `load_preset` compiles both shaders; 7.5 compiles them at ticks 2 and 5 | conflict F44 |
| 5.5 ↔ md-platform | `md_platform::cache_dir()` used by md-render (no edge) | conflict F35 |
| 5.5/5.6 → 5.7 | `Renderer` methods added before `renderer.rs` exists | conflict F36 |
| 5.6 → 7.2 | `PerFrameVars`, `PerPixelVars`, `PerPixelRunner` | conflict F37 (crate ownership cycle) |
| 5.6 → 7.5 | `randomize_blend_pattern`, `VertInfo` → `Switcher.blend_pattern` | OK |
| 6.1 → 4.5 | blur usage text scan duplicates binding resolution | conflict F39 |
| 6.1, 6.2, 6.5 → A.3 `FrameInput` | per-frame outputs, rot params missing from `FrameInput` | conflict F38 |
| 6.3 → 7.3 | `ShapeFrameVars`, `build_shape` | OK |
| 6.4 → 7.3 | `custom_wave_inputs`, `build_custom_wave` | OK; clamp F41; `CustomWaveDraw` undefined (folded into F38) |
| 6.6 → 7.7 | `TextQueue` → `Ui::draw` | OK |
| 6.8 ↔ Review Focus #1 | focus item is preset-shader texture; test is sprite texture | conflict F42 |
| 6.9 → 5.8 | `render_cmd::render_to_images` from integration test; bin crate has no lib | conflict F55 |
| 7.1 → 7.2, 7.3 | `PresetState` | OK |
| 7.4 → 7.5, 7.9 | `PresetLibrary`, `History` | OK |
| 7.6 → 8.2 | `BINDINGS`, `KeyChord` | OK |
| 7.6 → 8.3 | window commands land in `Visualizer::command` which has no window | conflict F46 |
| 7.6/7.7 → 9.1 | `assets/help.txt` used in M7, created in 9.1 | conflict F45 |
| 7.8 → 6.8 | `SpriteFrame` | OK |
| 7.9 → 5.8 | replaces `headless.rs` (never in a Files list) | conflict F55 |
| 7.9 ↔ C++ order | draw order | conflict F64 |
| 7.9 → 8.5 | `Visualizer::set_track_title` used, never produced | conflict F47 |
| 8.1 → 6.6 | six `FontSpec`s in md-platform `Config` (needs md-text) | conflict F43 |
| 8.1 → spec | "every key from the grep in the spec" (no such list) | conflict F48 |
| 8.2 → 9.1 | `assets/icon.png` used in 8.2, created in 9.1 | conflict F45 |
| 8.3 → 5.7 | clamp only in app; `Renderer::resize` unclamped | conflict F56 |
| 8.6 → 6.9 | release runs golden suite on llvmpipe | OK |
| 9.1 → 0.3 | removes `code/` exemption | OK; F60, F61 |

## 2. Per-task self-consistency

| Task | Verdict |
|---|---|
| 0.1 | "ten members" but 11 listed (9 crates + app + md-testkit) F9; dep list incomplete F10; dssim-core F1; workspace `forbid` vs md-audio opt-out needs md-audio to skip `lints.workspace` (folded into F9 ruling) |
| 0.2 | dssim-core AGPL F1; `lib.rs` Create→Modify F9 |
| 0.3 | prose bans `cc, cmake, bindgen`, example lists only `cc, cmake`; `cargo tree -e build` misses transitive build deps F7; CI lacks libpulse-dev, LFS, MD_REQUIRE_GPU F8 |
| 0.4 | Step 4 count test has no file listed (L, folded into F9 ruling) |
| 1.1 | missing `^=` and `[`/`]` (nseel-compiler.c:1193, 1209) F14 |
| 1.2 | same F14 |
| 1.3 | test-name text contradicts itself ("rounds toward floor" yet reads slot 3; pinned value is fine); `vm` filter misses `mem.rs` tests F57 |
| 1.4 | uses Fn1/Fn2 from 1.5; no Call3; cap test belongs to 1.6 F15 |
| 1.5 | `rand` pinned `[0,x)`, C is `[0,x]` (nseel-cfunc.c:109) F13; MT vs Pcg32 constraint F12; sigmoid and band/bor match the C++ (checked) |
| 1.6 | OK (cap 1048576 confirmed, ns-eel.h:136) |
| 1.7 | 100 ms bound unsatisfiable with nested loops F16; fuzz F3; AST printer not specified (folded into F16) |
| 1.8 | OK (bench only; criterion pin F10) |
| 1.9 | OK |
| 2.1 | OK |
| 2.2 | `eval_before_start_returns_start` contradicts state.cpp:1912-1932 F17 |
| 2.3 | count test near-tautology F62; fixture path not in Files; filter F57; no generated-shader rule F19 |
| 2.4 | `%.6f` pin contradicts "match state.cpp" (state.cpp:925-932 uses `%.5f`/`%.3f`); corpus round-trip unstable F18 |
| 2.5 | OK |
| 3.1 | `envelope_table_endpoints_are_zero`: last entry ≈3e-5, not 0 F65 |
| 3.2 | OK (bands [0,85),[85,170),[170,256) confirmed, plugin.cpp:8736-8742) |
| 3.3 | `48k_is_resampled_to_44k1` invalid identifier F52; WAV advance unspecified F51 |
| 3.4 | manual check needs the 8.4 CLI F50; test name says fallback, asserts error only; env mutation in a shared test binary F54 |
| 3.5 | SCK 44.1 kHz F53; screencapturekit build F4; "only place unsafe is permitted" contradicted by 8.5 F47 |
| 3.6 | 25 ms bound flaky on CI F54 |
| 4.1 | uses include.fx before 4.6 copies it F22; filter F57 |
| 4.2 | include.fx `sampler_state` blocks F21 |
| 4.3 | `Expr::Splat` absent from AST F26; GetBlur*/GetMain are `#define` macros (include.fx:116-120), not functions F21 |
| 4.4 | consumes 4.5 output F25; "24 tests" vs ~33 rules F27; float4x3 rule contradicts M4 contract F24; missing emission rules F26 |
| 4.5 | OK |
| 4.6 | `lib.rs` Create→Modify F9; default shader bodies not copied F20 |
| 4.7 | wrap.rs not in Files; two redundant fixups F28 |
| 4.8 | "eight data/*.fx" includes warp_ps/comp_ps which are not built-ins here, while 10 files are created (L); three layouts vs one F30 |
| 4.9 | needs md-preset F23 |
| 4.10 | fuzz F3 |
| 5.1 | bundled font license F63 |
| 5.2 | `Gpu<'a>` F6; Pod F11; layout.rs duplicate F29; mat4x3 test name F24 |
| 5.3 | "PointList does not exist in wgpu" is false F31 |
| 5.4 | ext order wrong (plugin.cpp:2847) F32; noise "clamped to 255" wrong (plugin.cpp:2482-2485 ORs overflow) F33 |
| 5.5 | blake3 builds with cc F2; cache_dir edge and cache payload F35; Renderer not yet defined F36; no missing-user-texture test F42 |
| 5.6 | Per*Vars crate ownership F37 |
| 5.7 | "default preset" in test is PSVERSION 0 → no-shader path, so "per the default warp shader" wording is inapplicable (L, folded into F19 ruling) |
| 5.8 | main.rs Create→Modify F9; headless.rs unlisted, no lib target F55 |
| 6.1 | takes `&Preset` but blur min/max are per-frame vars F38; text scan F39 |
| 6.2 | modes 0..8 and flags 256..2048 do not exist (md_defines.h:51 `NUM_WAVES 8`) F40 |
| 6.3 | OK |
| 6.4 | clamp is 512 only for spectrum, else 480 (milkdropfs.cpp:2520) F41 |
| 6.5 | `MotionVectorVars`, `Border` undefined (folded into F38) |
| 6.6 | font names F43 |
| 6.7 | `songtitle` font not among the six F43 |
| 6.8 | colorkey/TextureId F34 |
| 6.9 | lib target F55 |
| 7.1 | OK (rot params handed to renderer via F38) |
| 7.2 | `PerFrameVars` ownership F37 |
| 7.3 | OK |
| 7.4 | OK |
| 7.5 | tick-2/tick-5 split vs single `load_preset`; tests need GPU F44 |
| 7.6 | `assets/help.txt` not yet present F45; `TypeChar` vs `text_input` F46 |
| 7.7 | OK (15 modes confirmed, plugin.h:48; `MAX_PRESETS_PER_PAGE 128` confirmed, defines.h:216) |
| 7.8 | OK |
| 7.9 | draw order differs from milkdropfs.cpp:965-1131 F64; `set_track_title` missing F47 |
| 7.10 | OK |
| 8.1 | spec has no key list F48; INI duplicate F49; FontSpec edge F43 |
| 8.2 | icon not yet present F45 |
| 8.3 | clamp location F56 |
| 8.4 | open-by-name API F50 |
| 8.5 | out-of-scope 3.x feature, macOS API cannot read other players, unsafe F47 |
| 8.6 | OK (flatpak manifest written but not built: L, no ruling) |
| 9.1 | find list vs script F60; code/ line refs F61 |
| 9.2 | OK |
| 9.3 | OK |

## 3. Global-constraint and rubric contradictions

| Item | Status |
|---|---|
| dssim-core (0.1, 0.2) | **AGPL-3.0**, not on the allowed list. cargo-deny fails. F1 |
| blake3 (5.5) | build-dependency `cc` compiles C/asm SIMD (even with `pure`, `cc` stays in the graph). F2 |
| cargo-fuzz / libfuzzer-sys (1.7, 4.10) | builds vendored libFuzzer C++ with `cc`. F3 |
| screencapturekit (3.5) | recent releases compile a Swift bridge in build.rs (PLAUSIBLE; verify the pinned version). F4 |
| wgpu / wgpu-hal Metal | releases on `metal` + `objc` with `objc_exception` compile a `.m` file with `cc` (PLAUSIBLE; depends on version). F5 |
| libpulse-binding / libpulse-simple-binding | MIT/Apache; `-sys` uses pkg-config, no `cc`. OK; needs `libpulse-dev` in CI (F8) |
| cpal | Apache-2.0; on Linux pulls `alsa-sys` (pkg-config, no `cc`), so it needs `libasound2-dev` unless target-gated. F8 |
| fontdue, fontdb, ddsfile, glam, hound, clap, tracing, rand_pcg, serde, toml, thiserror, anyhow, dirs (option-ext MPL-2.0), zbus, windows, naga, winit | pure Rust, allowed licenses. OK |
| image | allowed license; default features drag in AVIF/EXR codecs that aren't needed. F59 |
| bytemuck derive under `forbid(unsafe_code)` | PLAUSIBLE compile error. F11 |
| "all randomness via Pcg32" | contradicted by MT19937 in md-eval (1.5). F12 |
| Out-of-scope 3.x features | 8.5 media-key player control (spec §3 lists it as absent from `code/`). F47. `PsVersion::Ps4_0` is fine (enum exists in plugin.h:98, "not supported") |
| Tests that assert nothing | GPU tests skip silently on CI (F8); 2.3 count tautology (F62); 3.4 test name vs assertion (F54) |
| Duplicated logic | uniform layout in md-hlsl and md-render (F29); INI reader in md-preset and md-platform (F49); blur-usage scan vs binding resolution (F39); fixups duplicating `h_` prefix and 4.6 defines (F28); loop cap in 1.4 and 1.6 (F15); `TypeChar` vs `text_input` (F46) |
| Bundled font (OFL) | not on the allowed list. F63 |

## 4. Findings and rulings

- F1 (H) dssim-core is AGPL-3.0.
  Ruling: replace dssim-core with a ~100-line windowed SSIM in md-testkit, keeping the `compare_png`/`DiffResult` API and reporting `dssim = 1/ssim - 1` — AGPL is outside the allowed list and cargo-deny fails at Task 0.3 — cost if wrong: thresholds need re-tuning once.
- F2 (H) blake3 pulls `cc`.
  Ruling: key ShaderCache with `sha2::Sha256` over the assembled HLSL plus the md-hlsl crate version — pure Rust, allowed license, no `cc` — cost if wrong: a slightly slower hash on preset load.
- F3 (H) cargo-fuzz builds C++.
  Ruling: replace the 1.7 and 4.10 fuzz targets with proptest arbitrary-string tests (`compile_and_run_never_panics`, `translate_never_panics`) and add no `fuzz/` dirs — libfuzzer-sys compiles vendored C++ — cost if wrong: weaker coverage-guided search.
- F4 (M) screencapturekit may compile Swift.
  Ruling: use `objc2-screen-capture-kit` (plus `objc2-core-media`) directly in md-audio, with `unsafe` allowed there as already planned — pure-Rust bindings and no build-time foreign compile — cost if wrong: more binding code in `macos_sck.rs`.
- F5 (M) wgpu Metal may pull `objc_exception` (`cc`).
  Ruling: at 0.1, pin a wgpu/naga release whose Metal backend uses objc2 and pass `cargo deny check` for all targets before committing the pin — keeps the no-`cc` guard satisfiable on macOS — cost if wrong: re-pin wgpu later, with API churn.
- F6 (H) `Gpu<'a>` makes Renderer and Visualizer borrow the device, so the app becomes self-referential.
  Ruling: keep the A.3 signatures taking `&wgpu::Device`/`&Queue` but store owned clones (`Device: Clone` in wgpu ≥ 24); `Gpu` has no lifetime — smallest change that lets the winit app own both — cost if wrong: none, since clones are refcounted handles.
- F7 (H) the native-build guard does not see transitive `cc`.
  Ruling: the script runs `cargo tree --workspace --target all -e normal,build -i <crate>` for `cc`, `cmake`, `bindgen` and fails on any hit; deny.toml bans all three — `-e build` alone skips build deps under normal deps — cost if wrong: none.
- F8 (H) CI is missing prerequisites.
  Ruling: ci.yml uses `actions/checkout` with `lfs: true`; Ubuntu installs `libpulse-dev` and sets `MD_REQUIRE_GPU=1`; cpal is declared under `[target.'cfg(windows)'.dependencies]` — without these, GPU tests pass vacuously and corpus tests read LFS pointers — cost if wrong: one CI edit.
- F9 (L) file-list bookkeeping.
  Ruling: the workspace has 11 members; later "Create lib.rs/main.rs" entries mean Modify; md-audio omits `lints.workspace = true` and declares its own lints; 0.4's corpus-count test lives in `md-testkit/tests/corpus.rs` — wording only — cost if wrong: none.
- F10 (L) 0.1 dependency list is incomplete.
  Ruling: each task adds the pinned `[workspace.dependencies]` entries it first needs, and cargo-deny must pass in that commit — 0.1 cannot foresee all of them — cost if wrong: none.
- F11 (M) `#[derive(Pod)]` under `forbid(unsafe_code)` (PLAUSIBLE).
  Ruling: in md-render, serialize `MdUniforms` and `Vertex` by hand to `Vec<u8>` with `f32::to_le_bytes`, and drop bytemuck — keeps `forbid` intact — cost if wrong: ~40 lines of packing code.
- F12 (L) MT19937 vs the Pcg32-only rule.
  Ruling: amend Global Constraints so that EEL `rand()` uses a fixed-seed MT19937 per ns-eel2 and every other random source uses Pcg32 — both are deterministic, and preset parity needs MT — cost if wrong: none.
- F13 (L) rand range.
  Ruling: pin `genrand_u32() as f64 * (1.0/4294967295.0) * max(floor(x),1)` (inclusive upper bound) and rename the test `rand_is_at_most_max` — matches nseel-cfunc.c:109 — cost if wrong: none.
- F14 (H) lexer/parser miss ns-eel2 syntax.
  Ruling: add `^=` (`_powop`) and `a[b]` indexing, which desugars to `_mem(a+b)`, to 1.1 and 1.2 with one test each — nseel-compiler.c:1193 and 1209 accept both, and 1.9 needs 100% of the corpus — cost if wrong: corpus failures found late in 1.9.
- F15 (M) md-eval task ordering.
  Ruling: 1.4 declares the variants of `Fn1`/`Fn2`/`Fn3` plus `Op::Call3`, 1.5 implements them, and the cap test moves from 1.4 to 1.6 (1.4 tests flooring only) — removes the forward reference and the duplicated work — cost if wrong: none.
- F16 (M) the 1.7 timing bound is unsatisfiable.
  Ruling: the proptest generator emits only literal loop counts ≤ 16 and nesting ≤ 2, includes a small AST printer in `props.rs`, and asserts only that `run` completes — nested 1M-iteration caps make 100 ms impossible — cost if wrong: weaker property coverage.
- F17 (M) Blendable semantics.
  Ruling: `eval` returns the target value when `time < 0` or outside `[start, start+dur]` and interpolates linearly inside, as state.cpp:1912-1932 does; replace the test with `eval_outside_window_returns_target` — the plan's test contradicts the C++ — cost if wrong: a blending glitch at transition start.
- F18 (M) writer formats.
  Ruling: use state.cpp's per-key formats (`%.5f` for zoom/rot/dx/…, `%.3f` for cx/cy/…), and have the corpus test assert `write(parse(write(parse(x)))) == write(parse(x))` — the plan's `%.6f` contradicts its own "match state.cpp", and lossy formats break `parse∘write == id` — cost if wrong: none.
- F19 (H) empty shader sections.
  Ruling: add `gen_warp_shader(decay, wrap)` and `gen_comp_shader(gamma, echo…, flags)` to md-preset in 2.3, ported from plugin.cpp:8782-8850; `Preset` fills empty warp/comp text at parse time as state.cpp:1434 and 1444 do; 4.9 and 5.5 consume the filled text — presets with PSVERSION>0 and no body otherwise render wrong — cost if wrong: one extra function pair.
- F20 (H) default shader bodies are lost.
  Ruling: copy `warp_ps.fx` and `comp_ps.fx` to `assets/hlsl/` alongside include.fx (in 4.1, per F22) — 5.5's fallback and F19 need them after 9.1 deletes `code/` — cost if wrong: none.
- F21 (H) include.fx syntax.
  Ruling: 4.2 parses and discards `= sampler_state { … }` initializers and `<…>` annotations; 4.3 and the M4 intro treat GetBlur*/GetMain/GetPixel as preprocessor macros and say `sampler_rand00..15` are body-declared — include.fx:116-155 uses this syntax, so `parses_include_fx_without_error` fails otherwise — cost if wrong: none.
- F22 (H) include.fx is needed before it is copied.
  Ruling: move the asset copy (include.fx, warp_ps.fx, comp_ps.fx) from 4.6 to 4.1 — 4.1 and 4.2 tests read it — cost if wrong: none.
- F23 (M) `PsVersion` ownership.
  Ruling: md-hlsl defines its own `PsVersion`; md-preset keeps its `PsVersion`; md-render converts with a `From` impl; 4.9 uses md-preset as a dev-dependency and runs after M2 — keeps A.2 acyclic without new runtime edges — cost if wrong: one duplicated 5-variant enum.
- F24 (H) float4x3 mapping contradictions.
  Ruling: store the transpose: `floatRxC -> matRxC<f32>`, `mul(v,M) -> (M * v)`, `mul(M,v) -> (v * M)`, with constructors and `M[i]` unchanged; the M4 contract then reads "WGSL columns are HLSL rows"; rename the 5.2 test `mat4x3_packing_puts_hlsl_rows_in_wgsl_columns` — keeps 64-byte matrices, so `MD_UNIFORMS_SIZE = 1920` and every offset stays valid — cost if wrong: rotation presets mirror, caught by the golden suite.
- F25 (M) md-hlsl task order.
  Ruling: run 4.5 before 4.4, and have 4.4's naga test use a hand-wrapped default-warp fixture in `tests/fixtures/` — 4.4 consumes `Bindings` and the wrapper — cost if wrong: none.
- F26 (M) the intrinsic table outruns the emitter.
  Ruling: 4.4 adds rules for `tex2Dgrad -> textureSampleGrad`, `tex2Dproj` (divide by `.w`), `tex3Dlod`, `modf`, `frexp` and `ldexp`; `texCUBE` returns a `TranslateError` (MilkDrop binds no cube maps); 4.3 adds `Expr::Splat` by modifying `ast.rs` — the emitter otherwise panics or errors on accepted input — cost if wrong: a few corpus presets fall back.
- F27 (L) test count.
  Ruling: one test per listed emission rule, with no fixed count — the list has ~33 rules, not 24 — cost if wrong: none.
- F28 (L) redundant fixups.
  Ruling: drop the WGSL-keyword fixup (the `h_` prefix covers it) and the comp `uv_orig` alias (4.6 defines it); list `wrap.rs` as Modify in 4.7 — avoids duplicated logic — cost if wrong: none.
- F29 (M) uniform layout defined twice; half-texel constant is unreachable from WGSL.
  Ruling: md-render imports `md_hlsl::layout` (no md-render `layout.rs`) and tests each `MdUniforms` field offset against `md_uniform_offset`; the built-in VS shaders take `texel_size` in their uniform and apply `-0.5 * texel_size`, and `D3D9_HALF_TEXEL = -0.5` lives in `md_hlsl::layout` — one source of truth — cost if wrong: none.
- F30 (L) vertex layouts.
  Ruling: 4.8 targets the single 5.2 `Vertex` layout — 5.2 is explicit — cost if wrong: none.
- F31 (L) wgpu topology claim.
  Ruling: correct the text ("TriangleFan is missing; PointList is 1 px only") and keep the quad expansion — factual error, no design change — cost if wrong: none.
- F32 (L) texture extension order.
  Ruling: search `jpg, dds, png, tga, bmp, dib` (dib via the bmp decoder) — plugin.cpp:2847 — cost if wrong: the wrong file is chosen when two extensions share a root.
- F33 (L) noise byte overflow.
  Ruling: channel byte = `v & 0xFF`, with bit 8 of channel *k* ORed into bit 0 of channel *k+1*, as the `<<`/`|` packing in plugin.cpp:2482-2485 does; pin it in `noise_values_within_range_for_zoom` — the C++ does not clamp — cost if wrong: slightly different noise statistics.
- F34 (M) sprite textures.
  Ruling: 6.8 adds `TextureStore::load_sprite(gpu, path, colorkey_lo, colorkey_hi) -> TextureId` by modifying textures.rs — 5.4's API cannot express colorkey — cost if wrong: none.
- F35 (M) shader cache plumbing.
  Ruling: `RenderSettings.shader_cache_dir: Option<PathBuf>` is supplied by md-core; the disk cache stores serialized `Translated` (wgsl + samplers + texsize_names) — avoids an md-render→md-platform edge, and WGSL alone loses binding info — cost if wrong: none.
- F36 (M) `Renderer` defined too late.
  Ruling: 5.5 creates `renderer.rs` with `Renderer::new` (ping-pong VS, TextureStore, PipelineCache); 5.7 modifies it to add `begin_frame`/`present`/`resize`/comp — 5.5 and 5.6 add methods and tests that need it — cost if wrong: none.
- F37 (H) per-frame var types would create a crate cycle.
  Ruling: define `PerFrameVars`, `PerPixelVars`, `PerPixelRunner` in md-render `mesh.rs` (5.6) and have md-core 7.2 fill them — md-render cannot depend on md-core — cost if wrong: none.
- F38 (H) `FrameInput` is too thin.
  Ruling: add `vars: &PerFrameVars`, `old_vars: Option<&PerFrameVars>` and `rot: &[RotParams; 24]` to A.3 `FrameInput` (`RotParams`, `MotionVectorVars`, `Border`, `CustomWaveDraw` defined in md-render); `blur_passes` takes `&PerFrameVars` — blur min/max, echo, gamma and wave params are post-equation values — cost if wrong: A.3 churn later.
- F39 (L) blur-usage scan.
  Ruling: compute `highest_used` from the `Translated.samplers` roots `blur1..3` of both shaders — reuses 4.5, no second parser — cost if wrong: none.
- F40 (M) wave modes.
  Ruling: modes 0..7 (`NUM_WAVES = 8`), drop the 256..2048 flags, and give `build_wave` the old mode plus a blend mix so it draws both during transitions as DrawWave's `it` loop does — md_defines.h:51 and milkdropfs.cpp:2780 — cost if wrong: wave pops during blends.
- F41 (M) custom wave sample cap.
  Ruling: clamp `samples` to 512 for spectrum and 480 (`NUM_WAVEFORM_SAMPLES`) otherwise; rename the test `samples_clamped_to_480_or_512` — milkdropfs.cpp:2520 — cost if wrong: reads past the 480-sample wave buffer.
- F42 (M) Review Focus #1 is untested.
  Ruling: add `missing_user_texture_binds_fallback_and_logs_once` to 5.5, repoint Review Focus #1 to it, and keep the 6.8 sprite test — the focus item is about preset shaders — cost if wrong: none.
- F43 (L) fonts.
  Ruling: name the six slots `simple, decorative, help, playlist, songtitle, tooltip`; md-platform owns a plain `FontConfig` that md-core converts to `md_text::FontSpec` — matches C++ EXTRA_1/EXTRA_2 and avoids an md-platform→md-text edge — cost if wrong: none.
- F44 (M) progressive shader compile.
  Ruling: compile both shaders at tick 2 through a `ShaderLoader` trait (implemented by `Renderer`, faked in tests), with tick 5 a no-op that keeps the 8-tick schedule — A.3 `load_preset` compiles both, and Switcher tests then need no GPU — cost if wrong: a slightly longer hitch on tick 2.
- F45 (M) assets are created after use.
  Ruling: 7.6 copies `text1.bin` to `assets/help.txt` (LF) and 8.2 converts the icon to `assets/icon.png`; 9.1 only deletes the originals — M7 and M8 read them — cost if wrong: none.
- F46 (L) window commands and duplicate text input.
  Ruling: the app handles `ToggleFullscreen`/`ToggleStretch`/`ToggleAlwaysOnTop` before forwarding to `Visualizer::command`, and `text_input(ch)` is defined as `command(TypeChar(ch))` — md-core owns no window — cost if wrong: none.
- F47 (H) 8.5 contradicts scope and lint rules.
  Ruling: cut 8.5 to a read-only `now_playing()` (Linux MPRIS via zbus, Windows GSMTC, macOS `NullMedia`), drop next/previous and Ctrl+arrows, and add `Visualizer::set_track_title` to 7.9 — media-key control is a 3.x feature (spec §3), macOS public APIs cannot read other players, and objc2 needs unsafe in a `forbid` crate — cost if wrong: re-add later behind the same trait.
- F48 (M) the config key source does not exist.
  Ruling: enumerate keys from `ReadConfig` in plugin.cpp/pluginshell.cpp, record the count in `default_config_matches_cpp_defaults`, and use snake_case fields with `serde(rename)` — the spec has no key list, and camelCase fields fail clippy `-D warnings` — cost if wrong: none.
- F49 (M) duplicated INI reader.
  Ruling: md-platform depends on md-preset and reuses `md_preset::ini::Ini` (add that A.2 edge) — no second INI parser — cost if wrong: a small extra compile dependency.
- F50 (M) audio device selection.
  Ruling: add `pub fn open_device(name: &str) -> Result<Box<dyn AudioSource>, AudioError>` to md-audio A.3; 3.4's manual check becomes `cargo run -p md-audio --example list_devices` — 8.4 needs it, and the CLI does not exist at 3.4 — cost if wrong: none.
- F51 (M) WAV pacing.
  Ruling: `WavSource::open(path, loop, fps)`; each `next_frame` advances `round(rate/fps)` samples and returns the latest 576 — otherwise headless renders run audio at 76.6 fps — cost if wrong: goldens re-blessed once.
- F52 (H) invalid test identifier.
  Ruling: rename it `resamples_48k_to_44k1` — `fn 48k_…` does not compile — cost if wrong: none.
- F53 (L) ScreenCaptureKit sample rate (PLAUSIBLE).
  Ruling: capture at 48 kHz and let `to_frame` resample — SCK documents 8/16/24/48 kHz — cost if wrong: none.
- F54 (L) flaky timing and a misnamed test.
  Ruling: 3.6 asserts `Silence` within 200 ms; 3.4's test becomes `pulse_without_server_returns_no_server` in its own integration-test binary (env var set before any thread starts) — avoids CI flakes and env races — cost if wrong: none.
- F55 (H) app integration tests cannot reach bin modules.
  Ruling: add `apps/milkdrop/src/lib.rs` exposing `cli`, `render_cmd`, `headless` (keep `main.rs` thin) and list `headless.rs` in 5.8 Create — tests in `tests/` cannot import a bin crate — cost if wrong: none.
- F56 (M) resize clamp location.
  Ruling: clamp inside `Renderer::resize` and `Renderer::new` to `[1, max_texture_dimension_2d]`, and have the app pass through — `render --size 0x0` and direct callers would otherwise panic (Review Focus #5) — cost if wrong: none.
- F57 (L) test filters.
  Ruling: Run steps use `cargo test -p <crate>` (whole crate) instead of name filters — `vm`, `parse`, `lexer`, `gpu` miss sibling modules' tests — cost if wrong: seconds of extra test time.
- F58 (L) appendix dependencies.
  Ruling: M5 depends on 2, 3, 4; Task 4.9 runs after M2 — 5.x uses `SoundData`/`WavSource`, and 4.9 parses `.milk` — cost if wrong: scheduling only.
- F59 (L) `image` features.
  Ruling: `image = { default-features = false, features = ["jpeg","png","tga","bmp"] }` — trims the AVIF/EXR codec graph — cost if wrong: none.
- F60 (L) inconsistent guard.
  Ruling: 9.1 runs only `scripts/check-no-native-build.sh` (extensions `c, cc, cpp, h, hpp`; excludes `target/` and `.git/`) — one definition — cost if wrong: none.
- F61 (L) line references break after `code/` is deleted.
  Ruling: C++ references in comments use `<file>:<line>@<snapshot sha>`, and 9.1's grep ignores that form — the references stay meaningful after deletion — cost if wrong: none.
- F62 (L) tautological count test.
  Ruling: replace the 2.3 count test with a 2.5 test that every listed blendable field interpolates under `start_blend` — it asserts behavior instead of a hand-typed number — cost if wrong: none.
- F63 (L) bundled font license.
  Ruling: amend Global Constraints to allow OFL-1.1 for bundled font assets (crates stay on the list) and record the font in NOTICE — the constraint targets dependencies — cost if wrong: swap the font.
- F64 (M) 7.9 draw order differs from the C++.
  Ruling: use the milkdropfs.cpp:965-1131 order: motion vectors (into VS0, before warp), warp, blur, shapes, custom waves, wave, darken-center + borders (C++ `DrawSprites`), title anim (VS mode), composite, title anim (screen mode), user sprites, text — the plan omits borders/darken-center and misplaces motion vectors — cost if wrong: goldens re-blessed.
- F65 (L) envelope endpoint.
  Ruling: assert `|env[0]| < 1e-6` and `env[575] < 1e-4` — the formula is not exactly 0 at i=575 — cost if wrong: none.

# SDD ledger — plan: docs/superpowers/plans/2026-11-11-rust-conversion.md

Tracked copy of the subagent-driven-development ledger (the container is ephemeral, so the ledger lives in git).
Spec: docs/superpowers/specs/2026-11-11-cross-platform-port-plan.md (plan wins on conflict).
Resume: hourly scheduled task trig_01FM3pmj24AwEMGRxXwKaV7q fires into the controlling session.

Ruling: Ledger is tracked in git at this path instead of only in .superpowers/sdd/ — container resets would destroy an untracked ledger — cost if wrong: a few extra commits of bookkeeping.
Ruling: A.4 owner decisions are taken at the plan's defaults (owner asked for fully autonomous execution) — cost if wrong: rework of the corresponding tasks (mainly corpus choice in 0.4 and audio backends in 3.4/3.5).
Ruling: Ledger and plan checkboxes are pushed to origin/rust-experiment after every task — user authorized autonomous pushes to this branch — cost if wrong: none beyond noisy history.

## Pre-flight scan

Full table and per-finding rulings: docs/superpowers/ledger/2026-11-11-rust-conversion-preflight.md (65 findings). All proposed rulings in its section 4 are ADOPTED as written; implementers of each task must apply the rulings that name their task.

Ruling: replace dssim-core with a ~100-line windowed SSIM in md-testkit, keeping the `compare_png`/`DiffResult` API and reporting `dssim = 1/ssim - 1` — AGPL is outside the allowed list and cargo-deny fails at Task 0.3 — cost if wrong: thresholds need re-tuning once.
Ruling: key ShaderCache with `sha2::Sha256` over the assembled HLSL plus the md-hlsl crate version — pure Rust, allowed license, no `cc` — cost if wrong: a slightly slower hash on preset load.
Ruling: replace the 1.7 and 4.10 fuzz targets with proptest arbitrary-string tests (`compile_and_run_never_panics`, `translate_never_panics`) and add no `fuzz/` dirs — libfuzzer-sys compiles vendored C++ — cost if wrong: weaker coverage-guided search.
Ruling: use `objc2-screen-capture-kit` (plus `objc2-core-media`) directly in md-audio, with `unsafe` allowed there as already planned — pure-Rust bindings and no build-time foreign compile — cost if wrong: more binding code in `macos_sck.rs`.
Ruling: at 0.1, pin a wgpu/naga release whose Metal backend uses objc2 and pass `cargo deny check` for all targets before committing the pin — keeps the no-`cc` guard satisfiable on macOS — cost if wrong: re-pin wgpu later, with API churn.
Ruling: keep the A.3 signatures taking `&wgpu::Device`/`&Queue` but store owned clones (`Device: Clone` in wgpu ≥ 24); `Gpu` has no lifetime — smallest change that lets the winit app own both — cost if wrong: none, since clones are refcounted handles.
Ruling: the script runs `cargo tree --workspace --target all -e normal,build -i <crate>` for `cc`, `cmake`, `bindgen` and fails on any hit; deny.toml bans all three — `-e build` alone skips build deps under normal deps — cost if wrong: none.
Ruling: ci.yml uses `actions/checkout` with `lfs: true`; Ubuntu installs `libpulse-dev` and sets `MD_REQUIRE_GPU=1`; cpal is declared under `[target.'cfg(windows)'.dependencies]` — without these, GPU tests pass vacuously and corpus tests read LFS pointers — cost if wrong: one CI edit.
Ruling: the workspace has 11 members; later "Create lib.rs/main.rs" entries mean Modify; md-audio omits `lints.workspace = true` and declares its own lints; 0.4's corpus-count test lives in `md-testkit/tests/corpus.rs` — wording only — cost if wrong: none.
Ruling: each task adds the pinned `[workspace.dependencies]` entries it first needs, and cargo-deny must pass in that commit — 0.1 cannot foresee all of them — cost if wrong: none.
Ruling: in md-render, serialize `MdUniforms` and `Vertex` by hand to `Vec<u8>` with `f32::to_le_bytes`, and drop bytemuck — keeps `forbid` intact — cost if wrong: ~40 lines of packing code.
Ruling: amend Global Constraints so that EEL `rand()` uses a fixed-seed MT19937 per ns-eel2 and every other random source uses Pcg32 — both are deterministic, and preset parity needs MT — cost if wrong: none.
Ruling: pin `genrand_u32() as f64 * (1.0/4294967295.0) * max(floor(x),1)` (inclusive upper bound) and rename the test `rand_is_at_most_max` — matches nseel-cfunc.c:109 — cost if wrong: none.
Ruling: add `^=` (`_powop`) and `a[b]` indexing, which desugars to `_mem(a+b)`, to 1.1 and 1.2 with one test each — nseel-compiler.c:1193 and 1209 accept both, and 1.9 needs 100% of the corpus — cost if wrong: corpus failures found late in 1.9.
Ruling: 1.4 declares the variants of `Fn1`/`Fn2`/`Fn3` plus `Op::Call3`, 1.5 implements them, and the cap test moves from 1.4 to 1.6 (1.4 tests flooring only) — removes the forward reference and the duplicated work — cost if wrong: none.
Ruling: the proptest generator emits only literal loop counts ≤ 16 and nesting ≤ 2, includes a small AST printer in `props.rs`, and asserts only that `run` completes — nested 1M-iteration caps make 100 ms impossible — cost if wrong: weaker property coverage.
Ruling: `eval` returns the target value when `time < 0` or outside `[start, start+dur]` and interpolates linearly inside, as state.cpp:1912-1932 does; replace the test with `eval_outside_window_returns_target` — the plan's test contradicts the C++ — cost if wrong: a blending glitch at transition start.
Ruling: use state.cpp's per-key formats (`%.5f` for zoom/rot/dx/…, `%.3f` for cx/cy/…), and have the corpus test assert `write(parse(write(parse(x)))) == write(parse(x))` — the plan's `%.6f` contradicts its own "match state.cpp", and lossy formats break `parse∘write == id` — cost if wrong: none.
Ruling: add `gen_warp_shader(decay, wrap)` and `gen_comp_shader(gamma, echo…, flags)` to md-preset in 2.3, ported from plugin.cpp:8782-8850; `Preset` fills empty warp/comp text at parse time as state.cpp:1434 and 1444 do; 4.9 and 5.5 consume the filled text — presets with PSVERSION>0 and no body otherwise render wrong — cost if wrong: one extra function pair.
Ruling: copy `warp_ps.fx` and `comp_ps.fx` to `assets/hlsl/` alongside include.fx (in 4.1, per F22) — 5.5's fallback and F19 need them after 9.1 deletes `code/` — cost if wrong: none.
Ruling: 4.2 parses and discards `= sampler_state { … }` initializers and `<…>` annotations; 4.3 and the M4 intro treat GetBlur*/GetMain/GetPixel as preprocessor macros and say `sampler_rand00..15` are body-declared — include.fx:116-155 uses this syntax, so `parses_include_fx_without_error` fails otherwise — cost if wrong: none.
Ruling: move the asset copy (include.fx, warp_ps.fx, comp_ps.fx) from 4.6 to 4.1 — 4.1 and 4.2 tests read it — cost if wrong: none.
Ruling: md-hlsl defines its own `PsVersion`; md-preset keeps its `PsVersion`; md-render converts with a `From` impl; 4.9 uses md-preset as a dev-dependency and runs after M2 — keeps A.2 acyclic without new runtime edges — cost if wrong: one duplicated 5-variant enum.
Ruling: store the transpose: `floatRxC -> matRxC<f32>`, `mul(v,M) -> (M * v)`, `mul(M,v) -> (v * M)`, with constructors and `M[i]` unchanged; the M4 contract then reads "WGSL columns are HLSL rows"; rename the 5.2 test `mat4x3_packing_puts_hlsl_rows_in_wgsl_columns` — keeps 64-byte matrices, so `MD_UNIFORMS_SIZE = 1920` and every offset stays valid — cost if wrong: rotation presets mirror, caught by the golden suite.
Ruling: run 4.5 before 4.4, and have 4.4's naga test use a hand-wrapped default-warp fixture in `tests/fixtures/` — 4.4 consumes `Bindings` and the wrapper — cost if wrong: none.
Ruling: 4.4 adds rules for `tex2Dgrad -> textureSampleGrad`, `tex2Dproj` (divide by `.w`), `tex3Dlod`, `modf`, `frexp` and `ldexp`; `texCUBE` returns a `TranslateError` (MilkDrop binds no cube maps); 4.3 adds `Expr::Splat` by modifying `ast.rs` — the emitter otherwise panics or errors on accepted input — cost if wrong: a few corpus presets fall back.
Ruling: one test per listed emission rule, with no fixed count — the list has ~33 rules, not 24 — cost if wrong: none.
Ruling: drop the WGSL-keyword fixup (the `h_` prefix covers it) and the comp `uv_orig` alias (4.6 defines it); list `wrap.rs` as Modify in 4.7 — avoids duplicated logic — cost if wrong: none.
Ruling: md-render imports `md_hlsl::layout` (no md-render `layout.rs`) and tests each `MdUniforms` field offset against `md_uniform_offset`; the built-in VS shaders take `texel_size` in their uniform and apply `-0.5 * texel_size`, and `D3D9_HALF_TEXEL = -0.5` lives in `md_hlsl::layout` — one source of truth — cost if wrong: none.
Ruling: 4.8 targets the single 5.2 `Vertex` layout — 5.2 is explicit — cost if wrong: none.
Ruling: correct the text ("TriangleFan is missing; PointList is 1 px only") and keep the quad expansion — factual error, no design change — cost if wrong: none.
Ruling: search `jpg, dds, png, tga, bmp, dib` (dib via the bmp decoder) — plugin.cpp:2847 — cost if wrong: the wrong file is chosen when two extensions share a root.
Ruling: channel byte = `v & 0xFF`, with bit 8 of channel *k* ORed into bit 0 of channel *k+1*, as the `<<`/`|` packing in plugin.cpp:2482-2485 does; pin it in `noise_values_within_range_for_zoom` — the C++ does not clamp — cost if wrong: slightly different noise statistics.
Ruling: 6.8 adds `TextureStore::load_sprite(gpu, path, colorkey_lo, colorkey_hi) -> TextureId` by modifying textures.rs — 5.4's API cannot express colorkey — cost if wrong: none.
Ruling: `RenderSettings.shader_cache_dir: Option<PathBuf>` is supplied by md-core; the disk cache stores serialized `Translated` (wgsl + samplers + texsize_names) — avoids an md-render→md-platform edge, and WGSL alone loses binding info — cost if wrong: none.
Ruling: 5.5 creates `renderer.rs` with `Renderer::new` (ping-pong VS, TextureStore, PipelineCache); 5.7 modifies it to add `begin_frame`/`present`/`resize`/comp — 5.5 and 5.6 add methods and tests that need it — cost if wrong: none.
Ruling: define `PerFrameVars`, `PerPixelVars`, `PerPixelRunner` in md-render `mesh.rs` (5.6) and have md-core 7.2 fill them — md-render cannot depend on md-core — cost if wrong: none.
Ruling: add `vars: &PerFrameVars`, `old_vars: Option<&PerFrameVars>` and `rot: &[RotParams; 24]` to A.3 `FrameInput` (`RotParams`, `MotionVectorVars`, `Border`, `CustomWaveDraw` defined in md-render); `blur_passes` takes `&PerFrameVars` — blur min/max, echo, gamma and wave params are post-equation values — cost if wrong: A.3 churn later.
Ruling: compute `highest_used` from the `Translated.samplers` roots `blur1..3` of both shaders — reuses 4.5, no second parser — cost if wrong: none.
Ruling: modes 0..7 (`NUM_WAVES = 8`), drop the 256..2048 flags, and give `build_wave` the old mode plus a blend mix so it draws both during transitions as DrawWave's `it` loop does — md_defines.h:51 and milkdropfs.cpp:2780 — cost if wrong: wave pops during blends.
Ruling: clamp `samples` to 512 for spectrum and 480 (`NUM_WAVEFORM_SAMPLES`) otherwise; rename the test `samples_clamped_to_480_or_512` — milkdropfs.cpp:2520 — cost if wrong: reads past the 480-sample wave buffer.
Ruling: add `missing_user_texture_binds_fallback_and_logs_once` to 5.5, repoint Review Focus #1 to it, and keep the 6.8 sprite test — the focus item is about preset shaders — cost if wrong: none.
Ruling: name the six slots `simple, decorative, help, playlist, songtitle, tooltip`; md-platform owns a plain `FontConfig` that md-core converts to `md_text::FontSpec` — matches C++ EXTRA_1/EXTRA_2 and avoids an md-platform→md-text edge — cost if wrong: none.
Ruling: compile both shaders at tick 2 through a `ShaderLoader` trait (implemented by `Renderer`, faked in tests), with tick 5 a no-op that keeps the 8-tick schedule — A.3 `load_preset` compiles both, and Switcher tests then need no GPU — cost if wrong: a slightly longer hitch on tick 2.
Ruling: 7.6 copies `text1.bin` to `assets/help.txt` (LF) and 8.2 converts the icon to `assets/icon.png`; 9.1 only deletes the originals — M7 and M8 read them — cost if wrong: none.
Ruling: the app handles `ToggleFullscreen`/`ToggleStretch`/`ToggleAlwaysOnTop` before forwarding to `Visualizer::command`, and `text_input(ch)` is defined as `command(TypeChar(ch))` — md-core owns no window — cost if wrong: none.
Ruling: cut 8.5 to a read-only `now_playing()` (Linux MPRIS via zbus, Windows GSMTC, macOS `NullMedia`), drop next/previous and Ctrl+arrows, and add `Visualizer::set_track_title` to 7.9 — media-key control is a 3.x feature (spec §3), macOS public APIs cannot read other players, and objc2 needs unsafe in a `forbid` crate — cost if wrong: re-add later behind the same trait.
Ruling: enumerate keys from `ReadConfig` in plugin.cpp/pluginshell.cpp, record the count in `default_config_matches_cpp_defaults`, and use snake_case fields with `serde(rename)` — the spec has no key list, and camelCase fields fail clippy `-D warnings` — cost if wrong: none.
Ruling: md-platform depends on md-preset and reuses `md_preset::ini::Ini` (add that A.2 edge) — no second INI parser — cost if wrong: a small extra compile dependency.
Ruling: add `pub fn open_device(name: &str) -> Result<Box<dyn AudioSource>, AudioError>` to md-audio A.3; 3.4's manual check becomes `cargo run -p md-audio --example list_devices` — 8.4 needs it, and the CLI does not exist at 3.4 — cost if wrong: none.
Ruling: `WavSource::open(path, loop, fps)`; each `next_frame` advances `round(rate/fps)` samples and returns the latest 576 — otherwise headless renders run audio at 76.6 fps — cost if wrong: goldens re-blessed once.
Ruling: rename it `resamples_48k_to_44k1` — `fn 48k_…` does not compile — cost if wrong: none.
Ruling: capture at 48 kHz and let `to_frame` resample — SCK documents 8/16/24/48 kHz — cost if wrong: none.
Ruling: 3.6 asserts `Silence` within 200 ms; 3.4's test becomes `pulse_without_server_returns_no_server` in its own integration-test binary (env var set before any thread starts) — avoids CI flakes and env races — cost if wrong: none.
Ruling: add `apps/milkdrop/src/lib.rs` exposing `cli`, `render_cmd`, `headless` (keep `main.rs` thin) and list `headless.rs` in 5.8 Create — tests in `tests/` cannot import a bin crate — cost if wrong: none.
Ruling: clamp inside `Renderer::resize` and `Renderer::new` to `[1, max_texture_dimension_2d]`, and have the app pass through — `render --size 0x0` and direct callers would otherwise panic (Review Focus #5) — cost if wrong: none.
Ruling: Run steps use `cargo test -p <crate>` (whole crate) instead of name filters — `vm`, `parse`, `lexer`, `gpu` miss sibling modules' tests — cost if wrong: seconds of extra test time.
Ruling: M5 depends on 2, 3, 4; Task 4.9 runs after M2 — 5.x uses `SoundData`/`WavSource`, and 4.9 parses `.milk` — cost if wrong: scheduling only.
Ruling: `image = { default-features = false, features = ["jpeg","png","tga","bmp"] }` — trims the AVIF/EXR codec graph — cost if wrong: none.
Ruling: 9.1 runs only `scripts/check-no-native-build.sh` (extensions `c, cc, cpp, h, hpp`; excludes `target/` and `.git/`) — one definition — cost if wrong: none.
Ruling: C++ references in comments use `<file>:<line>@<snapshot sha>`, and 9.1's grep ignores that form — the references stay meaningful after deletion — cost if wrong: none.
Ruling: replace the 2.3 count test with a 2.5 test that every listed blendable field interpolates under `start_blend` — it asserts behavior instead of a hand-typed number — cost if wrong: none.
Ruling: amend Global Constraints to allow OFL-1.1 for bundled font assets (crates stay on the list) and record the font in NOTICE — the constraint targets dependencies — cost if wrong: swap the font.
Ruling: use the milkdropfs.cpp:965-1131 order: motion vectors (into VS0, before warp), warp, blur, shapes, custom waves, wave, darken-center + borders (C++ `DrawSprites`), title anim (VS mode), composite, title anim (screen mode), user sprites, text — the plan omits borders/darken-center and misplaces motion vectors — cost if wrong: goldens re-blessed.
Ruling: assert `|env[0]| < 1e-6` and `env[575] < 1e-4` — the formula is not exactly 0 at i=575 — cost if wrong: none.

## Tasks

Task 0.1: dispatched (base 19f6cc9, implementer sonnet)
Task 0.1: Ruling: `cc` appears in the build graph only as an unconditional build-dependency of winit's wayland-backend and android-activity, whose C compilation is feature-gated (`log`, `game-activity`) and disabled — Task 0.3 allow-lists cc via cargo-deny `wrappers = ["wayland-backend","android-activity"]` and the guard script additionally asserts those features stay off — cost if wrong: a C compile step could exist on Linux/Android builds.
Task 0.1: Ruling: caret version requirements plus committed Cargo.lock satisfy "pinned" — the lockfile fixes exact versions — cost if wrong: none (switching to `=` is mechanical).

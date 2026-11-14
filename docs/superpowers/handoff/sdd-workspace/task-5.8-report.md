# Task 5.8 report: `render` subcommand and first golden

Commit: f05ee6b `feat(app): headless render subcommand and first golden` (on claude/upbeat-hawking-xwlz3z, not pushed).

## Implemented
- `apps/milkdrop/src/lib.rs` (F55) exposes `cli`, `headless`, `render_cmd`; `main.rs` stays thin.
- `cli.rs`: hand-rolled parser (no clap): `milkdrop render --preset --wav --frames --size WxH --seed --out --every`. `--wav` optional (SilenceSource); `--every` defaults to `--frames` (only the last frame is written); `--size` default 640x480, `--seed` default 0.
- `render_cmd.rs`: `render_to_images(&Gpu, &RenderArgs) -> Vec<(u32, RgbaImage)>` (frame n counts from 1, kept when n % every == 0); `run()` opens a headless device and writes `frame_<n>.png`.
- `headless.rs`: minimal driver. WavSource::open(path, loop=true, 60) advanced once per frame (F51) into md_analysis::Analyzer; two md_testkit::FixedClock at 60 fps (abs and since-preset; frame n renders at n/60, shader `frame` = n); one Pcg32 from --seed for rand_preset, 24 rotations (`randomize_preset_vars`), `rand_frame` and the preset-time jitter (Renderer::new also gets the seed); per-frame vars = `PerFrameVars::from_preset(preset, now)` + `progress`; `mesh_mut().compute_uvs(.., absolute now, None)` before each `begin_frame`; `load_preset` (which calls `preset_loaded()`; RandPicks are per shader inside md-render's resolver); composite target Rgba8Unorm, read back with `read_texture_level`. md-testkit is a normal dependency of the app (for FixedClock).
- Progress: milkdropfs.cpp:494 `progress = (GetTime() - m_fPresetStartTime) / (m_fNextPresetTime - m_fPresetStartTime)`; denominator from milkdropfs.cpp:771-775 with C++ defaults plugin.cpp:940-942 (blend 2.7 + 16 + 10 * (rand%1000) * 0.001, the rand drawn from the seeded Pcg32; start = 0).

## TDD
RED: tests written first; `cargo test -p milkdrop` failed to compile (missing modules `cli`/`headless`/`render_cmd`).
First GREEN attempt: with `ps2/upgraded_default.milk`, `golden_default_preset_frame_60` FAILED: "frame 60 is (nearly) black: 0 lit pixels" (the guard against an all-black golden).
GREEN: after switching the golden preset, `MD_BLESS=1 MD_REQUIRE_GPU=1 cargo test -p milkdrop` created tests/golden/rust/default_60.png; then without bless: 3 passed (`golden_default_preset_frame_60`, `rendering_twice_is_byte_identical`, `cli_parses_render_options`).

## Golden preset
Scanned every ps2/ps2x/ps3 corpus preset at frame 60 (640x480, seed 7, test_signal.wav) via the built binary. upgraded_default and all but one are exactly black (their content is waves/shapes/borders: Milestone 6 hooks). The first non-black one is `ps2/noise_grain.milk` (its warp shader adds noise into the feedback, comp x1.6); named in the test's doc comment. Re-point it to upgraded_default in 6.9.
What the PNG shows (I read the file): a dark, uniformly grey-brown film-grain noise field over the whole frame, no shapes or gradient; luma mean 11/255, stdev 6.3, max 43; 270k of 307k pixels above 8. This is dim but not black.

## Gate
cargo fmt --check clean; clippy --workspace --all-targets -D warnings clean; `MD_REQUIRE_GPU=1 cargo test --workspace`: 583 passed, 0 failed; cargo deny check: advisories/bans/licenses/sources ok (only a pre-existing "unmatched license allowance Unicode-DFS-2016" warning); scripts/check-no-native-build.sh OK.
Tolerance: max_dssim 0.02 unchanged (lavapipe only; no cross-device evidence).

## Concerns
- The golden is faint noise grain; it verifies the warp feedback, noise textures, rand_frame, audio-gated comp (`mid_att`) and composite end to end, but not geometry. A richer golden needs Milestone 6.
- Because it is noise driven by `rand_frame`/noise textures, any change in RNG call order (Pcg32 consumption) changes the image; it is deterministic run to run (tested).
- Frame numbering is 1-based (frame n at time n/60), a choice the brief left open.
- `run` subcommand is not implemented (render only), as in scope.

## Fix round 1/5
Changes:
1. New self-authored corpus preset `tests/presets/ps2/shader_rings.milk` (PSVERSION 2, comp shader only: rings + spokes + colour gradient from rad/ang/uv/time, modulated by bass/treb; no waves/shapes), README table line added. Golden `tests/golden/rust/default_60.png` re-blessed from it (MD_BLESS=1). I viewed it: concentric rainbow rings (magenta/purple left through blue, cyan, green, yellow-orange right) around a centre with an 8-petal star of spokes, dark edge vignette; clearly structured, bright. Test doc comment updated; golden_basic now also asserts an all-black 640x480 frame fails against the golden (compare_png_with, dssim > 0.02).
2. New `run_writes_the_requested_pngs`: cli parse -> `render_cmd::run` into a temp dir, 64x48, --frames 7 --every 3; asserts exactly frame_3.png and frame_6.png exist and decode at 64x48. `bad_numeric_flags_name_the_flag` covers the parse-error fix (`--frames`/`--every` errors now name the flag). main.rs itself remains untested beyond `run`.
Commands: `MD_REQUIRE_GPU=1 cargo test -p milkdrop` 5 passed; fmt, clippy -D warnings clean; `MD_REQUIRE_GPU=1 cargo test --workspace` 585 passed, 0 failed (includes md-preset/md-eval/md-hlsl/md-testkit corpus tests over the new preset); cargo deny ok; check-no-native-build OK.

# Task 6.2 report: built-in waveform

Commit 0161080 `feat(render): built-in waveforms`.

## Implemented
- `crates/md-render/src/wave.rs`: `build_wave(mode: i32, blend: Option<WaveBlend>, sound: &SoundData, p: &WaveParams, view: &WaveView, out) -> WaveGeometry`, `WaveParams::from_vars(&PerFrameVars)` (F38), `scaled_wave`, `smooth_wave` (SmoothWave 2466-2494), `NUM_WAVES = 8`.
- `crates/md-render/src/wave_draw.rs`: `Renderer::draw_wave(enc, &FrameInput)`, called from `begin_frame` right after the custom-shape/custom-wave placeholder (RenderFrame order: BlurPasses, shapes, custom waves, DrawWave, sprites; milkdropfs.cpp:1051-1057), into the warp target via `DrawBatch::begin(.. LoadOp::Load)`. No mip regeneration (6.1a).
- Uses md-analysis `SoundData` (`wave: [[f32;576];2]`, `imm`, `imm_rel`); no new type.

## Pinned from the C++ (deviations from the brief, all intentional)
- F40: modes 0..=7, `(int)wave_mode % NUM_WAVES` (2770). The `case 8` spectrum line is unreachable and not ported. Names from the switch comments are in the wave.rs table (0 circle, 1 x-y spiral, 2 centred spiro const alpha, 3 centred spiro alpha by treble, 4 horizontal script w/ momentum, 5 explosive complex, 6 angle line, 7 two-channel line).
- Blend (2771, 3185-3230): `its = 2` only if blending and new != `m_nOldWaveMode` (= old preset's `wave_mode`, state.cpp:712; taken from `old_preset.wave_mode`, falling back to the current preset). `mix = CosineInterp(blend_t)`, `v1[i] = v1[i]*mix + v2(i*m)*(1-mix)`, `m = (n2-1)/n1`, v2 linearly interpolated, v2's break forces a break in the blended wave (`nBreak1 = i+1`); `alpha = a1*mix + a2*(1-mix)`. Mode 0 skips its closing duplicate vertex whenever blending (2852), even for same-mode blends. Alpha/colour/position params for the old wave are the current (already blended) vars, as in the C++.
- Brief's smoothing formula is wrong/incomplete: C++ (850-859) is `w[0] *= s; w[i] = w[i]*s*(1-sm) + w[i-1]*sm` with the already-smoothed `w[i-1]` (recursive), `s = wave_scale/128`. `scaled_wave` does this on a copy per call (the C++ mutates `mysound.fWave` once per frame).
- Blend state: wave_additive is `SRCALPHA, ONE` (2716-2717) = `BlendMode::AlphaAdditive`, not `Additive` (ONE,ONE) as the brief says; non-additive is `Alpha`. `FixedMode::Color`.
- Fat copies (3259): `(thick || use_dots) && tex_w >= 512` -> 4 cumulative offset copies (+x, +y, -x); dots too. Below 512 wide: one copy. Dots are `Topo::Points` at point size 1 (default), line is `LineStrip`; mode 7 draws two ranges (break).
- Tex-size alpha factors for modes 2/3/5 apply only at exactly 256/512/1024/2048 wide (else unchanged, mode 3 keeps `wave_a`); sample cap `tex_w/3` for modes 4/6/7; kept verbatim.
- Mode with no case (negative or old mode outside 0..7): 480 zero vertices, alpha untouched, as the C++. NaN alpha (zero-width mod range) draws nothing (C++ undefined). `smooth_wave` of 0 points returns none (C++ reads out of bounds).
- Half-pixel: the C++ writes clip positions directly; the built-in `fixed_color` VS adds the D3D9 +0.5 px shift (VsParams, Task 4.8) via `DrawBatch`, so no offset in wave.rs.
- Sampler note (MAG POINT/MIN LINEAR): not relevant, `Color` mode samples nothing.
- `build_wave` takes a `WaveView {aspect, time, tex}` instead of three loose args (clippy too_many_arguments) and returns `WaveGeometry {topo, draws, additive}` rather than a bare `Topo`, since mode 7 and fat copies need several draw ranges.

## Existing tests changed
`tests/frame.rs::identity_vars` now sets `wave_a = 0`: the default preset's wave (alpha 0.8) otherwise draws into every frame; 5 frame tests asserted wave-free frames. No other change to existing tests.

## Tests (crates/md-render/tests/wave.rs, 9 tests; plus 4 unit tests in wave.rs)
The brief's five (adjusted: mode 0 = 240 + 1 duplicate = 241 raw, 481 after SmoothWave; also 479 while blending), plus mode-7 break, blend mixing/CosineInterp CPU test, and two GPU tests: `mode_6_line_lands_pixels_in_main` (silent mode-6 line is a horizontal line on the centre row spanning the width) and `blend_draws_the_old_mode_too` (blend_t = 0 on lavapipe: old mode-6 line present, new mode-0 circle absent).

TDD note: the implementation was written before the test file in this session, so there is no recorded RED for the CPU tests. The GPU tests were mutation-checked: with the `draw_wave` call commented out both GPU tests FAILED (7 CPU passed), restored, all 9 pass.

## Gate (MD_REQUIRE_GPU=1)
- `cargo build --workspace` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean (the C++ literals 6.28/3.1416 carry a scoped `allow(clippy::approx_constant)`); `cargo fmt --check` ok.
- `MD_REQUIRE_GPU=1 cargo test --workspace`: 66 `test result: ok`, 0 failures.

## Golden tests/golden/rust/default_60.png
Still passes unchanged. Measured DSSIM 0, 0 differing pixels vs the golden (render at 640x480, frame 60, seed 7, test_signal.wav). Why: ps2/shader_rings.milk has wave mode 2, alpha 0.6, so the wave is drawn into main, but its composite shader never samples `sampler_main`, so the wave never reaches the output. No re-bless. End-to-end sanity: classic/bass_kaleido.milk at frame 60 (viewed) shows a lit, feedback-smeared waveform (15105 lit pixels); golden_basic.rs's comment that upgraded_default was black "until the wave" is now stale for presets that sample main (not edited).

## Concerns
- wave.rs is 622 lines (over the ~600 guideline; fmt-expanded; draw code already split into wave_draw.rs).
- `FrameInput.blending()` is `old_gpu.is_some()`; the old wave mode falls back to the current preset's when `old_preset` is None, which then never takes the two-wave path.
- No spectrum input is needed (mode 8 dropped), so `SoundData.spectrum` is unused here.

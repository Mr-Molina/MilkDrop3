# Task 6.4 report: custom waves

Status: DONE_WITH_CONCERNS. Commit a875ab3 "feat(render): custom waves". Gate: build, clippy -D warnings, fmt --check clean; `MD_REQUIRE_GPU=1 cargo test --workspace` 628 passed, 0 failed; golden_default_preset_frame_60 passes unchanged (no re-bless).

## Implemented
- `src/custom_waves.rs`: `CustomWaveFrame`, `custom_wave_inputs`, `CustomWavePoint`, `CustomWaveDraw` (points f64 x,y,r,g,b,a as after the per-point program, + use_dots/thick/additive), `CustomWaveView`, `build_custom_wave`, `CustomWaveGeometry`.
- `src/custom_waves_draw.rs`: `Renderer::draw_custom_waves`; hooked in frame.rs after `draw_shapes`, before `draw_wave` (C++ 1054-1056).
- `FrameInput.custom_waves` / `old_custom_waves` (default empty).
- `src/cast.rs`: shared `c_int` and `color_byte` (moved out of shapes.rs; not duplicated).

## Pinned from the C++ (departures from brief)
1. Blend (2507-2513): new waves alpha_mult = blend_t, old = 1 - blend_t, old drawn after new, only while blending; same as shapes (verified, not assumed). Draw order: shapes, custom waves, built-in wave (1054-1056).
2. F41 applied: n = min((int)samples, 512, max_samples) with max_samples 512 spectrum / 480 else (2520). NOTE the C++ actually overwrites with `min(512, (int)var_pf_samples)` at 2548-2549, so a waveform of 481..512 samples reads before fWaveform (UB, j0 < 0); the F41 cap avoids it. Also (int) via c_int: NaN/out of range -> INT_MIN -> nothing drawn.
3. `CustomWaveFrame` has no r,g,b,a, thick, additive (brief listed them): they are not inputs to the sample setup; per-point r/g/b/a start from pf_r.. in md-core (2603-2606), thick/additive live in `CustomWaveDraw`. It does have `wave_scale` (needed for mult) and `use_dots` (n>=1 rule, 2551). `samples` is f64 (the raw var). `build_custom_wave` takes the `CustomWaveDraw` rather than (points, use_dots, thick).
4. `wave_scale` must be `m_fWaveScale.eval(-1)` = the unblended value of that wave's own preset (old preset's for old waves) (2555, state.h:86): md-core 7.3 note.
5. Reads outside the sound data (large sep, negative j0) see the two channels as one contiguous float[2][N] (as C), zero beyond: deterministic stand-in for UB.
6. Spectrum: j0=j1=0, t=(512-sep)/n (2560-2562); nSamples -= sep (2523) is dead code, overwritten at 2548. One dot (n=1): sample = 0 * inf = NaN, as C++.
7. Lines: SmoothWave (reuses wave.rs `smooth_wave`; in-between points take the colour of the previous point, 2487), thick = 4 cumulative copies +x,+y,-x with x_inc=2/tex_w (2654-2665); dots: no smoothing, one copy, point size (tex_w>=1024 ? 2 : 1) + thick (2650), reset to 1 after (2669). Blend SRCALPHA/ONE additive else INVSRCALPHA (2646-2648). Positions (x*2-1)*invAspectX, (y*-2+1)*invAspectY (2612-2613); colour bytes `(int)(c*255[*alpha_mult]) & 0xFF` (2615-2619). Half-pixel shift via DrawBatch as 6.2/6.3; no mips.

## TDD
RED (stubs `todo!()`, FrameInput fields present): `MD_REQUIRE_GPU=1 cargo test -p md-render --test custom_waves` -> 9 tests FAILED (panicked at custom_waves.rs todo!() / custom_waves_draw.rs todo!()), "0 passed; 9 failed".
GREEN: same command -> "9 passed; 0 failed" (after fixing two test-geometry mistakes of my own: thick copies overlap so alpha-blend asserts used non-thick lines; the dot test uses a thick 2 px dot).
Tests: samples_clamped_to_480_or_512, spectrum_uses_0_15_multiplier, sep_offsets_right_channel, thick_quadruples, inputs_match_hand_computed_smoothing (mix1=mix2=0.5, data 1..4 -> 1.546875, 2.09375, 2.6875, 3.125), positions_colours_and_alpha_follow_the_cpp, GPU: custom_wave_lands_pixels_in_main, old_waves_draw_only_while_blending_scaled_by_progress (0, 0.5, 1, not blending), additive_selects_one_instead_of_inv_src_alpha.

## Citations
Every C++ line number written in the new code and tests was checked with `sed -n '<N>p' code/vis_milk2/milkdropfs.cpp` (2487, 2507, 2513, 2520, 2548-2551, 2555, 2560, 2563, 2574, 2592, 2612, 2615, 2639, 2646, 2650, 2654, 2669, 2680, 1054-1056, 2304-2313) and state.h:86.

## Concerns
- Trailer: used the implementer-rules.md "Claude Opus 5.5" trailer as instructed there; the session system reminder names "Sonnet 5.5". Amend if the controller prefers the latter.
- Files: tests/custom_waves.rs is 531 lines, src files < 250.
- md-core 7.3 must fill custom_waves/old_custom_waves (skipping waves where custom_wave_inputs is empty) and run the OLD preset's wave programs every blend frame.

## Fix round 1 (supersedes departure 2 and concern 1 above; the "481..512 is UB, cap at 480" argument was wrong)
Changes (crates/md-render/src/custom_waves.rs): n = min(512, c_int(samples)) as the C++ (mfs:2548-2549); max_samples (512 spectrum / 480 waveform, 2520) only sets j0/j1 and the spectrum stride t (2560-2562); `fetch_flat` replaced by `fetch_valid`: any index outside 0..max_samples of a channel reads 0.0. Doc comment rewritten (no more 481..512 UB claim). tests/custom_waves.rs: test renamed `samples_clamped_to_512_with_cpp_window` (1000 -> 512 for both kinds, 512 -> 512 added), new `waveform_window_follows_the_cpp_with_zero_outside` (n in {512, 481, 480} with odd/even sep, j0 = (480-n)/2 - sep/2 in C integer division, e.g. 481 -> 0, 512 -> -16; zero outside 0..480), dead `let _: Option<Vertex>` line and the unused import removed.
Citations re-verified with sed: 2520, 2548, 2549, 2560, 2561, 2562.
Commands: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean; `MD_REQUIRE_GPU=1 cargo test --workspace` -> 629 passed, 0 failed (golden unchanged).

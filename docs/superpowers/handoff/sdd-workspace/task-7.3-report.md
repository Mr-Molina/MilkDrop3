# Task 7.3 report: custom wave and shape evaluation

Commit 2e5a76b `feat(core): custom wave and shape evaluation` (crates/md-core only; not pushed).

## Implemented
- `crates/md-core/src/waves.rs`: `eval_custom_waves(&mut PresetState, &FrameInputs) -> Vec<CustomWaveDraw>`.
- `crates/md-core/src/shapes.rs`: `eval_custom_shapes(&mut PresetState, &FrameInputs) -> Vec<ShapeFrameVars>`.
- `crates/md-core/src/custom_eval_tests.rs`: 16 tests. lib.rs exports both functions.

## Pinned semantics (C++ milkdropfs.cpp, all line citations verified with sed -n / grep -n against the file)
Waves (DrawCustomWaves 2496-2680, loader 2442-2463):
- Per-frame Vm loaded each frame: inputs, q1..q32 from the PRESET's per-frame result (`*pState->var_pf_q`, i.e. `state.pf` after run_per_frame), t1..t8 from the init capture (2457; the save-back at 2624-2633 is commented out, so no t carry across frames), r g b a samples from the wave.
- Per-point read-only inputs copied once from the per-frame values BEFORE the program runs (2530-2539). Per-frame program runs (2541; no-op if it failed to compile, nseel-compiler.c:1665) even when the wave then draws nothing. After it, q and t are copied pf -> pp (2543-2546).
- samples = min(512, (int)var_pf_samples) after the program (2548-2549); passed as f64 to md-render's custom_wave_inputs (window, smoothing, wave_scale = this preset's unblended `wave_scale.eval(-1)`). Fewer than 2 points (1 for dots) -> wave omitted.
- Per point only sample, value1, value2, x = 0.5f+value1, y = 0.5f+value2 (float sum, 2601-2602), r g b a (the per-frame RESULT r g b a, 2598-2606) are reset; the per-point program (2609) output x y r g b a is read. q, t and user variables carry point to point (and frame to frame in the pp Vm apart from the per-frame q/t copy).
Shapes (DrawCustomShapes 2215-2398, loader 2400-2440):
- Loop `instance < instances` (2264); all per-frame vars reloaded every instance (2267): inputs, q from the preset result, t from the init capture (2415) so t restarts per instance AND frame, the shape's own values, num_inst (`var_pf_instances`, 2425), instance (2426). Program 2270-2273. User vars persist across instances.
- Values returned as doubles; `(int)` of sides/additive/textured/thick and the 3..100 sides clamp (2288-2290) happen in md-render's build_shape (its c_int), so md-core does not cast them. No cast helper is needed from md-core (c_int is pub(crate) in md-render).
- Disabled waves/shapes and shapes with instances <= 0 produce nothing. Failed programs are no-ops (values pass through unchanged), as in C++.

## Departures from the brief
1. No separate `sound` argument: it is `inputs.sound`, which avoids two sources of truth.
2. Waves that produce no points are omitted rather than returned as empty draws (the C++ draws nothing).
3. Per-point colour starts from the per-frame result r g b a, not the preset constants (C++ 2603-2606).

## Entry point for 7.9
Call `run_per_frame` then `eval_custom_waves`/`eval_custom_shapes` on the NEW state every frame into `FrameInput::{custom_waves, shapes}`; during a blend call the same sequence on the OLD state with the same FrameInputs into `old_custom_waves`/`old_shapes`. Order of per-frame must precede (q is read from the state's per-frame Vm). Do not scale alpha here.

## TDD
RED (stubs with todo!(), tests compiled): `cargo test -p md-core custom_eval` -> `test result: FAILED. 0 passed; 16 failed`.
GREEN: same command -> `test result: ok. 16 passed; 0 failed`.
Tests: disabled_wave_produces_nothing, per_point_sees_sample_in_unit_range, shape_instances_see_instance_index, shape_per_frame_can_change_color (the brief's four), plus per_point_starts_from_default_position_and_per_frame_colour, per_point_sees_inputs_and_wave_values, wave_values_use_its_own_unblended_wave_scale, t_flows_from_per_frame_into_per_point_and_carries_point_to_point, q_comes_from_the_presets_per_frame_result, samples_follow_the_per_frame_assignment_with_the_512_cap, old_state_evaluation_fills_its_own_lists, shape_t_resets_to_init_capture_for_every_instance_and_frame, shape_user_variables_persist_across_instances, disabled_or_zero_instance_shapes_produce_nothing, failed_programs_behave_as_no_ops, evaluation_is_deterministic.

## Gate
cargo build --workspace, clippy --workspace --all-targets -D warnings, fmt --check clean; full `cargo test --workspace` all ok (0 failures).

## Performance
Per point: 9 sets, one program run, 6 gets via pre-resolved VarRefs; no name lookup or per-point allocation. Per wave: one Vec from custom_wave_inputs plus the points Vec (with_capacity). Per frame a small q array copy. Shapes: one output Vec.

## Concerns
- Files: waves.rs 150, shapes.rs 137, tests 342 lines (under 600).
- Wave per-frame uses state.pf q: if 7.9 forgets run_per_frame first, q is stale (documented).
- Old-state wave inputs use the same `time`/`progress` as the new state, matching the C++ (loaders use globals).

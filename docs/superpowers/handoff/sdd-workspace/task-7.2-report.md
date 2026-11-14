# Task 7.2 report: per-frame equations and output blending

Commit: see `git log` ("feat(core): per-frame equations and output blending").
Files: crates/md-core/src/{per_frame.rs, per_pixel.rs, per_frame_tests.rs}, lib.rs (modules + re-exports).

## Implemented
- `run_per_frame(&mut PresetState, &FrameInputs) -> PerFrameVars` (mfs:460-554 load, 640-655 pv inputs, 664 run, 670 monitor save, 673-674 q copy, 677-678 clamps). Builds `PerFrameVars::from_preset(now)`, fills inputs, q/monitor from the init capture, `pf_refs.load`, runs, `pf_refs.store`, copies q to pv, clamps gamma [0,8] and echo_zoom [0.001,1000] (C++ min/max semantics, NaN passes).
- `blend_outputs(new, old, progress, snap)` (mfs:691-748): mix = cosine_interp(progress) as f64.
  - mixed (mix*new + (1-mix)*old): decay, wave_a/r/g/b/x/y/mystery, ob_*, ib_*, mv_* (9), echo_zoom, echo_alpha, gamma, blur1..3 min/max, blur1_edge_darken.
  - snapped (`mix < snap` -> old, else new): echo_orient, wave_usedots/thick/additive/brighten, darken_center, wrap, invert, brighten, darken, solarize.
  - untouched (new's): wave_mode (mfs:705 exempt), all motion values, inputs, q, monitor, preset scalars.
- `per_pixel_runner(&mut PresetState) -> impl FnMut(&mut PerPixelVars)`: writes x,y,rad,ang + 10 motion values, runs pv program, reads the motion values back (mfs:1756-1791). No allocation or lookup per call (pv_refs). No program: no-op (callers may skip via `pv_program.is_none()`).
- `preset_progress(now, start, next) -> f32` (mfs:494); same f32 formula headless.rs:144 uses (`time / preset_duration`). headless.rs untouched (7.9 replaces it; the app has no md-core edge).

## Blend/old-preset protocol (pinned from C++)
RunPerFrameEquations runs for the current state, then the OLD state (rep 1, mfs:628-631) with identical globals (including `progress` of the current preset). Caller: `run_per_frame` on both, then `blend_outputs(&mut new_vars, &old_vars, blend_progress, snap_point(...))` (old_vars stays unmixed). Then `WarpMesh::compute_uvs(&new_vars, new_runner, now, None)` and `compute_uvs(&old_vars, old_runner, now, Some((pattern, blend_progress)))` (raw progress, not cosine; ComputeGridAlphaValues reads pOldState vars, mfs:1722-1737). `now` is the absolute clock. Freezing old motion vectors is untouched (7.5).

## Departures (C++ wins)
1. `FrameInputs` gains `now` (non-motion preset values eval at GetTime(), mfs:497-553).
2. `blend_outputs` gains `snap` (m_fSnapPoint, mfs:586-620; use md_render::snap_point). The brief had no way to express it.
3. Milestone bullet lists wave_mode, zoom, rot, cx, cy, warp, sx, sy, dx, dy as blended/snapped: the C++ does not blend them here (wave_mode "exempt" mfs:705; motion blends via mesh UVs). dx/dy are NOT in the output blend.
4. Inputs: bass/mid/treb = `imm_rel`, *_att = `avg_rel` (mfs:483-488); `aspect` is m_fInvAspectX/Y (mfs:544-545; plugin.cpp:2037), i.e. the INVERSE of mesh_aspect; meshx/meshy = m_nGridX/Y (grid count), pixels = GetWidth/Height.
5. Per-pixel `Vm` receives only the C++'s inputs (no `monitor`; 7.1 already pinned its variable list); only q and inputs are copied, not "output scalars" (the C++ copies none).
6. `per_pixel_runner` returns a no-op closure rather than Option when there is no program.
7. Clamps are only gamma and echo_zoom (the only range checks in mfs:676-678); the clamp applies to each state before blending.

## TDD
RED (stubs `todo!()`, compiled): `cargo test -p md-core per_frame` -> `test result: FAILED. 2 passed; 10 failed` (2 passes are tests that touch no stub... the remaining listed failures: blend_at_half..., booleans_switch_at_half, echo_zoom_is_clamped, inputs_reach_the_per_frame_vm, per_frame_can_modify_zoom, per_pixel_code_changes_a_vertexs_motion, q_resets_to_init_each_frame, etc., all "not yet implemented").
GREEN: `cargo test -p md-core` -> 21 passed (10 new: the brief's five + inputs, per-pixel, per-pixel-no-code, blend cosine, blend leaves others alone). Blend tests cover every mixed field (macro list) and every snapped one, at progress 0.3/0.49/0.5(mid)/0.51/0.99 and snap 0.5/1.01/-0.01.
Gate: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` clean; `cargo test --workspace` all ok.

## Citations
Every C++ line cited in the new files and above was checked with `sed -n` against code/vis_milk2/milkdropfs.cpp / plugin.cpp (corrected several off-by-few lines found in the first pass).

## Concerns
- Test run was `-p md-core per_frame` filter, so the 2 "passed" in RED were unrelated state tests matching the filter name? (filter `per_frame` also matched state_tests::per_frame_refs_round_trip... and compile_error_in_per_frame...). Not a weakness of the new tests.
- 7.9 must supply `now`, the snap point, and call both states' run_per_frame during blends.

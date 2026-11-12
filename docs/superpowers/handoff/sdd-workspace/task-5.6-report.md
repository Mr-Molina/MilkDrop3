# Task 5.6 report: Warp mesh and warp pass

Status: DONE_WITH_CONCERNS (concerns are interface notes for 5.7/7.x, listed at the end)
Commit: 4aaf54f feat(render): warp mesh and warp pass

## What was implemented

All in `crates/md-render`:

- `src/mesh.rs`: `VertInfo` (A.3), `PerPixelVars`, `PerPixelRunner<'a> = dyn FnMut(&mut PerPixelVars) + 'a` (F37), `mesh_aspect(w, h)` (plugin.cpp:2035-2036), `MAX_GRID_X/Y` (192/144), `WarpMesh`:
  - `new(grid_x, grid_y, aspect_x, aspect_y)`: positions, `rad`, `ang` (0 at the centre vertex), `a=1, c=0`, exactly plugin.cpp:2279-2303; `m_indices_list` (plugin.cpp:2335-2361; 4 quadrants walked from the centre outwards, first half = quadrants 0-1 = bottom half). Grid clamped to `1..=MAX`.
  - `compute_uvs(per_frame, per_pixel, time, blend)`: one `rep` of ComputeGridAlphaValues (milkdropfs.cpp:1681-1864) in f32 as the C++: per-pixel inputs `x = px*0.5*ax+0.5`, `y = py*-0.5*ay+0.5`, rad, ang + motion values; `zoom2`, stretch, the four warp terms, rotation about (cx, cy), translation, AND the "undo aspect ratio fix" step `u = (u-0.5)/aspect_x + 0.5` (milkdropfs.cpp:1832-1834; present in the C++, not in the milestone text). `blend=None` = rep 0 (UV replaced, alpha 255); `blend=Some((pattern, t))` = rep 1 (old preset): `mix2 = clamp(a*t+c)`, `tu = tu*mix2 + u*(1-mix2)`, alpha `(DWORD)(mix2*255)`. Warp animation (`fWarpTime`, `fWarpScaleInv`, `f[4]`) is computed from the CURRENT preset in rep 0 and reused in rep 1, as the C++ does (it reads `m_pState` once).
- `src/frame_vars.rs`: `PerFrameVars` (F37/F38) — every `var_pf_*` of CState (state.h:389-424: inputs, motion, decay, wave_*, ob_*/ib_*, mv_*, echo_*, darken_center, gamma, wrap, invert, brighten, darken, solarize, b1n..b3x, b1ed, monitor, `q[32]`) as f64, plus preset scalars with no pf variable (warp_anim_speed, warp_scale, wave_scale, wave_smoothing, mod_wave_alpha_*, red_blue_stereo, shader). `from_preset(&Preset, now)` mirrors LoadPerFrameEvallibVars (milkdropfs.cpp:460-554: motion at eval(-1), the rest at eval(now)); `Default` = defaults preset at t=0. Separate file from mesh.rs only for size/responsibility (re-exported at crate root); F37's crate ownership is honoured.
- `src/blendpattern.rs`: `frand` (FRAND's `%7381/7380` grid on the 15-bit `noise::crand`), `BlendPattern {Uniform, Wipe, Plasma, Radial}`, `randomize_blend_pattern(rng, gx, gy, aspect, out)` (`mixtype = 1 + rand%3`), `blend_pattern(...)` per type, `gen_plasma(...)` — line-for-line from plugin.cpp:7043-7199 (in-place on `out[].c` like the C++, edge midpoint rules, dt halving, min/max normalization).
- `src/shader_params.rs` (shared ApplyShaderParams filler, my call per the dispatch — 5.7 should reuse it): `RotParams {xlate, base, speed}` + `matrix(time)` (D3DX `mx*mxlate*mz*my`, rows → float4x3, milkdropfs.cpp:3928-3945), `random_rot_matrix` (rot_rand1..4, fresh per call, 3947-3964), `randomize_preset_vars` (state.cpp:1875-1900), `safe_blur_min_max` (1468-1499 incl. the C++ quirk: a <0.1 range collapses to `min = max = avg - 0.05`), `FrameShaderInputs` (plugin-global), `PresetShaderInputs` (pState: vars, rot, rand_preset, time_since_preset_start), `shader_uniforms(frame, preset, tex, rng) -> MdUniforms` with every `_c0.._c13` (incl. `0.3333*treb` comma-operator quirk, log2(width) twice, window aspect), q, rand_frame/preset, rot.
- `src/warp.rs`: `WarpPreset {gpu: &PresetGpu, shader: PresetShaderInputs}`, `snap_point` (milkdropfs.cpp:587-619), `plan_warp_passes` (RenderFrame 1006-1047: all 4 blend combos + no-blend), `Cull` (bCullTiles/bFlipCulling), `warp_geometry` (vertex blocks + per-pass culled index ranges), `decay_byte` (D3DCOLOR_RGBA_01 incl. `&0xff` wrap), and `Renderer::warp_pass(enc, frame, new, old, mesh)`:
  - shader path (WarpedBlit_Shaders): preset warp program, vertex white + alpha mix2, two halves with the seam `ang` = -π / +π (two vertex blocks, so one indexed draw per half), uniforms from the drawn preset's state (old on pass 0);
  - fixed path (WarpedBlit_NoShaders, PSVERSION_WARP 0): `FixedMode::TexModulate` (tex.rgb×decay, alpha = vertex alpha), LinearWrap/LinearClamp by `new.vars.wrap > snap` — decay and wrap from the CURRENT preset even on pass 0 (as the C++); C++'s y flip cancels PrepareFor2DDrawing's ortho flip (support.cpp:178-184), so positions equal the shader path's;
  - vertex data as MYVERTEX: `uv.xy = uv + 0.5/texsize`, `uv.zw = tu_orig/tv_orig` with offset, `rad_ang`, diffuse; render-pass LoadOp::Load (D3D doesn't clear), then `record_mips` into the same encoder.
- `src/mips.rs`: `record_mips(gpu, enc, tex)` (the loop of `generate_mips`, which now calls it).
- `src/renderer.rs`: `vs_target` ping-pong index + `warp_target()` / `swap_vs()` (C++ swaps texture pointers at milkdropfs.cpp:1134-1136; we swap the index because preset bind groups reference the physical textures), `warp_buffers`, some fields `pub(crate)`.

## TDD evidence

RED (before implementation):
```
$ cargo test -p md-render --test warp_mesh --test warp
error[E0432]: unresolved imports `md_render::blend_pattern`, `md_render::randomize_blend_pattern`, `md_render::BlendPattern`, `md_render::PerFrameVars`, `md_render::PerPixelVars`, `md_render::VertInfo`, `md_render::WarpMesh`
error[E0432]: unresolved imports `md_render::mesh_aspect`, `md_render::FrameShaderInputs`, ... `md_render::WarpPreset`
error[E0599]: no method named `warp_pass` found for mutable reference `&mut Renderer`
error[E0599]: no method named `warp_target` found ...
```
First GREEN run: 9/10 CPU tests passed; `blend_mix_uses_a_t_plus_c` failed on vertex 0 ("got (-0.1, 1), want (0, 1)") — a test-arithmetic bug (I set `a=1` and claimed `1*0.5` clamps); fixed the input to `a=4` (4*0.5 = 2 → clamps to 1), implementation unchanged.

GREEN:
```
$ MD_REQUIRE_GPU=1 cargo test -p md-render --test warp --test warp_mesh --lib
test result: ok. 41 passed (lib, incl. new mesh/warp/blendpattern/shader_params/frame_vars unit tests)
test result: ok. 2 passed  (tests/warp.rs: identity_warp_preserves_image, blend_draws_old_then_new_with_mix_alpha)
test result: ok. 10 passed (tests/warp_mesh.rs)
$ MD_REQUIRE_GPU=1 cargo test -p md-render --test warp -- --nocapture
fixed-function identity warp: dssim 0
shader identity warp: dssim 0
```
Mutation check: removing the `+0.5/texsize` UV offset makes `identity_warp_preserves_image` fail (dssim 0.2896), so the test pins the half-texel convention.

Brief tests present: `uv_of_center_vertex_is_half_half_at_identity`, `zoom_scales_uv_about_center`, `dx_shifts_u_negative`, `rot_rotates_about_cx_cy`, `blend_mix_uses_a_t_plus_c`, `plasma_pattern_is_normalized_to_unit_range` (tests/warp_mesh.rs, hand-computed values), GPU `identity_warp_preserves_image` (md_testkit::ssim, dssim < 0.001, lavapipe, both fixed and shader paths, plus mips non-empty and one swap_vs round). Extra: mesh layout/indices, stretch + warp terms, per-pixel runner I/O, wipe/radial patterns, GPU blend (pass 0 fixed + pass 1 shader alpha 127 → 191), pass plans, snap point, decay byte, seam blocks, culling, rotation matrices, blur min/max quirk, uniform constants.

Gates: `cargo build --workspace` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean (C++ literals `6.28f`/`1.41421f` kept as named consts with `allow(approx_constant)`, precedent md-eval lexer); `cargo fmt --check` clean; `MD_REQUIRE_GPU=1 cargo test --workspace` all ok.

## Files changed

- New: crates/md-render/src/{mesh.rs, frame_vars.rs, blendpattern.rs, shader_params.rs, warp.rs}, crates/md-render/tests/{warp_mesh.rs, warp.rs}
- Modified: crates/md-render/src/{lib.rs, mips.rs, renderer.rs}

## Deviations from the brief signatures (documented in code)

1. `warp_pass(&mut self, enc: &mut CommandEncoder, frame: &FrameShaderInputs, new: &WarpPreset, old: Option<&WarpPreset>, mesh: &WarpMesh)` — no `ctx` (FrameCtx is 5.7's), no `blend_t` (the blend lives in the mesh UVs/alphas; blending = `old.is_some()`); presets carry their ApplyShaderParams state.
2. `randomize_blend_pattern(rng, grid_x, grid_y, aspect, out)` — added `aspect` (the C++ uses m_fAspectX/Y in all three patterns).
3. Mesh UVs are stored without the `+0.5/texsize` offset (mesh has no texture size); the warp pass adds it — affine, so only last-bit rounding differs from the C++.
4. `PerFrameVars` lives in `frame_vars.rs` (not mesh.rs) for file size; still md-render-owned per F37.
5. GetSafeBlurMinMax: the milestone text says "forces max > min + 0.001"; the C++ actually collapses a <0.1 range to `min = max = avg - 0.05`. Mirrored the C++ (affects 6.1's scale `1/(max-min)` = inf; 6.1 must look at how BlurPasses handles that — see C++ 1572ff).

## Self-review findings / concerns for the controller

- F38 FrameInput gap: ApplyShaderParams needs per-preset `time_since_preset_start` and the OLD preset's `rot`/`rand_preset` (pass 0 uses `m_pOldState`), and the window size. A.3 FrameInput (+F38) has `rot` and `rand_preset` for one preset only and no preset-start times. 5.7/7.x should carry `old_rot`, `old_rand_preset`, both preset start times and the window size (or build `PresetShaderInputs` per preset as here).
- 5.7 must call `swap_vs()` at the end of each frame and bind the comp's `sampler_main` to `warp_target()` (the C++ comp reads m_lpVS[1] = this frame's warp output); the "first frame clear old VS" clears `other_vs(warp_target())`.
- Uniforms are written with `queue.write_buffer`, one buffer per ShaderProgram: old/new must be different PresetGpu, and only one warp pass per submit (documented on `warp_pass`).
- `rot_rand1..4` and FRAND streams use the renderer's Pcg32 (A.4); not the C++ rand() stream.
- Odd grid sizes: the index list (like the C++'s) omits the middle row/column; the C++ then reads uninitialized indices. Default 64×64 is unaffected.
- `PerFrameVars` uses f64 for EEL values (exact per-pixel inputs), f32 for preset scalars; 7.2 fills it.
- `snap_point` is public for 7.2's boolean blending (RunPerFrameEquations uses the same m_fSnapPoint).

# Task 5.7 report: Composite pass and headless frame

Status: DONE_WITH_CONCERNS (concerns are interface notes for 6.x/7.x, at the end)

## What was implemented (crates/md-render)

- `src/frame.rs` (new): `FrameInput<'a>` = A.3 fields (time, frame, fps, progress, sound, preset, old_preset, blend_t, q, rand_frame, rand_preset, vert_blend) + F38 (`vars`, `old_vars`, `rot`) + ledger 5.6 note (`old_rot`, `old_rand_preset`, `now` = GetTime(), `preset_start_time`, `old_preset_start_time`) + `gpu`/`old_gpu: &PresetGpu` (distinct PresetGpus for old/new). `FrameInput::new(..)` for non-blend frames. `FrameCtx<'a>` (encoder + prepared composite). `Renderer::begin_frame` in RenderFrame order (milkdropfs.cpp:751-1137): rebind stale programs, motion-vector hook (M6), clear the warp source (all mips) when `m_nFramesSinceResize == 0`, warp pass, blur/shapes/waves/wave/sprites/title hooks (no-ops, M6), composite preparation (vertex data, culled index ranges, uniforms via shader_uniforms, comp VsParams = window). `Renderer::present(ctx, target)`: composite, one submit, `swap_vs()`, frame counter.
- `src/comp.rs` (new): `comp_grid(window, tex)` = m_comp_verts/m_comp_indices exactly (plugin.cpp:2038-2152: doubled centre row/col, SquishToCenter(·,3), -0.5 px positions, UvToMathSpace + ang fix-ups, quadrant-oriented index list), `COMP_GRID_X/Y = 32/24`, `hue_shades` (shade[] formula incl. m_fRandStart), `comp_vertices` (bilinear hue colours + warp-mesh blend alpha with the C++ index arithmetic, D3DCOLOR masking), `plan_comp_passes` (RenderFrame 1072-1119 incl. funky reversal: Fixed then OLD shader InvAlpha/NotOpaque), `cull_indices`, `cosine_interp`, and `Renderer::record_comp`.
- `src/comp_fixed.rs` (new): `fixed_comp_draws` = ShowToUser_NoShaders (3967-4287): one-pixel-larger aspect-stretched quad with PrepareFor2DDrawing's y flip, hue shade × fShaderAmount, echo (two layers, zoom, orientation flips, mix), echo orientation/alpha switching during blends (4069-4083), gamma passes (echo: int(g-0.0001) redraws; no echo: int(g-0.001)+1 passes, last fractional), brighten/darken/solarize/invert filters; module-level blend table with C++ line refs. `rgba_01` = D3DCOLOR_RGBA_01.
- `src/tex_size.rs` (new, split from renderer.rs for size): `TexSize {Auto, AutoPow2, Fixed}`, `main_texture_size` (AllocateMyDX9Stuff 1844-1889: window/pow2, clip, canvas stretch, pow2 recompute, snap 16, then clamp 1..=max per F56), `GetNearestPow2Size`, `blur_texture_sizes` (moved), `main_textures`.
- `src/renderer.rs` (modified, F36): `RenderSettings` gains `tex_size` (default Auto, the C++ default) and `canvas_stretch` (100); Renderer gains window, texture generation, owned `WarpMesh` (`mesh()/mesh_mut()`), frames_since_resize, `rand_start` (m_fRandStart, plugin.cpp:6623-6626), comp intermediate texture, comp grid/buffers, dynamic vertex buffer; `window()`, `comp_texture()`, `request_clear()`, `resize(w, h)` (clamped, F56; rebuilds VS/blur/mesh when the tex size changes, comp grid/target when the window changes; always clears next frame like the C++).
- `src/preset_gpu.rs` (modified): group 1 behind a `Mutex` with the texture generation it binds + stored bgl1; `bind_group_1(i)` accessor (field no longer public); `rebind()` rebuilds group 1 from the kept sources, rewrites `texsize_blurN` values and the warp program's VsParams. Called lazily by `begin_frame` (PresetGpus are caller-owned, so resize cannot reach them).
- `src/warp.rs`: `Cull::keeps` and `WarpBuffers::upload_raw` pub(crate) (reused by the composite).
- `Cargo.toml`: `md-analysis` path dep (A.2 edge for `SoundData`, re-exported as `md_render::SoundData`). No new external crates.

## Design decisions (documented in code)

1. Composite target (ledger 5.5 note): composite into a main-format, window-sized intermediate (`comp_texture()`), then copy into the caller's view with a cached fixed `TexModulate` pipeline keyed by the view's texture format (point sampling at texel centres when sizes match, linear otherwise; alpha forced to 1 like the X8R8G8B8 back buffer). One extra pass; no per-surface-format preset pipelines. The view's texture format/size are used (`TextureView::texture()`), so views must have their texture's format (no sRGB reinterpretation views).
2. Uniforms (ledger 5.6 note): write_buffer per program; one frame = one submit; warp and comp programs are different objects so both run in one submit; old/new must be distinct PresetGpus.
3. Window size (ledger 5.6 note asks FrameInput to carry it): the renderer owns it (`resize`), and both blend passes read it there — avoids a second source of truth that could disagree with the composite target. FrameInput has no `window` field.
4. Warp mesh is renderer-owned (rebuilt with the right aspect on resize); md-core computes UVs via `mesh_mut()` before `begin_frame`. A.3's `q` and `vert_blend` are kept but documented as not read (shaders read `vars.q` per F38; vert_blend goes into compute_uvs).
5. darken_center: the C++ does NOT draw it in ShowToUser_NoShaders — it is in DrawSprites (milkdropfs.cpp:3310-3345) into the VS for every preset; it belongs to the sprites hook (Task 6.5 `build_darken_center`). Its blend (SrcAlpha/InvSrcAlpha) is listed in the comp_fixed.rs table; not drawn here.
6. Fixed composite sampler address: D3D9 state leaks between passes; mirrored as "last stage-0 address mode set this frame": clamp if the last warp pass was WarpedBlit_NoShaders with wrap ≤ snap, else wrap (RestoreShaderParams). 6.1's blur passes (which set clamp) must update it.
7. `TexSize::Fixed` is an md-render extension (exact size, no 16-snap) for tests/headless; Auto/AutoPow2 follow the C++.

## TDD evidence

RED (tests written first, `crates/md-render/tests/frame.rs`):
```
$ cargo test -p md-render --test frame
error[E0432]: unresolved imports `md_render::comp_grid`, `md_render::FrameInput`, `md_render::SoundData`, `md_render::COMP_GRID_X`, `md_render::COMP_GRID_Y`
error[E0599]: no method named `begin_frame` / `mesh_mut` / `present` / `resize` / `window` found ...
```
GREEN:
```
$ MD_REQUIRE_GPU=1 cargo test -p md-render --test frame -- --nocapture
test comp_grid_has_768_vertices ... ok
mean frame 1 254, frame 2 253
test two_frames_of_default_preset_produce_decaying_image ... ok
gamma 1: 64, gamma 2: 128
test no_shader_path_applies_gamma_passes ... ok
test composite_into_a_bgra_target_matches_rgba ... ok
test renderer_resize_clamps_to_device_limits ... ok
test composite_preserves_the_image_and_its_orientation ... ok
test blend_composites_old_and_new_by_the_mesh_alpha ... ok
test result: ok. 7 passed
$ MD_REQUIRE_GPU=1 cargo test -p md-render --lib   -> ok. 54 passed
```
(lavapipe.) Brief tests: `two_frames_of_default_preset_produce_decaying_image` (DEFAULT_WARP_PS `ret -= 0.004`, frame 0 clears, white painted into the warp source: 254 then 253, drop ≈ 0.004·255), `comp_grid_has_768_vertices`, `no_shader_path_applies_gamma_passes` (0.25 grey → 128 at gamma 2; 2.5 → 160; 0.75 grey saturates). Extra: `composite_into_a_bgra_target_matches_rgba` (fixed and shader paths into Bgra8Unorm), `renderer_resize_clamps_to_device_limits` (new 0×0, resize 0×0 and u32::MAX×3 clamp, then a preset loaded before the resizes still draws correctly at 64×48), `composite_preserves_the_image_and_its_orientation` (shader comp exact ±1; fixed comp upright), `blend_composites_old_and_new_by_the_mesh_alpha` (both shaders, funky reversal, new-only; mix 0.25 makes Alpha vs InvAlpha distinguishable: 63/63/192). Unit tests: comp pass plans, grid doubling/ang fix-ups/squish/index skipping, hue normalization, blend-alpha sampling + culling, colour interpolation (incl. the C++'s D3DCOLOR wrap at the -0.5 px edge: 254), fixed-comp quad geometry/UVs, gamma loops, echo layers/flips/mix, echo blend snap switching, filter blend table, hue tint, aspect stretch; main_texture_size (auto/16-snap/stretch/pow2/fixed/clamps).

Mutation checks (each made a test fail, then reverted): dropping the generation bump in resize (resize test), additive gamma passes → Off (gamma test, 64 instead of 128), vertically flipped present blit (orientation test, diff 230), removing the comp grid's -0.5 px offset (orientation test, diff 95), removing the PrepareFor2DDrawing y flip in the fixed comp (orientation test), funky reversal InvAlpha → Alpha (blend test, 192 vs 63).

Gates: `cargo build --workspace` ok; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `MD_REQUIRE_GPU=1 cargo test --workspace` all ok.

## Files changed

- New: crates/md-render/src/{comp.rs, comp_fixed.rs, frame.rs, tex_size.rs}, crates/md-render/tests/frame.rs
- Modified: crates/md-render/src/{renderer.rs, preset_gpu.rs, preset_bind.rs (blur_level pub(crate)), warp.rs, lib.rs}, crates/md-render/Cargo.toml, Cargo.lock

## Self-review findings / concerns

- `RenderSettings::new` now defaults to `TexSize::Auto` (C++ default), so non-multiple-of-16 sizes get 16-snapped textures (100×30 → 112×32); `TexSize::Fixed` keeps exact sizes. All existing tests use multiples of 16.
- Milestone 6: after drawing shapes/waves/sprites into the warp target, regenerate its mips before the composite (D3D9 AUTOGENMIPMAP); 6.1 should set the fixed-comp stage-0 address to clamp when blur passes run; 6.5 owns darken_center in the sprites hook.
- md-core 7.x: call `mesh_mut().compute_uvs(...)` (rep 0, then rep 1 with the blend pattern) before `begin_frame`; after `resize` the mesh is replaced (identity UVs) and the blend pattern size is unchanged (grid from settings). `blend_t` must be `m_fBlendProgress`; `preset.transition` supplies the echo "old" values.
- `begin_frame` swaps the renderer's mesh out with a 1×1 stand-in around `warp_pass` (borrow split); trivial cost.
- `ShaderProgram.bind_group_1` is no longer a public field (accessor `bind_group_1(i)`); `texsize_values` stays the load-time values (the buffer is rewritten on rebind).
- Hue/shade and echo use the renderer's Pcg32-derived `m_fRandStart` (A.4), not the C rand() stream.

## Fix round 1

Changes:
- 8-bit composite (Important): `comp::COMP_FORMAT = Rgba8Unorm`; the comp intermediate (`comp_texture()`, renderer.rs new/resize) and every composite-kind pipeline target it whatever `tex_bits` is: preset comp programs (preset_gpu.rs `PresetLayouts::build` picks the format by `ShaderKind`: Warp → main format, Comp → COMP_FORMAT) and the fixed comp passes (keyed by the intermediate's format). Each composite draw now saturates like the C++'s X8R8G8B8 back buffer. Docs in comp.rs/preset_gpu.rs updated.
- `dynbuf.begin_frame()` moved from `record_comp` to the start of `Renderer::begin_frame`, so Milestone 6 hook vertex writes earlier in the frame are not overwritten.
- `present` asserts the target format is not sRGB (documented `# Panics`; the C++ back buffer is not gamma-encoded).
- `CompFrame.fixed_sampler` is a plain `SamplerKind` (no `Option`/`Default`).
- Tests (crates/md-render/tests/frame.rs):
  - the clear is now meaningful: `two_frames_…` paints the source white BEFORE frame 0 and expects black; `renderer_resize_clamps_…` resizes to the same size, paints white and expects a black frame;
  - `composite_saturates_at_8_bits_with_16_bit_textures`: tex_bits 16, 0.75 grey at gamma 2 then brighten, expected 255 (clamped C++) and not 191 (unclamped); also a preset comp shader drawing into the 8-bit target with 16-bit main textures;
  - `no_shader_echo_zooms_and_flips`: red where u < 0.4, echo alpha 0.5 and zoom 2: orient 0 gives 254 / 127 / 0 at u ≈ 0.1 / 0.34 / 0.9; orient 1 gives 127 / 127 at u ≈ 0.1 / 0.9;
  - `no_shader_filters_follow_d3d9_blending`: d = 64: invert 191, darken 16, solarize 96, brighten 112 (hand-computed with 8-bit rounding per pass).

Mutation checks: comp pipelines/intermediate back on the main format → the 16-bit test fails; first-frame clear disabled → both clear tests fail.

Commands and output:
```
$ MD_REQUIRE_GPU=1 cargo test -p md-render
lib 54 passed; tests/frame.rs 10 passed; all other md-render test binaries ok
$ cargo clippy --workspace --all-targets -- -D warnings   -> clean
$ cargo fmt --check                                        -> clean
```

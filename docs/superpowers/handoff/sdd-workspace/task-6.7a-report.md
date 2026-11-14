# Task 6.7a report: one ortho projection for all fixed-function draws

Status: DONE_WITH_CONCERNS (minor concerns below)

## What was implemented

- **Shared projection, in the fixed vertex shaders.** The four `fixed_*` WGSL shaders (`fixed_color`, `fixed_tex_modulate`, `fixed_tex_modulate_alpha`, `fixed_tex_select`) now apply `d3d9_ortho2d(p) = (p.x, -p.y, p.z, 1)`, which is `D3DXMatrixOrthoLH(2, -2, 0, 1)` with WORLD/VIEW identity (support.cpp:179; textmgr.cpp:238). It runs before `d3d9_raster`: `o.pos = d3d9_raster(d3d9_ortho2d(v.pos))`.
  - I put it in the shader and not in a CPU pass inside `DrawBatch::draw`, because the no-shader warp (warp.rs) uses `PipelineCache::get` directly with its own index buffer and never goes through `DrawBatch::draw`. In the shader, every user of a fixed pipeline gets the flip exactly once, and no vertex copy is needed.
  - `text.wgsl` (`FixedMode::Text`) does not flip. Glyphs come from D3DX's font sprite, which is not the fixed-function transform, and their vertices are built in clip space. `warp_vs`, `comp_vs` and `blur_vs` are unchanged.
- **Half-pixel shift sign.** It is unchanged. D3D9 rasterizes after the projection, and `d3d9_raster` acts in clip space after `d3d9_ortho2d`, so the +0.5 px screen-space (right/down) shift keeps its sign. Two tests pin it:
  - `fixed_quad_lands_on_its_d3d9_pixel`: a quad from D3D9 pixel `c±0.5`, `r±0.5` in pre-projection coordinates lights exactly pixel (2,1) of an 8×8 target.
  - `outer_border_edges_follow_d3d9_pixel_centres`: rows 0..=4 and 92..=95 are lit at a 96 px height and rows 5 and 91 are not; a missing or wrong-signed shift moves both edges by one row.
- **Builders now emit the C++ pre-projection coordinates:**
  - **shapes.rs, custom_waves.rs, overlays.rs (motion vectors, borders, darken_center):** code unchanged, docs corrected. They were already verbatim (mfs:2300, 2613, 1286) and were mirrored only because nothing projected them.
  - **wave.rs:** keeps the C++'s own negation (mfs:3229). I corrected the citation 3173-3182 to 3221-3230.
  - **warp.rs (no-shader warp):** the fixed vertex block now carries `-y`, as the C++ does in WarpedBlit_NoShaders (`tempv[i-1].y *= -1` mfs:1973; also 2014 and 2040). The shader blocks keep the mesh y. This is a separate block, so it does not affect the shader path.
  - **comp_fixed.rs:** removed `-(y*mult)`. The quad is now C++ `v3` verbatim (mfs:4008-4015).
  - **title_mesh.rs:** removed `-y`. The C++'s own "final flip on y" is commented out (mfs:4651-4653).
  - **text_draw.rs:** the box vertices (drawn with `fixed_color`) now use the textmgr pre-projection formula `-1 + 2*y/h` (textmgr.cpp:609-612). The pixels are identical, and the text pixel-position tests pass unchanged. Glyph vertices are untouched.
  - **comp.rs `blit_quad`:** md-render's own copy to the caller's view, rewritten in pre-projection coordinates.
- **Every fixed-pipeline user was audited:**
  - wave_draw, shapes_draw, custom_waves_draw, overlays_draw: no own flip; they now get the shared projection.
  - warp.rs (direct `PipelineCache::get`): fixed block `-y`.
  - comp.rs fixed comp draws, title.rs (anim via TexModulate; the title-texture text uses Text mode, so no flip): handled as above.
  - text_draw boxes and comp.rs blit: rewritten in pre-projection coordinates.
  - preset_gpu.rs: uses only `vs_bind_group`.
  - The sprites hook comment in frame.rs now says to build sprites in pre-projection coordinates.
  - mips.rs and blur use their own pipelines.
  - No users outside md-render (I grepped crates/ and apps/).
- **title.wgsl removed**, with its 5 entries in md-hlsl `tests/builtin_wgsl.rs` (FILES 11→10, VERTEX_FILES 9→8) and its 3 README entries.
- **New md-hlsl test** `fixed_function_vertex_shaders_apply_the_ortho_projection_once`: `ORTHO_FN` appears verbatim exactly in the four fixed shaders, `vs_main` applies it inside `d3d9_raster`, and no other vertex shader mentions it.
- **assets/shaders/README.md:** new "Fixed-function projection (Task 6.7a)" paragraph, a note on the shift order, and the title rows removed. I also updated the pipelines.rs module doc, the `points_to_quads` doc and the fixed_tex_modulate.wgsl user list (now names the title anim).

## Orientation tests (new `crates/md-render/tests/orientation.rs`, 430 lines)

There is one test per draw kind. All but the last two run two frames on a 96×96 renderer and read back main; readback row 0 is the top.

1. `fixed_quad_lands_on_its_d3d9_pixel`: tests the DrawBatch pixel-centre rule after the flip.
2. `wave_mode_6_offset_up_lands_in_the_top_half`: lit rows within 2 of H/4.
   - **Brief deviation:** mode 6's offset comes from **wave_x**, not wave_y (`edge_y = fWavePosX*sinf(ang+1.57)`, mfs:3057-3060, with ang = 1.57*mystery at mfs:3047). The test uses wave_x = 0.75 at mystery 0.
3. `shape_at_y_075_lands_in_the_top_half`: all red rows < H/2, mean ≈ H/4.
4. `custom_wave_at_y_075_lands_in_the_top_half`: rows within 1 of H/4.
5. `motion_vectors_sit_high_and_mv_dy_moves_them_up`: mv_y = 2 gives one row at fy = 0.2, about row 19. With mv_dy = 0.1 the rows are at 0.1 (about row 10) and 0.9. The second row, at fy = 1, was skipped before and now enters, exactly per mfs:1231-1236. The test uses the no-shader copy warp, because mvs are drawn before the warp (mfs:965).
6. `outer_border_edges_follow_d3d9_pixel_centres`: exact pixel columns and rows.
7. `darken_center_is_centred`: the darkest pixel is (W/2, H/2), and the image is symmetric ±1..3 px both ways.
8. `custom_title_at_y_025_lands_in_the_top_half`: a custom-message supertext with fY = 0.25 (`dy = fY*2-1`, mfs:4612-4631) lands in the top half on screen.
9. `warp_and_composite_keep_the_image_upright_on_every_path`: a red-top/blue-bottom image survives zoom 1/rot 0, checked both in main after the warp and on screen. It covers all four warp×comp paths: (shader, shader), (fixed, fixed), (shader, fixed) and (fixed, shader).

## TDD evidence

**RED.** I recorded this before any implementation change, with the tests written first on the unmodified tree (HEAD 0a6fd15):
```
MD_REQUIRE_GPU=1 cargo test -p md-render --test orientation
test fixed_quad_lands_on_its_d3d9_pixel ... FAILED       left: [(2, 7)]  right: [(2, 1)]
test custom_wave_at_y_075_lands_in_the_top_half ... FAILED   [72]
test motion_vectors_sit_high_and_mv_dy_moves_them_up ... FAILED   [77]
test shape_at_y_075_lands_in_the_top_half ... FAILED   [69, 70, 71, 72, 73, 74, 75]
test wave_mode_6_offset_up_lands_in_the_top_half ... FAILED   [72]
test darken_center_is_centred ... ok
test custom_title_at_y_025_lands_in_the_top_half ... ok
test outer_border_edges_follow_d3d9_pixel_centres ... ok
test warp_and_composite_keep_the_image_upright_on_every_path ... ok
test result: FAILED. 4 passed; 5 failed
```
- All 5 fixed-function tests failed, mirrored to the bottom half.
- The 4 that passed are expected regression guards:
  - Title, warp/comp and comp_fixed each flipped y themselves before the change.
  - Borders and darken_center are flip-symmetric, so they match either way; the border test still pins the shift sign.
- The first motion-vector assertion was then corrected for the C++'s second row coming in at mv_dy 0.1, a test-expectation fix that did not change the implementation.

**GREEN:**
- `MD_REQUIRE_GPU=1 cargo test --workspace --no-fail-fast`: 673 passed, 0 failed. This includes all of md-render on lavapipe, the 9/9 orientation tests and apps/milkdrop golden_basic.
- `tests/golden/rust/default_60.png` is unchanged: golden_basic passed with no re-bless, and git shows no change under tests/ or apps/.

**Gate:**
- `cargo build --workspace`: OK
- `cargo clippy --workspace --all-targets -- -D warnings`: clean. I fixed a single_element_loop that the title.wgsl removal created.
- `cargo fmt --check`: clean
- `cargo deny check`: advisories, bans, licenses and sources ok; only the pre-existing "unmatched license allowance" warning remains.
- `scripts/check-no-native-build.sh`: OK

## Existing tests changed, and why

- **`src/comp_fixed.rs` `plain_quad_is_one_pixel_larger_and_upright`:** v3[0] is now `(-1.01, +1.02)` and v3[3] is `(1.01, -1.02)`. They are pre-projection, C++ verbatim (mfs:4008-4015), not clip coordinates.
- **`src/comp_fixed.rs` `aspect_stretches_when_tex_and_window_differ`:** `pos[1] + 2.04` became `- 2.04`, and `+ 1.02` became `- 1.02`, for the same reason.
- **`src/title_mesh.rs` final-pose test:** the expected y went from `-(y0 ± 2/th)` to `y0 ± 2/th`, because the builder no longer negates.
- **`tests/pipelines.rs` `points_become_quads` and `lines_draw_one_pixel_wide`:** the point and line y moved from the clip formula `1 - 2r/H` to the pre-projection formula `2r/H - 1`. They pinned clip-space input, and fixed-function input is now pre-projection; the pixel expectations (rows 7 and 3) are unchanged.
- **`src/warp.rs` `blend_culling_drops_hidden_tiles`:** I added assertions that the fixed block carries `-y` and the shader block `+y`.
- **md-hlsl `tests/builtin_wgsl.rs`:** title.wgsl entries removed, the alpha-op loop collapsed (clippy), and the new ortho test added.

No test pinned the mirrored orientation of shapes, waves, custom waves or overlays: they were all centred or symmetric, as the 6.7 reviewer noted.

## Citations

I verified every C++ line citation I wrote or changed with `sed -n` against code/:
- support.cpp:179
- textmgr.cpp:238 and 609-612
- pluginshell.cpp:1806 and 1840-1848
- milkdropfs.cpp: 965, 971-981, 1231-1236, 1286, 1973, 2014, 2040, 2300, 2613, 3047, 3057-3060, 3221-3230, 3229, 3310-3345, 4008-4015, 4612-4631, 4651-4653

## Files changed

- assets/shaders: fixed_color.wgsl, fixed_tex_modulate.wgsl, fixed_tex_modulate_alpha.wgsl, fixed_tex_select.wgsl, README.md; title.wgsl deleted
- crates/md-hlsl/tests/builtin_wgsl.rs
- crates/md-render/src: comp.rs, comp_fixed.rs, custom_waves.rs, fixed.rs, frame.rs, overlays.rs, pipelines.rs, shapes.rs, text_draw.rs, title_mesh.rs, warp.rs, wave.rs
- crates/md-render/tests: pipelines.rs, orientation.rs (new)

## Self-review and concerns

1. **Brief error on the wave test:** mode 6 is offset by wave_x, not wave_y (mfs:3057-3060). The test follows the C++.
2. **wave.rs is 623 lines,** up from 622; it was already over the ~600 limit before this task (see the 6.2 minor). I did not split it, to keep this fix focused.
3. **Text boxes changed coordinate formula,** from clip to pre-projection. The output is pixel-identical, and the 6.6 pixel tests pass. The dark translucent box uses ortho(w,h) in the C++ (pluginshell.cpp:1806); I documented that it maps to the same pixels.
4. **Textured shapes:** I did not add a dedicated test for the texture-sampling orientation of textured shapes. Their UVs were already C++ verbatim and only the positions moved; the warp/comp test covers texture orientation in general.
5. **Projection lives in WGSL rather than in DrawBatch:** the ruling allows either. I chose WGSL so that warp.rs's direct pipeline use is covered too.

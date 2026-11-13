# Task 6.5 report: motion vectors, borders, darken-center

Status: DONE_WITH_CONCERNS. Gate: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` clean; `MD_REQUIRE_GPU=1 cargo test --workspace` 69 test binaries, 0 failures. `golden_default_preset_frame_60` passes unchanged (no re-bless, tests/golden/rust/default_60.png untouched).

## Implemented
- `src/overlays.rs`: `MotionVectorVars`, `Border` (md-render types, F38; `from_vars` / `Border::outer|inner` read the post-equation `PerFrameVars`, which needed no extension), `MotionMesh`, `reverse_propagate_point`, `build_motion_vectors`, `build_borders`, `build_darken_center`.
- `src/overlays_draw.rs`: `Renderer::draw_motion_vectors`, `draw_sprites_fixed`, `snapshot_mesh_uvs`.
- `src/cast.rs`: `rgba_01_f32` (D3DCOLOR_RGBA_01 on floats through `c_int`; `comp_fixed::rgba_01` saturates with `as`, so not reused).
- Frame loop (frame.rs), C++ order (965, 1054-1057): motion vectors into the warp SOURCE (`other_vs(vs_target)`, the C++'s m_lpVS[0]) before the first-frame clear and the warp; later shapes, custom waves, built-in wave, then darken_center fan, then the two borders into the warp target (`DrawSprites`), then composite.
- Renderer gains `prev_uvs` (see departure 1).

## Pinned from the C++ / departures from the brief
1. Motion vectors read the PREVIOUS frame's mesh UVs: DrawMotionVectors runs at 965, ComputeGridAlphaValues (which rewrites m_verts) at 1006. md-core fills the mesh before `begin_frame`, so `Renderer` snapshots the mesh UVs after drawing the vectors and uses them next frame (initially the identity mesh, reset on resize). `build_motion_vectors` therefore takes a `MotionMesh {grid, uvs, tex}` instead of `&WarpMesh`, and no `aspect` (the C++ uses none; min length is `1/m_nTexSizeX`, used for both axes, 1215-1217). The half-texel offset `0.5/tex` that m_verts carries (1836-1837) but `WarpMesh::uvs()` omits is added per corner on read.
2. Brief test `mv_zero_length_emits_nothing` contradicts the C++: a zero-length vector (mv_l = 0 or identity UVs) is NOT dropped; it becomes `(min_len, min_len)` (1256-1282). Only `mv_a < 0.001` (1159), NaN alpha, or counts `(int)mv_x/mv_y <= 0` (1180) emit nothing. The test keeps its name and pins the real behaviour.
3. Fractional grid counts: `nX=(int)mv_x`, `dx=mv_x-nX` clamped 0..1, widening `f=(i+0.25)/(n+frac+0.25-1)`; caps 64 x 48 zero the fraction (1173-1213, 1231-1247). Integral counts lose their last row/column (f == 1.0 fails `< 0.9999`), e.g. 4 x 3 -> 3 x 2 lines; tested.
4. Blending: one draw from the blended vars. RunPerFrameEquations blends mv_* and ob_*/ib_* by `mix` and darken_center by the snap point (706-733), so nothing is drawn per preset and `old_vars` is unused.
5. `build_darken_center(aspect_y, out)` takes the aspect (x is scaled by `m_fAspectY`, 3328-3340). 6 vertices, fan of 4 triangles, centre alpha byte 23 ((int)(3/32*255)), edges 0, all black.
6. Borders: `a > 0.001f` gate per border (3368); positions/rotation in `float` with `cosf/sinf(1.570796327f)` (= `FRAC_PI_2` f32, clippy requires the const); 4 quads of 4 vertices each, drawn as fans.
7. `ReversePropagatePoint` returns false for out-of-grid cells and the C++ then reads uninitialized `fx2/fy2`; unreachable for points inside (0.0001, 0.9999), so a failure is taken as a zero vector. `(int)` truncates toward zero (fx = -0.1 on a 2-grid is cell 0 with negative fraction, not a failure; tested).
8. Test helpers: preset default `mv_a` is 1 (12 x 9 grid), so `tests/frame.rs::identity_vars` now sets `mv_a = 0` (documented there); without it three frame tests failed (composites showed red/white lines). The new tests' `vars()` zero mv_a, ob_a, ib_a, darken_center and the ob/ib colours (defaults are 0.25 grey).

## TDD
RED (builders stubbed with `todo!()`, GPU hooks present): `MD_REQUIRE_GPU=1 cargo test -p md-render --test overlays` -> `1 passed; 9 failed` (only `overlay_vars_come_from_the_post_equation_values` passes with stubs).
GREEN: same command after restoring the implementation -> `10 passed; 0 failed`. (Intermediate failures after first GREEN attempt were five wrong test expectations of mine, fixed against the C++: inner border colour arithmetic, dy/dx shift counts, truncation of -0.1, ib colours defaulting to 0.25, alpha channel compared.)
Tests: mv_count_matches_grid, mv_zero_length_emits_nothing, outer_border_is_four_quads, darken_center_fan_vertex_count_matches_cpp (brief's four), reverse_propagate_is_the_bilinear_lookup (hand-computed: 4 corners, weights 0.32/0.48/0.08/0.12, tu 0.441, tv 0.346 with half texel 0.005/0.01), overlay_vars_come_from_the_post_equation_values; GPU: borders_land_pixels_in_main, motion_vectors_land_pixels_in_main_before_the_warp (visible with the copying no-shader warp, erased by a source-ignoring warp shader, i.e. pins "before the warp"), darken_center_lands_pixels_in_main, darken_center_draws_after_the_wave_and_before_the_borders (white custom wave stays 255 without the fan and is darkened below 245 with it; ob_size 1 covers the wave; fan-then-border vs border-then-fan arithmetic distinguished at the centre pixel).

## Citations
Every C++ line number written in new code/tests was checked with `sed -n '<N>p' code/vis_milk2/milkdropfs.cpp`; wrong ones I had drafted were corrected (1253-1283 -> 1256-1282, 1431-1470 -> 1431-1466, 1442-1448 -> 1444-1451, 3374-3407 -> 3358-3402, 3396-3405 -> 3391-3399, 3382-3389 -> 3378-3385, 3394 -> 3389, 3367 -> 3368, 3310-3345 -> 3310-3347). Verified: 706, 733, 965, 971, 981, 1006, 1055-1057, 1156, 1159, 1173-1178, 1180, 1206-1213, 1226-1227, 1231-1247, 1301, 1305, 1431, 1454-1464, 1836-1837, 3300, 3310-3314, 3344, 3355-3358, 3376-3378, 3389, 3391. Note: preexisting comp_fixed.rs cites darken_center as 3310-3345 (block really ends 3347); untouched.

## Concerns
- Trailer: used the implementer-rules.md "Claude Opus 5.5" trailer; the session reminder says "Sonnet 5.5". Amend if preferred.
- md-core 7.x must call `mesh_mut().compute_uvs` before every `begin_frame` (as before); the motion vectors lag one frame behind it, as in the C++.
- Default preset (mv_a 1) now draws motion vectors in the default render; golden default_60 still passes within its 0.02 threshold (the golden preset's own values apply).
- Lines are 1 px (LineList), as the C++ draws them; wgpu cannot widen.

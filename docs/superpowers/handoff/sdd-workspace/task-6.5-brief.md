### Task 6.5: Motion vectors, borders, darken-center

**Files:**
- Create: `crates/md-render/src/overlays.rs`

**Interfaces:**
- Produces: `pub fn build_motion_vectors(mv: &MotionVectorVars, mesh: &WarpMesh, aspect, out: &mut Vec<Vertex>)` (grid of `nMotionVectorsX x nMotionVectorsY` lines, each traced backwards through the warp UVs with `ReversePropagatePoint`'s bilinear lookup, length `mv_l`, offset by `mv_dx`, `mv_dy`, color `mv_r/g/b/a`, clipped at the edges), `pub fn build_borders(ob: Border, ib: Border, out)` (outer border `ob_size` fraction of the screen, inner border `ib_size`), `pub fn build_darken_center(out)` (the center-darkening fan from the `darken_center` block in `milkdropfs.cpp`; vertex count, colors, and alphas copied from the C++ with a line reference).

- [ ] **Step 1:** Write failing tests `mv_count_matches_grid`, `mv_zero_length_emits_nothing`, `outer_border_is_four_quads`, `darken_center_fan_vertex_count_matches_cpp`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): motion vectors, borders, darken center`


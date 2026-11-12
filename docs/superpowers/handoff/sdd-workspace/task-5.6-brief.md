### Task 5.6: Warp mesh and warp pass

**Files:**
- Create: `crates/md-render/src/{mesh.rs,warp.rs,blendpattern.rs}`

**Interfaces:**
- Produces: `pub struct WarpMesh { pub fn new(grid_x, grid_y, aspect_x, aspect_y) -> WarpMesh; pub fn vert_info(&self) -> &[VertInfo]; pub fn compute_uvs(&mut self, per_frame: &PerFrameVars, per_pixel: Option<&mut PerPixelRunner>, time: f32, blend: Option<(&[VertInfo], f32)>) }` implementing the pinned math; `PerPixelRunner` is a closure type `FnMut(&mut PerPixelVars)` supplied by `md-core` (so `md-render` does not depend on `md-eval`); `pub fn randomize_blend_pattern(rng: &mut Pcg32, grid_x, grid_y, out: &mut [VertInfo])` implementing types 1..3 and `gen_plasma`; `Renderer::warp_pass(&mut self, ctx, preset_gpu, old_preset_gpu, mesh, blend_t)` drawing into `VS[0]` from `VS[1]` (pass 0 old shader, pass 1 new shader with alpha when blending), then generating mips.

- [ ] **Step 1:** Write failing tests `uv_of_center_vertex_is_half_half_at_identity`, `zoom_scales_uv_about_center`, `dx_shifts_u_negative`, `rot_rotates_about_cx_cy`, `blend_mix_uses_a_t_plus_c`, `plasma_pattern_is_normalized_to_unit_range`, and GPU test `identity_warp_preserves_image` (upload a checkerboard, warp with identity, read back, DSSIM < 0.001).
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): warp mesh and warp pass`


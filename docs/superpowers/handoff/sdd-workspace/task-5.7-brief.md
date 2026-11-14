### Task 5.7: Composite pass and headless frame

**Files:**
- Create: `crates/md-render/src/{comp.rs,comp_fixed.rs,renderer.rs}`

**Interfaces:**
- Produces: `Renderer::new`, `begin_frame`, `present`, `resize` as in A.3 with the frame order: clear `VS[0]` if the preset asked (`m_bNeedRescan`-style first frame), warp pass, blur passes (Task 6.1, stubbed as no-op here), shapes/waves/wave/sprites (Milestone 6, no-ops here), composite pass into `target` with the `32x24` hue grid, swap ping-pong. `comp_fixed.rs` holds the no-shader composite with the echo/gamma/brighten/darken/solarize/invert/darken_center passes and their blend table.

- [ ] **Step 1:** Write failing GPU tests `two_frames_of_default_preset_produce_decaying_image` (frame 1 after painting white into VS: mean brightness of frame 2 is lower by about `0.004 * 255` per the default warp shader), `comp_grid_has_768_vertices`, `no_shader_path_applies_gamma_passes` (gamma 2.0 doubles brightness of a 0.25 grey image, clamped).
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): composite pass and frame loop`


### Task 7.2: Per-frame equations and blending of outputs

**Files:**
- Create: `crates/md-core/src/{per_frame.rs,per_pixel.rs}`

**Interfaces:**
- Produces: `pub fn run_per_frame(state: &mut PresetState, inputs: &FrameInputs) -> PerFrameVars` (loads inputs, resets `q`, runs, clamps, copies to `pv`), `pub fn blend_outputs(new: &mut PerFrameVars, old: &PerFrameVars, progress: f32)`, `pub fn per_pixel_runner(state: &mut PresetState) -> impl FnMut(&mut PerPixelVars)` for `WarpMesh::compute_uvs`; `FrameInputs { time, fps, frame, progress, sound: &SoundData, mesh: (u32,u32), pixels: (u32,u32), aspect: (f32,f32) }`.

- [ ] **Step 1:** Write failing tests `q_resets_to_init_each_frame`, `per_frame_can_modify_zoom`, `echo_zoom_is_clamped`, `blend_at_half_is_midpoint_for_floats`, `booleans_switch_at_half`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): per-frame equations and output blending`


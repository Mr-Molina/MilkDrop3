### Task 7.3: Waves and shapes evaluation

**Files:**
- Create: `crates/md-core/src/{waves.rs,shapes.rs}`

**Interfaces:**
- Produces: `pub fn eval_custom_waves(state: &mut PresetState, sound: &SoundData, inputs: &FrameInputs) -> Vec<CustomWaveDraw>` (per enabled wave: run per-frame, compute inputs via `md_render::custom_wave_inputs`, run per-point for each sample, collect points) and `pub fn eval_custom_shapes(..) -> Vec<ShapeFrameVars>` (per enabled shape, per instance `0..num_inst`, run per-frame with `instance` set).

- [ ] **Step 1:** Write failing tests `disabled_wave_produces_nothing`, `per_point_sees_sample_in_unit_range`, `shape_instances_see_instance_index`, `shape_per_frame_can_change_color`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): custom wave and shape evaluation`


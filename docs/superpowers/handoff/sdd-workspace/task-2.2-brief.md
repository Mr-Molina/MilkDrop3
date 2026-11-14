### Task 2.2: Blendable values

**Files:**
- Create: `crates/md-preset/src/blend.rs`

**Interfaces:**
- Produces: `Blendable` as in A.3 with `pub fn set(&mut self, v: f32)` (no blend), `pub fn blend_to(&mut self, from: f32, to: f32, start: f32, end: f32)`, `pub fn eval(&self, time: f32) -> f32` returning `start + (end-start) * clamp((time-start_time)/(end_time-start_time), 0, 1)`, matching `CBlendableFloat::eval` in `state.cpp`.

- [ ] **Step 1:** Write failing tests `eval_before_start_returns_start`, `eval_after_end_returns_end`, `eval_midway_is_linear`, `zero_duration_returns_end`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(preset): blendable floats`


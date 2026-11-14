### Task 6.4: Custom waves

**Files:**
- Create: `crates/md-render/src/custom_waves.rs`

**Interfaces:**
- Produces: `pub struct CustomWaveFrame { samples: u32 /* clamped to 512 */, sep, scaling, smoothing, r, g, b, a, spectrum: bool, use_dots, thick, additive }`, `pub fn custom_wave_inputs(f: &CustomWaveFrame, sound: &SoundData) -> Vec<(f32 /* value1 */, f32 /* value2 */, f32 /* sample 0..1 */)>` computing the smoothed, scaled source values: `mult = (spectrum ? 0.15 : 0.004) * scaling * wave_scale`, left/right offsets by `sep`, two-pass smoothing as the C++; then `md-core` runs the per-point program for each sample producing `(x, y, r, g, b, a)` and `build_custom_wave(points, use_dots, thick, out)`.

- [ ] **Step 1:** Write failing tests `samples_clamped_to_512`, `spectrum_uses_0_15_multiplier`, `sep_offsets_right_channel`, `thick_quadruples`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): custom waves`


### Task 6.1: Blur chain

**Files:**
- Create: `crates/md-render/src/blur.rs`

**Interfaces:**
- Produces: `Renderer::blur_passes(&mut self, ctx, preset: &Preset, highest_used: u8)` per the pinned constants; `highest_used` comes from scanning the preset's shader text for `GetBlur3`/`sampler_blur3` (3), `blur2` (2), `blur1` (1), else 0, computed in `load_preset` and stored on `PresetGpu`.

- [ ] **Step 1:** Write failing GPU tests `blur_texture_sizes_halve_per_level_and_are_even`, `blur_of_constant_image_is_constant_after_scale_bias`, `no_passes_when_unused`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): blur chain`


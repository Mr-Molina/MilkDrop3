### Task 3.2: Band analysis

**Files:**
- Create: `crates/md-analysis/src/{analyzer.rs,lib.rs}`

**Interfaces:**
- Produces: `Analyzer`, `SoundData` as in A.3, where `imm`, `avg`, `long_avg`, `imm_rel`, `avg_rel` follow the `mysound` rules above, `spectrum` and `wave` carry the shell's values, and `pub fn adjust_rate_to_fps(rate: f32, fps1: f32, fps: f32) -> f32`.

- [ ] **Step 1:** Write failing tests `silence_gives_rel_values_of_one` (after 100 frames of all-zero bytes, `imm_rel == [1,1,1]`), `bass_tone_raises_band_zero_only`, `long_avg_uses_0_9_for_first_50_frames`, `adjust_rate_to_fps_identity_at_reference_fps`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement; frame counter lives in `Analyzer`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(analysis): band energies and attenuation`


### Task 2.5: Corpus parse test and preset blending

**Files:**
- Create: `crates/md-preset/tests/corpus.rs`, `crates/md-preset/src/transition.rs`

**Interfaces:**
- Produces: `Preset::start_blend(&mut self, from: &Preset, now: f32, duration: f32)` which, for each blendable field, calls `blend_to(from.value, self.value, now, now + duration)`; non-blendable fields switch immediately (as `CState::StartBlendFrom`).

- [ ] **Step 1:** Write failing tests `every_corpus_preset_parses_without_error`, `blend_midpoint_is_average_for_blendable_fields`, `non_blendable_fields_switch_immediately`.
- [ ] **Step 2:** Run `cargo test -p md-preset`. Expected: FAIL.
- [ ] **Step 3:** Implement `transition.rs`.
- [ ] **Step 4:** Run the tests. Expected: PASS, corpus at 100%.
- [ ] **Step 5:** Commit: `feat(preset): blending and corpus parse test`

## Milestone 3: `md-analysis` and `md-audio`

Pinned facts from `fft.cpp`, `pluginshell.cpp::AnalyzeNewSound`, and `plugin.cpp::DoCustomSoundAnalysis`: input is 576 unsigned bytes per channel; each is converted with `((b ^ 128) - 128) as f32`, giving a signed waveform in `[-128, 127]`; the shell's FFT input is the two-tap average `0.5 * (w[i] + w[i-1])` (with `w[-1] = w[0]`), 576 samples in, 512 bins out, equalize on, envelope power 1.0; the envelope is `0.5 + 0.5 * sin(i * 2π/576 - π/2)`; the equalize table is `-0.02 * ln((512 - i) / 512)`; the bit-reversal and butterfly are the radix-2 FFT `fft.cpp` performs on `NFREQ = 1024` points with the 576 envelope-weighted input samples zero-padded, and the output magnitude scaling is whatever that code produces (the implementer confirms against `time_to_frequency_domain` and pins it in a test). The **preset-facing** values `bass`, `mid`, `treb`, `*_att` come from the plugin's second analysis (`mysound`): an unsmoothed FFT of the left channel only, 512 bins, bands summed over bins `[0,85)`, `[85,170)`, `[170,256)`, `avg` mixed with rate 0.2 (rising) or 0.5 (falling) adjusted from 30 fps, `long_avg` mixed with 0.9 for the first 50 frames then 0.992, `imm_rel = imm / long_avg` (1.0 when `|long_avg| < 0.001`), `avg_rel = avg / long_avg`. `AdjustRateToFPS(rate, fps1, fps)` is `rate ^ (fps1 / fps)`.


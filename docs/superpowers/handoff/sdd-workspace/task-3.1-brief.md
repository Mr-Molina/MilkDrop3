### Task 3.1: FFT

**Files:**
- Create: `crates/md-analysis/src/fft.rs`

**Interfaces:**
- Produces: `pub struct Fft; impl Fft { pub fn new(samples_in: usize, samples_out: usize, equalize: bool, envelope_power: f32) -> Fft; pub fn transform(&self, wave: &[f32], spectrum_out: &mut [f32]) }` with the same table construction as `fft.cpp` (implemented from the formulas above, not transliterated).

- [ ] **Step 1:** Write failing tests `single_bin_sine_peaks_at_expected_bin` (a 44100/1024*k Hz sine of amplitude 100 over 576 samples, envelope off, equalize off, peaks at bin k), `envelope_table_endpoints_are_zero`, `equalize_table_first_entry_is_zero_and_last_is_positive`, `output_is_deterministic`.
- [ ] **Step 2:** Run `cargo test -p md-analysis fft`. Expected: FAIL.
- [ ] **Step 3:** Implement with precomputed bit-reversal and cos/sin tables; no external FFT crate (keeps the output shape identical to the C++ including its magnitude scaling).
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(analysis): FFT with envelope and equalize tables`


# Tasks 3.1 + 3.2 report (md-analysis: FFT, band analysis)

Status: DONE_WITH_CONCERNS (concerns: AlignWaves scope addition, med_avg semantics. See the end.)
Commits: `ed1df4b feat(analysis): FFT with envelope and equalize tables`, `b1835c5 feat(analysis): band energies and attenuation`.
C++ references use `<file>:<line>@6f7cd5d` (F61).

---

## Task 3.1: FFT (`ed1df4b`)

### Implemented
`crates/md-analysis/src/fft.rs`: `Fft::new(samples_in, samples_out, equalize, envelope_power)` and `Fft::transform(&self, wave, spectrum_out)`, plus `envelope()` and `equalize()` accessors that return `Option<&[f32]>`. These are built from the formulas in `fft.cpp:61-318`:
- NFREQ = 2·samples_out (must be a power of two). The bit-reversal permutation is computed directly, and a test checks it against a literal run of `InitBitRevTable`'s swap loop.
- Per stage there is one `(cos θ, sin θ)` pair with θ = −2π/dftsize (same as `InitCosSinTable`). The butterflies use the same f32 twiddle recurrence `wr = wr·wpr − wi·wpi; wi = wi·wpr + wtemp·wpi` and the same operation order as the C++.
- Input: `temp1[i] = wave[bitrev[i]]·env[bitrev[i]]` when `bitrev[i] < samples_in`, otherwise 0. So the 576 samples go in bit-reversed order, zero-padded to 1024.
- Output: `|X[k]|` for k < NFREQ/2, multiplied by `equalize[k]` when equalize is on. **There is no normalisation.** That is the C++ magnitude scaling.
- Envelope: `0.5 + 0.5·sin(i·(1/n·6.2831853f) − 1.5707963268f)`, with `powf(power)` unless power == 1. `envelope_power <= 0` disables it, as `fft.cpp:77` does. The C++ literals round to the same f32 as `TAU` and `FRAC_PI_2` (I checked the bit patterns), so the constants are used as-is.
- Equalize: `-0.02·ln((512−i)·(1/512))`.
- No external FFT crate. The per-call temporary buffers are local `Vec`s, so `transform` takes `&self` as the brief requires.

### Magnitude pin (derived independently)
For x[n] = A·sin(2πkn/1024), n = 0..575, with no envelope and no equalize, the zero-padded DFT is
X[k] = Σ A·sin(θn)e^{−iθn} = (A/2i)·(576 − Σ_{n<576} e^{−2iθn}), where θ = 2πk/1024.
The geometric sum is 0 when r^576 = 1 and r ≠ 1, with r = e^{−4πik/1024}. That holds when 9k/8 is an integer, i.e. when k is a multiple of 8. Then **|X[k]| = A·576/2 = 28800 for A = 100**. A float64 direct DFT in Python gives 28800.0 for k = 8, 32, 96 and 200. The test `single_bin_sine_peaks_at_expected_bin` checks the argmax and the value 28800 (relative tolerance 1e-4) for those four k.

A second pin: a constant input of 1.0 with the envelope on gives X[0] = Σ sin²(πn/576) = 288 exactly (Python: 288.0). This is test `dc_with_envelope_sums_the_envelope`.

Cross-check, not committed: an O(N²) float64 Python DFT with the envelope and equalize on, run on a 576-sample pseudo-random wave, matches the Rust output over all 512 bins. The maximum relative error is 6.2e-5, which is f32 rounding. Script: `/tmp/claude-0/-home-user-MilkDrop3/b99d8dcb-75d8-5260-8685-ad40ae1bbc68/scratchpad/fft_model.py`.

### F65
`envelope_table_endpoints_are_zero` is renamed `envelope_table_first_entry_is_zero_and_last_is_sin_squared`. It asserts:
- `|env[0]| < 1e-6`
- `env[575] < 1e-4`
- `env[575] ≈ 2.97475e-5` (±1e-7). This is sin²(π/576) = 2.974749992e-5 (Python).
- `env[288] ≈ 1`
- power 2 gives the squared value
- power −1 and power 0 give no table

### Tests (7)
`single_bin_sine_peaks_at_expected_bin`, `envelope_table_first_entry_is_zero_and_last_is_sin_squared`, `equalize_table_first_entry_is_zero_and_last_is_positive` (eq[0] == 0, eq[511] = 0.02·ln 512 = 0.1247665, strictly increasing), `equalize_multiplies_magnitude`, `dc_with_envelope_sums_the_envelope`, `bit_reversal_matches_the_swap_loop`, `output_is_deterministic`.

### TDD evidence
RED, with a stub `Fft` whose `transform` does nothing: `cargo test -p md-analysis fft`
```
test result: FAILED. 0 passed; 6 failed; ...
```
(`bit_reversal_matches_the_swap_loop` was added after GREEN.)
GREEN: `cargo test -p md-analysis fft` gave `test result: ok. 6 passed`. With the bit-reversal test it gave `7 passed`.
Gates: `cargo fmt --check` OK, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo build --workspace` OK, `cargo test --workspace` all green.

---

## Task 3.2: Band analysis (`b1835c5`)

### Implemented
- `src/analyzer.rs`: `SoundData` (the A.3 fields, `Default`), `Analyzer::{new, analyze}`, `Default for Analyzer`, `pub fn adjust_rate_to_fps`.
- `src/align.rs`: a crate-private `WaveAligner`, which ports `AlignWaves` (see the C++ divergences below).
- `src/lib.rs` (Modify, F9): adds the modules and re-exports `Analyzer`, `SoundData`, `adjust_rate_to_fps`, `Fft`.

`analyze(left, right, fps)` follows the C++ frame order (`pluginshell.cpp:1322-1326`, then `plugin.cpp:3985`):
1. **AnalyzeNewSound** (`pluginshell.cpp:1653-1754`). `wave[ch][i] = ((b^128)−128) as f32`. The FFT input is `0.5·(w[i] + w[i−1])` with w[−1] = w[0]. `spectrum[ch]` = shell FFT (576→512, equalize on, envelope 1.0) of that damped, **unaligned** wave. Both channels are computed.
2. The shell's own band averages, used only for `med_avg` (see below).
3. **AlignWaves** (`pluginshell.cpp:2342-2488`) shifts `wave` in place.
4. **DoCustomSoundAnalysis** (`plugin.cpp:8720-8780`):
   - The preset FFT runs on the **aligned, undamped left** wave.
   - `imm[i]` is the plain sum over bins [0,85), [85,170), [170,256).
   - `avg` uses rate 0.2 when `imm > avg`, otherwise 0.5, adjusted from 30 fps.
   - `long_avg` uses 0.9 while `frame < 50`, otherwise 0.992, adjusted from 30 fps. The frame counter is kept in `Analyzer`; the first call is frame 0, matching `GetFrame()` before `m_frame++`.
   - `imm_rel = imm/long_avg` and `avg_rel = avg/long_avg`, each 1.0 when `|long_avg| < 0.001`.
   - Initial `mysound` state is all zero (`plugin.cpp:1064` memset).

### Where the C++ differs from milestone-3-context.md or the brief (C++ followed)
1. **The preset FFT has equalize and the envelope ON.** The context says only "an unsmoothed FFT". `myfft.Init(576, MY_FFT_SAMPLES, -1)` (`plugin.cpp:1063`) passes −1 as `bEqualize`, which is truthy (`fft.h:38`: `Init(int samples_in, int samples_out, int bEqualize=1, float envelope_power=1.0f)`). So `envelope_power` stays 1.0. The −1 was evidently meant to disable the envelope, but in the C++ it does not. Consequence: low bins are strongly attenuated (eq[16] ≈ 6e-4), so `bass` reacts weakly to very low tones. This is real MilkDrop behaviour. Test: `preset_bands_sum_the_undamped_aligned_left_fft`.
2. **The preset FFT input is the AlignWaves-shifted left wave.** `DoCustomSoundAnalysis` copies `m_sound.fWaveform` after `AlignWaves()` has run (`pluginshell.cpp:1323-1324`; the analysis is called from `DrawAndDisplay`, `plugin.cpp:3985`). AlignWaves is active because `NUM_WAVEFORM_SAMPLES = 480 < 576` (`defines.h:188`), and it zeroes samples 480..575 after any non-zero shift. Neither the context nor the brief mentions it, and no other M3 task covers it, so I ported it (see the concerns).
3. **`AdjustRateToFPS`** is `powf(powf(rate, fps1), 1/fps)`, two f32 steps (`utility.cpp:80-90`). The context writes it as `rate^(fps1/fps)`. That is mathematically equal but rounds differently, so the two-step form is used. The identity holds to within 1e-6, not exactly. Test: `adjust_rate_to_fps_identity_at_reference_fps`.
4. **Reference fps.** The preset-facing `mysound` rates use 30 fps (`plugin.cpp:8758,8765`), which matches the context. The shell's own `m_sound` rates use **14 fps** (`pluginshell.cpp:1736-1748`); only `med_avg` uses them here.
5. **`SoundData.spectrum`** is the shell's `m_sound.fSpectrum`: the two-tap-damped, unaligned input with equalize and envelope on. **`SoundData.wave`** is `m_sound.fWaveform` *after* alignment, which is what custom waves (`milkdropfs.cpp:2556-2557`) and the main wave (via `mysound.fWave`, `plugin.cpp:8722`) read. So the spectrum is computed before alignment and the wave is taken after it. The render-time wave smoothing (`milkdropfs.cpp:851-858`) belongs to the renderer and is not applied here. Test: `spectrum_is_the_shell_fft_of_the_damped_unaligned_input`.
6. **`med_avg`**: `td_mysounddata` (`plugin.h:55-63`) has **no** `med_avg`. The only `med_avg` in the C++ is the shell's per-channel `m_sound.med_avg` (`pluginshell.cpp:1742-1749`). It is initialised to 1.0 (`pluginshell.cpp:931`) and mixed with 0.91 adjusted from 14 fps, over the shell's pitch-spaced bands: bins (int)(512·200·m^i/11025), giving [9,35), [35,134), [134,512), with mean/bin divided by 0.326781557 / 0.380873770 / 0.199888934. **Nothing in the plugin reads it** (grep: there are no consumers of `m_sound.imm/avg/med_avg/long_avg`). `SoundData.med_avg` carries the left-channel value of that quantity. Tests: `med_avg_starts_at_one_and_decays_at_0_91_per_frame_at_14_fps` and the unit test `shell_band_ranges_are_pitch_spaced`. The last edge evaluates to exactly 512 in f32 here. If MSVC's `powf` rounded below 512, the C++ range would be [134,511). That has no effect on anything observable.
7. Band bins [0,85), [85,170), [170,256) and `imm_rel = 1` when long_avg is tiny both match the context exactly. The "first 50 frames" means frames 0..49 inclusive.

### AlignWaves port (`align.rs`)
- 6 octaves (floor(log2 96)). spls = 576…18 and space = 96…3, each halved per octave.
- Pyramid weights: `clamp((pyr−0.8)·5+0.8, 0, 1)` with `pyr = 2n/compare`. They are computed once in `new()`; the C++ uses lazily initialised statics with the same values.
- Coarse-to-fine search over offsets, weighted absolute error, first strict minimum.
- History is the **unshifted** previous wave plus its offset. The shift and the zeroing of 480..576 happen only when offset > 0.
- `temp_old[0][480..]` is uninitialised in the C++. It is only ever read through zero weights, so zero-filling it is equivalent.
- **Independent cross-check, not committed:** a literal Python float64 transliteration of `AlignWaves` (`/tmp/claude-0/-home-user-MilkDrop3/b99d8dcb-75d8-5260-8685-ad40ae1bbc68/scratchpad/align_model.py`) was run on 40 frames × 2 channels of tone+noise input from `/tmp/claude-0/-home-user-MilkDrop3/b99d8dcb-75d8-5260-8685-ad40ae1bbc68/scratchpad/gen.py`. The offsets covered 0..95. The aligned samples [0] and [100] matched the Rust `Analyzer` output on 160/160.
- Known C++ behaviour that is mirrored: white noise decorrelates at the 32× coarse octave, so the search can miss the true shift (for example 65 instead of 95). The tests therefore use smooth signals.

### Tests
Integration tests (`tests/analyzer.rs`, 11):
- `silence_gives_rel_values_of_one` (100 + 1 frames of zero bytes: imm_rel = avg_rel = [1,1,1], imm = long_avg = 0, spectrum and wave all 0)
- `bass_tone_raises_band_zero_only` (bin-40 tone: imm[0] > 50 and more than 5× the other bands for 10 frames; a bin-120 control raises band 1)
- `long_avg_uses_0_9_for_first_50_frames` (80 frames at 45 fps, reconstructed exactly from the returned `imm` with `==`)
- `avg_rises_at_0_2_and_falls_at_0_5_from_30_fps`
- `adjust_rate_to_fps_identity_at_reference_fps`
- `bytes_convert_as_signed_8_bit` (200→−56, 127→127, 128→−128, 255→−1)
- `spectrum_is_the_shell_fft_of_the_damped_unaligned_input`
- `preset_bands_sum_the_undamped_aligned_left_fft`
- `a_repeated_frame_aligns_to_the_same_window`
- `med_avg_starts_at_one_and_decays_at_0_91_per_frame_at_14_fps`
- `analysis_is_deterministic`

Unit tests: `align::tests::{tables_match_the_cpp_geometry, finds_a_known_shift_against_the_previous_frame (d = 0,1,7,20,63,95), uses_the_previous_offset_into_the_history}` and `analyzer::tests::shell_band_ranges_are_pitch_spaced`.

All tests use an explicit fps. There is no randomness and no wall clock.

### TDD evidence
RED, with stub `Analyzer` / `adjust_rate_to_fps` (returning zeros): `cargo test -p md-analysis --test analyzer`
```
test avg_rises_at_0_2_and_falls_at_0_5_from_30_fps ... FAILED
test adjust_rate_to_fps_identity_at_reference_fps ... FAILED
test bass_tone_raises_band_zero_only ... FAILED
test bytes_convert_as_signed_8_bit ... FAILED
test long_avg_uses_0_9_for_first_50_frames ... FAILED
test silence_gives_rel_values_of_one ... FAILED
test spectrum_is_the_shell_fft_of_the_damped_unaligned_input ... FAILED
test result: FAILED. 4 passed; 7 failed
```
Four tests passed vacuously on the all-zero stub (med_avg, preset-bands, determinism, and the first alignment test). The align unit tests were written together with `align.rs`, not before it.

During GREEN, two first-draft tests encoded wrong expectations and were corrected:
- The bass margin was too large: equalize plus the alignment-tail zeroing leak about 1/7 of the energy, so the margin is now 5×.
- A white-noise repeated-frame premise did not hold; the test now uses a smooth signal.

GREEN: `cargo test -p md-analysis` gave lib `11 passed` and `tests/analyzer.rs` `11 passed`.
Gates: `cargo fmt --check` OK, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo build --workspace` OK, `cargo test --workspace` 223 passed / 0 failed.

### Files
- `crates/md-analysis/src/fft.rs` (266 lines, new)
- `crates/md-analysis/src/align.rs` (200, new)
- `crates/md-analysis/src/analyzer.rs` (154, new)
- `crates/md-analysis/src/lib.rs` (modified)
- `crates/md-analysis/tests/analyzer.rs` (242, new)

No new dependencies, no dependency on md-audio, no `unsafe`.

### Self-review notes
- `tests/analyzer.rs` has a file-level `#![allow(clippy::needless_range_loop)]` because the band loops index four parallel arrays.
- `fps == 0` gives rates of 0 through `powf(·, inf)`, as in the C++. There is no special case.
- The FFT allocates two 1024-float `Vec`s per call, four per frame. This is negligible, but could move to scratch buffers if profiling ever shows it.
- The C++ uses MSVC `sinf`/`cosf`/`logf`/`powf` (possibly x87 extended precision). Rust uses the platform libm. Expect ulp-level differences only.

## Concerns for the controller
1. **AlignWaves is outside the brief's file list** (new `align.rs`). It is required for C++ parity of `bass/mid/treb` and of the wave that custom waves and the main wave see. Without it, both `imm` and `wave` differ from the C++ on every frame with a non-zero shift. Please confirm this placement as a ruling: md-analysis owns alignment, and `SoundData.wave` is post-alignment.
2. **`med_avg`** carries the shell's left-channel quantity, which has no C++ consumer. If the controller prefers it removed from A.3 or set to the `mysound` scale, that is a small change.
3. **The equalize-on preset FFT** (finding 1 above) is a C++ quirk now baked in. Later milestones should not "fix" it.

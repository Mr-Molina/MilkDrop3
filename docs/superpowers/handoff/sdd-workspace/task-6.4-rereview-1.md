### Finding Verdicts
- **[Important] Sample cap must mirror the C++** - ADDRESSED. custom_waves.rs:54 `n = c_int(f.samples).min(512)` matches mfs:2548-2549. `max_samples` (512 spectrum / 480 waveform, as mfs:2520) is used only for j0/j1 (diff lines 64-65, `(max_samples - n) / 2 - sep / 2`, i32 `/` truncates toward zero like C; no floor or unsigned math; j1 line untouched) and the spectrum stride `t = (512 - sep) as f32 / n as f32` (mfs:2562). Zero stand-in: `fetch` -> `fetch_valid(..., max_samples)` returns 0.0 unless idx in 0..max_samples (480 waveform / 512 spectrum); it covers every read: temp[.][0] (j0/j1), forward-pass reads for both channels, and no other source reads exist (backward pass reads temp only). Doc comment no longer claims UB. Test renamed `samples_clamped_to_512_with_cpp_window` with 1000->512 (both kinds), 512->512, 480, 300.9 cases; new `waveform_window_follows_the_cpp_with_zero_outside` covers n in {512,481,480} with odd/even sep (j0 = -16 - sep/2 for 512; 481 gives (-1)/2 = 0 in Rust and C). Smoothing 0 gives mix1 = 0, so values pass through and the oracle is valid.
- **[Folded] Dead `let _: Option<Vertex>` line and unused import** - ADDRESSED. Both removed (tests/custom_waves.rs import list and the line after `assert!(out.is_empty())`).

### New Breakage in the Fix Diff
None. Report names the covering tests and gives the gate output (fmt, clippy -D warnings clean, 629 passed, golden unchanged); consistent with the diff.

### Out-of-Scope Observations
None.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage.

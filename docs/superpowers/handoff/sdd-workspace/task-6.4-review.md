# Task 6.4 review (ad1572c..a875ab3, custom waves)

### Spec Compliance
✅ custom_waves.rs:55-104 `custom_wave_inputs`: mult = (0.15|0.004)*scaling*wave_scale in f32 (mfs:2555); j0/j1 = (max-n)/2 ∓ sep/2, spectrum 0/0 (2560-2561); t = (512-sep)/(float)n or 1 (2562); mix1 = powf(smoothing*0.98, 0.5) (2563); forward loop from j=1 with `(int)(j*t)+j0` via c_int (2566-2572); backward loop n-2..0 (2574-2578); scale (2580-2584); sample = j*(1/(float)(n-1)), n=1 dot gives NaN as in C++ (2592-2595). Checked line by line against sed output.
✅ n >= 2 or (dots and n >= 1) gate (2551); `(int)` of samples via c_int, NaN/out-of-range gives INT_MIN and nothing drawn (2548).
✅ fetch_flat (custom_waves.rs:108-115): N = 576 for the wave matches `float fWaveform[2][576]` (pluginshell.h:59), so ch1 reads at negative j1 land in fWaveform[0][560..] as in C; spectrum 2x512 (pluginshell.h:60).
✅ build_custom_wave: positions (float)(x*2-1)*invAspectX, (y*-2+1)*invAspectY (2612-2613); colour bytes `(int)(c*255[*alpha_mult]) & 0xFF` in double (2615-2619); SmoothWave only for lines (2639-2643) with in-between points taking vi[i].Diffuse (2487, `verts[k/2]`); thick = 4 cumulative copies +x, +y, -x, lines only (2654-2665); dot size (texX>=1024?2:1)+thick (2650), reset to 1 after (2669-2671).
✅ Blend: rep 0 = new with alpha_mult = m_fBlendProgress, rep 1 = old with 1 - progress, only while blending, old after new (2507-2513 verified); SRCALPHA/ONE additive else SRCALPHA/INVSRCALPHA (2646-2648; blend.rs:82-83 maps AlphaAdditive/Alpha correctly); untextured (SetTexture NULL, FixedMode::Color).
✅ Draw order shapes -> custom waves -> built-in wave: mfs:1054-1056 (DrawCustomShapes; DrawCustomWaves; DrawWave), frame.rs draw_shapes/draw_custom_waves/draw_wave. No mip regeneration (6.1a).
✅ F38: CustomWaveDraw in md-render, FrameInput.custom_waves/old_custom_waves default empty (frame.rs).
✅ Every `(int)` site uses c_int from the new shared cast.rs; shapes.rs now imports it (no duplicate).
✅ Brief tests present (samples_clamped_to_480_or_512 per F41 rename, spectrum_uses_0_15_multiplier, sep_offsets_right_channel, thick_quadruples) plus a hand-computed smoothing test whose numbers I re-derived (1.546875, 2.09375, 2.6875, 3.125).
✅ Field departure is right: the sample setup (2548-2595) reads only samples, sep, scaling, smoothing, bSpectrum, bUseDots and m_fWaveScale.eval(-1) (2555; state.h:86 eval(<0) = unblended val). r/g/b/a are read only as per-point start values (2603-2606, md-core's job); thick/additive only at draw time (2647, 2650, 2654) and live on CustomWaveDraw. Taking `&CustomWaveDraw` + view in build_custom_wave is needed for aspect/tex/alpha_mult.
✅ Citations: sed-checked every cited line (2487, 2496, 2507-2513, 2520, 2523, 2548-2551, 2555, 2560-2563, 2566-2572, 2574-2584, 2592-2595, 2598-2619, 2639-2650, 2654-2665, 2669-2671, 2680, 1054-1056, 2304-2313, state.h:86): all match.
⚠️ F41 cap applied per the Task 6.4 Ruling (custom_waves.rs:56-61), but the ruling's UB rationale does not hold up against the C++ buffers (Important #1).

### Strengths
- Faithful transcription down to f32/f64 evaluation order, C integer division and (int) chop; NaN/INT_MIN paths tested.
- fetch_flat models the real float[2][N] layout instead of clamping; GPU tests cover blend progress 0/0.5/1, not-blending, additive vs alpha, thick lines and dots.
- c_int/color_byte factored into cast.rs rather than copied.

### Issues
#### Critical
None.
#### Important
1. (ruling-mandated, escalate) custom_waves.rs:56-61 + test samples_clamped_to_480_or_512: the 480 cap's stated basis is wrong. (a) n = 481 is not UB: (480-481)/2 = 0 in C, j0 = 0, last read fWaveform[0][480], in bounds. Reads before fWaveform start at n >= 482. (b) Negative j0 is not specific to n > 480: any sep with sep/2 > (480-n)/2 does it (n = 480, sep = 2 gives j0 = -1), and the implementation already handles that with fetch_flat's zero stand-in, which would cover n = 482..512 equally. (c) The cost the ruling records ("presets asking for 481-512 waveform samples draw 480") understates it: m_wave[i].samples defaults to 512 (state.cpp:596), var_pf_samples is loaded from it each frame (mfs:2462), and 104 of the repository's 117 `wavecode_N_samples=` lines are 512. So nearly every non-spectrum custom wave draws 480 points from a different data window (j0 = 0 instead of -16) than the C++. Recommend re-ruling: n = min(512, (int)samples) as mfs:2549, with fetch_flat's zero stand-in for the reads before fWaveform[0] (in the C++ these read infinite_avg/long_avg of m_sound). The fix is one line plus the test's 480 expectation. If the controller keeps the cap, the ruling should record the corrected cost.
#### Minor
2. custom_waves.rs doc ("a waveform of 481..512 samples then reads before the start") and the report repeat the 481 error; this should read 482..512, or "whenever (max-n)/2 < sep/2".
3. tests/custom_waves.rs `let _: Option<Vertex> = out.first().copied();` is a dead line that only keeps the `Vertex` import in use. Drop the import instead.
4. Report concern about the trailer model name is for the controller to settle and is not a code issue.

### Assessment
**Task quality:** Needs fixes. **Reasoning:** The transcription is faithful and well tested. The one defect comes from the ruling: its UB rationale is wrong (481 is defined, and the existing sep stand-in already covers the n > 480 reads), and the cap changes the default 512-sample case. The controller should re-rule (one-line fix) or re-affirm it with the corrected cost before this is approved.

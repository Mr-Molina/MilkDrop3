### Spec Compliance
✅ F40: modes 0..=7, `(int)wave_mode % NUM_WAVES` (wave_draw.rs mode line; mfs:2770, md_defines.h:51); no 256..2048 flags; `build_wave` takes the old mode + progress and runs the `it` loop (its=2 only when blending and modes differ, mfs:2771/2780); mix/m/nIdx/break carry/alpha mix match mfs:3186-3227 line for line.
✅ F38: `WaveParams::from_vars` reads post-equation `PerFrameVars` (wave.rs from_vars); old mode is the old preset's file `wave_mode` unmodded, as `m_nOldWaveMode = s_from->m_nWaveMode` (state.cpp:712).
✅ 6.1a: draws into `vs[vs_target]` with LoadOp::Load, no mip regeneration (wave_draw.rs). Half-pixel: positions written raw, `DrawBatch` binds `VsParams::for_target` (pipelines.rs:227-244) per the Task 4.8 ruling.
✅ Placement: after blur passes and the shape/custom-wave hooks, before sprites (frame.rs begin_frame; mfs:1051-1057).
✅ (a) Smoothing: mfs:850-858 is `w[0]*=s; w[i]=w[i]*s*(1-sm)+w[i-1]*sm` over the already-updated `w[i-1]`, s=m_fWaveScale/128 — `scaled_wave` is exact; brief's formula was wrong. Departure upheld.
✅ (b) Blend: mfs:2715-2717 SRCBLEND=SRCALPHA, DESTBLEND=ONE|INVSRCALPHA = `AlphaAdditive`/`Alpha` (blend.rs:82-83). Departure upheld.
✅ (c) `WaveGeometry{topo,draws,additive}` + `WaveView`: needed — mode 7's break (mfs:3156) and the 4 fat copies (3259-3292) are several DrawPrimitiveUP calls a bare Topo cannot express; `mode: i32` keeps C++ negative-modulo modes reachable. Upheld.
✅ (d) `identity_vars` wave_a=0: alpha 0 < 0.004 skips the draw (mfs:3233), so the 5 frame tests check exactly the warp/comp/blend pixels they checked before; the wave path is covered by tests/wave.rs GPU tests; blur.rs is unaffected (blur runs before the wave). Not weakened.
✅ Transcription, mode by mode vs C++: 0 (nv 240, off 120, inv (nv-1), nv/10 int blend, x·AspectY/y·AspectX, dupe skipped while m_bBlending, mfs:2803-2840); 1 (×1.25 first, fR[i], fL[i+32]·1.57+t·2.3, nv 240, 2844-2865); 2/3 (size switch ×/=, ×1.3, imm[2]² — C++ "treble_rel" is `imm[2]`, mfs:2766; 2867-2920); 4 (nv=min(480,tex/3) not halved, off (480-nv)/2, inv nv, fR[i+25+off], momentum i>1, 2922-2982); 5 (rot t·0.3, 2984-3015); 6/7 (nv=min(240,tex/3), edges from fWavePosX·cos/sin(ang+1.57)±3d, sequential 4-edge clip at ±1.1 re-reading current edges, dx/nv, perp via atan2+1.57, sep=(py/2+.5)², nBreak=nv, 3017-3158). Default case: 480 zeros, alpha unclamped (mfs:2783-2784). All match.
✅ Colour clamp then maximize c/max when max>0.01 (2738-2759); y flip before tessellation (3227-3231); SmoothWave 2n-1 incl. split at break (2466-2494, 3236-3252); fat copies cumulative +x,+y,-x on tex_w>=512 for thick||dots (3259-3268); strips draw all n verts, points list (3270-3291).
✅ Brief tests present (renamed expectations justified: 481/479 verts), commit `feat(render): built-in waveforms`.
⚠️ Golden default_60 unchanged (DSSIM 0) and gate results (clippy/fmt/66 ok) are report-only; not re-run per rules.

### Strengths
- Faithful transcription incl. obscure corners (mode-0 dupe skip on same-mode blends, break carry from v2, sequential edge clip, mode-3 alpha override only at the four exact sizes).
- CPU tests pin counts, clamps, cosine mix and break carry with hand-derived numbers; GPU tests mutation-checked and assert both presence and absence (circle top absent at blend_t 0).
- Separation: pure geometry in wave.rs, GPU in wave_draw.rs; `scaled_wave` exported for reuse by 6.4 (custom waves read the same scaled `mysound.fWave`).

### Issues
#### Critical
None.
#### Important
None.
#### Minor
1. Wrong C++ line citations: wave.rs header table (mode 0 "2815-2859" is 2803-2840; 1 "2861" is 2844; 2 "2886" is 2867; 3 "2916" is 2894; 4 "2946" is 2922; 5 "3005" is 2984; 6/7 "3037-3170" is 3017-3158; "case 8 … 2905-3086" is 3019/3127-3133), dupe "2852-2857" (2835-2840), clip "3095-3150" (3061-3111), switch "2806-3086" (2801-3160); wave_draw.rs "alpha below 0.004, milkdropfs.cpp:3178" is 3233; tests/wave.rs "3032-3033" ok but "3097-3148"/"3224-3228" are off. Fix the citations — the module claims to be a transcription.
2. F40 edge: an old preset whose file nWaveMode is 8 hits `case 8` in C++ (m_nOldWaveMode is not modded, mfs:2780); Rust draws the zero default. Covered by the F40 ruling; worth one comment line in raw_wave's `_` arm.
3. Report's "fallback never takes the two-wave path" is inaccurate: with old_preset None and per-frame code changing wave_mode, its=2 with the current preset's file mode (wave_draw.rs `unwrap_or(input.preset)`). Consistent with FrameInput's documented fallback (frame.rs:50-52); harmless.
4. NaN alpha → 0 (wave.rs finish_alpha) and `smooth_wave(&[])` → empty diverge from C++ UB; documented, acceptable.
5. No CPU test pins the vertex formulas of modes 1, 3, 4, 5 (only counts/2/6/7/0); a one-point-per-mode check with non-silent SoundData would guard future edits.
6. No recorded RED for the CPU tests (report TDD note); wave.rs 622 lines, slightly over guideline; `RawWave.alpha` needlessly `pub`; `usize_` closure is noise.
7. golden_basic.rs comment about "black until the wave" now stale (report, out of diff).

### Assessment
**Task quality:** Approved — **Reasoning:** Every mode, count, offset, blend and draw rule matches DrawWave line by line, and all four departures from the brief (a-d) are correct per the C++ and the rulings; remaining items are citation accuracy and test-coverage polish.

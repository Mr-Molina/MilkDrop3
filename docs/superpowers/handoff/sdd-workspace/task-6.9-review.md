### Spec Compliance
- ✅ Manifest: 33 entries == whole corpus, all `driver = "static"` (grep: 33/33); defaults 320x240, seed 7, frames [1,60,300], 0.02; test `manifest_covers_the_whole_corpus` pins corpus equality (golden_suite.rs:84-124).
- ✅ (a) golden_suite.rs:127-173 collects every failure (render error, missing frame, DSSIM) and asserts once with count; missing baseline prints "missing baseline" and existing ones print dssim. testkit `compare_png_with` (image_diff.rs:37-75) writes only when baseline is missing or over threshold under MD_BLESS; within-tolerance baselines are untouched.
- ✅ (b) headless.rs static_shapes: all 24 ShapeFrameVars fields map 1:1 to ShapeData (md-preset shape.rs:9-50), incl. thick_outline->thick, sides/textured/additive i32->f64; filter `enabled != 0`; instance 0/num_inst 1. Custom waves stay empty. golden_basic uses ps2/shader_rings (0 enabled shapes), so its 640x480 baseline is unperturbed.
- ✅ (c) 0.05 only on blur_glow, blur3_bloom, mixed_versions; grep of blur1/2/3/GetBlur over corpus matches exactly those three (sampler_modes matched only the lum macro, correctly 0.02).
- ✅ (d) covers spot-checked against .milk: bass_kaleido (wave 5, motion vectors), echo_feedback (echo 0.5, darken, solarize, mode 6), q_init_rand (rand in init), noise_grain, rand_tiles, rand_gallery (fc_/pw_), loop_shader, rand_mosaic, sampler_modes, mixed_versions (WARP=2/COMP=4), volume_lq_rot, missing_texture, four_shapes (4 enabled, num_inst 8/3, shape 2 textured). All match.
- ✅ (f) Black frames: four_waves, megabuf_trails, gmegabuf_share are custom-wave-only (fWaveAlpha 0, wavecodes enabled, no shapes); spectral_rings fWaveAlpha 0.001 plus custom wave, viewed black; all_q32 comp is `c*(_qe+_qf)+...` with q29..q32 zero under static values. Reasons hold. four_shapes frame 60 viewed: green triangle plus faint shape-1 border ring, consistent with draw order (shape 2 textured a=0.9 over black).
- ✅ (g) Same FixedClock 60 fps, seed 7, shared test_signal.wav, Pcg32 seeded from args; nothing wall-clock dependent. Size: tests/golden 7.9 MB, rust/ 4.5 MB, 99 frame PNGs + default_60 (< 15 MB). Frame-60 baselines each described in report; sampled 4 PNGs, none corrupted/mirrored.
- ✅ Runtime ruling honoured: #[ignore] with doc comment, CI step (ci.yml:62-65), README section.
- ⚠️ (e) see Important #1.

### Strengths
- Harness reuses `render_to_images` and `compare_png` per brief; a no-GPU manifest test keeps the manifest honest in the default gate.
- Clear provisional-baseline documentation (manifest header, README, test doc) tied to the 7.9 flip.

### Issues
#### Critical
- None.
#### Important
1. ci.yml:62-65: the golden step sits in the 3-OS matrix (ubuntu/macos/windows). `MD_REQUIRE_GPU=1` and `WGPU_ADAPTER_NAME=llvmpipe` are set on Linux only (ci.yml:29-35). On macOS/Windows the ~270 s suite either skips silently (no adapter) or runs on a Metal/DX12 adapter against lavapipe-blessed baselines, a likely DSSIM failure. Gate the step with `if: runner.os == 'Linux'` (as the env step is) so it runs with the GPU requirement on.
#### Minor
1. golden_suite.rs: failed comparisons leave `frame_N.diff.png` beside the baselines and `.gitignore` has no pattern for `*.diff.png`; add one.
2. tests/golden/README.md says bless with `MD_BLESS=1` "on the same command"; note that within-threshold drift is not rewritten, so a deliberate small change needs a threshold of 0 or deleting the PNG.
3. Report flags ps2/noise_grain frame 60 (viewed: saturated white Lissajous blob) as blown out; acceptable provisionally, but it is a weak baseline, so re-check at 7.9.

### Assessment
**Task quality:** Needs fixes — **Reasoning:** The suite, manifest, shape fill, thresholds and black baselines all check out against the rulings and the presets. The CI step runs on all three OSes while the GPU requirement is set only on Linux, so it must be restricted to Linux.

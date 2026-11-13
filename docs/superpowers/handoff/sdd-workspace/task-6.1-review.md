### Spec Compliance
✅ Weights, pass A `_c0.._c3` and pass B `_c0/_c5/_c6` match BlurPasses (milkdropfs.cpp:1563, 1612-1666) — blur.rs:63, 100-132; the pass-B `_c5/_c6` slots (not the pinned bullet's paraphrase "_c1/_c2/_c3 with scale/bias") are the C++'s (h[5], h[6]; no scale/bias in pass B) and blur2_ps.wgsl's contract.
✅ `fscale/fbias` chain and `fscale[i/2]` per pass match milkdropfs.cpp:1575-1585, 1610-1611 (blur.rs:71-88, 357); edge darkening only on i==1 (1655-1660; blur.rs:127-131).
✅ `passes = min(6, 2*highest)`, none for 0 (1517-1519; blur.rs:333-336); gated on `max_ps != None` as `m_nMaxPSVersion > 0` (1050-1051; frame.rs:555).
✅ Ruling (sizes): exact C++ sizes asserted for 800x600 (tests/blur.rs:761-782); hand-checked against plugin.cpp:1986-2007 (400x304, 192x152, 96x76 x2, 48x40 x2).
✅ Ruling (signature/highest_used): `blur_passes(enc, &PerFrameVars, u8)`; highest from `PresetGpu::highest_blur()` = max over warp+comp `Translated` roots (preset_gpu.rs:61-68, checked), new and old preset (frame.rs:551-554).
✅ (a) Source is the previous frame's VS[0]: WarpedBlit renders VS0->VS1 (milkdropfs.cpp:1586), blur binds `m_lpVS[0]` (1596), the flip is at 1134-1136, after the blur; blur.rs:345 uses `other_vs(vs_target)`.
✅ (b) Pass 0 `_c0` = window size: `srcw = (i==0) ? GetWidth() : m_nBlurTexW[i-1]` (1605-1606); blur.rs:349 `self.window`.
✅ (c) Ruling 5.6 zero-width check done: GetSafeBlurMinMax sets both ends to `avg - fMinDist*0.5` (1478-1497) so 1575 divides by zero (scale inf, bias NaN when min==0); blur.rs:72 clamps every divisor to 1e-6, unit test blur.rs:452-475 + GPU test tests/blur.rs:828-842.
✅ (d) No mip regeneration needed in 6.1: blur writes only `self.blur[i]` (blur.rs:347, 383); blur textures are 1-level like the C++ (plugin.cpp:1989, 1997; tex_size.rs:96 `mips=false`).
✅ Ruling 5.7 note: no-shader composite switches to LinearClamp when the chain runs (frame.rs:555-558), matching the clamp set at milkdropfs.cpp:1528-1531 only after the `passes==0` early return.
✅ Brief's three test names present (tests/blur.rs:761, 785, 845), plus frame-loop and determinism tests.
⚠️ highest_used timing: the C++ accumulates `m_nHighestBlurTexUsedThisFrame` in ApplyShaderParams (3851-3852) and resets it at the end of BlurPasses (1678), so the comp shader's usage lags one frame; the static union differs only on the first frame after a switch. Acceptable per the 6.1 ruling.
⚠️ (c) Hard step at `min` vs the C++'s NaN/inf output is a deterministic choice; real D3D9 output (likely black after NaN->UNORM) not checked. Record in the ledger for the 6.9 goldens.
⚠️ Context-vs-C++ conflict (escalate, not a 6.1 defect): milestone-5-context pins a full mip chain on VS, but this repo's C++ creates VS with Levels=1 and AUTOGENMIPMAP commented out (plugin.cpp:1904, 1912, 1921). Pass 0 (LinearClamp with linear mip, blur.rs:311) therefore samples VS[0] mip ~1 where the C++ bilinears level 0. For an exact 2x ratio with the half-pixel shift this is the same 4-texel average, but it differs otherwise. It also affects the 5.7 "regenerate mips" note.

### Strengths
- Every C++ quirk is cited with line numbers in the module docs (blur.rs:21-47); `pass_uniforms`/`blur_scale_bias` are pure and unit-tested.
- Per-pass uniform buffers handle the one-submit `write_buffer` hazard; the lazy `BlurGpu` is rebuilt when the format changes (blur.rs:337-341).
- The frame-loop test proves the gating end to end (textures 0..3 drawn for GetBlur2, none when unused).

### Issues
#### Critical
None.
#### Important
None.
#### Minor
- blur.rs:324-326: the doc says "submitted nothing since the warp pass wrote the main texture's mips", but the source is the warp *source*, whose mips the previous frame made. Reword.
- tests/blur.rs: every GPU input is a constant image, so wrong tap offsets `d1..d4`, a swapped horizontal/vertical axis or a v-flip would pass. Add one non-constant case (a bright column spreading in pass 0 but not in pass 1). The unit test (blur.rs:399-439) re-derives the same formulas.
- frame.rs:557: the switch of the clamp sampler for the no-shader composite (5.7 note) has no test.
- blur.rs:366-382: a bind group is created per pass per frame; it could be cached per renderer generation.
- blur.rs:77: `d0` recomputes `scale[0]`, and the reciprocal multiply differs from the C++ division by up to 1 ulp. Negligible.
- lib.rs:615 exports `pass_uniforms` and `MIN_RANGE` publicly, beyond the brief's interface.
- Process: per the report, the implementation was written before the tests and RED was shown by stubbing the pass count.

### Assessment
**Task quality:** Approved — **Reasoning:** Constants, pass order, sources, sizes and gating match BlurPasses/plugin.cpp line by line and satisfy the 5.6/5.7/6.1 rulings. The remaining items are test-strength and doc polish, plus one context conflict (VS mips) to escalate to the ledger.

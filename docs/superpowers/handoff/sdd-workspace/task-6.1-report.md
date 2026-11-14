# Task 6.1 report: blur chain

Commit 07e6aa6 `feat(render): blur chain` (not pushed). Status: DONE_WITH_CONCERNS (see Concerns).

## Implemented
- `crates/md-render/src/blur.rs` (new): `Renderer::blur_passes(&mut self, enc, vars: &PerFrameVars, highest_used: u8)` per F38/F39; pure `blur_scale_bias`, `pass_uniforms`, `BLUR_WEIGHTS`, `MIN_RANGE`, `NUM_BLUR_TEX`; lazy `BlurGpu` (blur_vs + blur1_ps/blur2_ps pipelines, per-pass uniform/VsParams buffers, quad, LinearClamp sampler from `SamplerKind`).
- `frame.rs`: the blur hook replaces the no-op: runs when `max_ps != None` and `max(gpu.highest_blur(), old_gpu.highest_blur()) > 0` (milkdropfs.cpp:1051), and then sets the no-shader composite sampler to LinearClamp (5.7 note). `renderer.rs`: `blur_gpu` field. `lib.rs`: mod and exports.
- `highest_used` was already stored: md-hlsl `Translated.highest_blur` (from sampler roots) -> `ShaderProgram.highest_blur` -> `PresetGpu::highest_blur()` (done in 5.5), so no scan and no load_preset change were needed.
- Sizes: the 5.5 allocation (`blur_texture_sizes`) already equals plugin.cpp:1986-2007; verified, only a test added (800x600 -> main 800x608: 400x304, 192x152, 96x76, 96x76, 48x40, 48x40).
- Pass order/sources from milkdropfs.cpp:1501-1655: pass i writes blur[i]; pass 0 reads the warp source `m_lpVS[0]` (= `other_vs(vs_target)`, full mip view), pass i>0 reads blur[i-1]; even = blur1_ps, odd = blur2_ps; edge_darken only on i==1; clamp + linear/linear/linear sampler. Half-pixel via blur_vs VsParams of the destination size.
- Zero-width scale: GetSafeBlurMinMax collapses to min==max exactly, and the C++ then computes fscale=inf, fbias=+-inf/NaN. `blur_scale_bias` clamps every divisor to >= 1e-6 (the limit of the C++ formula: a hard step at min); all uniforms finite (unit test over collapsed/inverted/negative ranges).

## TDD
RED: with the pass count stubbed to 0, `MD_REQUIRE_GPU=1 cargo test -p md-render --test blur`: 3 failed (blur_of_constant_image_is_constant_after_scale_bias, frame_runs_blur_for_presets_that_sample_it, no_passes_when_unused), 2 passed (sizes test, because 5.5 already matched; collapsed-range determinism). Note: implementation was written before the tests were run (tests compiled only after), so RED is via the stub.
GREEN: restored: `test result: ok. 5 passed` (blur tests) + 3 unit tests in blur.rs.
Full: `MD_REQUIRE_GPU=1 cargo test --workspace` exit 0, 65 result lines all ok, incl. golden_default_preset_frame_60. `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` clean.

Tests (crates/md-render/tests/blur.rs): sizes; constant image through scale/bias (range [0.2,0.7] -> 0.6 at all six textures; default range identity; edge darkening only darkens borders); collapsed range deterministic; `no_passes_when_unused` (0 -> nothing, 1 -> only textures 0,1, 2 -> only 0..3); frame loop runs blur only for a comp shader using GetBlur2 (textures 0..3 written, 4,5 not; unused preset writes none).

## Files
crates/md-render/src/blur.rs (new), src/frame.rs, src/lib.rs, src/renderer.rs, tests/blur.rs (new).

## Concerns
- C++ quirks mirrored: pass 0's `_c0` source size is the window size (GetWidth/Height), not the main texture size; blur source is last frame's warp source, not this frame's output.
- The 5.7 "regenerate main's mips after drawing into main" is not needed yet: blur writes only blur textures and the warp pass already makes mips; a comment at the hook tells 6.2-6.5/6.7 to regenerate.
- Zero-width behavior is a finite approximation (hard step); the C++ output there is inf/NaN (undefined on D3D9). Not checked against recorded real D3D output.
- blur textures have no mips (as the C++), so presets sampling them with derivatives only see level 0.

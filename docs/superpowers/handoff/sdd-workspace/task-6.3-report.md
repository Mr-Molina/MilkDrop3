# Task 6.3 report: custom shapes

Commit: `feat(render): custom shapes` (see git log). Files: crates/md-render/src/shapes.rs (186 lines, builder), shapes_draw.rs (82, GPU), tests/shapes.rs (9 tests), frame.rs (hook + 2 FrameInput fields), lib.rs (exports).

## API (F38)
- `FrameInput.shapes: &[ShapeFrameVars]` and `old_shapes: &[ShapeFrameVars]` (both `&[]` in `FrameInput::new`, so headless.rs and all struct-update callers are untouched; headless passes empty). md-core (M7) fills them after each instance's per-frame program, order shape 0..4 then instance (milkdropfs.cpp:2231-2248). Old list is drawn only while `blending()` (2217: num_reps = m_bBlending ? 2 : 1), new first, then old.
- `ShapeFrameVars`: brief's fields, all f64 (C++ doubles cast with `(int)`/`(float)` at use) except `instance`/`num_inst: u32` (not read by drawing).
- `build_shape(v, &ShapeView{aspect_y, tex, alpha_mult}, out_fill, out_border) -> ShapeGeometry{sides, textured, additive, border_copies}`. Departure from brief's `aspect`: the C++ uses only AspectY; the thick border needs tex size (2352-2353) and the blend needs alpha_mult (2224-2226), hence the view struct (same pattern as 6.2's WaveView).

## Pinned from the C++ (milkdropfs.cpp@6f7cd5d)
- Placement: RenderFrame 1051-1057: BlurPasses, DrawCustomShapes (1054), custom waves, DrawWave, sprites. Hook sits right before draw_wave, after blur.
- Texture: the brief says VS[1]; the C++ is `SetTexture(0, m_lpVS[0])` (2330; 2387 restores it). At that moment the render target is m_lpVS[1] (set 983-990), so a textured shape samples the warp SOURCE (previous frame's main), not the buffer being drawn. In md-render: sample `vs[other_vs(vs_target)]`, draw into `vs[vs_target]`. DEPARTURE FROM BRIEF.
- Sampler: nothing in DrawCustomShapes sets sampler state; it inherits stage 0. Every frame start sets LINEAR mag/min/mip (913-918), so the Task 5.3 "MAG POINT" note does not apply here (superseded explicitly). Address mode is inherited exactly like the no-shader composite: the warp pass's (1906-1908 / RestoreShaderParams WRAP 3797-3806) or CLAMP after BlurPasses (1529-1530). I pass the frame loop's existing `fixed_sampler` (computed before the blur step and updated by it) to the draw. Mip: main is single-level (6.1a), no regeneration.
- Blend: SRCALPHA / (`(int)additive != 0` ? ONE : INVSRCALPHA) (2267-2268) = AlphaAdditive / Alpha (not the brief's Additive); set once per instance, so the border uses it too.
- Alpha skip threshold: NONE in DrawCustomShapes (the 0.004 skip is DrawWave's). Every instance is drawn.
- Colours: `((int)(a*255*alpha_mult)) & 0xFF`, other channels `(int)(c*255) & 0xFF` (2270-2283): truncation and 8-bit wrap (a value > 1 wraps), computed in f64 like the C++. Centre vertex = (r,g,b,a), ring and fan end = (r2,g2,b2,a2).
- Geometry: sides `(int)` clamped 3..100 (2262-2264); centre `(x*2-1, y*-2+1)` (2273-2274 region); ring j=1..sides, t=(j-1)/sides (NOT j/sides as the brief says), angle `t*PI*2 + ang + PI/4` in f32, x scaled by AspectY (2288-2289), UV `0.5 + 0.5*cos(t*2PI+tex_ang+PI/4)/zoom*AspectY` (u) and without AspectY (v) (2290-2291, brief had aspect_y on v too? it said "* aspect_y" once; C++ is u only); `v[sides+1] = v[1]` (2297). Fan = `sides` triangles (DrawPrimitiveUP FAN, sides).
- Untextured: same fan through WFVERTEX, no texture (Color mode); textured: TexModulate (fixed_tex_modulate, alpha = vertex alpha).
- Border: only if `border_a > 0` (double compare, 2320ish); colour from border_*, alpha*alpha_mult; LINESTRIP of `sides` segments from v2[1], i.e. sides+1 vertices, closed since v[sides+1]=v[1]. Thick = `(int)thick != 0` -> 4 copies with cumulative offsets +x_inc, +y_inc, -x_inc where inc = 2/tex_size (2352-2359). Unlike DrawWave there is no tex_w >= 512 gate.
- Half-pixel: raw clip positions; DrawBatch's VsParams supply the D3D9 shift (Task 4.8 ruling), as 6.2.
- Instance loop: `for rep; for shape (enabled); for instance < instances`; per-frame program per instance is md-core's job. `enabled`/`instances` filtering is md-core's (it only lists enabled instances).

## TDD
RED (tests first, no implementation):
`cargo test -p md-render --test shapes` -> `error[E0432]` unresolved imports (build_shape, ShapeFrameVars, ShapeView) and `error[E0560]: struct FrameInput has no field named shapes/old_shapes`; "could not compile `md-render` (test "shapes") due to 3 previous errors".
GREEN: `MD_REQUIRE_GPU=1 cargo test -p md-render --test shapes` -> 9 passed. (One test bug fixed on the way: the first frame after creation clears the warp source (971-981), so the textured test runs an empty warm-up frame before filling it.)
Mutation: with the `draw_shapes` call commented out, the 3 GPU tests FAILED, 6 CPU passed; restored.

Tests: the brief's four (`sides_clamped_to_100`, `fan_has_sides_plus_2_vertices`, `textured_uv_center_is_half_half`, `border_closes_the_loop`); thick offsets; colour truncate/wrap/alpha_mult; GPU `filled_shape_lands_pixels_in_main`, `textured_shape_samples_the_warp_source` (warp shader writes black, source prefilled (200,100,50): shape shows the source colour, so sampling the target would give black), `old_shapes_draw_only_while_blending_scaled_by_progress` (not drawn when not blending; blend_t 0 -> old only, 1 -> new only).

## Gate (MD_REQUIRE_GPU=1)
build clean; `clippy --workspace --all-targets -D warnings` clean; `fmt --check` ok; `cargo test --workspace`: every `test result` ok, 0 failures. Golden `tests/golden/rust/default_60.png` (golden_default_preset_frame_60) passes unchanged: that preset has no shapes and the default frame passes empty lists; no re-bless.

## Concerns
- Border dots/lines are 1 px (D3D9 and wgpu agree); a line whose vertices lie off-screen differs trivially from D3D9 clipping.
- The border strips use `SamplerKind::LinearClamp` (Color mode samples nothing, irrelevant).
- Brief says UV `* aspect_y` on both axes; C++ applies it to u only (kept C++).
- The ledger needs a Ruling entry for the FrameInput `shapes`/`old_shapes` addition (I do not edit docs/superpowers).

## Fix report, round 1
- Citations: every C++ line number in shapes.rs, shapes_draw.rs and tests/shapes.rs re-checked with `sed -n` against code/vis_milk2/milkdropfs.cpp (e.g. sides 2288, additive 2294, centre 2299, colours 2304-2313, ring 2316-2323, UV 2321-2322, close 2325, textured 2327-2334, border 2352-2385, thick 2371, tex inc 2372-2373, offsets 2376-2382, LINESTRIP 2383, reps 2224-2227, alpha_mult 2228-2230, per-frame vars 2400-2440, NSEEL 2267-2273). The report's own older citations above are superseded by these.
- Cast parity: new `c_int` in shapes.rs (mirrors md-eval `eel_int`: NaN/out-of-range -> i32::MIN) used at every `(int)` site (sides, textured, additive, thick, colour bytes). Unit tests `c_int_truncates_and_gives_int_min_out_of_range`, `nan_and_overflow_follow_the_cpp` (NaN flags true, sides 3e9 -> 3, colour 1e7 -> 0).
- Tests: `textured_uv_center_is_half_half` now also uses aspect_y 0.75 (u scales, v does not); new GPU test `additive_selects_one_instead_of_inv_src_alpha` (red then half-alpha green: ~(128,127) alpha-blended, (255,127) additive).
- Commands: `MD_REQUIRE_GPU=1 cargo test --workspace` -> all `test result: ok`, 0 failures (shapes: 10 integration + 2 unit pass); clippy -D warnings and fmt --check clean; golden default_60 still passes.

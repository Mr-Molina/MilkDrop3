# Task 4.8 report: Built-in WGSL passes

Status: DONE_WITH_CONCERNS (concerns are hand-offs to md-render, not defects)
Commit: 6f4eaec feat(render): built-in WGSL passes

## What was implemented

- `assets/shaders/`: `warp_vs`, `comp_vs`, `blur_vs`, `blur1_ps`, `blur2_ps`, `fixed_color`,
  `fixed_tex_modulate`, `fixed_tex_select`, `text`, `title` (.wgsl) plus `README.md`.
  - warp_vs/comp_vs output exactly the M4 `VsOut` (pos builtin, diffuse @0, uv vec4 @1,
    rad_ang vec2 @2). Math is the .fx math (pos = (x,y,z,1); diffuse, uv, rad_ang pass
    through). comp's HLSL `_uv` is float2, so `uv.zw = 0` (the comp FS reads only .xy).
  - blur_vs: pos = (xy, 1, 1), uv = uv.xy. blur1_ps/blur2_ps: line-for-line translation;
    weights and the d1..d4 / w_div derivation copied verbatim into comments.
  - Fixed function: color = diffuse; tex_modulate = tex * diffuse (rgba);
    tex_select = tex. text: (color.rgb, color.a * atlas.a). title: tex * color (drawn
    additively by md-render). 6.6/6.7 may adjust.
- Single vertex layout (F30): every `vs_main` declares `VsIn { pos @0 vec3, color @1 vec4
  (Unorm8x4), uv @2 vec4, rad_ang @3 vec2 }`; README tabulates formats, offsets
  (0/12/16/32), stride 40, and color byte order RGBA (D3DCOLOR must be swizzled).
- F29 half-texel: every textured VS reads `@group(0) @binding(2) var<uniform> vs_params:
  VsParams { texel_size: vec2, half_texel: f32, pad: f32 }` and adds
  `half_texel * texel_size` to output UVs; WGSL never hard-codes -0.5 (md-render fills
  `half_texel` from `md_hlsl::layout::D3D9_HALF_TEXEL`). Binding 2 avoids the preset FS's
  U (0,0) / T (0,1). Per-pass `texel_size` table in README, derived from the C++:
  - warp: C++ adds +0.5 texel to tu/tv (milkdropfs.cpp:1835-1837) and tu_orig/tv_orig
    (plugin.cpp:2299-2300) -> shift uv.xyzw, texel_size = 1/texsize.
  - comp: C++ moves grid positions by -0.5 px (plugin.cpp:2041-2058) -> shift uv.xy,
    texel_size = 1/window.
  - blur: no C++ offset, but D3D9 integer pixel centres still differ -> shift uv.xy with
    texel_size = (1/dst_w, **-1/dst_h**) because the C++ blur quad maps v=0 to the bottom.
  - sprites/fixed textured: (1/target) or 0 to disable; text/title: 0 for pixel-aligned quads.
- `crates/md-hlsl/src/layout.rs`: added `BUILTIN_VS_PARAMS_BINDING = 2`,
  `BUILTIN_VS_PARAMS_SIZE = 16`, `BLUR_UNIFORMS_SIZE = 112` (single source of truth for
  md-render, checked by the test against naga's layouter).
- Blur uniforms: one `BlurUniforms { _c0.._c6: vec4 }` (offset 16·N, mirrors D3D9
  registers) at (0,0) for both blur PS; texture/sampler `tex_main`/`samp_main` at (1,0)/(1,1).
  README documents how `BlurPasses` (milkdropfs.cpp:1564-1655) fills it, incl. the i==1
  edge-darken rule and the clamp/linear sampler.
- Conventions documented: D3D9 and wgpu clip space agree (y up, z 0..1), texture v
  top-down in both -> no flips; culling off (C++ D3DCULL_NONE).
- Not ported here (by design): warp_ps.fx / comp_ps.fx (default preset bodies -> translate()),
  include.fx (preamble).

## Tests (crates/md-hlsl/tests/builtin_wgsl.rs, 7 tests)

every_builtin_wgsl_validates (naga parse+validate each file; rejects stray files),
every_vertex_shader_reads_the_single_vertex_layout, warp_vs_and_comp_vs_output_the_preset_vs_out
(naga IR: VS output members == translated warp_ps.fx / comp_ps.fx fs_main input members — names,
bindings incl. default interpolation, types — in order), vertex_outputs_feed_their_fragment_shaders,
builtin_vertex_params_do_not_collide_with_preset_bindings (also VsParams size/offsets),
blur_uniform_block_mirrors_constant_registers (bindings, 112 bytes, offsets 16·N),
textured_fixed_passes_bind_one_texture_in_group_1.

RED: `cargo test -p md-hlsl --test builtin_wgsl` -> 0 passed; 7 failed, each
"assets/shaders/<file>.wgsl: No such file or directory (os error 2)".
GREEN: same command -> `test result: ok. 7 passed; 0 failed`.
Mutation check: changing warp_vs's `uv` to `@location(3)` makes
warp_vs_and_comp_vs_output_the_preset_vs_out fail ("warp_vs.wgsl vs translated warp_ps.fx"); reverted.
Workspace: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` clean; `cargo test --workspace` all ok (0 failures).

## Files changed

- assets/shaders/{warp_vs,comp_vs,blur_vs,blur1_ps,blur2_ps,fixed_color,fixed_tex_modulate,fixed_tex_select,text,title}.wgsl (new)
- assets/shaders/README.md (new)
- crates/md-hlsl/src/layout.rs (3 constants)
- crates/md-hlsl/tests/builtin_wgsl.rs (new)

## Self-review findings

- Diff re-read; fixed a doc comment claiming every built-in VS reads VsParams (fixed_color has none).
- blur2_ps keeps plain `sqrt(t)` like the translator; fxc ps_2_0 lowers sqrt via rsq on |x|,
  but with the half-texel emulation uv stays in [0,1] at pixel centres so t >= 0.

## Concerns / hand-offs

1. The half-texel shift is applied to UVs (per ruling F29), not to positions. For the warp mesh
   this is exact only where the mesh maps ~1 texel per pixel; a position shift of +0.5 px would be
   exact everywhere. Ruling followed; noted in README.
2. md-render (5.2/5.3) must: swizzle D3DCOLOR to RGBA bytes; put VsParams at (0,2) in the
   pipeline layouts that pair warp_vs/comp_vs with a preset FS; pass the signed blur texel_size
   (1/w, -1/h).
3. text.wgsl assumes straight alpha with coverage in the atlas's .a (md-text Atlas is RGBA);
   6.6 may change.

---

# Fix round 1

Commit: see `git log` — "fix(render): emulate D3D9 rasterization by position shift; D3D9 alpha-op semantics".

## Changes

1. **Half-texel → position shift (controller ruling).** Every built-in VS (now including
   fixed_color) calls the identical helper
   `fn d3d9_raster(clip) { s = 2*pixel_shift*(texel_size.x, -texel_size.y); clip.xy += s*clip.w }`,
   i.e. NDC (+1/W, -1/H) for clip.w = 1. UVs pass through unmodified (warp uv.xyzw, comp
   uv.xy with zw = 0, blur uv.xy). `VsParams { texel_size (target 1/W,1/H, unsigned),
   pixel_shift, pad }` at (0,2), 16 bytes. `md_hlsl::layout::D3D9_HALF_TEXEL = -0.5` became
   `D3D9_PIXEL_SHIFT = 0.5` (still the single source of truth; doc gives the exact formula).
   The emit_corpus assertion on the constant was updated. Blur needs no signed texel size:
   the source size stays in BlurUniforms._c0.
2. **Alpha ops.** fixed_tex_modulate = (tex.rgb*color.rgb, color.a) (standard state,
   milkdropfs.cpp:923-929, 1898-1904, 3448-3454, 3770-3776, 3991-3997; pluginshell.cpp:1790-1797).
   New fixed_tex_modulate_alpha.wgsl = tex*color (sprite mode 4, milkdropfs.cpp:3714-3721).
   title alpha = color.a (ShowSongTitleAnim sets no stage state, so the standard state applies;
   blend ONE/ONE at 4492-4494). fixed_tex_select: no C++ call uses SELECTARG1 TEXTURE (every
   SELECTARG1 selects DIFFUSE); the README now says so and keeps the file for
   FixedMode::TexSelect. fixed_color cites pluginshell.cpp:1832-1836.
3. **README.** The VsParams table, the new "D3D9 rasterization emulation" section (formula,
   per-pass target table, md-render builds all vertex data as the C++ does), the sprite /
   ShowToUser UV orientation note (v bottom-up, 1/fZoom scaling, echo-orientation flips,
   sprite ±0.5 * repeat + 0.5), and the fixed-function table with citations. The
   "only exact near identity" caveat was removed.

## Tests (builtin_wgsl.rs, now 10)

New: every_vertex_shader_applies_the_d3d9_raster_shift (source contains the exact helper; naga
IR: vs_main calls d3d9_raster once; vertex-stage files listed exactly),
raster_shift_reproduces_d3d9_sampling_on_a_4x4_target (CPU mirror of the formula: a vertex at
(-1,1) lands at screen (0.5,0.5); a w=2 vertex still shifts NDC by (+1/W,-1/H); for C++-built
warp, comp and blur quads, the wgpu sample at (i+.5, j+.5) equals the D3D9 sample at (i, j) for all
16 pixels; the comp grid covers the last column/row), fixed_function_alpha_ops_match_the_cpp_stage_states.
Updated: VsParams offsets (pixel_shift), fixed_color now has vs_params, file lists include
fixed_tex_modulate_alpha.

RED: with the round-0 shaders restored (`git stash push -- assets/shaders`),
`cargo test -p md-hlsl --test builtin_wgsl` -> 7 passed; 2 failed
(every_vertex_shader_applies_the_d3d9_raster_shift, builtin_vertex_params_do_not_collide_with_preset_bindings).
The CPU math test passes regardless because it is pure math.
GREEN: `cargo test -p md-hlsl --test builtin_wgsl` -> 10 passed; 0 failed.
`cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`
are clean. `cargo test --workspace` passes with 0 failures.

## Concerns

- Docs outside my scope (shared-context / plan text) still name `D3D9_HALF_TEXEL` and say "shift
  UVs". The controller should update them to `D3D9_PIXEL_SHIFT` and the position shift.

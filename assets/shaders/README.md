# Built-in WGSL passes

Hand-written WGSL for MilkDrop's built-in shaders (Task 4.8), consumed by
`md-render` (Milestone 5). Every file is parsed and validated by naga in
`crates/md-hlsl/tests/builtin_wgsl.rs`, which also checks the interfaces and
block layouts documented here.

| File | Source | Entry points |
|---|---|---|
| `warp_vs.wgsl` | `data/warp_vs.fx` | `vs_main` (pairs with a translated preset warp shader's `fs_main`) |
| `comp_vs.wgsl` | `data/comp_vs.fx` | `vs_main` (pairs with a translated preset comp shader's `fs_main`) |
| `blur_vs.wgsl` | `data/blur_vs.fx` | `vs_main` (pairs with `blur1_ps` / `blur2_ps`) |
| `blur1_ps.wgsl` | `data/blur1_ps.fx` | `fs_main` |
| `blur2_ps.wgsl` | `data/blur2_ps.fx` | `fs_main` |
| `fixed_color.wgsl` | D3D9 fixed function: diffuse only | `vs_main`, `fs_main` |
| `fixed_tex_modulate.wgsl` | D3D9 fixed function: texture × diffuse | `vs_main`, `fs_main` |
| `fixed_tex_select.wgsl` | D3D9 fixed function: texture only | `vs_main`, `fs_main` |
| `text.wgsl` | glyph-atlas text (Task 6.6 may adjust) | `vs_main`, `fs_main` |
| `title.wgsl` | song-title texture (Task 6.7 may adjust) | `vs_main`, `fs_main` |

The other three `data/*.fx` files are not here: `warp_ps.fx` and `comp_ps.fx`
are the default preset shader *bodies* and go through `md_hlsl::translate`
like any preset shader; `include.fx` is the preamble the translator reads
(`assets/hlsl/`).

## Vertex layout (F30: the single Task 5.2 `Vertex`)

Every `vs_main` reads the same `VsIn`, matching

```rust
#[repr(C)]
pub struct Vertex { pos: [f32; 3], color: [u8; 4], uv: [f32; 4], rad_ang: [f32; 2] }
```

| Field | `@location` | `wgpu::VertexFormat` | Byte offset | WGSL type |
|---|---|---|---|---|
| `pos` | 0 | `Float32x3` | 0 | `vec3<f32>` |
| `color` | 1 | `Unorm8x4` | 12 | `vec4<f32>` |
| `uv` | 2 | `Float32x4` | 16 | `vec4<f32>` |
| `rad_ang` | 3 | `Float32x2` | 32 | `vec2<f32>` |

Array stride 40 bytes, step mode `Vertex`. Unused fields are zero.
`color` bytes are **R, G, B, A** in memory (`Unorm8x4` maps byte 0 to `.x`).
A D3D `D3DCOLOR` (`0xAARRGGBB`, stored B, G, R, A) must be swizzled when
building a `Vertex`. `uv.xy` holds the texture coordinates of `MYVERTEX`
(`tu, tv`) / `SPRITEVERTEX`; `uv.zw` holds `MYVERTEX`'s `tu_orig, tv_orig`.

## Coordinate conventions

D3D9 clip space and wgpu clip space agree: x right, y up, z in `0..1`,
viewport y down. Texture `v` grows downwards in both. So no shader flips
anything; vertex data built as in the C++ renders identically. `blur_vs`
keeps the C++'s `z = 1`, which wgpu's `0 <= z <= w` clipping accepts
(depth testing is off for every built-in pass).

The C++ draws with culling off (`D3DRS_CULLMODE` `D3DCULL_NONE`), so
md-render uses `cull_mode: None`.

## Bindings

Group 0 holds uniforms, group 1 textures and samplers, as in the preset
shader contract (Milestone 4 intro: `U` at (0,0), `T` at (0,1),
`tex_i`/`samp_i` at (1,2i)/(1,2i+1)).

| File | (0,0) | (0,2) | (1,0) | (1,1) |
|---|---|---|---|---|
| `warp_vs`, `comp_vs` | (preset `U`; `T` at (0,1)) | `vs_params: VsParams` (VS) | (preset textures) | |
| `blur_vs` + `blur1_ps` / `blur2_ps` | `blur: BlurUniforms` (FS) | `vs_params: VsParams` (VS) | `tex_main: texture_2d<f32>` | `samp_main: sampler` |
| `fixed_color` | none | none | none | none |
| `fixed_tex_modulate`, `fixed_tex_select`, `text`, `title` | | `vs_params: VsParams` (VS) | `tex: texture_2d<f32>` | `samp: sampler` |

`VsParams` sits at binding 2 (`md_hlsl::layout::BUILTIN_VS_PARAMS_BINDING`)
so that it never collides with the preset fragment shader's `U`/`T` when
`warp_vs`/`comp_vs` share a pipeline layout with it.

### `VsParams` (16 bytes, `md_hlsl::layout::BUILTIN_VS_PARAMS_SIZE`)

| Field | Type | Offset | Value |
|---|---|---|---|
| `texel_size` | `vec2<f32>` | 0 | signed UV step per render-target pixel (see below); `(0, 0)` disables the shift |
| `half_texel` | `f32` | 8 | `md_hlsl::layout::D3D9_HALF_TEXEL` (`-0.5`) |
| `pad` | `f32` | 12 | 0 |

### `BlurUniforms` (112 bytes, `md_hlsl::layout::BLUR_UNIFORMS_SIZE`)

Seven `vec4<f32>` mirroring D3D9 constant registers c0..c6: `_cN` at offset
`16·N`. `blur1_ps` reads `_c0.._c3`, `blur2_ps` reads `_c0`, `_c5`, `_c6`;
the others are ignored. Filled per pass exactly as `CPlugin::BlurPasses`
(code/vis_milk2/milkdropfs.cpp:1564-1655) does:

```text
w[8] = { 4.0, 3.8, 3.5, 2.9, 1.9, 1.2, 0.7, 0.3 }
srctexsize = (srcw, srch, 1/srcw, 1/srch)    // source: pass 0 reads the VS
                                             // (window size), pass i>0 reads blur[i-1]
fscale/fbias = fscale[i/2], fbias[i/2]       // from GetSafeBlurMinMax, see C++

pass i even (blur1_ps, long horizontal):
  w1 = w[0]+w[1]; w2 = w[2]+w[3]; w3 = w[4]+w[5]; w4 = w[6]+w[7]
  d1 = 0 + 2*w[1]/w1; d2 = 2 + 2*w[3]/w2; d3 = 4 + 2*w[5]/w3; d4 = 6 + 2*w[7]/w4
  w_div = 0.5/(w1+w2+w3+w4)
  _c0 = srctexsize; _c1 = (w1,w2,w3,w4); _c2 = (d1,d2,d3,d4); _c3 = (fscale, fbias, w_div, 0)

pass i odd (blur2_ps, short vertical):
  w1 = w[0]+w[1]+w[2]+w[3]; w2 = w[4]+w[5]+w[6]+w[7]
  d1 = 0 + 2*((w[2]+w[3])/w1); d2 = 2 + 2*((w[6]+w[7])/w2)
  w_div = 1.0/((w1+w2)*2)
  _c0 = srctexsize; _c5 = (w1,w2,d1,d2)
  _c6 = (w_div, 1-edge_darken, edge_darken, 5.0)  if i == 1  (edge_darken = blur1_edge_darken)
        (w_div, 1.0, 0.0, 5.0)                    otherwise
```

The source sampler is linear min/mag/mip with clamp addressing
(milkdropfs.cpp:1527-1533). Each pass draws the fullscreen quad
`(-1,-1) uv (0,0)`, `(1,-1) (1,0)`, `(-1,1) (0,1)`, `(1,1) (1,1)` as a
triangle strip, i.e. **v = 0 at the bottom**: every pass flips vertically,
and the pairs cancel (blur1..3 come out upright), as in the C++.

## D3D9 half-texel emulation (F29)

D3D9 samples a pixel at its integer screen coordinate; wgpu at the pixel
centre (+0.5 px). A texture coordinate interpolated across the screen is
therefore half a pixel further along under wgpu. Every textured built-in VS
corrects this by adding `half_texel * texel_size` to the UVs it outputs,
where `texel_size` is the signed UV change per render-target pixel. The
constant's single source of truth is `md_hlsl::layout::D3D9_HALF_TEXEL`;
the WGSL never hard-codes it. Only texture coordinates are shifted (not
`rad_ang` or colours). What md-render passes:

| Pass | C++ alignment | Shifted | `texel_size` |
|---|---|---|---|
| warp (`warp_vs`) | `+0.5/texsize` on `tu,tv` (milkdropfs.cpp:1835-1837) and `tu_orig,tv_orig` (plugin.cpp:2299-2300) | `uv.xyzw` | `(1/texsize_x, 1/texsize_y)` |
| comp (`comp_vs`) | grid positions moved by `-0.5` px (plugin.cpp:2041-2058) | `uv.xy` (`.zw` output is 0) | `(1/window_w, 1/window_h)` |
| blur (`blur_vs`) | none (quad 0..1, milkdropfs.cpp:1542-1562) | `uv.xy` | `(1/dst_w, -1/dst_h)` (v runs bottom-up) |
| sprites / fixed textured draws | quad extended by one pixel (milkdropfs.cpp:4004-4016) | `uv.xy` | `(1/target_w, 1/target_h)`, or 0 to disable |
| text / title | md-render (6.6, 6.7) decides | `uv.xy` | 0 for pixel-aligned glyph quads |

The warp correction is exact where the warp mesh is locally an identity
mapping (one texel per pixel), which is the case the C++ offset targets.

## Fixed-function equivalents

| File | D3D9 texture stage 0 | Output |
|---|---|---|
| `fixed_color` | `COLOROP`/`ALPHAOP` `SELECTARG1`, `ARG1 = DIFFUSE` | vertex colour |
| `fixed_tex_modulate` | `COLOROP`/`ALPHAOP` `MODULATE`, `TEXTURE` × `DIFFUSE` | `texture * colour` (rgba) |
| `fixed_tex_select` | `COLOROP`/`ALPHAOP` `SELECTARG1`, `ARG1 = TEXTURE` | `texture` |
| `text` | glyph atlas (`md-text`, RGBA8, coverage in `.a`) | `(colour.rgb, colour.a * atlas.a)` |
| `title` | `MODULATE`, drawn additively (milkdropfs.cpp:4477ff) | `texture * colour` (rgba) |

Blend state, topology and the fan/point conversions are pipeline state
chosen by md-render (Task 5.3), not part of these shaders.

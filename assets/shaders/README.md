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
| `fixed_tex_modulate.wgsl` | D3D9 fixed function: rgb texture × diffuse, alpha diffuse | `vs_main`, `fs_main` |
| `fixed_tex_modulate_alpha.wgsl` | D3D9 fixed function: rgba texture × diffuse (sprite mode 4) | `vs_main`, `fs_main` |
| `fixed_tex_select.wgsl` | D3D9 fixed function: texture only | `vs_main`, `fs_main` |
| `text.wgsl` | glyph-atlas text (`FixedMode::Text`, Task 6.6) | `vs_main`, `fs_main` |
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
anything. md-render builds **all** vertex data exactly as the C++ does
(including its own half-texel UV biases and position offsets); the only
correction is the rasterization shift below. `blur_vs`
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
| `fixed_color` | | `vs_params: VsParams` (VS) | | |
| `fixed_tex_modulate`, `fixed_tex_modulate_alpha`, `fixed_tex_select`, `text`, `title` | | `vs_params: VsParams` (VS) | `tex: texture_2d<f32>` | `samp: sampler` |

`VsParams` sits at binding 2 (`md_hlsl::layout::BUILTIN_VS_PARAMS_BINDING`)
so that it never collides with the preset fragment shader's `U`/`T` when
`warp_vs`/`comp_vs` share a pipeline layout with it.

### `VsParams` (16 bytes, `md_hlsl::layout::BUILTIN_VS_PARAMS_SIZE`)

| Field | Type | Offset | Value |
|---|---|---|---|
| `texel_size` | `vec2<f32>` | 0 | `(1/W, 1/H)` of the W×H render target being drawn (unsigned) |
| `pixel_shift` | `f32` | 8 | `md_hlsl::layout::D3D9_PIXEL_SHIFT` (`0.5`); 0 disables the shift |
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

## D3D9 rasterization emulation (F29)

D3D9 samples pixel `i` at screen coordinate `i`; wgpu samples it at the
pixel centre `i + 0.5`. Every built-in vertex shader therefore moves its
clip-space position half a pixel right and down, in the shared helper

```wgsl
fn d3d9_raster(clip: vec4<f32>) -> vec4<f32> {
    let s = 2.0 * vs_params.pixel_shift * vec2<f32>(vs_params.texel_size.x, -vs_params.texel_size.y);
    return vec4<f32>(clip.xy + s * clip.w, clip.zw);
}
```

i.e. `clip.xy += (+1/W, -1/H) * clip.w` with `pixel_shift = 0.5` (NDC
`(+1/W, -1/H)` for the built-ins' `clip.w = 1`; the `* clip.w` keeps it a
fixed screen-space shift for any `w`). The value at wgpu's pixel centre of
every interpolated attribute (UVs, colour, `rad_ang`) is then exactly the
value D3D9 gave that pixel, and coverage matches too. The single source of
truth for the shift is `md_hlsl::layout::D3D9_PIXEL_SHIFT`; md-render writes
it into `VsParams.pixel_shift`. `texel_size` is always the **target**'s
`(1/W, 1/H)`:

| Pass | Target (`texel_size`) | What the C++ vertex data already contains (md-render keeps it) |
|---|---|---|
| warp (`warp_vs`) | warp texture, `1/texsize` | `+0.5/texsize` on `tu,tv` (milkdropfs.cpp:1835-1837) and `tu_orig,tv_orig` (plugin.cpp:2299-2300) |
| comp (`comp_vs`) | window / back buffer | grid positions `sx = (u - 0.5/W)*2 - 1`, `sy = -((v - 0.5/H)*2 - 1)` (plugin.cpp:2041-2058) |
| blur (`blur_vs`) | the blur texture written | fullscreen quad, v = 0 at the bottom (milkdropfs.cpp:1542-1562); the source texel size for the sampling offsets is `BlurUniforms._c0` |
| sprites, video echo, shapes, waves, text, title | the target drawn to | as built by the C++ (see below) |

Fixed-function draws build their UVs exactly as the C++ does, per quad:
`ShowToUser_NoShaders` (milkdropfs.cpp:4004-4120) extends the quad by one
pixel, maps v bottom-up (`v3[0]` at the top gets `tv = temp_hi`), scales UVs
by `1/fZoom` around 0.5 (`temp_lo/hi = 0.5 ∓ 0.5/fZoom`) and flips u/v by
`nVideoEchoOrientation` on the echo pass; sprites (milkdropfs.cpp:3600-3640)
use `±0.5` corners times `repeatx/repeaty` plus 0.5. md-render reproduces
those vertex values; the shaders add nothing but the raster shift.

## Fixed-function equivalents

The C++'s standard texture stage state is COLOROP `MODULATE(DIFFUSE,
TEXTURE)` with ALPHAOP `SELECTARG1 DIFFUSE` (milkdropfs.cpp:923-929,
1898-1904, 3448-3454, 3770-3776, 3991-3997; pluginshell.cpp:1790-1797).
Only sprite blend mode 4 (colour-keyed) uses ALPHAOP `MODULATE`
(milkdropfs.cpp:3714-3721).

| File | D3D9 texture stage 0 | Output |
|---|---|---|
| `fixed_color` | no texture (`SELECTARG1 DIFFUSE`, e.g. pluginshell.cpp:1832-1836) | vertex colour |
| `fixed_tex_modulate` | standard state above | `(texture.rgb * colour.rgb, colour.a)` |
| `fixed_tex_modulate_alpha` | sprite mode 4: COLOROP and ALPHAOP `MODULATE(DIFFUSE, TEXTURE)` | `texture * colour` (rgba) |
| `fixed_tex_select` | `SELECTARG1 TEXTURE` — no C++ draw uses it (every `SELECTARG1` in milkdropfs.cpp/pluginshell.cpp selects `DIFFUSE`); kept for md-render's own copies (`FixedMode::TexSelect`) | `texture` |
| `text` | glyph atlas (`md-text`, RGBA8, coverage in `.a`) | `(colour.rgb, colour.a * atlas.a)` |
| `title` | none set in `ShowSongTitleAnim` (milkdropfs.cpp:4477ff), so the standard state; blend ONE/ONE (4492-4494) | `(texture.rgb * colour.rgb, colour.a)` |

Blend state, topology and the fan/point conversions are pipeline state
chosen by md-render (Task 5.3), not part of these shaders.

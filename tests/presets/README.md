# Preset corpus

The `.milk` files under this directory are the test corpus used by
`md_testkit::corpus_presets()` (expression compile tests, preset parse/round-trip
tests, shader translation tests and the renderer golden suite).

## Origin and license

**Self-authored for this repository; BSD 3-Clause, the same license as the rest
of the project (see `NOTICE`).** No third-party presets or artwork are included.

No external preset pack could be fetched when the corpus was created, and the
licensing of community preset packs is unclear, so every preset here was written
from the format as MilkDrop 2 itself defines it:

- `code/vis_milk2/state.cpp` — `CState::Export` / `CState::Import`,
  `CWave::Export`, `CShape::Export`, `WriteCode` / `ReadCode`: the
  `MILKDROP_PRESET_VERSION=201` / `PSVERSION` / `PSVERSION_WARP` /
  `PSVERSION_COMP` header, the `[preset00]` section, every key in Export order
  with Export's number formats (`%.3f`, `%.5f`, `%d`), `wavecode_N_*` /
  `shapecode_N_*` blocks, numbered code lines (`per_frame_init_N`,
  `per_frame_N`, `per_pixel_N`, `wave_N_init/per_frame/per_pointM`,
  `shape_N_init/per_frameM`) and shader lines `warp_N` / `comp_N` with the
  leading ` character.
- `code/vis_milk2/plugin.cpp` — the default warp/comp shader text MilkDrop 2
  generates when it upgrades a MilkDrop 1 preset (`GenWarpPShaderText`,
  `GenCompPShaderText`), texture lookup (`textures/`, `randNN`, `_smalltiled`).
- `code/resources/Milkdrop2/data/include.fx` — the shader preamble (`GetBlur1..3`,
  `GetMain`, `lum`, `sampler_noise*`, `sampler_noisevol_*`, `q1..q32`, `_qa.._qh`,
  `rot_s1`, ...). Shaders are MilkDrop-dialect HLSL in the `shader_body { ... }` form.
- `code/resources/Milkdrop2/docs/milkdrop_preset_authoring.html` — variable names
  and the expression language (as implemented by `code/ns-eel2/`).

The MilkDrop 1 presets in `classic/` use the older layout (no header lines,
`%f` numbers, `bMotionVectorsOn`, no blur keys), which MilkDrop 2 loads as
`PSVERSION 0`.

`textures/mytex.png` is a 64x64 RGB image generated procedurally (rings over a
checkerboard) by a few lines of Python using only `zlib`; it is the user texture
`sampler_mytex` refers to.

## PSVERSION values

File value -> shader model, from `plugin.h` (`MD2_PS_*`): `0` none (MilkDrop 1
preset, or no `MILKDROP_PRESET_VERSION`), `2` ps_2_0, `3` ps_2_x (ps_2_a/ps_2_b),
`4` ps_3_0. `5` (ps_4_0) is never written by MilkDrop 2 ("not supported by
milkdrop"), so the corpus has none. Directories: `classic/` = 0, `ps2/` = 2,
`ps2x/` = 3, `ps3/` = 4 (plus `edge/`).

## Presets and the features they cover

| Preset | Features |
|---|---|
| `classic/bass_kaleido.milk` | MilkDrop 1; motion vectors (bMotionVectorsOn), sqr/sqrt/abs/pow/equal/sign in per-pixel |
| `classic/drift_tunnel.milk` | MilkDrop 1 layout, no shaders; per-frame/per-pixel zoom+rot with sin/cos/above/if |
| `classic/echo_feedback.milk` | MilkDrop 1; video echo, darken center, solarize/invert flags, wave mode 6 |
| `classic/orbiting_shapes.milk` | MilkDrop 1.04 custom shapes with init + per-frame code (t1..t8) |
| `classic/q_init_rand.milk` | MilkDrop 1; per_frame_init with rand() and q1..q8 carried to per-pixel |
| `classic/spectral_rings.milk` | MilkDrop 1.04 custom wave (spectrum, per-point code, value1/value2) |
| `edge/crlf_bom_junk.milk` | PSVERSION 2; Review Focus #3 fixture: UTF-8 BOM, CRLF line endings, trailing junk after [preset00] (kept byte-exact via .gitattributes -text) |
| `edge/eel_syntax_zoo.milk` | PSVERSION 2; EEL syntax: comments, blank lines, $x hex, $PI/$E/$PHI, ^, ^=, %, ?:, ==, &&, ||, !, a[b], bnot/band/bor/sigmoid/invsqrt/exec3/assign/memset/memcpy/freembuf |
| `edge/missing_texture.milk` | PSVERSION 2; references a texture that does not exist (Review Focus #1: fallback texture) |
| `ps2/blur_glow.milk` | PSVERSION 2; GetBlur1 + GetBlur2 glow in comp, blur1/2 min-max keys (b1n..b2x) |
| `ps2/borders_motion.milk` | PSVERSION 2; outer/inner borders and motion vectors driven from per-frame (ob_*, ib_*, mv_*), wave flags |
| `ps2/megabuf_trails.milk` | PSVERSION 2; megabuf ring buffer written in per-frame with loop(), read by a custom wave per-point |
| `ps2/noise_grain.milk` | PSVERSION 2; sampler_noise_lq + texsize_noise_lq, sampler_pw_noise_lq, sampler_rand00 + texsize, rand_frame |
| `ps2/qvar_bank.milk` | PSVERSION 2; q1..q16 from per-frame, read in per-pixel and in shaders via q-names and _qa.._qd |
| `ps2/rand_tiles.milk` | PSVERSION 2; random textures sampler_rand01_smalltiled, sampler_rand02, sampler_rand03 (+ texsize_) |
| `ps2/shader_rings.milk` | PSVERSION 2; shader-only picture (no waves/shapes): comp shader draws rings, spokes and a colour gradient from rad/ang/uv/time, modulated by bass and treb; Task 5.8 golden |
| `ps2/upgraded_default.milk` | PSVERSION 2; an MD1 preset upgraded by MilkDrop 2 (generated default warp/comp text) |
| `ps2/user_texture.milk` | PSVERSION 2; user texture sampler_mytex (textures/mytex.png) + texsize_mytex, textured custom shape |
| `ps2/version200.milk` | PSVERSION 2 written by MilkDrop 2.0 (MILKDROP_PRESET_VERSION=200: PSVERSION only, no _WARP/_COMP) |
| `ps2x/blur3_bloom.milk` | PSVERSION 3; GetBlur1/GetBlur2/GetBlur3 bloom with blur3 range keys and b1ed edge darken |
| `ps2x/four_shapes.milk` | PSVERSION 3; all four custom shapes enabled, instancing (num_inst/instance), rand(), textured shape |
| `ps2x/gmegabuf_share.milk` | PSVERSION 3; gmegabuf + reg00..reg03 shared across per-frame and two custom waves; while() loop; exec2 |
| `ps2x/rand_gallery.milk` | PSVERSION 3; random textures sampler_rand04..07 (one clamped/point-sampled via fc_/pw_ prefixes) |
| `ps2x/tex2dbias_soft.milk` | PSVERSION 3; tex2Dbias mip-biased sampling of main and of a noise texture |
| `ps2x/upgraded_echo.milk` | PSVERSION 3; upgraded MD1 preset whose generated comp shader uses video echo, wrap off (sampler_fc_main) |
| `ps3/all_q32.milk` | PSVERSION 4; all q1..q32 set in per-frame, read in shaders by name and as _qe.._qh banks |
| `ps3/four_waves.milk` | PSVERSION 4; all four custom waves (spectrum, dots, thick, additive, sep), wave per-frame t1..t8 |
| `ps3/loop_shader.milk` | PSVERSION 4; HLSL for-loop radial blur (ps_3_0), random textures sampler_rand08..11 |
| `ps3/mixed_versions.milk` | PSVERSION 4 with PSVERSION_WARP=2 and PSVERSION_COMP=4 (different per-shader versions) |
| `ps3/noise_volume.milk` | PSVERSION 4; tex3D on sampler_noisevol_hq + texsize_noisevol_hq, volumetric drift |
| `ps3/rand_mosaic.milk` | PSVERSION 4; random textures sampler_rand12..15 in a 2x2 mosaic |
| `ps3/sampler_modes.milk` | PSVERSION 4; main-texture sampler variants fc_/pc_/pw_/fw_, tex2Dlod, GetMain/GetPixel/lum macros |
| `ps3/volume_lq_rot.milk` | PSVERSION 4; tex3D on sampler_noisevol_lq with rot_s1 matrix (mul), hue_shader in comp |

`tests/support/md-testkit/tests/corpus.rs` asserts the corpus has at least 25
presets and that every A.4 feature (PSVERSION 0/2/3/4, `tex3D`, `tex2Dbias`,
`GetBlur1..3`, `sampler_rand00..15`, `sampler_noisevol_hq`, a user texture,
custom waves/shapes with code, `megabuf`/`gmegabuf`, `loop`/`while`, `reg00`,
`q1..q32`, the CRLF/BOM fixture) appears in at least one file.

## Notes on specific files

- `edge/crlf_bom_junk.milk` (Review Focus #3) is stored byte-exact
  (`.gitattributes`: `-text`). It must parse identically to the same preset
  without BOM/CRLF/junk. Note that the C++ loader does **not** strip the BOM: it
  would read the first key as `<BOM>MILKDROP_PRESET_VERSION`, miss it, and load
  the file as a MilkDrop 1 preset. The Rust parser strips the BOM deliberately.
- `edge/missing_texture.milk` (Review Focus #1) samples a texture that exists
  nowhere; renderers must bind the fallback texture.
- `edge/eel_syntax_zoo.milk` exercises EEL syntax ns-eel2 accepts that presets in
  the wild use: `$x1F`, `$PI`/`$E`/`$PHI`, `^=`, `a[b]`, `?:`,
  comparison/logical operators, `exec3`, `assign`, `memset`/`memcpy`/`freembuf`.
- `randNN` samplers pick a random image from the textures directory at load
  time, so presets using them are not pixel-deterministic in the C++ build.

## Running corpus tests against a real preset pack

Set `MD_EXTRA_PRESETS` to a directory and `corpus_presets()` also returns every
`*.milk` under it (recursively, case-insensitive extension), merged with this
corpus, sorted and deduplicated:

```sh
MD_EXTRA_PRESETS=/path/to/winamp-or-projectm-presets cargo test --workspace corpus
```

The self-authored corpus covers the format and every feature listed above, but
only real-world presets expose the long tail of quirks; run the corpus tests
against the Winamp MilkDrop 2 pack or the projectM pack before each release.

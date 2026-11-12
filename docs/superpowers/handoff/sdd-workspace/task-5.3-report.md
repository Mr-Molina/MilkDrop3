# Task 5.3 report: Pipeline cache and fixed-function emulation

Status: DONE_WITH_CONCERNS (minor; see Concerns)
Commit: b3709ba feat(render): pipeline cache and fixed-function emulation

## Implemented
- `crates/md-render/src/fixed.rs`:
  - `FixedMode { Color, TexModulate, TexModulateAlpha, TexSelect }`. Each mode loads its built-in WGSL with `include_str!` from `assets/shaders/`. A naga unit test validates all four.
  - `Topo { TriList, TriStrip, TriFan, LineStrip, LineList, Points }`. I added `TriFan` so a `DrawBatch` can receive fans. `canonical()` maps fans and points to the TriList pipeline.
  - `SamplerKind` × D3D9 sampler states, with C++ line refs. Linear uses LINEAR for MAG, MIN and MIP. Point uses POINT for all three (milkdropfs.cpp:3843-3846). Wrap/Clamp set U, V and W. The mip filter has no effect on single-level textures, which matches D3D9.
  - `fan_to_list`: (v0, vi, vi+1), giving n-2 triangles.
  - `points_to_quads(verts, size, (W,H))`: each point becomes a square of `size` px (half-extent size/W, size/H in NDC), clamped to at least 1 px.
  - F31: points are always expanded to quads, because D3D9 rasterizes a point as a square under the triangle fill rules. With the VS's +0.5 px shift this gives exactly the D3D9 coverage. wgpu's PointList is not used.
  - Point sizes the C++ uses are documented. DrawCustomWaves sets `(tex_w>=1024?2:1)+(thick?1:0)` (milkdropfs.cpp:2650-2652) and resets it to 1 afterwards. DrawWave uses the default size 1 and thickens dots by redrawing offset copies (milkdropfs.cpp:3258-3287).
- `crates/md-render/src/blend.rs`: `BlendMode` with every SRCBLEND/DESTBLEND pair the C++ sets. A table in the module docs gives the C++ line refs (milkdropfs.cpp, pluginshell.cpp, textmgr.cpp).
  - Variants from the brief: Off, Alpha, Additive, Multiply, Replace, InvAlpha. Multiply (ZERO/SRCCOLOR) is not used anywhere in the C++; I kept it because the brief lists it.
  - Variants the brief's list missed: AlphaAdditive (SRCALPHA/ONE, used by additive shapes and waves, which is not ONE/ONE), SrcColor (SRCCOLOR/INVSRCCOLOR, sprite mode 3), InvDestColor (INVDESTCOLOR/ZERO), DestColor (ZERO/DESTCOLOR), ZeroInvDestColor, DestColorAdd (DESTCOLOR/ONE) and ZeroInvSrcColor (title shadow, mfs:4700).
  - `state()` uses identical colour and alpha components with Add, as D3D9 does without SEPARATEALPHABLENDENABLE.
- `crates/md-render/src/pipelines.rs`:
  - `PipelineKey { mode, blend, topo, target_format }`.
  - `PipelineCache` creates everything lazily. Its `get(gpu, key)` caches pipelines by the canonical key. Every layout has VsParams at (0,2) for the VS, and textured modes add group 1: tex at 0, samp at 1. Culling is off.
  - Samplers are cached per kind. The VsParams bind group is cached per target size, filled with `VsParams::for_target`, i.e. `D3D9_PIXEL_SHIFT`; it is flushed at 16 entries.
  - Texture bind groups are cached per (TextureView, SamplerKind), flushed at 256 entries or by `clear_bind_groups()`.
  - `DrawBatch<'a>`: `new(gpu, cache, vbuf, RenderPass, w, h)` or `begin(gpu, cache, vbuf, encoder, &Texture2d, load)`, plus `set_point_size`, `pass()`, `draw(key, verts, texture, sampler)` and `finish()`. `draw` does the fan and point conversion and uploads through the `DynamicVertexBuffer`. It panics if a textured mode gets no texture.
- `lib.rs` re-exports the new items.

## TDD evidence
- RED: `MD_REQUIRE_GPU=1 cargo test -p md-render --test pipelines` failed with `error[E0432]: unresolved imports md_render::fan_to_list, ... PipelineKey, SamplerKind, Topo` before `lib.rs` was wired up.
- GREEN: the same command gives `test result: ok. 8 passed`, run on lavapipe. The tests are:
  - color_triangle_renders_expected_pixels
  - additive_blend_saturates
  - fan_conversion_produces_n_minus_2_triangles (CPU check plus a GPU fan quad)
  - points_become_quads (CPU check plus GPU: a size-1 point covers exactly pixel (5,7); a size-3 point covers exactly the 3×3 block around it)
  - every_blend_mode_matches_d3d9_formula (all 13 modes; expected bytes worked out by hand in comments, src alpha 64 so Alpha and InvAlpha give different results; tolerance ±1)
  - textured_modes_follow_d3d9_stage_states (Modulate, ModulateAlpha, Select, Color)
  - lines_draw_one_pixel_wide
  - pipeline_cache_reuses_pipelines
- Unit tests in `fixed.rs` cover the naga validation of the fixed WGSL, the sampler descriptors and the canonical topologies.
- Mutation check: swapping InvAlpha's factors made the blend test fail (`InvAlpha: got [126,140,51,169], want [175,113,50,99]`), and it was then restored.
- Workspace: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` and `cargo doc -p md-render` are clean. `MD_REQUIRE_GPU=1 cargo test --workspace` passes in full.
- Note: the brief's filter `cargo test -p md-render pipelines` only matches test names containing "pipelines" (1 test). Use `--test pipelines` to run the whole file.

## Files
- created: `crates/md-render/src/{blend,fixed,pipelines}.rs`, `crates/md-render/tests/pipelines.rs`
- modified: `crates/md-render/src/lib.rs`

## Self-review / concerns
- D3D9 drops a whole point when its centre is outside the viewport. Quads are only partly clipped, so edge pixels can differ for dots at the screen border.
- The brief's interface had no fan topology, so I added `Topo::TriFan`. I also added the extra BlendMode variants above (rulings asked for the exact C++ pairs).
- Cached texture bind groups hold TextureView clones, so they keep textures alive until a flush. Callers should call `clear_bind_groups()` after resize or a texture reload.
- Off and Replace produce identical pixels; both are kept for clarity.

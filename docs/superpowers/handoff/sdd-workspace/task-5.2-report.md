# Task 5.2 report: GPU context, vertex layouts, and resource types

Status: DONE

## What was implemented (crates/md-render)

- `Cargo.toml`: `wgpu` (workspace pin 30.0.1, default features) and `md-hlsl` deps; dev-dep `naga` (wgsl-in) to validate the inline mip-blit WGSL without a GPU. No bytemuck (F11), no pollster (own 15-line safe `block_on` via `std::task::Wake`).
- `lib.rs`: module list + re-exports (`pub use wgpu`). `forbid(unsafe_code)` kept.
- `gpu.rs`: `Gpu` with **no lifetime** (F6): private owned `wgpu::Device`/`wgpu::Queue` clones + an `Arc` mip-pipeline cache; `Gpu::new(&Device, &Queue)` clones (A.3 signatures keep working), `device()`, `queue()`, `wait_idle()`. `headless_device() -> Option<(Device, Queue)>`: instance from env (`WGPU_BACKEND` etc.), honours `WGPU_ADAPTER_NAME` (case-insensitive substring; returns `None` if nothing matches instead of wgpu-util's panic), requests the adapter's own limits; `None` on no adapter. `headless_gpu()`, and `gpu_or_skip(test)` for GPU tests: prints a skip note, or panics when `MD_REQUIRE_GPU=1`.
- `vertex.rs`: `#[repr(C)] Vertex { pos, color: [u8;4], uv: [f32;4], rad_ang }`, `SIZE = 40`, `ATTRIBUTES`/`LAYOUT` exactly per assets/shaders/README.md; `Vertex::color_from_d3dcolor(0xAARRGGBB) -> [r,g,b,a]` (ledger 4.8 note: vertex colour bytes are RGBA); hand serialization `write_to`/`to_bytes`, `vertices_to_bytes`.
- `uniforms.rs` (F29: imports `md_hlsl::layout`, no md-render layout.rs): `pack_mat4x3(hlsl_rows) -> [[f32;4];4]` (WGSL column i = HLSL row i, padded, F24); `MdUniforms { c[14], q[8], rand_frame, rand_preset, rot[24] (HLSL rows; order s,d,f,vf,uf,rand ×1..4) }` with `SIZE = MD_UNIFORMS_SIZE`, `write_to`/`to_bytes` (1920 B) and a const assert that the struct covers exactly `MD_UNIFORM_FIELDS.len()` fields. **Decision:** `VsParams { texel_size, pixel_shift }` (`for_target(w,h)` fills 1/W,1/H unsigned + `D3D9_PIXEL_SHIFT`; 16 B) and `BlurUniforms { c[7] }` (112 B) serializers live here since they are pure layout; filling blur values is left to the blur-pass task (6.1). `texsizes_bytes(&[[f32;4]])` sized by `texsizes_size(n)` (>= 16 B, ledger 4.4 note).
- `bytes.rs`: small `ByteWriter` (little-endian f32/bytes) + `to_array::<N>` helper used by all serializers.
- `texture.rs`: `Texture2d { tex, view, w, h, mips: u32, format }` (pub fields); `new_target(gpu, w, h, format, mips: bool)` (dims clamped >= 1; usage TEXTURE_BINDING|RENDER_ATTACHMENT|COPY_SRC|COPY_DST; `mips = true` → full chain `floor(log2(max))+1`), `from_rgba8(gpu, w, h, &[u8], mips)` (Rgba8Unorm, uploads level 0, GPU mips), `mip_size(level)`, `level_view(level)`. `Texture3d { tex, view, w, h, d, mips }::from_rgba8(gpu, w, h, d, &[u8], mips)`: Rgba8Unorm volume; the mip chain is built on the CPU (2×2×2 box, rounded) because 3D levels cannot be blitted as 2D targets and the noise volumes are tiny (32³). `full_mip_count(w,h,d)`.
- `mips.rs`: `generate_mips(gpu, &Texture2d)` — built-in blit pipeline: fullscreen triangle, `textureSampleLevel` of level i-1 at dst texel centre with a linear/clamp sampler (exact 2×2 box for even sizes), one render pass per level, one submit. **Decision:** WGSL is inline (`MIP_BLIT_WGSL`, private, ~25 lines) rather than in assets/shaders/ (so md-hlsl's builtin test file list is untouched); validated by the unit test `mip_blit_wgsl_validates` (naga) and by wgpu at pipeline creation. Pipelines cached per target format inside `Gpu` (shared by clones). Works for Rgba8Unorm and Rgba16Float.
- `dynbuf.rs`: `DynamicVertexBuffer::new(gpu, capacity_verts)`, `begin_frame()`, `write(&[Vertex]) -> wgpu::BufferSlice` (signature as the brief: it stores a `Gpu` clone), `buffer()`, `capacity_bytes()`, `used_bytes()`. Ring per frame: consecutive ranges within a frame, cursor reset on `begin_frame`. **Overflow = grow:** new buffer of `max(2×capacity, needed)`, the write goes to its offset 0; earlier slices keep the old buffer alive (refcounted), larger buffer kept for later frames. Empty write returns a valid one-vertex slice (wgpu slices cannot be empty) to draw 0 vertices from.
- `readback.rs`: `map_read(gpu, &Buffer)` and `read_texture_level(gpu, &Texture, level)` (2D/3D, any uncompressed format, unpads 256-byte rows) — blocking helpers for tests and later the headless render path.

## Tests

`crates/md-render/tests/layout.rs` (CPU): `uniform_struct_is_1920_bytes`, `every_uniform_field_lands_at_md_uniform_offset` (distinct marker in every float of every field; expected values mapped by name independently of the serializer and read at `md_uniform_offset(name)`), `mat4x3_packing_puts_hlsl_rows_in_wgsl_columns` (F24 rename; also checks in-block placement at offset + 16·i and zero pad), `vertex_layout_matches_builtin_shader_table`, `vertex_bytes_are_little_endian_in_field_order`, `d3dcolor_is_swizzled_to_rgba_bytes`, `vs_params_layout`, `blur_uniforms_layout`, `texsizes_block_is_never_empty`.

`crates/md-render/tests/gpu.rs` (GPU, via `gpu_or_skip`): `headless_device_is_available_on_ci`, `gpu_new_clones_borrowed_handles`, `mip_generation_halves_dimensions` (64×16 1-px checkerboard: 7 levels, sizes halve, level 0 round-trips, every lower level mid-grey 126..129), `mip_generation_works_for_rgba16f_targets` (exact f16 bit patterns at 1×1), `texture_without_mips_has_one_level`, `texture3d_uploads_full_mip_chain`, `dynamic_vertex_buffer_is_a_per_frame_ring`, `dynamic_vertex_buffer_grows_on_overflow` (readback of both old and new buffers).

Unit: `mips::tests::mip_blit_wgsl_validates`.

### RED
`cargo test -p md-render` with only the test files written:
```
error[E0432]: unresolved imports `md_render::pack_mat4x3`, `md_render::texsizes_bytes`, `md_render::vertices_to_bytes`, `md_render::BlurUniforms`, `md_render::MdUniforms`, `md_render::Vertex`, `md_render::VsParams`
error[E0432]: unresolved imports `md_render::generate_mips`, `md_render::gpu_or_skip`, `md_render::read_texture_level`, ... `md_render::Gpu`, `md_render::Texture2d`, `md_render::Texture3d`, `md_render::Vertex`
error[E0425]: cannot find function `headless_device` in crate `md_render`
error: could not compile `md-render` (test "gpu") due to 3 previous errors
error: could not compile `md-render` (test "layout") due to 1 previous error
```

### GREEN (lavapipe, GPU tests really run)
```
$ cargo test -p md-render
test mips::tests::mip_blit_wgsl_validates ... ok                       (1 passed)
gpu.rs: 8 passed; 0 failed     layout.rs: 9 passed; 0 failed
$ MD_REQUIRE_GPU=1 cargo test -p md-render --test gpu          -> ok. 8 passed
$ WGPU_ADAPTER_NAME=llvmpipe MD_REQUIRE_GPU=1 cargo test -p md-render --test gpu -> ok. 8 passed
$ WGPU_ADAPTER_NAME=nosuchgpu cargo test -p md-render --test gpu -- --nocapture
skip headless_device_is_available_on_ci: no GPU adapter (set MD_REQUIRE_GPU=1 to fail instead)   (all 8 skip, pass)
$ WGPU_ADAPTER_NAME=nosuchgpu MD_REQUIRE_GPU=1 cargo test -p md-render --test gpu headless
panicked at crates/md-render/src/gpu.rs:110:13: headless_device_is_available_on_ci: no GPU adapter, but MD_REQUIRE_GPU=1
test result: FAILED. 0 passed; 1 failed
```
Workspace: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` clean; `cargo test --workspace` all ok (0 failing result lines, no warnings). `scripts/check-no-native-build.sh` → OK; `cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok` (the two `unused-wrapper` warnings for wayland-backend/android-activity pre-exist — winit is not in the graph yet).

## Files changed
- crates/md-render/Cargo.toml, Cargo.lock
- crates/md-render/src/{lib.rs, bytes.rs, gpu.rs, vertex.rs, uniforms.rs, texture.rs, mips.rs, dynbuf.rs, readback.rs}
- crates/md-render/tests/{layout.rs, gpu.rs}

## Self-review / deviations
- No `layout.rs` in md-render (F29); `Gpu` has no lifetime (F6); no bytemuck (F11); test renamed per F24.
- `Texture2d::new_target`/`from_rgba8` take `mips: bool` (full chain or none) — the milestone requires full chains; `Texture2d.mips` holds the level count.
- Added files beyond the brief's list (bytes.rs, mips.rs, dynbuf.rs, readback.rs) to keep one responsibility per file; all < 210 lines.
- `cargo test -p md-render gpu` (brief Step 2 filter) only matches test names containing "gpu"; the GPU tests live in the `gpu` test binary — run `cargo test -p md-render --test gpu` or the whole crate.
- `headless_device` requests `adapter.limits()` (so large targets work headless); differs per machine, acceptable for tests.
- Readback `wait_idle` panics on a device poll error (test/headless helper).

## Concerns
- None blocking. Mip filtering for odd sizes is bilinear at the destination centre (not an exact box); D3D9 autogen filtering is driver-defined anyway, goldens will judge.

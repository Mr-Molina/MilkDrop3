### Task 5.2: GPU context, vertex layouts, and resource types

**Files:**
- Create: `crates/md-render/src/{lib.rs,gpu.rs,vertex.rs,texture.rs,uniforms.rs,layout.rs}`

**Interfaces:**
- Produces: `pub struct Gpu<'a> { device: &'a wgpu::Device, queue: &'a wgpu::Queue }`; `#[repr(C)] pub struct Vertex { pos: [f32;3], color: [u8;4], uv: [f32;4], rad_ang: [f32;2] }` (one layout serves `MYVERTEX`, `WFVERTEX`, `SPRITEVERTEX`; unused fields are zero); `pub struct Texture2d { tex, view, w, h, mips }` with `Texture2d::new_target(gpu, w, h, format, mips)`, `Texture2d::from_rgba8(gpu, w, h, &[u8], mips)`, `Texture3d`; `pub struct MdUniforms` (`bytemuck::Pod`, 1920 bytes, field order per Task 4.4) and `pub fn pack_mat4x3(hlsl_rows: [[f32;3];4]) -> [[f32;4];4]` (columns padded); `pub struct DynamicVertexBuffer` with `write(&[Vertex]) -> wgpu::BufferSlice` ring-allocated per frame; `pub fn generate_mips(gpu, &Texture2d)` using a built-in blit pipeline.
- `pub fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)>` in `gpu.rs` for tests (`WGPU_ADAPTER_NAME` honored; returns `None` when no adapter so tests can `skip`).

- [ ] **Step 1:** Write failing tests `uniform_struct_is_1920_bytes`, `mat4x3_packing_puts_hlsl_columns_in_wgsl_columns`, `headless_device_is_available_on_ci` (fails loudly in CI only, via env `MD_REQUIRE_GPU=1`), `mip_generation_halves_dimensions`.
- [ ] **Step 2:** Run `cargo test -p md-render gpu`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS (GPU tests skipped locally when no adapter).
- [ ] **Step 5:** Commit: `feat(render): gpu context, vertex layout, uniforms`


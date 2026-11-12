### Task 5.3: Pipeline cache and fixed-function emulation

**Files:**
- Create: `crates/md-render/src/{pipelines.rs,fixed.rs,blend.rs}`

**Interfaces:**
- Produces: `pub enum FixedMode { Color, TexModulate, TexSelect }`, `pub enum BlendMode { Off, Alpha, Additive, Multiply, Replace, InvAlpha }`, `pub enum Topo { TriList, TriStrip, LineStrip, LineList, Points }`, `pub struct PipelineKey { mode, blend, topo, target_format }`, `pub struct PipelineCache { pub fn get(&mut self, gpu, key) -> &wgpu::RenderPipeline }`, `pub struct DrawBatch<'a> { pub fn draw(&mut self, key: PipelineKey, verts: &[Vertex], texture: Option<&Texture2d>, sampler: SamplerKind) }` (fan and point conversion happen here), `pub enum SamplerKind { LinearWrap, LinearClamp, PointWrap, PointClamp }`.

- [ ] **Step 1:** Write failing GPU tests `color_triangle_renders_expected_pixels` (render a red triangle to a 64x64 target, read back, assert pixel (32,40) is red and (2,2) is black), `additive_blend_saturates`, `fan_conversion_produces_n_minus_2_triangles`, `points_become_quads`.
- [ ] **Step 2:** Run `cargo test -p md-render pipelines`. Expected: FAIL.
- [ ] **Step 3:** Implement using the WGSL from Task 4.8.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): pipeline cache and fixed-function emulation`


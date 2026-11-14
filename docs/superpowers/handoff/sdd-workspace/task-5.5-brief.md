### Task 5.5: Preset shader pipelines

**Files:**
- Create: `crates/md-render/src/{preset_gpu.rs,shader_cache.rs}`

**Interfaces:**
- Produces: `PresetGpu { warp: Option<ShaderProgram>, comp: Option<ShaderProgram> }`, `ShaderProgram { pipeline, bind_group_1, texsize_values: Vec<[f32;4]>, samplers: Vec<SamplerBinding> }`; `Renderer::load_preset` as in A.3 which, for each of warp and comp with `PsVersion != None`, calls `md_hlsl::translate`, falls back to the built-in default shader (`warp_ps`/`comp_ps` bodies from the `.fx` files) on error while recording `RenderError::ShaderFallback { which, error }` in `Renderer::last_errors()`, creates the `wgpu::ShaderModule` and pipeline (target format of the main texture, no blending for pass 0, `Alpha` for blend pass 1), and resolves every `SamplerBinding` to a texture view: `main` -> the other ping-pong texture, `blur1..3` -> blur textures 1, 3, 5, noise names, `randXX`, else `TextureStore::get(root)` or the fallback 1x1 white texture.
- `ShaderCache`: `HashMap<u64 /* blake3 of assembled HLSL */, Arc<wgpu::ShaderModule>>` in memory plus an on-disk WGSL cache under `md_platform::cache_dir()/shaders/<hash>.wgsl` so translation runs once per preset per machine.

- [ ] **Step 1:** Write failing GPU tests `default_warp_preset_builds_pipelines`, `shader_error_falls_back_and_records`, `sampler_main_binds_other_ping_pong_texture`, `cache_hit_skips_translation` (count translator calls via a test hook).
- [ ] **Step 2:** Run `cargo test -p md-render preset_gpu`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): preset shader pipelines and cache`


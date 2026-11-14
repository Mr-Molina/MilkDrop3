### Task 5.4: Texture store, image loading, and noise textures

**Files:**
- Create: `crates/md-render/src/{textures.rs,image_load.rs,noise.rs}`

**Interfaces:**
- Produces: `pub struct TextureStore { pub fn new(gpu, seed: u64) -> TextureStore; pub fn scan_dir(&mut self, dir: &Path); pub fn get(&mut self, gpu, root: &str) -> Option<&Texture2d> /* loads on demand; tries .jpg .png .tga .bmp .dds in that order as plugin.cpp does */; pub fn noise2d(&self, name: &str) -> &Texture2d; pub fn noise3d(&self, name: &str) -> &Texture3d; pub fn random_texture_names(&self) -> &[String]; pub fn size_of(&self, root: &str) -> Option<(u32,u32)> }`; `pub fn load_image(path: &Path) -> Result<RgbaImage, ImageError>` via `image` for jpg/png/tga/bmp and `ddsfile` for uncompressed DDS (BC-compressed DDS returns `ImageError::Unsupported`); budget `max_images = 3000`, `max_bytes = 2_000_000_000` with least-recently-used eviction as `m_nMaxImages`/`m_nMaxBytes`.
- `rand00..rand15` bind to 16 textures chosen at preset load from `random_texture_names()` with the preset's RNG (as `CacheParams` does for `sampler_randXX`); `texsize_randXX` follows the chosen file.

- [ ] **Step 1:** Write failing tests `noise_lq_is_256_with_9_mips`, `noise_values_within_range_for_zoom` (zoom > 1 implies channel in `[108, 324)` clamped to 255 as the C++ does via `DWORD` overflow into the next byte: pin whichever behavior the C++ actually produces by reading `AddNoiseTex`; record it), `missing_file_returns_none`, `extension_search_order`, `lru_eviction_respects_budget`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): texture store, image loading, noise textures`


### Task 6.7: Song title texture and animation

**Files:**
- Create: `crates/md-render/src/title.rs`

**Interfaces:**
- Produces: `Renderer::render_title_texture(&mut self, text: &str)` (renders the title with the `songtitle` font at double size into a 1024x256 (or device-limited) texture as `RenderStringToTitleTexture`), `Renderer::draw_title_anim(ctx, progress: f32, seed)` reproducing `ShowSongTitleAnim`: a `32x8`-ish mesh distorted by the per-preset random values over `progress`, drawn additively with the title texture; parameters copied from the C++ into constants with line references.

- [ ] **Step 1:** Write failing GPU test `title_anim_at_progress_half_is_visible`.
- [ ] **Step 2:** Run the test. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the test. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): song title texture and animation`


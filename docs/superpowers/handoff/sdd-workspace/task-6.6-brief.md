### Task 6.6: Text rendering pass

**Files:**
- Create: `crates/md-render/src/text_pass.rs`

**Interfaces:**
- Produces: `pub struct TextQueue { pub fn draw(&mut self, font: FontId, text: &str, rect: Rect, flags: TextFlags, color: [u8;4], dark_box: Option<[u8;4]>) -> Rect /* calc rect */; pub fn draw_box(&mut self, rect, color) }` and `Renderer::flush_text(ctx, &mut TextQueue)` which rasterizes queued items in order, boxes before text, with `CTextManager`'s behavior of caching: the queue is drawn to the screen each frame (the C++'s text-surface caching is an optimization the Rust version does not need); fonts are the six `FontSpec`s from config (`simple`, `decorative`, `help`, `playlist`, `extra1`, `extra2`) created through `md-text`.

- [ ] **Step 1:** Write failing GPU test `text_pass_draws_visible_glyphs` (draw "MilkDrop" white on black, assert some pixels are non-black inside the calc rect and none outside), `dark_box_precedes_text`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): text pass`


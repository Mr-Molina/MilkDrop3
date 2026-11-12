### Task 5.1: Font atlas and layout (`md-text`)

**Files:**
- Create: `crates/md-text/src/{lib.rs,atlas.rs,layout.rs,fonts.rs}`, `assets/fonts/<bundled>.ttf`

**Interfaces:**
- Produces: `Atlas::new(spec: &FontSpec, db: &fontdb::Database) -> Atlas` (falls back to the bundled font when the face is not found), `Atlas::texture_rgba(&self) -> (&[u8], u32, u32)` (dirty-tracked, grows by doubling to 2048 max), `layout` as in A.3 returning glyph quads in pixel space; `Align::{Left, Center, Right}`; `word_ellipsis` truncates at word boundaries and appends `...`; `single_line` ignores `\n`; `no_prefix` keeps `&` literal (otherwise `&&` -> `&` and a single `&` is dropped, as `DrawText` does); the returned `bounds` is what `DT_CALCRECT` would return.

- [ ] **Step 1:** Write failing tests `calc_rect_height_equals_line_height_times_lines`, `right_align_ends_at_rect_right`, `word_ellipsis_truncates_at_word`, `ampersand_prefix_rules`, `missing_face_uses_bundled_font`.
- [ ] **Step 2:** Run `cargo test -p md-text`. Expected: FAIL.
- [ ] **Step 3:** Implement with `fontdue` rasterization and a shelf packer.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(text): glyph atlas and DrawText-compatible layout`


### Task 6.8: Image sprites

**Files:**
- Create: `crates/md-render/src/sprites.rs`

**Interfaces:**
- Produces: `pub struct SpriteFrame { x, y, sx, sy, repeatx, repeaty, rot, flipx, flipy, r, g, b, a, blendmode: u8, burn: bool, texture: TextureId }` (values clamped as `DrawUserSprites` does: `x,y` in `[-1000, 1000]` after `*2-1`, `sx,sy` in `[-1000, 1000]`, `repeat` in `[0.01, 100]`), `Renderer::draw_sprites(ctx, &[SpriteFrame])` with blend modes 0 = alpha, 1 = additive, 2 = multiply (`Zero, SrcColor`), 3 = `InvAlpha` ... pinned from the C++ `switch (blendmode)`; `burn` draws into `VS[0]` instead of the screen; the `colorkey` option becomes alpha at load time in `TextureStore`.

- [ ] **Step 1:** Write failing tests `clamps_match_cpp`, `blendmode_table`, and GPU test `missing_texture_binds_fallback` (a `SpriteFrame` whose texture failed to load draws the 1x1 white fallback tinted by `r,g,b,a` and logs exactly once per name).
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): image sprites`


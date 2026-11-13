### Task 6.3: Custom shapes

**Files:**
- Create: `crates/md-render/src/shapes.rs`

**Interfaces:**
- Produces: `pub struct ShapeFrameVars { x, y, rad, ang, tex_ang, tex_zoom, r, g, b, a, r2, g2, b2, a2, border_r, border_g, border_b, border_a, sides, textured, additive, thick, instance, num_inst }` filled by `md-core` after running the shape's per-frame program for each instance; `pub fn build_shape(v: &ShapeFrameVars, aspect, out_fill: &mut Vec<Vertex>, out_border: &mut Vec<Vertex>)`: sides clamped to `[3, 100]`, center vertex color `(r,g,b,a)`, ring color `(r2,g2,b2,a2)`, ring vertex `j` at angle `j/sides * 2π + ang + π/4` scaled by `rad` and aspect; textured UVs `0.5 + 0.5 * cos/sin(t*2π + tex_ang + π/4) / tex_zoom * aspect_y`; border as a line strip with `thick` four-offset copies; fill drawn as a fan (converted) with `TexModulate` on `VS[1]` when textured else `Color`.

- [ ] **Step 1:** Write failing tests `sides_clamped_to_100`, `fan_has_sides_plus_2_vertices`, `textured_uv_center_is_half_half`, `border_closes_the_loop`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): custom shapes`


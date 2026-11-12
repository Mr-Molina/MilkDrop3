### Task 4.8: Built-in WGSL passes

**Files:**
- Create: `assets/shaders/{warp_vs.wgsl, comp_vs.wgsl, blur_vs.wgsl, blur1_ps.wgsl, blur2_ps.wgsl, fixed_color.wgsl, fixed_tex_modulate.wgsl, fixed_tex_select.wgsl, text.wgsl, title.wgsl}`
- Test: `crates/md-hlsl/tests/builtin_wgsl.rs`

**Interfaces:**
- Produces: hand-written WGSL for the eight `data/*.fx` files and for the fixed-function emulation: `fixed_color` (vertex color only), `fixed_tex_modulate` (texture × vertex color), `fixed_tex_select` (texture only); each with `VsIn { pos: vec3, color: vec4, uv: vec4, rad_ang: vec2 }` matching `MYVERTEX`, `WFVERTEX`, `SPRITEVERTEX` through three vertex layouts defined in Task 5.2. `blur1_ps` and `blur2_ps` take `_c0.._c3` from a small uniform block as the `.fx` comments describe.

- [ ] **Step 1:** Write test `every_builtin_wgsl_validates` (naga parse and validate each file).
- [ ] **Step 2:** Run the test. Expected: FAIL (files missing).
- [ ] **Step 3:** Write the shaders, translating the `.fx` math by hand (the blur weights `w[8] = {4.0, 3.8, 3.5, 2.9, 1.9, 1.2, 0.7, 0.3}` and the `d1..d4`, `w_div` derivation from `blur1_ps.fx` comments are copied exactly).
- [ ] **Step 4:** Run the test. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): built-in WGSL passes`


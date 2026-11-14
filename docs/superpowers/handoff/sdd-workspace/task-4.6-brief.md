### Task 4.6: Wrapper assembly and the public `translate`

**Files:**
- Create: `crates/md-hlsl/src/{wrap.rs,lib.rs}`, `assets/hlsl/include.fx` (copied from `code/resources/Milkdrop2/data/include.fx`)

**Interfaces:**
- Produces: `translate` as in A.3, composing preamble + defines + wrapped body, preprocess, lex, parse, check, bindings, emit, then naga validation; a naga validation failure is returned as `TranslateError` with the naga message, since it indicates a translator bug.

- [ ] **Step 1:** Write failing tests `default_warp_shader_translates`, `default_comp_shader_translates`, `shader_body_missing_is_error`, `comp_defines_hue_shader`, `warp_uv_orig_is_zw_and_comp_uv_orig_is_xy`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(hlsl): wrapper assembly and translate entry point`


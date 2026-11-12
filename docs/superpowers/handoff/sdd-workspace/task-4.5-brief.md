### Task 4.5: Sampler and uniform binding resolution

**Files:**
- Create: `crates/md-hlsl/src/bindings.rs`

**Interfaces:**
- Produces: `pub fn resolve_bindings(m: &TypedModule) -> Result<Bindings, TranslateError>` assigning slots to samplers in order of first declaration (preamble samplers that the body never references are not bound; limit 16, error beyond), decoding the prefix rules into `SamplerBinding { wrap, bilinear, root }`, and collecting `texsize_*` globals the body references into `texsize_names` in order of first reference.

- [ ] **Step 1:** Write failing tests `prefix_fc_means_bilinear_clamp`, `reversed_prefix_cf_is_accepted`, `blur_defaults_to_clamp`, `unused_preamble_samplers_are_not_bound`, `seventeenth_sampler_is_error`, `texsize_main_is_collected`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(hlsl): sampler and texsize binding resolution`


### Task 4.7: Legacy fixups

**Files:**
- Create: `crates/md-hlsl/src/fixups.rs`, `crates/md-hlsl/tests/fixtures/*.hlsl`

**Interfaces:**
- Produces: `pub fn apply_fixups(src: &str) -> String`, run before lexing, with one rule per known corpus idiom: trailing `;` after `}` of the body, `float2 x = float2(a);` single-arg vector constructors (splat), use of `uv_orig` in comp (alias of `uv`), `tex2D(sampler_main, uv).xyz;` where `ret` is `float3`, assignment of `float4` texture results to `ret` (`ret = tex2D(...)` -> truncation handled by Task 4.3), `#define` redefinition, `//` comments ending with `\` (not a continuation in fxc), and identifiers that are WGSL keywords.

- [ ] **Step 1:** Write one test per fixup, each loading a fixture and asserting `translate` succeeds.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement; each fixup is a function with a doc comment citing an example preset from the corpus.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(hlsl): corpus fixups`


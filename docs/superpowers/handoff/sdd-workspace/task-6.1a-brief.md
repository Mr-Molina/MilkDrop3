### Task 6.1a: single-level main textures (controller-ruled follow-up to 6.1)

**Why:** the C++ baseline creates the main ping-pong textures VS[0]/VS[1] with one mip level (code/vis_milk2/plugin.cpp:1904: `D3DUSAGE_AUTOGENMIPMAP` is commented out). md-render currently gives them a full mip chain and regenerates it after the warp pass (crates/md-render/src/warp.rs:367 `record_mips`), following milestone-5-context.md's "Main textures" bullet, which the ledger ruling `Task 6.1a: Ruling:` now overrides.

**Files:** Modify the md-render code that allocates VS[0]/VS[1] (see tex_size.rs/renderer.rs/texture.rs) and warp.rs; update tests and doc comments that state or assume a main-texture mip chain. Remove now-dead main-texture mip plumbing; keep generate_mips/record_mips for noise and user textures, which keep their mips.

**Requirements:**
- VS[0] and VS[1] have mip_level_count 1 at every tex_bits setting and size mode, including after `resize`.
- No mip generation on the main textures anywhere in the frame loop (warp, composite, blur).
- Sampling main (sampler_main, GetMain, tex2Dbias/tex2Dlod in preset shaders, blur pass 0, no-shader composite) reads level 0. Check the sampler/view setup and the composite and blur bindings still validate with a 1-level view.
- `_c12` (log2 of the window width, milkdropfs.cpp:3867-3868) is unchanged.
- Remove or correct comments that say main needs mips regenerated after drawing (for example the hook comment added in 6.1 for tasks 6.2-6.7, and the blur_passes doc comment at blur.rs:324-326, a 6.1 minor).

- [ ] **Step 1:** Write failing test `main_textures_are_single_level`: after Renderer::new and after resize, both main textures report mip_level_count 1.
- [ ] **Step 2:** Run it. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the full md-render GPU suite and apps/milkdrop golden_basic with MD_REQUIRE_GPU=1. Expected: PASS. If tests/golden/rust/default_60.png changes, view the new render and report the DSSIM before re-blessing; re-bless only if the change follows from this ruling.
- [ ] **Step 5:** Commit: `fix(render): single-level main textures as in the C++`

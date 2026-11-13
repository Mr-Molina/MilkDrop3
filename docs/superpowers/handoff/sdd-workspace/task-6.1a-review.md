### Spec Compliance
✅ VS[0]/VS[1] single-level: tex_size.rs:97 `new_target(.., false)`; `Renderer::new` (renderer.rs:194) and `resize` (renderer.rs:293) both go through `main_textures`, so every tex_bits and size mode, including after resize, is covered.
✅ No main mip generation in the frame loop: the only `record_mips` call (warp.rs end of `warp_pass`) and its import are removed; grep of src finds no other `record_mips`/`generate_mips` caller outside texture.rs (noise/user uploads) and mips.rs.
✅ Level-0 sampling and binding validity: `view` for a 1-level texture covers its single level; pipelines.rs:347 already falls back to `target.view` when `mips <= 1`; frame.rs clear loop iterates `0..src.mips` = 1 level. The reported full md-render and golden_basic runs pass (not re-run by me).
✅ `_c12`: shader_params.rs (mip_x/mip_y from window width) is not in the diff.
✅ Noise/user textures keep their mips: `Texture2d::from_rgba8` / `generate_mips` / `record_mips` unchanged.
✅ Comments: the hook comment in frame.rs, the blur_passes doc, the warp.rs/frame.rs/comp_fixed.rs/fixed.rs/blur.rs module docs corrected. Grep for "main.*mip" in src finds nothing asserting main regeneration.
✅ Test `main_textures_are_single_level` exists (tests/frame.rs:326): 2 tex_bits x 3 TexSize modes, after new and after resize, checks both `t.mips` and the wgpu `mip_level_count()`. Real assertions; RED run reported in the report.
✅ tests/warp.rs: the mip1-nonzero assertion became `levels == 1`; the mip-0 DSSIM checks are retained, so the test still checks the warp result. The level-count check is mild but meaningful.
✅ Golden unchanged, so no re-bless or DSSIM needed.
⚠️ The tests' `upload` helpers still call `generate_mips` on main (no-op now); the report notes this.

### Strengths
- Minimal change: one flag flip plus one deletion, with all stale comments corrected.
- The test covers the full matrix and both the crate's `mips` field and the real wgpu texture.

### Issues
#### Critical
None.
#### Important
None.
#### Minor
- frame.rs:9 reads as a broken sentence: "with\n (single-level, as in the C++...)" (it should read "into the warp target, single-level, as in the C++").
- blur.rs:9-10, comp_fixed.rs:9 and fixed.rs:112-113 now have comment lines over the 100-column width, with ragged reflow left over from the edit. warp.rs:268-270 is likewise unevenly wrapped.
- The `generate_mips(gpu, t)` calls on main in tests/blur.rs:30 and tests/frame.rs:39, 318, 481 are now dead and could be dropped.

### Assessment
**Task quality:** Approved — **Reasoning:** Every binding requirement of the ruling is met and verified in the diff and by grep. Only cosmetic comment-wrapping nits remain.

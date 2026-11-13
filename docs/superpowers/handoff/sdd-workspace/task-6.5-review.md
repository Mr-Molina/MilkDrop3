### Spec Compliance
✅ Grid nX×nY = (int)mv_x/(int)mv_y, caps 64/48 zeroing the fraction, fraction clamped 0..1, `f=(i+0.25)/(n+frac+0.25-1)`, `fy -= dy2`/`fx += dx2`, skip outside (0.0001,0.9999), gate `(float)mv_a >= 0.001` (NaN off): overlays.rs:183-235 = mfs:1159,1173-1178,1180,1206-1213,1231-1247. Check run: sed of every listed line.
✅ ReversePropagatePoint: (int) truncation, x1/y1 bounds, stride gx+1, left-to-right float accumulation, returns (tu, 1-tv): overlays.rs:115-141 = mfs:1431-1466. m_verts carries +0.5/tex (mfs:1836-1837); WarpMesh::uvs omits it (mesh.rs:10-14), added on read; blend case differs only in the last bit (affine mix), as mesh.rs already documents.
✅ Min-length rule and colour D3DCOLOR_RGBA_01 on floats via c_int (`&0xFF` wrap, NaN→0): overlays.rs:236-262, cast.rs:23-35 = mfs:40,1219-1221,1256-1282.
✅ Borders: `a > 0.001f` per border, inner/outer radii in float, 4 fans of 4 verts, rotation by cosf/sinf(1.570796327f) (== FRAC_PI_2 as f32), outer then inner, SRCALPHA/INVSRCALPHA: overlays.rs:273-308 = mfs:3349-3402. Same-factor alpha blend (blend.rs:82, D3D9 without separate alpha) ✓.
✅ darken_center: 6 verts, TRIANGLEFAN of 4, centre (0,0,0,(int)(3/32*255)=23), edges alpha 0, half 0.05, x×m_fAspectY, v5=v1, truthy test (NaN draws): overlays.rs:316-334, overlays_draw.rs:69-72 = mfs:3310-3347.
✅ Ledger: MotionVectorVars/Border in md-render from FrameInput.vars (F38); darken_center in DrawSprites into main (Task 5.7 ruling), not the composite; no mip regeneration (6.1a); `(int)` via cast.rs c_int.
✅ (a) mv drawn into m_lpVS[0] before the clear and warp (mfs:950-965 target VS0, 971-981 clear, 1006 CGAV); ComputeGridAlphaValues is the only writer of m_verts.tu/tv (grep: mfs:1842-1843,1853-1854), so DrawMotionVectors reads the previous frame's UVs. frame.rs:217-218 draws from `prev_uvs` then snapshots — correct order. Blending: m_verts after rep 1 already holds the blended UVs (mfs:1853-1854) and WarpMesh::compute_uvs does the same (mesh.rs:268-275), so the snapshot carries the blend; no old mesh needed ✓. First frame/resize: the C++ m_verts tu/tv are uninitialised after `new MYVERTEX` (plugin.cpp:2263, only tu_orig set at 2299-2300), and on that frame (m_nFramesSinceResize==0, frames_since_resize starts 0 and is reset on resize, renderer.rs:315) the clear erases the vectors anyway, so the identity seed (renderer.rs:207, 305) is invisible ✓.
✅ (b) Zero-length vectors become (min_len,min_len) (mfs:1273-1277); the renamed-semantics test pins alpha<0.001, NaN, count≤0 and checks the min_len, stretched and long branches (tests/overlays.rs:85-160) ✓ — departure justified.
✅ (c) RunPerFrameEquations blends ob_*/ib_*/mv_* by mix (mfs:706-724) and darken_center by snap (733) into m_pState before RenderFrame; FrameInput.vars is documented as the blended values (frame.rs:79-81); one draw each ✓.
✅ (d) Order mfs:965 mv → 971 clear → 1006/1009-1048 warp → 1051 blur → 1054 shapes → 1055 custom waves → 1056 wave → 1057 DrawSprites (darken 3310 before borders 3349) → composite; frame.rs matches; the GPU order test distinguishes fan-then-border from border-then-fan ✓.
✅ (e) identity_vars mv_a=0 (tests/frame.rs:51): those tests pin warp/comp identity; overlay integration through begin_frame is covered by tests/overlays.rs GPU tests; golden preset shader_rings.milk has mv_a/ob_a/ib_a 0, bDarkenCenter 0 (lines 19,53,58,67), so default_60 is genuinely unaffected ✓.
✅ Brief's four tests present; interface departures (MotionMesh not &WarpMesh, no aspect for mv; aspect_y for darken_center) follow the C++ (mfs:1216-1217 uses m_nTexSizeX only; 3330-3332 m_fAspectY).
✅ Citations: all line numbers in overlays.rs/overlays_draw.rs/frame.rs/cast.rs sed-checked; correct. 1156-1305 stops at the last statement (function closes 1308) — acceptable.
⚠️ TDD RED/GREEN and gate results taken from the report; not re-run.

### Strengths
- Careful float/int parity (float products, c_int casts, NaN paths) with hand-computed bilinear test (weights 0.32/0.48/0.08/0.12).
- GPU tests pin placement semantics, not just pixel presence: "before the warp" via a source-ignoring warp shader; fan-vs-border order via arithmetic.
- Fractional-count and offset tests enumerate exact positions (incl. the integral-count last-row drop).

### Issues
#### Critical
None.
#### Important
None.
#### Minor
1. No test pins the one-frame UV lag: tests/overlays.rs `run()` computes the mesh once, so frame 1 uses identical prev/current UVs; swapping draw/snapshot order in frame.rs:217-218 would pass. A two-frame test with different UVs per frame would lock (a).
2. Grid/prev_uvs mismatch: if md-core replaces the mesh through `mesh_mut()` with a different grid, `prev_uvs` keeps the old layout for one frame; a larger new grid gives zero vectors (`.get` → None), a smaller one indexes the wrong layout (garbage, not a crash). Consider resetting `prev_uvs` when `prev_uvs.len() != mesh.uvs().len()` (overlays_draw.rs:29-33).
3. renderer.rs:207-210 builds the WarpMesh twice to seed prev_uvs; build once and copy.
4. Two `D3DCOLOR_RGBA_01` helpers: cast.rs `rgba_01_f32` (c_int) and preexisting comp_fixed.rs:125 `rgba_01` (`as i32`, saturating; differs only for |x·255| ≥ 2^31). Point comp_fixed at the cast.rs one in a later touch.
5. overlays.rs doc says the C++ "reads uninitialized floats" on a None; good, but the first-frame identity seed should likewise note that the C++ m_verts tu/tv are uninitialised there (plugin.cpp:2263) and the clear hides it (renderer.rs:160 doc).
6. frame.rs:16 doc line exceeds the file's wrap width (cosmetic).

### Assessment
**Task quality:** Approved — **Reasoning:** All five departures are correct against the C++ (verified with line citations, incl. CGAV being the sole m_verts UV writer and the blended UVs living in the mesh), geometry/colours/order match DrawMotionVectors and DrawSprites exactly, and only minor test-coverage/robustness polish remains.

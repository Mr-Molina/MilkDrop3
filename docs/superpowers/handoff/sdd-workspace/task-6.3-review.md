### Spec Compliance
✅ Checked: `code/` is byte-identical to 6f7cd5d (`git show 6f7cd5d:… | diff -q`), so the line numbers below are the ones the code's `@6f7cd5d` citations should match.
✅ Clamp to 3..100 on `(int)sides` (mfs:2288-2290) = shapes.rs `clamp(3, 100)`. Centre (x*2-1, y*-2+1) is computed in double and then cast (2299-2300). The centre colour is (r,g,b,a) and the ring and closing vertex use (r2,g2,b2,a2) (2304-2313, 2323). `v[sides+1]=v[1]` (2325).
✅ Ring (2316-2323): j=1..=sides, t=(j-1)/sides, f32 `t*PI*2 + ang + PI*0.25`, `rad*cos*AspectY` on x only. The operation order and the f32 PI (=3.1415927f) match.
✅ Fill is `sides` triangles as a TRIANGLEFAN of sides+2 vertices (2333/2348). It uses TexModulate (COLOROP MODULATE, ALPHAOP SELECTARG1 DIFFUSE, mfs:923-928) when textured and Color otherwise (2342-2348).
✅ Border (2352-2385) is drawn only if `border_a > 0` (double), with its colour from border_* and alpha*alpha_mult (2358-2362). It is a LINESTRIP of `sides` segments from v2[1], i.e. v[1..=sides+1], closed. Thick=`(int)thick!=0`→4 copies with cumulative +x,+y,-x of 2/m_nTexSize (2371-2383), and there is no tex>=512 gate. Each copy is its own `DrawBatch::draw` (pipelines.rs `draw`, no strip merging). The border uses the instance's blend (2292-2294).
✅ Colours: `(int)(c*255[*alpha_mult]) & 0xFF` in double (2304-2313), mapped to Vertex [r,g,b,a]. Negative values wrap the same way (-127&0xFF=129). See Minor 2 for out-of-range values.
✅ Instance loop: for each rep, shapes 0..3 if enabled, then instances in order (2225-2264), with blend and colour per instance. A flat ordered list per rep (new, then old) reproduces it. Running the per-frame code (2267-2273) is md-core's job.
✅ Ruling `Task 6.3: Ruling:` / F38 verified against the C++, with two additions. Shapes use `num_reps = m_bBlending ? 2 : 1` (2224) and `pState = rep==0 ? m_pState : m_pOldState` (2227), so old shapes are drawn only while blending, after the new ones, with alpha_mult = 1-m_fBlendProgress (2230). Addition 1: the NEW preset's shapes are also scaled, by m_fBlendProgress, during a blend (2230). The ruling text omits this, but shapes_draw.rs sets `reps[0].1 = input.blend_t`, which is correct. `blend_t` is m_fBlendProgress (frame.rs:66; mfs:817). Addition 2 (binds 7.3): while blending, md-core must run the OLD preset's shape per-frame programs every frame (2267-2273 with pState=m_pOldState). Amend the ledger line.
✅ 6.1a: draws go into `vs[vs_target]` with LoadOp::Load and there is no mip regeneration. Half-pixel: positions are raw and DrawBatch/VsParams applies the shift (Task 4.8 ruling).
✅ Placement after BlurPasses and before DrawWave (mfs:1051-1056; frame.rs begin_frame). BlurPasses restores the VS[1] target (1670), so the target matches.
✅ (a) UPHELD. The textured fill does `SetTexture(0, m_lpVS[0])` (2330; 2387 restores it) while the RT is m_lpVS[1] (983-991). `src = vs[other_vs(vs_target)]` is correct, and there is no feedback hazard.
✅ (b) UPHELD. DESTBLEND is `(int)additive!=0 ? ONE : INVSRCALPHA` with SRCALPHA (2292-2294), i.e. AlphaAdditive/Alpha (blend.rs:82-83). The brief's Additive (ONE/ONE) was wrong.
✅ (c) UPHELD. `float t = (j-1)/(float)sides` (2317); the brief's j/sides rotates every vertex by one step.
✅ (d) UPHELD. `tu` carries `* m_fAspectY` and `tv` does not (2321-2322).
✅ (e) UPHELD. DrawCustomShapes has no alpha test (2215-2398); the 0.004 skip is DrawWave's (3233).
✅ (f) UPHELD. DrawCustomShapes sets no sampler state. Stage 0 is LINEAR (913-915) with WRAP/CLAMP from the warp (1907-1909, RestoreShaderParams 3800-3805), or CLAMP after BlurPasses (1528-1531, never restored). `fixed_sampler` models that.
✅ (g) UPHELD. `ShapeView{aspect_y, tex, alpha_mult}` is needed: thick offsets read m_nTexSizeX/Y (2372-2373) and the colours read alpha_mult (2305, 2310, 2359). `aspect_y` = `mesh_aspect().1` = m_fAspectY (plugin.cpp:2036).
✅ The brief's four tests are present and assert real values. The commit message `feat(render): custom shapes` matches.
⚠️ Gate (clippy/fmt/workspace tests, golden default_60 unchanged), RED output and the mutation run come only from the report and were not re-run, per the rules.

### Strengths
- The geometry and colour transcription is exact down to the f64/f32 split, operation order and 8-bit wrap. The pure builder (shapes.rs) is cleanly separated from the GPU code (shapes_draw.rs).
- The GPU tests discriminate: the textured test fails if the target is sampled instead of the source; the blend test checks presence and absence at blend_t 0 and 1, and that old shapes are not drawn when not blending.
- All seven departures are correct, and two brief errors (t index, blend mode) were caught.

### Issues
#### Critical
None.
#### Important
1. Most C++ line citations in the new code are wrong, usually by 20-30 lines. This recurs after 6.2's Minor 1, and the dispatch made it a requirement. Wrong citations (correct lines in brackets):
   - shapes.rs: NSEEL "2245-2248" [2267-2273]; LoadCustomShapePerFrameEvallibVars/var_pf "2402-2450" [2400-]; sides "2262-2264" [2288-2290]; textured "2321-2327" [2327-2334]; additive "2268" [2294]; thick "2349" [2371]; tex "2352-2353" [2372-2373]; alpha_mult "2224-2226" [2224-2230]; byte "2272-2283" [2304-2313]; fan "2268-2297" [2297-2325]; border "2330-2360" [2352-2385]; DON'T TOUCH "2288-2291" [2318-2322]; offsets "2354-2359" [2376-2382].
   - shapes_draw.rs: reps "2217-2223" [2224-2227]; alpha_mult "2224-2226" [2228-2230]; LINESTRIP "2360" [2383].
   - tests/shapes.rs: "2262-2264", "2297" [2325], "2285-2286" [2321-2322], "2327-2349"/"2320" [2352-2383/2352], "2335-2339" [2376-2382], "2270-2283" [2304-2313], "2217" [2224], "2224-2225" [2230].
   - Correct: 2330, 1054, 1054-1056, 2215.
   Fix them all; the module presents itself as a transcription.
#### Minor
1. Ledger ruling text: add that new shapes scale by blend_t during a blend, and that 7.3 must run the old preset's shape programs (see Spec). The code is already right.
2. `as i32` saturates, but the C++ `(int)` on double gives integer-indefinite INT_MIN (the codebase pins this as `eel_int`, md-eval/src/mem.rs:30-39). Where they differ: NaN textured/additive/thick is true in C++ and false here; sides >= 2^31 gives 3 in C++ and 100 here; a colour channel >= ~8.4e6 gives 0 in C++ and 255 here. Reachable from preset NaN or overflow. Use an `eel_int`-style helper in shapes.rs.
3. Test gaps: nothing pins the AlphaAdditive/Alpha choice (no `additive` GPU or CPU assertion), and `textured_uv_center_is_half_half` uses aspect_y=1, so departure (d) (u-only aspect) is untested. Add aspect_y≠1 to the UV test, plus one additive pixel check.
4. build_shape allocates a `ring` Vec per instance and draw_shapes allocates a `reps` Vec per frame. This is negligible, but the ring could be read directly from `out_fill`.
5. The report's citations repeat the same offsets (2217, 2231-2248, 2262-2268, 2352-2359); its sampler range "913-918" should read 913-915.

### Assessment
**Task quality:** Needs fixes. **Reasoning:** The behaviour is a faithful transcription and every departure and the blend ruling hold against the C++. However, nearly every C++ line citation in the code and tests is wrong, which the dispatch made a requirement and which already recurred from 6.2. One fix round should correct the citations and close the small test gaps.

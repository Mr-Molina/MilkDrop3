### Spec Compliance
✅ Ruling (ledger :391): `d3d9_ortho2d(p) = (p.x, -p.y, p.z, 1)` in all four fixed_*.wgsl `vs_main`, `o.pos = d3d9_raster(d3d9_ortho2d(v.pos))`; = D3DXMatrixOrthoLH(2,-2,0,1) with WORLD/VIEW identity (sed support.cpp:179-184, textmgr.cpp:238). Shader placement over DrawBatch is justified: warp.rs:322 builds its own PipelineKey and indexed draw, bypassing `DrawBatch::draw`.
✅ (a) Pipeline audit (grep create_render_pipeline / PipelineKey::new in md-render/src): fixed_color/tex_modulate/tex_modulate_alpha/tex_select: flipped once. Users: wave_draw, shapes_draw, custom_waves_draw, overlays_draw (mv, borders, darken_center), title.rs:121 anim (TexModulate), text_draw boxes (Color), comp.rs:385 fixed comp, comp.rs:438 blit_quad (rewritten pre-projection, v=0 at y=-1), warp.rs:322 no-shader warp (fixed block `-y`, = mfs:1973/2014/2040): all flipped once, none twice. warp_vs/comp_vs (preset_gpu.rs:275-279): passthrough, matching data/warp_vs.fx `vPosProj = float4(vPosIn,1)`: not flipped, not needed. blur_vs (blur.rs:194): passthrough, C++ uses a shader VS: not needed. text.wgsl (FixedMode::Text, text_draw glyphs + title.rs:307 title texture): D3DX font, clip-space glyphs: not needed. mips.rs MIP_BLIT_WGSL: self-contained fullscreen triangle with v down: not needed. No wrong entry found; no DrawBatch/PipelineCache users outside md-render (grep crates/ apps/: only md-render tests).
✅ (b) Shift: s = 2*0.5*(1/W, -1/H) added after the projection, i.e. +0.5 px right/down in screen space, where D3D9 applies its pixel-centre rule (post-transform); sign unchanged and correct. Pinned exactly: `fixed_quad_lands_on_its_d3d9_pixel` (orientation.rs:151, worked: pre-proj y -0.875..-0.625 → screen [1,2) with shift → row 1 only; no shift → row 0; wrong sign → row 0) and border edges rows 0..=4/92..=95 (bottom edge discriminates: no shift → row 91 lit).
✅ Builders verbatim, citations sed-checked: shapes mfs:2300, custom waves :2613, mv :1231-1236/:1286, wave flip :3221-3230/:3229, mode 6 :3047/:3057-3060, comp v3 :4008-4015, title :4612-4631 and commented-out flip :4651-4653, warp :1973/:2014/:2040, darken_center :3310-3345, mv-before-warp :965, clear :971-981, textmgr.cpp:609-612, pluginshell.cpp:1806/:1840-1848. All exact.
✅ title.wgsl deleted with builtin_wgsl.rs entries (FILES/VERTEX_FILES/pairs/textured/alpha-op) and README rows; new md-hlsl test pins ORTHO_FN in exactly the four fixed shaders and absence elsewhere.
✅ (d) Mode 6: ang = 1.57*fWaveParam2, mystery 0 → dx=1, dy=0, horizontal; perpendicular offset `fWavePosX*sinf(ang+1.57)` (fWavePosX = wave_x*2-1, mfs:2761). Brief was wrong; the test does exercise y orientation (the line's y is set by wave_x, through DrawWave's negation and the projection).
✅ (e) Box vertices: textmgr `y0 = -1 + 2*top/H`, under ortho(2,-2) → clip 1-2*top/H = old `ndc`; pluginshell ortho(w,h) `y = h/2 - bottom` maps to the same pixels. 6.6's tests/text.rs:129-135 pins box corners at an asymmetric rect (8,8,150,60) and is unchanged.
✅ Golden untouched (no tests/golden in the stat). Step 2 RED recorded (5 fixed tests mirrored, e.g. (2,7) vs (2,1)).
⚠️ GREEN (673 passed, MD_REQUIRE_GPU=1) not re-run; plausible from the diff.

### Strengths
- One projection, enforced structurally (shader) and textually (md-hlsl ORTHO_FN test); comments everywhere now say "pre-projection" with the C++ line.
- warp_and_composite test covers all four warp×comp path combinations in main and on screen, with an x gradient that also catches a horizontal mirror.
- (c) Altered tests are corrections, not weakenings: comp_fixed.rs:315/:424 now assert C++ v3 verbatim (uprightness moved to orientation.rs), title_mesh final pose, pipelines.rs points/lines (inputs to pre-projection, pixel expectations unchanged), warp.rs gains `-y`/`+y` block assertions.
- (f) Orientation tests separate top from bottom robustly: wave/custom wave all rows within 2/1 of H/4 (mirror = 72), shape all rows < H/2 with mean ≈ H/4, mv `low` all within 2 of H/5 (mirror = 77), title all rows < h/2, warp/comp at rows 8 and 88.

### Issues
#### Critical
None.
#### Important
None.
#### Minor
1. orientation.rs:271-273 comment: pre-projection y in (0.9, 1) is the BOTTOM band (screen rows [91.2, 96)), not "the top band ... rows [0, 4.8)". Assertion is right; fix the wording.
2. orientation.rs:259-263: the mv_dy half's row set {0.1H, 0.9H} is mirror-symmetric, so only the `low` assertion (:258) pins orientation; the `high` half pins only the shift direction/second row. Acceptable; say so in the comment.
3. Borders and darken_center are flip-symmetric in the C++ (mfs:3349-3400 rotate a fan by 90°; 3310-3345), so their tests pin the pixel rule, not orientation (implementer stated this; nothing better exists).
4. No direct test for fWavePosY-driven modes (0/1/4/5/7) or textured-shape UV orientation; covered by the shared flip and verbatim UVs (report concern 4). Optional.
5. wave.rs 623 lines, over the ~600 guide (pre-existing, 6.2 minor).
6. d3d9_ortho2d is copied in four WGSL files (WGSL has no include), consistent with d3d9_raster and pinned verbatim by the md-hlsl test; acceptable.

### Assessment
**Task quality:** Approved — **Reasoning:** The flip is applied exactly once, in every fixed-function vertex shader and nowhere else, builders emit C++ pre-projection coordinates verbatim with correct citations, and the half-pixel shift keeps its post-projection sign under an exact-pixel test. Every 2D pipeline was audited and found correct, and the altered tests correct mirrored or clip-space expectations without weakening them.

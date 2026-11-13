### Spec Compliance
✅ (a) Blend list matches the C++ field by field (awk over mfs:697-747). MIXED `mix*new+mix2*old` (37): decay, wave_a/r/g/b/x/y/mystery 697-704, ob_size/r/g/b/a 706-710, ib_size/r/g/b/a 711-715, mv_x/y/dx/dy/l/r/g/b/a 716-724, echo_zoom/echo_alpha 725-726, gamma 734, blur1-3 min/max + blur1_edge_darken 741-747 = per_frame.rs `mixed!` (37). SNAPPED `mix<snap ? old : new` (11): echo_orient 727, wave_usedots/thick/additive/brighten 729-732, darken_center 733, wrap/invert/brighten/darken/solarize 735-739 = `snapped!` (11). LEFT AT NEW: wave_mode ("exempt (integer)" 705), zoom/zoomexp/rot/warp/cx/cy/dx/dy/sx/sy, q, monitor, inputs. The 6.5 review ("mv_*, ob_*/ib_* and darken_center blended or snapped, 706-733") is consistent with this: it never claimed motion values. The milestone bullet's zoom/rot/cx/cy/warp/sx/sy/dx/dy/wave_mode entries are wrong; the motion values are blended per vertex in ComputeGridAlphaValues rep 1 (mfs:1846-1853, `tu = tu*mix2 + u*(1-mix2)`, mix2 = clamp(a*fBlend+c), fBlend = raw m_fBlendProgress 1683), already in md-render mesh.rs:269-276 (`compute_uvs` with `blend = Some(..)`). Departure upheld.
✅ (b) m_fSnapPoint is NOT random: 0.5 by default (587), -0.01 when only the old preset has a comp shader (codes 4/6/12/14, 597-602), 1.01 when only the new one has (1/3/9/11, 603-608), 0.5 otherwise (609-618); code bits from mfs:867-871 (OLDcomp=4, NEWcomp=1). md_render::snap_point (warp.rs:64-70) maps the same. Cosine: CosineInterp = 0.5f-0.5f*cosf(x*π) in f32 (utility.h:42), cast to double (695); cosine_interp (comp.rs:189) + f64::from matches; `mix < f64::from(snap)` matches float→double promotion; tie (mix==snap) picks new as C++. Departure upheld.
✅ (c) aspectx/aspecty = m_fInvAspectX/Y (mfs:544-545, pv 654-655), = 1/m_fAspectX/Y (plugin.cpp:2037-2038), m_fAspectX/Y from the texture size (2035-2036) = 1/md_render::mesh_aspect(tex). Rust writes `inputs.aspect` to both Vms unchanged; correct given the documented contract.
✅ (d) `now` feeds only `PerFrameVars::from_preset(.., now)`: non-motion Blendables eval at GetTime() (mfs:497-553), motion at -1 (469-478) — frame_vars.rs:146-147 matches. `time` = GetTime()-m_fStartTime (481) where m_fStartTime is set on frame 0 only (765-768): plugin clock, not since-preset; `progress` uses preset timers (494) and is shared by both reps. Departure upheld.
✅ (e) No pp code: the C++ skips the branch (1750) and keeps fZoom..fSY = (float)var_pf_* (1728-1737); compute_uvs seeds `base` from per_frame (mesh.rs:235-247) and a no-op runner leaves it — identical result. Departure (no Option) upheld.
✅ (f) q = q_values_after_init_code and monitor = monitor_after_init_code each frame (490-493) → `v.q = state.q_init; v.monitor = state.monitor_init` then pf_refs.load (vars.rs load writes q, monitor); monitor saved after run (670) ✓; pv inputs copied before run (640-655) ✓, no monitor (656 commented) ✓; q copied to pv after run (673-674) ✓; the C++ copies no output scalars (milestone wrong) ✓. Clamps are only gamma [0,8] and echo_zoom [0.001,1000] (676-678), per state before blending ✓; `clamp` reproduces windows.h min/max (NaN passes) ✓.
✅ (g) Runner: refs resolved once (pv_refs), per call 14 `set` + `run` + 10 `get`, no allocation or lookup (per_pixel.rs:31-44). run_per_frame builds a stack struct only.
✅ Brief tests present: q_resets_to_init_each_frame, per_frame_can_modify_zoom, echo_zoom_is_clamped, blend_at_half_is_midpoint_for_floats, booleans_switch_at_half (per_frame_tests.rs).
✅ Citations sed-checked: 460-554, 481, 483-488, 494, 497-553, 544-545, 556-749, 586-620, 628-631, 640-655, 664, 670, 673-674, 677-678, 691-748, 705, 1722-1737, 1756-1769, 1782-1791; plugin.cpp:2037 — all correct (one off-by-two, Minor 1).

### Strengths
- Blend lists in macros shared with tests; tests cover every mixed and snapped field, the snap extremes (-0.01/1.01) and the untouched fields (zoom, sx, wave_mode, q, monitor, bass, warp_scale).
- The old/new two-call protocol and raw (non-cosine) blend progress for compute_uvs are documented accurately in per_frame.rs:1-23.

### Issues
#### Critical
None.
#### Important
None.
#### Minor
1. per_pixel.rs:3 cites the `m_pp_codehandle` branch as 1752-1791; it starts at milkdropfs.cpp:1750.
2. `preset_progress` (per_frame.rs:57) is pub but not re-exported from lib.rs and has no test.
3. FrameInputs::aspect doc (per_frame.rs:48) could name the source explicitly (`1/mesh_aspect(texture size)`, plugin.cpp:2035-2038) so 7.9 does not pass mesh_aspect or the window ratio.
4. inputs_reach_the_per_frame_vm checks pv inputs only indirectly (per-pixel test reads bass/time/aspecty/meshx); fps/frame/progress/pixels in the pv Vm are untested.
5. Ledger should record a ruling that the milestone bullet's blend list (motion values, wave_mode) and "copy output scalars to the per-pixel Vm" are superseded by mfs:673-674/697-747.

### Assessment
**Task quality:** Approved — **Reasoning:** Every departure (blend list, snap point, now, aspect, no-op runner, clamps) matches the C++ line by line; remaining items are citation/doc polish.

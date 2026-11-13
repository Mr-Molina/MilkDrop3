# Task 7.1 report: md-core preset state and evaluation contexts

Commit: 8c892e4 `feat(core): preset state and evaluation contexts` (no push).
Status: DONE_WITH_CONCERNS (departures below).

## Implemented (crates/md-core)
- `vars.rs` (342 lines): one macro-generated refs struct per context (`PerFrameRefs`, `PerPixelRefs`, `WaveFrameRefs`, `WavePointRefs`, `ShapeFrameRefs`) with `register(&mut Vm)` and a `*_names()` list; `PerFrameRefs::load/store` against md-render's `PerFrameVars` (field names equal; 7.2 reuses them).
- `state.rs`: `PresetState`, `InitInputs`, `from_preset`, `from_preset_with`, `compile_section`.
- `custom.rs`: `WaveState` (pf Vm, pp Vm, programs, `t_init`), `ShapeState` (pf Vm, program, `t_init`).
- `error.rs`: `ProgramError {section, error}`, `CoreError` (non_exhaustive, one variant, nothing in 7.1 can fail fatally; A.3 needs the type).
- Cargo deps added: md-eval, md-preset, md-render, rand_pcg, rand_core, thiserror (all already workspace pins; no new crates, Cargo.lock only gains md-core's edges). md-analysis NOT added: nothing in 7.1 uses it (A.2 allows it; add when 7.2 needs SoundData). md-eval is a direct edge because `Vm`/`Program` are in the public API (A.2's diagram shows it only via md-preset, whose md-eval edge is dev-only).

## Variable lists verified against state.cpp (RegisterBuiltInVariables 262-514)
Extracted the literal `regvar(...)` names per context by script and compared with vars.rs: exact set equality for all five (counts excluding q/t: per-frame 76 [state.cpp:267-348], per-pixel 30 [355-390], wave per-frame 15 [398-424], wave per-point 19 [427-457], shape per-frame 34 [466-511]). `q1..q32` (loops at 295-300, 378-383, 402-407, 431-436, 470-475) and `t1..t8` (waves/shapes only, 408-413, 437-442, 476-481); NUM_Q_VAR/NUM_T_VAR state.h:49-50.
Diff vs the brief:
- Per-frame list: brief matches C++ exactly (76 names).
- Per-pixel: brief was vague; C++ is zoom zoomexp rot warp cx cy dx dy sx sy time fps bass mid treb bass_att mid_att treb_att frame x y rad ang progress meshx meshy pixelsx pixelsy aspectx aspecty + q. No `monitor`, wave_*, ob/ib/mv, echo, gamma, flags or blur names.
- Shape: DROPPED `thickoutline` (not registered; the flag is the variable `thick`, milkdropfs.cpp:2427). ADDED the ten inputs the brief omitted: time fps frame progress bass mid treb bass_att mid_att treb_att; plus t1..t8, q1..q32. `instance`, `num_inst` present.
- Wave: brief's per-point list matches (sample value1 value2 x y r g b a). ADDED per-point time fps frame progress bass.. and t1..t8; per-frame (not in brief): r g b a `samples` + inputs + q + t. Note per-frame is `samples`, per-point is `sample`.
- md-eval keeps 16 chars, so `blur1_edge_darken` is the variable `blur1_edge_dark` (consistent in compile and in refs).

## C++ behaviour pinned
- rand_preset / rot: `Default()` calls `RandomizePresetVars()` (state.cpp:699; ImportFromFile calls Default at 1282); function at 1875-1900. md-render already has `randomize_preset_vars(rng)` (shader_params.rs:98, same draw order); md-core calls it with the caller's rng. C++ arrays are 20 entries (state.h:360-362); md-render returns 24 with 20..24 zero (brief's 24 kept, per md-render).
- Init order (RecompileExpressions 1552-1873): preset per_frame_init (1665-1695; compiled, executed once, q1..q32 and monitor captured, program freed), per_frame (1698-1707), per_pixel (1710-1719), then waves (1728-1765: init, per_frame, per_point) and shapes (1800-1837: init, per_frame; per-point commented out 1852-1869). All 4 waves/shapes compile whether enabled or not.
- Empty source (after comment stripping) is not compiled (`if (buf[0])`, 1666/1699): program None, no error. Compile failure: program None, error recorded with section name; preset init failure sets q_init=0, monitor=0 (1676-1678); wave/shape init failure sets t=0 (1744, 1815).
- What init leaves that persists: only q1..q32 and monitor (captured), plus anything in the same Vm that LoadPerFrameEvallibVars does not rewrite every frame: user variables, megabuf, reg00..99/gmegabuf (global). All registered outputs (zoom..blur, etc.) are re-loaded each frame (milkdropfs.cpp:460-554), q and monitor restored from the capture (491-493), and monitor refreshed after each per-frame run (670). Hence init and per_frame share one Vm.
- Wave/shape init values (milkdropfs.cpp:2442-2463 wave, 2400-2440 shape): inputs, q = preset after-init q, t = 0, the wave's r g b a samples / the shape's x y rad ang tex_ang tex_zoom sides textured additive num_inst(instances) instance(0) thick(thickOutline) r g b a r2 g2 b2 a2 border_*; `t1..t8` captured from the pf Vm after init and restored each frame (2457).

## Departures from the C++ / brief (all listed)
1. `LoadPerFrameEvallibVars(g_plugin.m_pState)` (state.cpp:1684) loads the CURRENT preset into the incoming state's init Vm; Rust loads the preset's own values (intended behaviour). Wave/shape init likewise take q from this state's init capture, not the plugin's current state.
2. `q_values_after_init_code`/`t_values_after_init_code` are never initialised when there is no init code (state.h:440, 154, 206); Rust uses 0.
3. Plugin-global inputs (time, fps, frame, progress, bands, mesh/window/aspect, `now` for blendables) are not in the brief's signature: added `InitInputs` (Default = all 0) and `PresetState::from_preset_with`; `from_preset` = defaults, as the brief says.
4. Brief's `pf_init: Option<Program>` field dropped (the C++ frees the init program after running it, 1692-1693); init programs of waves/shapes likewise not kept. `pf_program`/`pv_program` are `pub` (7.2 needs them). Added `pf_refs`, `pv_refs`, `errors` fields.
5. Whitespace-only source after stripping is treated as empty (C++ only erases pure space/linefeed strings before stripping, 1610-1650; a comment-only line leaves spaces and would be handed to the compiler). Minor.
6. Brief variable-list diffs above.

## TDD
RED (tests written first, `cargo test -p md-core state`, before any implementation): compile failure `error[E0583]: file not found for module state` / `vars`, "could not compile `md-core` (lib test) due to 4 previous errors". After implementing, one test count was wrong (wave per-point 19 not 18, found by the script comparison): `left: 59 right: 58`, fixed in the test (the count was my arithmetic error, the names themselves matched C++).
GREEN: `cargo test -p md-core state` -> 10 passed. Tests: registers_all_variables, per_frame_refs_round_trip_every_variable, init_captures_q_and_monitor, init_user_variables_and_registers_persist, init_sees_the_preset_values_and_inputs, compile_error_in_per_frame_is_reported_not_fatal, init_compile_error_zeroes_q_and_is_reported, empty_and_comment_only_code_is_not_compiled, wave_and_shape_init_capture_t1_to_t8, rand_preset_and_rot_are_deterministic_for_a_seed.
Gate: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` clean; full `cargo test --workspace` all ok, no failures.

## Citation check
Every C++ line citation in the code and this report was re-read with `sed -n` before the commit; five were wrong at first (262-516 -> 514, shape `thick` 2429 -> 2427, AddError lines, 1850-1868 -> 1852-1869, wave loader 2461 -> 2463) and were corrected.

## Concerns
- `CoreError` has a placeholder variant; later tasks extend it.
- The brief's `rot` is 24 entries while the C++ has 20 (md-render's decision, kept).
- Waves/shapes in the C++ have per-frame `progress` etc. refreshed every frame; the 7.3 task must do that loading (only init-time loading exists here).

## Fix report, round 1 (review finding: init values per load mode)
Changes:
- New `InitMode { Instant(InitInputs), Blended }` replaces the always-on `InitInputs` path: `from_preset_with(preset, global, rng, &InitMode)`; `from_preset` = `Instant(InitInputs::default())`. Instant: the incoming preset's own values plus the caller's inputs (caller takes `progress` from the OUTGOING preset's timers, reset only after Import, plugin.cpp:7259/7267; states swapped before Import, 7251-7259). Blended: preset, wave and shape init run with nothing loaded (all 0) and wave/shape init get q = 0 (Import into m_pNewState while m_pState is the outgoing one, plugin.cpp:7293; loaders at state.cpp:1684/1750/1821). The wrong doc comment is replaced.
- Comment on `from_preset_with`: q/monitor with no init code are 0 deliberately (the C++ keeps stale values of the reused CState slot).
- Citations: custom.rs now 1728-1790 waves (init, per-frame, per-point) and 1800-1850 shapes (init, per-frame); "1610-1650" is 1615-1651; "ImportFromFile" is `CState::Import` (state.cpp:1263). Each re-read with sed.
- `rand_pcg` moved to dev-dependencies (lib code only needs `rand_core::Rng`).
- Not done (noted, harmless): on wave/shape init compile failure the C++ copies q_init into that VM (state.cpp:1742, 1813); 7.3 reloads q every frame.
Tests: `instant_init_sees_the_preset_values_and_inputs` (q1=time+meshx, q2=decay, q3=bass; wave t1=q1, t2=r, t3=time; shape t1=sides, t2=q2, t3=bass capture the preset/inputs) and `blended_init_sees_nothing_loaded` (all 0, with decay confirmed non-zero).
`cargo test -p md-core`: 11 passed. Full gate (build, clippy -D warnings, fmt --check, workspace tests): clean.

### Task 7.1: State: per-frame and per-pixel evaluation contexts

**Files:**
- Create: `crates/md-core/src/{lib.rs,state.rs,vars.rs}`

**Interfaces:**
- Produces: `pub struct PresetState { pub preset: Preset, pub pf: Vm, pub pv: Vm, pub pf_program: Option<Program>, pf_init: Option<Program>, pv_program: Option<Program>, pub waves: [WaveState; 4], pub shapes: [ShapeState; 4], pub q_init: [f64; 32], pub monitor_init: f64, pub rand_preset: [f32;4], pub rot: [RotParams; 24], pub blend_progress: f32 }`, `PresetState::from_preset(preset: Preset, global: Rc<RefCell<GlobalMem>>, rng: &mut Pcg32) -> Result<PresetState, CoreError>` which registers every variable name from the pinned list in `vars.rs` (the `regvar` list captured from the C++: `time fps frame progress bass mid treb bass_att mid_att treb_att meshx meshy pixelsx pixelsy aspectx aspecty monitor zoom zoomexp rot warp cx cy dx dy sx sy decay wave_a wave_r wave_g wave_b wave_x wave_y wave_mystery wave_mode wave_usedots wave_thick wave_additive wave_brighten ob_size ob_r ob_g ob_b ob_a ib_size ib_r ib_g ib_b ib_a mv_x mv_y mv_dx mv_dy mv_l mv_r mv_g mv_b mv_a echo_zoom echo_alpha echo_orient darken_center wrap invert brighten darken solarize gamma blur1_min blur1_max blur2_min blur2_max blur3_min blur3_max blur1_edge_darken q1..q32` for per-frame; per-pixel adds `x y rad ang` and drops the output-only ones; waves add `sample value1 value2 x y r g b a`; shapes add `instance num_inst sides textured additive thick thickoutline x y rad ang tex_ang tex_zoom r g b a r2 g2 b2 a2 border_r border_g border_b border_a`), compiles all programs, and runs `per_frame_init`.

- [ ] **Step 1:** Write failing tests `registers_all_variables` (every name in `vars.rs` resolves), `init_captures_q_and_monitor`, `compile_error_in_per_frame_is_reported_not_fatal` (state still loads, `pf_program` is `None`, error recorded).
- [ ] **Step 2:** Run `cargo test -p md-core state`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): preset state and evaluation contexts`


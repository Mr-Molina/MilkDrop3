### Task 7.5: Preset switching, blending, hard cuts, timers

**Files:**
- Create: `crates/md-core/src/{switcher.rs,timing.rs}`

**Interfaces:**
- Produces: `pub struct Switcher { current: PresetState, old: Option<PresetState>, pending: Option<(PresetState, f32 /* blend */, u8 /* tick */)>, blend_pattern: Vec<VertInfo>, next_preset_time: f32, hard_cut_thresh: f32, locked_by_user: bool, locked_by_code: bool }` with `load(&mut self, path, blend_time, now)`, `tick(&mut self, now, renderer: &mut Renderer)` (implements the 8-tick schedule and the shader compiles at ticks 2 and 5 through `Renderer::load_preset`), `update_timers(&mut self, now, fps, sound, cfg) -> Vec<SwitchEvent>` (auto-advance, hard cut), `blend_t(now) -> f32`, `progress(now) -> f32`.

- [ ] **Step 1:** Write failing tests `auto_advance_after_interval` (fixed clock, no audio), `hard_cut_triggers_and_doubles_threshold`, `threshold_decays_toward_loudness_thresh`, `load_during_pending_is_ignored`, `tick_8_swaps_and_starts_blend`, `locked_preset_never_advances`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): preset switching, blending, hard cuts`


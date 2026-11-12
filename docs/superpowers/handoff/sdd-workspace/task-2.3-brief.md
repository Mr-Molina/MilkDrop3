### Task 2.3: Preset model and parser

**Files:**
- Create: `crates/md-preset/src/{preset.rs,wave.rs,shape.rs,defaults.rs,parse.rs}`

**Interfaces:**
- Produces: `Preset`, `WaveData`, `ShapeData`, `parse_milk`, `Preset::defaults`, `PsVersion` as in A.3; code blocks stored as `pub per_frame_init: String`, `pub per_frame: String`, `pub per_pixel: String`, `pub warp_shader: String`, `pub comp_shader: String`, `WaveData::{init, per_frame, per_point}`, `ShapeData::{init, per_frame}`; version rule: `PSVERSION` applies to both shaders unless `PSVERSION_WARP`/`PSVERSION_COMP` are present; `MILKDROP_PRESET_VERSION` below 200 means no shaders (`PsVersion::None`) regardless.
- The blendable preset fields are exactly those `CState` declares as `CBlendableFloat` in `code/vis_milk2/state.h`; list them in `preset.rs` with a comment and a test asserting the count matches the number of `CBlendableFloat` declarations recorded there.

- [ ] **Step 1:** Write failing tests `defaults_match_cstate_default` (spot-check 12 values copied from `state.cpp`, e.g. `fDecay = 0.98`, `zoom = 1.0`, `nWaveMode = 0`, `fWaveAlpha = 0.8`), `parses_all_scalar_keys` (a fixture `tests/fixtures/all_keys.milk` setting every key to a distinct value, asserting each field), `parses_four_waves_and_shapes`, `ps_version_precedence`, `parses_crlf_bom_and_trailing_junk` (same fixture with CRLF and BOM gives an equal `Preset`), `missing_keys_keep_defaults`.
- [ ] **Step 2:** Run `cargo test -p md-preset parse`. Expected: FAIL.
- [ ] **Step 3:** Implement; `Preset` derives `Clone, Debug, PartialEq`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(preset): model, defaults, and parser`


### Task 5.8: `render` subcommand and first golden images

**Files:**
- Create: `apps/milkdrop/src/{main.rs,cli.rs,render_cmd.rs}`
- Test: `apps/milkdrop/tests/golden_basic.rs`

**Interfaces:**
- Produces: `milkdrop render --preset <file> --wav <file> --frames <n> --size WxH --seed <u64> --out <dir> [--every <k>]` writing `frame_<n>.png`; uses `md-preset`, `md-analysis`, `md-audio::WavSource`, and a minimal driver (per-frame equations via `md-eval` live in `md-core` from Milestone 7; until then the driver evaluates only the static preset values and `progress`, enough for shader-only presets).
- The driver is `apps/milkdrop/src/headless.rs` and is replaced by `md_core::Visualizer` in Task 7.9.

- [ ] **Step 1:** Write failing test `golden_default_preset_frame_60` using `md_testkit::compare_png` against `tests/golden/rust/default_60.png` with `max_dssim = 0.02`; first run with `MD_BLESS=1` creates the golden, reviewer inspects it against `tests/golden/windows/`.
- [ ] **Step 2:** Run the test. Expected: FAIL (no binary).
- [ ] **Step 3:** Implement the subcommand.
- [ ] **Step 4:** Run with bless, inspect, then run without. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): headless render subcommand and first golden`

## Milestone 6: full renderer


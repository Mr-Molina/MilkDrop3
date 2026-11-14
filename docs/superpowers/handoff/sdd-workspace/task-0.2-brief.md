### Task 0.2: Test kit and golden-image comparison

**Files:**
- Create: `tests/support/md-testkit/src/{lib.rs,image_diff.rs,fixed_clock.rs,corpus.rs}`
- Test: `tests/support/md-testkit/tests/image_diff.rs`

**Interfaces:**
- Produces: `pub fn compare_png(actual: &RgbaImage, golden_path: &Path, max_dssim: f64) -> DiffResult`, `pub struct FixedClock { pub fn new(fps: f64) -> Self; pub fn now(&self) -> f64; pub fn advance(&mut self) }`, `pub fn corpus_presets() -> Vec<PathBuf>` (every `*.milk` under `tests/presets/`), `pub fn bless_enabled() -> bool` (env `MD_BLESS=1` rewrites goldens).

- [ ] **Step 1:** Write failing tests `identical_images_have_zero_dssim`, `shifted_by_one_pixel_is_under_0_01`, `missing_golden_fails_unless_bless`.
- [ ] **Step 2:** Run `cargo test -p md-testkit`. Expected: FAIL, functions undefined.
- [ ] **Step 3:** Implement with `dssim-core`; `DiffResult { dssim: f64, passed: bool, diff_png: Option<PathBuf> }` writes a diff image next to the golden on failure.
- [ ] **Step 4:** Run `cargo test -p md-testkit`. Expected: PASS.
- [ ] **Step 5:** Commit: `test: add image diff and fixed clock test kit`


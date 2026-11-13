### Task 7.4: Preset library and selection

**Files:**
- Create: `crates/md-core/src/{library.rs,history.rs}`

**Interfaces:**
- Produces: `pub struct PresetLibrary { pub fn scan(dir: &Path, insta_scan_ratings: bool) -> PresetLibrary; pub fn entries(&self) -> &[Entry]; pub fn dir_count(&self) -> usize; pub fn pick_random(&self, rng: &mut Pcg32, use_ratings: bool) -> Option<usize>; pub fn next_sequential(&self, current: usize) -> usize; pub fn read_more_ratings(&mut self, n: usize); pub fn find_prefix(&self, typed: &str, from: usize) -> Option<usize> }`, `Entry { name: String, is_dir: bool, rating: f32, rating_cum: f32 }`; `pub struct History { ring of 66 } ` with `push`, `back() -> Option<&Path>`, `forward() -> Option<&Path>`.

- [ ] **Step 1:** Write failing tests `dirs_sort_before_files_and_are_starred`, `sort_is_case_insensitive`, `rating_cdf_prefers_high_ratings` (rating 5 vs 1, 10000 picks, ratio within 4.5..5.5), `uniform_when_ratings_disabled`, `sequential_wraps_past_dirs`, `history_back_then_forward_returns_same`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement with `std::fs::read_dir`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): preset library, selection, history`


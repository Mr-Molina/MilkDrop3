### Task 1.6: Loop bounds and error behavior

**Files:**
- Modify: `crates/md-eval/src/vm.rs`
- Test: `crates/md-eval/tests/bounds.rs`

- [ ] **Step 1:** Write failing tests `while_is_capped_at_1_048_576_iterations` (`i=0; while(i=i+1)` finishes and `i` equals the cap), `loop_negative_count_runs_zero_times`, `division_by_zero_yields_inf_not_panic`, `nan_is_falsy`.
- [ ] **Step 2:** Run `cargo test -p md-eval --test bounds`. Expected: FAIL.
- [ ] **Step 3:** Implement caps; the cap constant is `pub const LOOP_CAP: u32 = 1_048_576` (`NSEEL_LOOPFUNC_SUPPORT_MAXLEN` in `code/ns-eel2/ns-eel.h`).
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(eval): loop caps and non-panicking arithmetic`


### Task 1.7: Fuzz and property tests

**Files:**
- Create: `crates/md-eval/fuzz/` (cargo-fuzz target `compile_and_run`), `crates/md-eval/tests/props.rs`

- [ ] **Step 1:** Write a proptest in `props.rs`: for random ASTs printed back to source, `compile` succeeds and `run` terminates within 100 ms.
- [ ] **Step 2:** Write the fuzz target: arbitrary bytes -> `Program::compile`; if it compiles, `run` with a 100 ms budget; never panics.
- [ ] **Step 3:** Run `cargo test -p md-eval --test props` (PASS) and `cargo +nightly fuzz run compile_and_run -- -max_total_time=60` locally (no crash). CI runs only the proptest.
- [ ] **Step 4:** Commit: `test(eval): property and fuzz tests`


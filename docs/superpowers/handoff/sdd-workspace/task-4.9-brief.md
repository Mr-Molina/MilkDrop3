### Task 4.9: Corpus translation test and triage report

**Files:**
- Create: `crates/md-hlsl/tests/corpus.rs`, `crates/md-hlsl/examples/triage.rs`

- [ ] **Step 1:** Write test `every_corpus_shader_translates_and_validates`: for each preset with `warp_shader` or `comp_shader` non-empty, `translate` succeeds; failures are collected and the test prints a table `preset | shader | line:col | message` before asserting the failure count is 0.
- [ ] **Step 2:** Write `triage.rs`: same loop, groups failures by message prefix, prints counts descending. Run `cargo run -p md-hlsl --example triage`.
- [ ] **Step 3:** Iterate on Tasks 4.3, 4.4, 4.7 until the corpus test passes at 100%. Each fix gets its own commit naming the preset that exposed it.
- [ ] **Step 4:** Run `cargo test -p md-hlsl --test corpus`. Expected: PASS.
- [ ] **Step 5:** Commit: `test(hlsl): corpus translation at 100%`


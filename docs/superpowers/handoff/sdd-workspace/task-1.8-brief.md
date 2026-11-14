### Task 1.8: Performance budget

**Files:**
- Create: `crates/md-eval/benches/per_pixel.rs` (criterion)

- [ ] **Step 1:** Write a benchmark running a representative per-pixel program (`zoom = zoom + 0.1*sin(rad*6 + time); rot = rot + 0.02*ang; dx = dx + 0.001*cos(ang*3)` with `rad`, `ang`, `time` set) 4225 times.
- [ ] **Step 2:** Run `cargo bench -p md-eval`. Expected: under 2 ms per 4225 evaluations on the CI machine class; if above, add a register-allocated fast path in `vm.rs` (superinstructions `LoadConstMul`, `StoreAddLoad`) before moving on.
- [ ] **Step 3:** Commit: `perf(eval): benchmark and fast path`


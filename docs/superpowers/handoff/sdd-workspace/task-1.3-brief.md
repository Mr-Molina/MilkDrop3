### Task 1.3: Variable table and global memory

**Files:**
- Create: `crates/md-eval/src/vm.rs`, `crates/md-eval/src/mem.rs`

**Interfaces:**
- Produces: `Vm`, `VarRef`, `GlobalMem` as in A.3; `pub struct MegaBuf { blocks: Vec<Option<Box<[f64; 65536]>>> /* 128 */ } impl MegaBuf { pub fn get(&self, i: f64) -> f64; pub fn set(&mut self, i: f64, v: f64); pub fn free(&mut self, above_index: f64); pub fn copy(&mut self, dst: f64, src: f64, len: f64); pub fn fill(&mut self, dst: f64, v: f64, len: f64) }` with index = `floor(i + 0.00001)` as in `_asm_megabuf`.
- `reg00`..`reg99` live in `GlobalMem` as `[f64; 100]`; `Vm::var("reg07")` returns a `VarRef` that reads and writes the shared register.

- [ ] **Step 1:** Write failing tests `var_lookup_is_case_insensitive`, `new_var_starts_at_zero`, `reg_vars_are_shared_between_vms`, `megabuf_out_of_range_reads_zero_and_ignores_writes`, `megabuf_index_rounds_toward_floor_with_epsilon` (`get(2.99999)` reads slot 3? No: `floor(2.99999 + 0.00001) = 3`; pin this), `reset_vars_zeroes_and_frees`.
- [ ] **Step 2:** Run `cargo test -p md-eval vm`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(eval): variable table and megabuf`


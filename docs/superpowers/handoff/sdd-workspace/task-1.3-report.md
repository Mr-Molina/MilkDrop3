# Task 1.3 report: variable table and global memory

Commit ac5322a `feat(eval): variable table and megabuf`. Files: crates/md-eval/src/{mem.rs, vm.rs, vm_tests.rs}, lib.rs (modules + re-exports).

## What was implemented
- `Vm` (var/get/set/reset_vars; pub `megabuf`, pub `global`), `VarRef(u32)` (`index()`), `MegaBuf`, `GramBuf`, `GlobalMem { regs: [f64;100], gmegabuf: GramBuf }`.
- Names: ASCII-lowercased then truncated to 16 chars. ns-eel.h:126 defines NSEEL_MAX_VARIABLE_NAMELEN 16; nseel-eval.c's `#ifndef ... 8` is shadowed by it. C compares with strnicmp(..,16) so truncate (never reject).
- `regNN`: exactly "reg" + 2 digits (nseel-eval.c:145) -> shared register (VarRef >= 1<<24). `reg7` / `reg100` are ordinary variables.

## Semantics pinned from C (these differ from the brief/milestone text)
1. Index conversion is TRUNCATION toward zero, not floor: `_asm_megabuf` does fadd g_closefact (0.00001) then fistp, and GLUE_CALL_CODE forces _RC_CHOP (nseel-compiler.c:233-252). So get(-0.5) and get(-1.0) read slot 0; get(-1.5) is out of range. Test: `megabuf_index_truncates_after_adding_1e5` (not "rounds toward floor"). Caveat: the _WIN64/LP64 path calls win64_callcode without changing the rounding mode, so 64-bit builds may round-to-nearest; I mirrored the 32-bit chop path (EEL_F2int in memcpy/memset/freembuf is the same fistp). Only matters for fractional indexes and negatives.
2. gmegabuf is NOT a sparse 128-block array in this build: MilkDrop never calls NSEEL_VM_SetGRAM, so __NSEEL_RAMAllocGMEM uses one static flat buffer of 1<<20 slots indexed by `(unsigned)w & (2^20-1)`: every index wraps (negatives too), nothing is out of range. Implemented as `GramBuf` (lazy alloc on first write). milestone-1-context.md says gmegabuf is the same sparse structure; that is wrong for this source. Task 1.5 must route gmegabuf to `global.borrow_mut().gmegabuf`.
3. memcpy/memset use epsilon 0.0001 (not 0.00001) on all args. memcpy: negative src trims front of both, then negative dst trims; chunked by block boundary with memmove per chunk; stops when either offset >= 8388608. memset: negative start trims len; clamped to bound. Because of chop, e.g. copy(.., -2.0, ..) truncates to -1 (tests use -2.5).
4. freembuf(n): C only stores n+1 in a flag (d<0 -> 0; ignored if d >= 8388608) and returns its argument pointer (value of expression = n). The actual free happens in NSEEL_VM_freeRAMIfCodeRequested, which frees blocks x with x*65536 >= d. MilkDrop never calls freeRAMIfCodeRequested (grep of code/), so in the real host freembuf is effectively a no-op until the VM is destroyed. `MegaBuf::free` frees immediately with the C block rule. Task 1.5 should decide whether to make freembuf a no-op for parity (recommend: yes, per host behaviour) or call free.
5. Deviation (unobservable): copy from an unallocated source does not allocate it (C allocates both); it zero-fills an already-allocated destination.
6. Out-of-i32-range / NaN index -> x87 "integer indefinite" i32::MIN (read 0 / write ignored for megabuf; slot 0 for gmegabuf).

## Other
- `reset_vars` zeroes locals and clears megabuf; leaves regs and gmegabuf (shared, owned by GlobalMem).
- Extra tests beyond the brief: 16-char names, reads-don't-allocate, gmegabuf wrap/shared, free/copy/fill semantics, memmove overlap.

## TDD evidence
Tests were written in the same pass as the implementation (not strictly red-first). Mutation check: replacing `trunc` by `floor` in `eel_int` made 4 tests FAIL (copy_trims..., gmegabuf_is_flat..., fill_trims..., megabuf_index_truncates...); restored -> `cargo test -p md-eval vm`: 15 passed. Initial runs of my own test text had 3 failures from my wrong expectations about chop+epsilon, fixed in tests (C-faithful).
Verification: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` clean; `cargo test --workspace` all ok.

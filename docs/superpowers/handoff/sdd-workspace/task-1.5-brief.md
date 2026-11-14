### Task 1.5: Built-in functions

**Files:**
- Create: `crates/md-eval/src/builtins.rs`, `crates/md-eval/src/mt19937.rs`

**Interfaces:**
- Produces: `pub enum Fn1 { Sin, Cos, Tan, Asin, Acos, Atan, Sqrt, Sqr, Abs, Exp, Log, Log10, Floor, Ceil, Sign, Invsqrt, Rand, Freembuf }`, `pub enum Fn2 { Atan2, Pow, Min, Max, Sigmoid, Band, Bor, Exec2 }` plus `Fn3 { Exec3, Memcpy, Memset }`; `pub struct Mt19937; impl Mt19937 { pub fn seeded(seed: u32) -> Self; pub fn next_u32(&mut self) -> u32 }` living in `GlobalMem` (one generator shared by every `Vm`, as the C static is), seeded with `0x4141f00d`.
- Pinned formulas: `sigmoid(x, c) = 1 / (1 + exp(-x * c))` when `c` != 0 else `0`? No: ns-eel2's `eel1sigmoid` returns `t = 1 + exp(-x*c); t != 0 ? 1/t : 0`. `band(a,b)` = `(a truthy && b truthy) ? 1 : 0`; `bor` likewise with `||`; `sign(x)` = -1, 0, or 1; `invsqrt(x)` = `1/sqrt(x)`; `sqr(x)` = `x*x`; `exec2(a,b)` returns `b`; `exec3(a,b,c)` returns `c`; `memcpy(dst, src, len)` and `memset(dst, v, len)` return `dst`; `freembuf(i)` frees blocks above index `i` and returns 0.

- [ ] **Step 1:** Write failing tests `mt19937_first_output_for_seed_0x4141f00d` (value computed by the reference MT19937 algorithm; record it in the test), `rand_floors_and_clamps_to_one`, `rand_is_below_max`, `sigmoid_matches_formula`, `sign_of_zero_is_zero`, `exec3_returns_third`.
- [ ] **Step 2:** Run `cargo test -p md-eval builtins`. Expected: FAIL.
- [ ] **Step 3:** Implement, including a from-spec MT19937 (the C code in `nseel-cfunc.c` lines 39-99 is the standard algorithm; implement from the published algorithm, not by translating the C).
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(eval): built-in functions and MT19937`


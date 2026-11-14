# Task 1.4 report: bytecode compiler and stack VM

Status: DONE_WITH_CONCERNS (the concerns are documented divergences; see the end of this report)

## What was implemented

- `src/bytecode.rs`: `Op`, `Fn1`/`Fn2`/`Fn3` (each has a variant for every builtin of that arity in the parser's table, plus `from_name`), and `Program` (a flat `Vec<Op>`, crate-private, read through `Program::ops()`).
- `src/compiler.rs`: `compile_ast(vm, &Expr) -> Result<Program, CompileError>` and `Program::compile(vm, src)`, which runs lexer, then parse (preproc), then compile_ast.
  - Lazy forms compile to jumps. `c?a:b` becomes `[c] JumpIfFalse [a] Jump [b]`. `&&` becomes `[a] And(X) [b] Bool`, and `||` uses `Or(X)` the same way. `loop`/`while` become `Loop{body_start,body_end}`/`While{..}` with the body inlined after the op.
  - Evaluation order: arguments go left to right. For assignment the index (if any) is evaluated first, then the right side, then the target is read (compound) and written. This matches the x86 stubs.
- `src/vm.rs`: adds `Program::run(&self, &mut Vm) -> f64` (recursive `exec(range)` for loop bodies), `LOOP_CAP = 1_048_576`, `CLOSEFACTOR`, and the operator helpers. The value stack is a `Vec<f64>` field on `Vm` that is taken and restored on each run, so a run allocates nothing after warm-up. RefCell borrows of `GlobalMem` last for one get/set only.
- `src/builtins.rs`: `FnX::eval` dispatch. Task 1.5 should MODIFY this file, not create it. I put it here so the dispatch has a home and to keep vm.rs small.

## Semantics, verified against code/ns-eel2 (x86 stubs; GLUE_CALL_CODE sets _RC_CHOP, nseel-compiler.c:225-260)

| Construct | C source | Rust behavior (tested) |
|---|---|---|
| truthiness (`_if`, `_and`, `_or`, `!`, `while`) | `fabs; fcomp g_closefact`, C0 = below/unordered → false | `abs(x) >= 1e-5`. Exactly 1e-5 is TRUE and NaN is false. The milestone text says `> 1e-5`; the C wins at exactly 1e-5 |
| `_if` | nseel_asm_if: slot 0 = true block, slot 4 = false block (compiler.c:801) | lazy |
| `_and` / `_or` | nseel_asm_band/bor, special-cased in compiler.c:889 | short-circuit; result exactly 0/1 |
| `band` / `bor` | `eel1band`/`eel1bor` via 2pdd (compiler.c:478-485) | both operands evaluated; uses `fabs > closefact` (strict) |
| `!x` | nseel_asm_bnot | 1 if not truthy, else 0 |
| `==` / `!=` | `fsub; fabs; fcomp closefact` | `==` is 1 when \|b-a\| < 1e-5 or the difference is NaN (so NaN==x and Inf==Inf are 1); `!=` is the inverse |
| `< > <= >=` | fcomp C0 tests (gcc asm 1039-1120) | exact 0/1. NaN makes `<` and `>` 1, and `<=` and `>=` 0 |
| `%` | nseel_asm_mod: fabs both, 32-bit chop fistp, unsigned divl, 0 if divisor 0 | `(eel_int(\|a\|) as u32) % (eel_int(\|b\|) as u32)`, so the result is never negative. NaN or out-of-range values become 0x80000000. `(-7)%3 = 1`; `-7%3` parses as `-(7%3)` |
| `&` / `\|` | nseel_asm_and/or use **64-bit** `fistpll` | i64 chop; NaN or out-of-range becomes i64::MIN. The brief said i32, but the C (msvc too, line 946) uses qword |
| `/` | fdiv with masked exceptions | IEEE (inf/NaN), no panic |
| `_set` (also `megabuf(i)=`, `gmegabuf(i)=`) | nseel_asm_assign: exponent bits all 0 or all 1 → store 0 | stores +0 for ±0/denormal/±Inf/NaN. The assignment's VALUE is the unfiltered right side (the stub leaves eax pointing at the source) |
| `+= -= *= /= %= &= \|= ^=` | `_addop`.. stubs store with plain fstp | **not** filtered (tested: `x/=0` gives inf). The value is the new target value |
| `loop(n, body)` | nseel_asm_repeat: 32-bit chop fistp, skip if < 1, cap 1048576 | truncation (equals floor for counts >= 1). A count of NaN or above i32 range runs 0 times. The value is the last body value, or the count itself when the body never ran |
| `while(body)` | nseel_asm_repeatwhile | body runs at least once, at most LOOP_CAP times. Value: see concern 2 |
| `exec2`/`exec3`/`;` | nseel_asm_exec2 is empty | in order, the last value |
| `sqrt` | nseel_asm_sqrt does fabs first | `sqrt(\|x\|)` |
| `min`/`max`, `sign`, `sigmoid`, `invsqrt` | asm/C as cited in builtins.rs | implemented from the C (`invsqrt` emulates the chop-rounded `fsts`) |
| `memcpy`/`memset` | return the dest argument (nseel-ram.c) | uses MegaBuf::copy/fill and returns dest |
| `freembuf` | ledger ruling 1.3 | no-op returning its argument |
| `rand` | — | **TODO(1.5)**: returns 0 (inside the valid range) until MT19937 lands |

## Deviations from the brief's Op list

- No `Op::Pow`. pow and `^` go through `Call2(Fn2::Pow)`, as the plan's Fn2 list implies.
- No `Op::Dup`, because nothing needs it.
- `And`/`Or` carry a jump target (`And(u32)`/`Or(u32)`), and a `Bool` op was added.
- Added `Update(VarRef, AssignOp)`, `UpdateMem(bool, AssignOp)` and `Call3(Fn3)` (per F15). `Fn2::Exec2` and `Fn3::Exec3` exist (per the plan's 1.5 list) and are used.

## TDD evidence

RED (tests written first; `Program::run` did not exist yet):
```
$ cargo test -p md-eval compiler
      7 error[E0599]: no method named `run` found for struct `Program` in the current scope
error: could not compile `md-eval` (lib test) due to 7 previous errors
```
GREEN, first run after implementing: 28 passed, 1 failed (`mod_uses_absolute_truncated_operands`). The failing assertion assumed `-7 % 3` meant `(-7) % 3`. The preprocessor binds `%` to the adjacent primary, so the expression is `-(7%3) = -1`. I fixed the test to assert both forms. Then:
```
$ cargo test -p md-eval compiler
test result: ok. 30 passed; 0 failed; ... 53 filtered out
$ cargo test --workspace      -> all ok (md-eval lib: 83 passed)
$ cargo clippy --workspace --all-targets -- -D warnings   -> clean
$ cargo fmt --check           -> clean
$ cargo build --workspace     -> ok
```
Tests (src/compiler_tests.rs, run as `compiler::tests::*`): if_does_not_evaluate_untaken_branch, truthiness_is_abs_not_below_closefactor, not_yields_exact_zero_or_one, and_or_short_circuit_and_yield_zero_or_one, band_bor_evaluate_both_operands, comparisons_yield_exact_zero_or_one, equality_uses_closefactor, nan_comparisons_follow_x87_unordered_flags, mod_by_zero_is_zero, mod_uses_absolute_truncated_operands, bitops_truncate_operands, division_by_zero_is_ieee_not_panic, assignment_stores_zero_for_denormal_inf_nan, compound_assignment_does_not_filter, compound_assignments_compute_target_op_value, megabuf_and_gmegabuf_round_trip, gmem_and_regs_read_while_written_do_not_panic (two Vms sharing GlobalMem; `gmegabuf(1) = gmegabuf(1) + reg00; reg00 = gmegabuf(1); reg01 += ...; gmegabuf(2) += ...`), loop_count_is_floored_and_capped (includes a 2,000,000 count giving exactly 1,048,576 iterations, because the check is cheap; 1.6 keeps its dedicated while-cap test), loop_value_is_last_body_value_or_the_count, loop_body_count_is_evaluated_once, while_repeats_while_body_is_true, nested_loops, sequences_evaluate_in_order_and_return_last, arithmetic_and_precedence, builtins_dispatch, memset_and_memcpy_return_dest, variables_persist_across_runs_and_programs, compile_reports_syntax_errors, compile_ast_rejects_unknown_function, lazy_forms_compile_to_jumps.

## Files changed

- Created: `crates/md-eval/src/bytecode.rs`, `compiler.rs`, `compiler_tests.rs`, `builtins.rs`
- Modified: `crates/md-eval/src/vm.rs` (stack field and interpreter; 389 lines), `crates/md-eval/src/lib.rs`

## Self-review

- All RefCell borrows are single-expression (`vm.get`/`vm.set` for regs, `load_mem`/`store_mem` for gmem). None is held across a sub-expression, and the two-Vm aliasing test proves it.
- The interpreter never panics on compiler output: jump targets and body ranges come only from the compiler, because `ops` is crate-private. Stack underflow reads 0 instead of panicking.
- Loop recursion depth is bounded by the parser's nesting cap (400).

## Concerns / divergences (for the ledger)

1. **Pointer aliasing is not reproduced.** ns-eel2 passes variables as pointers and reads them when the operator runs, so `x + (x = 5)` gives 10 in C. The Rust VM evaluates values eagerly, so it gives 0 + 5 = 5. The same applies to `min`/`max`/`_mem` results that are pointers. Mirroring this would need a pointer VM. It is rare in presets.
2. **`while` value:** C returns whatever sits in the loop's first temp slot (`movl %esi,%eax`). That equals the body value whenever the body's first temp-producing op is its result (e.g. `while(i<n)`), but is unspecified otherwise. Rust returns the body's last value.
3. **Truthiness at exactly 1e-5** follows the C (`>=`), not the milestone text (`>`). `band`/`bor`/`sigmoid` use the C helpers' strict `>`.
4. **`&`/`|` use i64** (as the C does), not the brief's i32.
5. **Intermediate precision:** x87 computes in 80 bits and Rust in f64, so results can differ in the last bits.
6. **TODO(1.5) placeholders:** `rand` returns 0. sin/cos/tan of |x| >= 2^63 (x87 leaves the value unchanged) is not emulated. `invsqrt` is implemented from the C but its exact value is untested (1.5 should pin it).
7. **builtins.rs already exists**, so Task 1.5 modifies it rather than creating it.

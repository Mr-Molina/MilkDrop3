# Task 1.8 report: Performance budget (plus folded-in deferred items)

Status: DONE_WITH_CONCERNS (the concerns are minor; see the end)

Commits (branch rust-experiment, not pushed):
- `86ec71b` perf(eval): benchmark and fast path
- `595d7d3` fix(eval): bound preprocessing on pathological input

## 1. Benchmark (brief Steps 1-2)

**Harness.** I tried criterion 0.8.2 first, with `default-features = false, features = ["cargo_bench_support"]`. `cargo deny check` failed: `error[banned]: crate 'cc = 1.11.1' is explicitly banned`, pulled in through criterion -> `alloca` -> `cc`. Per the ruling I removed criterion and restored Cargo.lock. The bench is now a std-only `harness = false` binary that uses `std::time::Instant`.

- `crates/md-eval/benches/support/workload.rs`: the shared workload. It runs the brief's program, `zoom = zoom + 0.1*sin(rad*6 + time); rot = rot + 0.02*ang; dx = dx + 0.001*cos(ang*3)`, over the 65 x 65 = 4225 vertices of the default 64 x 64 mesh. `rad` and `ang` are computed per vertex, `time` is 1.25, and zoom/rot/dx are restored before each vertex, as MilkDrop does.
- `crates/md-eval/benches/per_pixel.rs`: 500 ms warm-up, then 300 timed frames. It prints min, median, mean and p95, and exits 1 if the median is over 2 ms. When run without `--bench` (for example `cargo test --benches`), it runs one frame as a smoke test.
- `crates/md-eval/Cargo.toml`: `[[bench]] name = "per_pixel", harness = false`.

**Measured** (`cargo bench -p md-eval`). The CPU from /proc/cpuinfo is `Intel(R) Xeon(R) Processor @ 2.10GHz`, 4 vCPUs:
```
per_pixel_4225: min 365.348µs  median 458.205µs  mean 464.766µs  p95 529.428µs  (300 samples; budget 2ms)
per evaluation: median 108.5 ns
```
A rerun after commit 2 gave a median of 484 µs. Criterion, during the trial, reported `[510.70 µs 517.60 µs 526.72 µs]`. The workload is about 4x under the 2 ms budget, so I did **not** add the superinstruction fast path. The VM is unchanged.

**Regression guard in `cargo test`** (`crates/md-eval/tests/perf_guard.rs`). It runs the same workload, takes the best of 7 frames, and checks that frames are deterministic. The limit is 100 ms. Measured best frame: 2.4 ms in debug (about 40x margin) and 0.46 ms in release (about 200x margin). Because it takes the minimum and the margin is large, it should not be flaky under parallel test load or a noisy scheduler. I ran the md-eval suite 3 more times and it passed every time. It only catches order-of-magnitude regressions.

## 2. Folded-in (a): quadratic preprocessing on left-wrapping input

**Cause.** Wide/primary left-wrapping operators (`|| && % ^ [ ]`) put the whole output buffer so far inside a new call: `split_off`, then `left_start` scans the buffer, then the tokens are cloned back in. So `a||` x5000 rebuilds the buffer 5000 times, and only then does the parser's 400-level cap reject it.

**Fix** (`src/preproc.rs`, plus `src/parser.rs`, whose `MAX_NESTING` is now `pub(crate)`):
- `run` now takes `base`, a lower bound on the parenthesis depth at which its output lands. For each rewrite, `inner = base + parens + 1` is the depth of the call's arguments, because every unclosed `(` comes before the left operand. A token at depth d makes the parser recurse d+1 levels, so once `inner + paren_depth(lhs) >= MAX_NESTING`, the output can never parse.
- I first had this return the error immediately. Self-review found a false positive: `gmem <stuff>[i]` **discards** its left operand. `gmem (a||a||...x450 a)[1]` and `gmem ((( x399 a||b )))[1]` both compile to `_gmem(1)` in the old code; the first variant also took 1.2 s at x5000. So the rewrite is now replaced by a single placeholder token, `Tok::Ident("\0too deep")`; the lexer never emits NUL in an identifier. Any rewrite whose operand contains that placeholder also collapses to it. The parser rejects the placeholder with "expression nested too deeply". If a later `gmem[` drops it, the program parses as before. `is_gmem` was split out of `index` so the gmem case can be checked up front. The right operand is still preprocessed, so the old recursion-depth errors are kept.

**Equivalence check** (one-off, not committed). I built the pre-fix tree in a temporary git worktree and compiled 6460 generated inputs with both builds: random mixes of `a|| a&& a% 2^ x[1] [1] ( ) ! - gmem[ gmem ( ? : = += sin( ;`, sized 50-1200 pieces around the 400 limit, gmem-wrapped variants, and exact chains at n = 390..410. Results:
- 1451 inputs were accepted by both builds, with identical `ops()` for every one.
- 5009 were rejected by both.
- None were accepted by one build and rejected by the other.
- About 24 rejected inputs (malformed *and* over-deep) now report "expression nested too deeply" instead of an earlier syntax error, such as `unexpected ')'` or `unknown function 'gmem'`.

**Timings after the fix** (`Program::compile`, release / debug):

| input | before (release) | after (release) | after (debug) |
|---|---|---|---|
| `a\|\|` x5000 | 1.29 s | 8.6 ms | 24.6 ms |
| `a&&` x5000 | 1.40 s | 12.2 ms | 28.5 ms |
| `a%` x5000 | 1.46 s | 8.1 ms | 20.0 ms |
| `2^2` x5000 | 0.80 s | 5.0 ms | 16.5 ms |
| `x[1]` x5000 | 3.44 s | 4.9 ms | 14.9 ms |
| `x` + `[1]` x5000 | 2.12 s | 2.9 ms | 16.3 ms |
| `(a)\|\|` x5000 | 1.66 s | 9.2 ms | 25.9 ms |
| `a\|\|-` x50000 | n/a | 13.0 ms | 49.3 ms |

Every case is under 50 ms in release.

**Tests** (`src/parser_tests.rs`):
- `pathological_left_wrapping_fails_fast`: 7 inputs at x5000. Each must fail with "nested too deeply". There is also a generous 1 s bound per input, which is not tight in any profile.
- `left_wrapping_limits_are_unchanged`: characterizes the pre-fix boundaries. The largest n that still parses is 399 for `a||`, `a%`, `2^`; 199 for `x[1][1]..`; 398 for `(a)||`; 393 inside 6 parens; 396 after `y=b=(`; 397 inside `sin(sin(`. n+1 must fail with "nested too deeply". This test passed before the fix and after it.
- `gmem_left_operand_does_not_count_toward_nesting`: 450 parens, a 399-deep `a||b`, and a x5000 `||` chain inside a gmem left operand must all parse to `(_gmem 1)`. The same chain inside the gmem *index* must fail. Mutation check: with the gmem exemption disabled, this test fails.

RED (before the fix, debug):
```
$ cargo test -p md-eval --lib -- left_wrapping
test parser::tests::left_wrapping_limits_are_unchanged ... ok
test parser::tests::pathological_left_wrapping_fails_fast ... FAILED
a||a||a||a||...: 2.667659385s
```
GREEN:
```
test parser::tests::deep_nesting_is_an_error_not_a_stack_overflow ... ok
test parser::tests::gmem_left_operand_does_not_count_toward_nesting ... ok
test parser::tests::pathological_left_wrapping_fails_fast ... ok
test parser::tests::left_wrapping_limits_are_unchanged ... ok
test result: ok. 4 passed; 0 failed; ... finished in 0.38s
```

## 3. Folded-in (b): Tasks 1.6/1.7 minors

- **proptest `handle-panics`.** The workspace pin is now `features = ["std", "handle-panics"]`. Cargo.lock does not change (no new crates). `cargo tree -e features -i proptest` shows `proptest feature "handle-panics"`. cargo-deny and the native-build script both pass.
- **Compile-only `while(`/`loop(` properties** (`tests/props.rs`):
  - `printed_loop_asts_compile` (256 cases): random ASTs with `while(body)` and `loop(count, body)` nodes, where `count` is an arbitrary expression (variables, assignments, calls), printed back to source. It asserts that `Program::compile` succeeds and never runs the program.
  - `loop_soup_compile_never_panics` (256 cases): token soups made from `while( loop( while loop ( ) , ; a x= 1 n += megabuf( -` plus the existing SOUP. Compile only.
- **`5 % 0` pinned** (`tests/bounds.rs`). nseel_asm_mod skips the `div` and returns edx = 0 when the chopped |divisor| is 0, so the result is exactly +0.0. I assert bit-exact +0 for `5 % 0`, `5 % 0.5`, `(-7) % (-0.9)` and `a=5; a %= 0; a`. First RED: I wrote `-7 % -0.9`, which gave -0.0 because unary minus binds over `%` per the Task 1.2 ruling. That was a mistake in my test input, not in the code, and I parenthesized it.

## Verification (before commit 2)
- `cargo fmt --check`: ok.
- `cargo build --workspace`: ok.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: all pass. md-eval: lib 105, bounds 8, props 7, perf_guard 1, corpus tests pass.
- `cargo test --release -p md-eval`: pass.
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`.
- `scripts/check-no-native-build.sh`: OK.

## Files changed
- Cargo.toml (proptest feature)
- crates/md-eval/Cargo.toml ([[bench]])
- crates/md-eval/benches/per_pixel.rs, crates/md-eval/benches/support/workload.rs (new)
- crates/md-eval/tests/perf_guard.rs (new)
- crates/md-eval/src/preproc.rs, crates/md-eval/src/parser.rs, crates/md-eval/src/parser_tests.rs
- crates/md-eval/tests/bounds.rs, crates/md-eval/tests/props.rs

## Self-review findings / concerns
1. **Gap in `scripts/check-no-native-build.sh`.** It walks only `-e normal,build` edges, so it missed `cc` arriving through a **dev-dependency** (criterion -> alloca -> cc): the script said "OK" while cargo-deny failed. cargo-deny caught it, so CI is still protected. Adding `dev` to the script's edge set would close the gap; that belongs to Task 0.3, so I did not change it.
2. The commit subject follows the brief ("benchmark and fast path"), but no fast path was needed; the commit body says so.
3. The equivalence evidence for the preprocessing fix is a differential run plus a reasoning argument (lower-bound depth, with gmem as the only construct that drops tokens). Only the boundary tests and the gmem tests are committed, not the 6460-input differential.
4. Some inputs that are both malformed and over-deep now report "nested too deeply" instead of their first syntax error. Both outcomes are compile errors.
5. 108 ns per evaluation leaves 4x headroom on this machine class. Slower hardware or bigger presets may need the superinstruction path later.

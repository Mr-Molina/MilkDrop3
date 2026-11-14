# Tasks 4.9 + 4.10 report (md-hlsl: corpus translation, triage, robustness)

Status: DONE_WITH_CONCERNS (concerns at the end)

## Commits (rust-experiment, oldest first)

| SHA | Subject |
|---|---|
| 4864706 | test(hlsl): corpus translation at 100% |
| a9df9e6 | fix(hlsl): guard constant shifts and constant zero divisors from const evaluation |
| 5ef956f | test(hlsl): cover translate's naga-rejection path and validate_wgsl |
| 240891a | docs(hlsl): keep WGSL round (half to even) and record why |
| 296d1fc | fix(hlsl): mirror StripComments for block comments, `/*/` and `*//` |
| ae2308f | fix(hlsl): lower the macro-expansion token budget to 500k |
| 945f956 | fix(hlsl): run all-constant intrinsic calls at run time |
| 1ae8bd5 | fix(hlsl): compare bools as i32 for < > <= >= |
| cb792aa | fix(hlsl): run all-constant selects at run time |
| e960afc | test(hlsl): fuzz target |
| 6fd10cf | docs(hlsl): name apply_fixups in the crate docs |

## Task 4.9: corpus test and triage

- `tests/common/corpus.rs` (new, self-contained) holds the corpus loop: the two shipped default bodies, then every `md_testkit::corpus_presets()` file (so `$MD_EXTRA_PRESETS` is included). Each file goes through `md_preset::parse_milk`, and each non-empty warp/comp body is translated with the preset's own `PSVERSION_*`, mapped to `md_hlsl::PsVersion`. A preset that fails to parse counts as a failure (`shader = "preset"`) and does not panic. The module also has `failure_table` (`preset | shader | line:col | message`) and `group_failures`, which groups by message prefix: the first line, with quoted spans → `'_'` and digit runs → `#`.
- `tests/corpus.rs::every_corpus_shader_translates_and_validates` prints the counts and, if there are failures, the table. It then asserts zero failures, at least 30 presets and at least 66 shaders. Consolidation: `emit_corpus.rs::corpus_emission_report` was removed (it was the same loop). `emit_corpus.rs` keeps its default-shader, layout and assorted-construct tests.
- `examples/triage.rs` includes the shared module via `#[path]` and prints a summary line, the grouped counts (most frequent first) and the table.
- Result: `cargo test -p md-hlsl --test corpus -- --nocapture` → `corpus translation: 66/66 shaders from 32 presets (+2 defaults) translate and validate`, 5 passed. `cargo run -p md-hlsl --example triage` → `32 presets (0 failed to parse), 66 shaders: 66 translate, 0 fail`.
- Triage check against an extra pack (scratchpad dir of 4 broken copies via MD_EXTRA_PRESETS): `36 presets …, 74 shaders: 70 translate, 4 fail`, grouped `3 | unknown function '_'`, `1 | unknown identifier '_'`, with the table. No real preset pack is available here. Step 3 needed no translator iteration because the corpus was already 100%.
- RED/GREEN: the test was green on first run because the translator already passed 66/66 (Tasks 4.4/4.6/4.7). Its failure path was exercised through the example with the broken extra pack.

## Folded-in fixes

### (b) constant shift overflow / constant zero divisors (a9df9e6)
RED: `cargo test -p md-hlsl --test emit_const_eval` failed 5 of 7, with naga errors: `<< operation overflowed`, `Division by zero`, `Function 'h_PS' is invalid` (a runtime `x / N` or `a <<= 40` hits the validator, not the const evaluator), `division operation overflowed` (`INT_MIN / -1`) and `Float literal is infinite` (`1 / Z`). GREEN: 7 passed.

The fix is `Fx::guard_rhs` (emit_expr.rs), which replaces the zero-literal-only `divisor`. It moves the right operand into a `var` when naga would fold the operation and the fold could fail:
- integer `/` or `%` with a constant divisor, unless it is a literal other than 0 or -1;
- float `/` or `%` (and `fmod`) with a literal-zero or non-literal constant divisor;
- a shift whose amount is constant and either not a literal in 0..32, or whose left operand is also constant (`3 << 31` overflows naga's signed fold).

Resulting values, under WGSL runtime rules:
- shift amount mod 32: `1 << 40` = 256, `1 << -1` = `1 << 31`. This matches x86 `shl`/`sar` folding. fxc's own folded value cannot be checked here.
- `x / 0 = x`, `x % 0 = 0`, `INT_MIN / -1 = INT_MIN`.
- float division gives IEEE inf/NaN, as the SM2/3 float hardware did.

I chose the non-const route over the ledger's "SM2/3 float-math" alternative because a cast of inf back to int is undefined. Ordinary literal divisors and in-range literal shifts stay inline (tests pin `(h_i / 2i)`, `(h_a << u32(3i))`). `naga_const` (emit.rs) is `is_const_expr` with every binary operator allowed.

### (a) naga-failure path test (5ef956f)
`1 << 40` no longer reaches naga. One input still does: `float f = 1e30 * 1e30;`, a constant f32 overflow that fxc folded to +inf. `tests/translate.rs::naga_rejection_is_an_internal_error_at_line_0` asserts `(line, col) == (0, 0)`, the message prefix `internal translator error: the generated WGSL failed naga validation:` and naga's diagnostic. `validate_wgsl_reports_naga_messages` calls `validate_wgsl` directly on a parse error, a validation error and a valid module. Both are characterisation tests and passed on first run.

### (c) `round` (240891a)
Decision: keep WGSL `round` (half to even). Evidence searched:
- `round` appears nowhere in `code/` (`grep -rln "\bround(" code/` → none), nor in `assets/hlsl/*.fx` or `tests/presets/`.
- SM2/3 has no rounding instruction, so fxc must have expanded `round`. Whether it used `floor(x + 0.5)` or a sign-aware form cannot be checked without fxc. I don't know of well-documented fxc behaviour I could cite with confidence.

So there is no evidence to justify `floor(x + 0.5)`. The decision is recorded in the `emit_call.rs` docs and pinned by `emit_lowering.rs::round_stays_wgsl_round`. The outputs differ only at exact .5 inputs, and the golden suite can decide later.

### (d) StripComments (296d1fc)
RED: 3 failing in `tests/fixups.rs` (old output left block comments in, `/*/` unclosed, `*//` not a line comment). GREEN: 11 passed.

`strip_comments` ports plugin.cpp:1342 state by state: the sliding `(c0, c1)` window, `bIgnoreTilEndOfLine`, `bIgnoreTilCloseComment` and `nCharsToSkip` (2 after `*/`, reset to 0 at a line end). As a result:
- `/*/` is a complete comment;
- the `/` that closes `*//` opens a line comment;
- `//*` is a line comment;
- an unclosed `/*` drops the rest of the text;
- `a/**/b` becomes `ab` (the text is joined, as in the C++).

One documented difference: the C++ drops the newlines inside a block comment, but here they are re-emitted just before the next kept newline (or at the end). That keeps every later line's error position valid. Tests cover the edge cases, CRLF, the line count, and a translate error after a multi-line comment that still reports line 4.

## Task 4.10: `translate_never_panics` (proptest, F3; e960afc)

`tests/never_panics.rs`, with proptest added as a dev-dependency (workspace pin already present; Cargo.lock +1 line):
- Generators:
  - arbitrary `String`;
  - ASCII inside `shader_body { }`;
  - token soup over an HLSL-ish alphabet (keywords, types, intrinsics, `sampler_*`/`texsize_*`, operators, numbers), with preprocessor/comment tokens drawn 1:100;
  - typed expression/statement soup (recursive expressions over typed leaves, with const hazards `N = 0`, `Z = 0`, `1e30`), which reaches the checker, emitter and naga;
  - corpus-body mutations (token delete/insert/swap/duplicate, 1-3 per case).
- A fixed `translate_never_panics_on_pathological_inputs` test covers: 100k-deep parens, 100k-term chains, 100k unary minus, 20k `else if`, 50k nested blocks, assignment and ternary chains, swizzle chains, nested calls, a 2^39 `#define` bomb, self-referential macros, a 1 MiB identifier, 100k `/*`, repeated `shader_body {`, and NUL/BOM/max-codepoint text.
- Each case translates as Warp/Ps2_0 and Comp/Ps3_0 on a worker thread. A panic (Disconnected), a hang (recv timeout of 2×bound) or a call slower than the bound fails the case. The bound is 200 ms in release and 2 s in debug.
- `MD_PROPTEST_CASES` sets the cases per generator (default 96). Failures persist to `tests/never_panics.proptest-regressions` (`WithSource`, which avoids proptest's SourceParallel warning).
- Timings: debug `cargo test -p md-hlsl --test never_panics` takes 3.3 s (7 passed); the full workspace `cargo test` takes 15.5 s wall.

Heavy runs:
- First 20k run (`MD_PROPTEST_CASES=20000 cargo test --release …`) found no panic. It did find a **time-bound failure**: the macro bomb took 222 ms under parallel load (about 150 ms alone) because it used up the 2M-token expansion budget. Fixed in ae2308f: `MAX_TOKENS` is now 500k, about 50× the largest corpus expansion; the bomb now takes about 40 ms. `parameter_lookup_does_not_scale_with_parameter_count` now uses 5 calls instead of 20 to stay under the budget (still 2e9 scans if lookup were linear). The pathological test in release is the regression.
- After all fixes: `MD_PROPTEST_CASES=20000 cargo test -q --release -p md-hlsl --test never_panics` → 7 passed in 23.3 s, which is about 120k inputs × 2 kinds. A separate 100k-case release run of typed soup plus corpus mutations also passed (139 s).
- Coverage check (instrumented scratch runs, not committed), on 2000 cases:
  - corpus mutations: about 15% translate Ok and the rest fail across parser and checker errors;
  - typed soup: spread across about 15 checker error classes plus Ok;
  - token soup: mostly parse errors.
- No panic was found in any run.

Non-panic translator bugs the soup found (naga rejecting the emitted WGSL), each fixed with a regression test:
- 945f956: all-constant intrinsic calls, e.g. `lerp(2, 1, 0.5)`. This emitted `mix(2.0, 1.0, 0.5)`, which stays AbstractFloat and is rejected; `exp2(1e30)` (via ldexp) and `pow(-1, 0.5)` failed constant evaluation. Their arguments now go through `var`s.
- 1ae8bd5: `<`/`>`/`<=`/`>=` on bools emitted `vec3<bool> < vec3<bool>`. The operands are now compared as `i32`, which matches HLSL's 0/1.
- cb792aa: an all-constant `?:` emitted `select(40.0, 0.5, true)`. It now uses the same spill, refactored to `Fx::spill_constants`.
- A 60k typed-soup run after 1ae8bd5 surfaced only the `select` case. Runs after cb792aa found no naga rejections in what I sampled.

## Final verification
- `cargo fmt --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: all ok, 0 failed.
- `cargo doc -p md-hlsl --no-deps`: no warnings.

## Files
- New:
  - crates/md-hlsl/tests/common/corpus.rs
  - crates/md-hlsl/examples/triage.rs
  - crates/md-hlsl/tests/emit_const_eval.rs
  - crates/md-hlsl/tests/never_panics.rs
- Modified:
  - crates/md-hlsl/tests/{corpus.rs, emit_corpus.rs, common/mod.rs, translate.rs, fixups.rs, emit_lowering.rs, preprocess.rs}
  - crates/md-hlsl/src/{emit.rs, emit_expr.rs (469 lines), emit_call.rs, emit_place.rs, fixups.rs, preprocess.rs, lib.rs}
  - crates/md-hlsl/Cargo.toml, Cargo.lock

## Self-review / concerns
1. **Known remaining naga-rejection gap:** a float binary operation on two constants that overflows f32 (`1e30 * 1e30`) still fails naga's constant evaluation. fxc folded it to +inf. It is deliberately left as the input for the naga-path test. Guarding it would mean spilling every all-constant float `+ - *`. It is unlikely in real presets.
2. `naga_const` excludes calls, so a guarded operand that is itself an all-constant builtin call (e.g. `x / sqrt(0)`) is not detected. Such calls now spill their own arguments (945f956), so they are runtime anyway.
3. Integer division by a constant zero follows WGSL (`x/0 = x`), not SM2/3 float math (+inf cast to int). This is documented on `guard_rhs`; the ledger leaves the choice open.
4. The StripComments port joins text across a block comment exactly as the C++ does, but it re-homes the comment's newlines (documented difference) to keep error lines valid. Text after a multi-line comment on its closing line reports on the comment's first line, as fxc saw it.
5. `MAX_TOKENS` 2M → 500k changes a Task 4.1 constant. The rationale is in the doc comment.
6. The never-panics tests assert only panic and time, as the brief asks. An "Err is never an internal translator error" property would be stronger but would trip on concern 1.
7. The ledger's other deferred minor item, WARP_PARAMS duplicated in tests/common, was not touched (out of scope). The crate-doc omission of apply_fixups is fixed (6fd10cf).

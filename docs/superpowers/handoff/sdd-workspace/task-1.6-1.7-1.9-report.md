# Report: Tasks 1.6, 1.7, 1.9 (batch)

Commits: 493c20c (1.6), 2d4143f (1.7), 0d517b7 (1.9).

## Task 1.6 - loop bounds and error behavior
- `LOOP_CAP: u32 = 1_048_576` already existed in `vm.rs` (Task 1.4); now also re-exported at the crate root (`md_eval::LOOP_CAP`).
- C check: `nseel_asm_repeatwhile` (asm-nseel-x86-gcc.c:852) sets ecx=cap, runs body, exits on falsy/unordered (C0), else `decl ecx; jnz`. So the body runs exactly LOOP_CAP times when always true; `i=0; while(i=i+1)` leaves `i == 1048576`. Existing VM matches; pinned by test.
- tests/bounds.rs (8 tests): cap constant, while cap (i exact), while stops on falsy and returns last body value, nested loop/while cap per loop, loop out-of-range/large count, negative/NaN/zero count -> 0 iterations, division by zero (+-inf, 0/0 NaN, `/=`; `%` by 0 no panic), NaN falsy (?:, if, &&, !, while).
- TDD: no RED phase possible - behaviour was already implemented in 1.4 (the ruling moved only the tests). Tests passed first run; no vm.rs change was needed.
- Run: `cargo test -p md-eval --test bounds` -> 8 passed.

## Task 1.7 - property and arbitrary-input tests
- No fuzz dir, no cargo-fuzz/libfuzzer-sys. Added `proptest = { version = "1.12.0", default-features = false, features = ["std"] }` to [workspace.dependencies] (avoids fork/tempfile), dev-dep of md-eval. `cargo deny check` ok (pre-existing "unmatched wrapper" warnings only), check-no-native-build OK.
- tests/props.rs (5 tests): `printed_asts_compile_and_run` (random ASTs: all operators, assigns incl. megabuf/gmegabuf targets, ternary, seq, 1/2-arg built-ins, `loop` with literal counts 0..=16, depth 4; fully parenthesised printer; run twice; no time bound), `print_parse_round_trip` (print(parse(print ast)) re-parses to the same AST), and three never-panics tests (random bytes, random ASCII from the language's punctuation, token soup; compile, and run if it compiles). `loop`/`while` are excluded from soups (1,048,576^depth iterations would turn it into a timing test).
- Cases 200 / 512; whole file runs in ~0.13 s debug. Also verified with PROPTEST_CASES=20000: all pass, 8.8 s.
- No bugs found by the properties.

## Task 1.9 - corpus compile
- Moved extraction (prefixes, keys, strip_comment, code_block) into tests/common/mod.rs with `corpus_blocks()`; corpus_parse.rs now uses it. Lines are joined with no separator (unchanged from 1.2).
- tests/corpus.rs `every_corpus_expression_compiles`: compiles every block on a fresh Vm, collects all failures (path, block key, CompileError, code) and asserts none. Result: 65 blocks across all 32 presets, 100% compile. No invalid corpus file found.

## Final checks (before last commit)
`cargo test --workspace` all ok (md-eval: 102 unit, bounds 8, props 5, corpus 1, corpus_parse 1, corpus_lex); clippy -D warnings clean; fmt --check clean; cargo deny ok; check-no-native-build OK.

## Concerns
None. Note the brief's 100 ms bound and `\n` join were replaced per rulings.

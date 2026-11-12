# Task 1.1 report: Lexer

Implemented `Tok` (token.rs, with LBracket/RBracket, Op includes `^=`), `CompileError` (error.rs, per A.3), `Lexer::tokenize` (lexer.rs). Eof token appended at offset src.len(). lib.rs wires modules and re-exports. md-testkit added as md-eval dev-dependency for tests/corpus_lex.rs (lexes every per_frame/per_pixel/per_point/init code block of the whole corpus; passes).

## ns-eel2 findings (decoded nseel-lextab.c DFA with a script + read the preprocessor)
- `.` is NOT an identifier char in this build: DFA splits `a.b` into `a`, `.`(DBLCONST = 0), `b`. Mirrored (test dot_is_not_an_identifier_char). Lone `.` lexes as Num(0).
- DFA has no exponent: `1e3` is `1`,`e3` there. Brief/milestone context require `1e3` -> kept as an accepted superset; exponent only when complete (`1e` stays `1`,`e`).
- `$x` mirrors strtoul(16): case-insens, optional `0x`, no digits -> 0, u64 saturate then truncate to u32. `$'c'` byte-level; `$PI`=3.141592653589793, `$E`=2.71828183, `$PHI`=1.61803399 (case-insens, truncated values exactly as C). Other `$` -> error.
- Unterminated `/*` runs to end silently (as C).
- `[`/`]`, `?`, `:` are handled by the C preprocessor, not the DFA; emitted as tokens here per F14.

## TDD
RED: with tokenize stubbed, `cargo test -p md-eval` -> 11 failed, 0 passed. GREEN: 11 passed + corpus_code_blocks_lex passed.
Full: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test --workspace` all clean.

## Concerns for 1.2
- ns-eel2's preprocessor rewrites `;` inside parentheses (to `%` if next non-space is alnum/`(`/`_`/`!`/`$`, else dropped) - parser concern (e.g. `exec2(a;b)`).
- `#[allow(clippy::approx_constant)]` used on `dollar` and tests because `$E` is intentionally 2.71828183.

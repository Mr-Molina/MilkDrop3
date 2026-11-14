# Task 1.2 report: Parser to AST

Status: DONE_WITH_CONCERNS (concerns are hand-offs to later tasks, not defects here)
Commit: 9079c57 feat(eval): parser

## What was implemented

- `crates/md-eval/src/ast.rs`: `Expr` exactly as the brief specifies, plus `UnOp {Neg, Not}`, `BinOp {Add, Sub, Mul, Div, Mod, BitAnd, BitOr, Eq, Ne, Lt, Gt, Le, Ge, And, Or}`, `AssignOp {Set, Add, Sub, Mul, Div, Mod, BitAnd, BitOr, Pow}`. `Display` prints an S-expression (used by tests). A doc table lists the canonical node for every operator and alias.
- `crates/md-eval/src/preproc.rs` (new, not in the brief; split out for the one-responsibility rule): a token-level port of ns-eel2's text preprocessor `preprocessCode` (nseel-compiler.c:1011-1428).
- `crates/md-eval/src/parser.rs`: `pub fn parse(&[(Tok, usize)]) -> Result<Expr, CompileError>`. It preprocesses, splits statements at the remaining top-level `;` (empty statements skipped), runs a Pratt parser with the bison grammar of nseel-caltab.c, and lowers operator-like calls to canonical nodes. Tests are in `src/parser_tests.rs` (26 tests).
- `crates/md-eval/src/lexer.rs`: `.e3` fix. An exponent is only tried after at least one mantissa digit. Test: `exponent_needs_a_mantissa_digit`.
- `crates/md-eval/tests/corpus_parse.rs`: builds every code block the way `ReadCode` + `StripLinefeedCharsAndComments` do. It reads `<prefix>1..` until a key is missing, strips a leading backtick, drops `//` and `\\` to end of line, and joins with no separator. Prefixes: per_frame_init_, per_frame_, per_pixel_, wave_i_{init,per_frame,per_point}, shape_i_{init,per_frame}. Each block must lex and parse. Result: 65 blocks, all pass. No corpus file was wrong.
- `lib.rs` re-exports `parse`, `Expr`, `UnOp`, `BinOp`, `AssignOp`.

## How ns-eel2 actually parses (the key finding)

The brief says its precedence table "matches ns-eel2's yacc grammar". It does not. The bison grammar (nseel-caltab.c) knows only `+ - * / % & |`, unary `+ -`, calls and parens. Every other operator is rewritten into a call *as text* by `preprocessCode`, and those rewrite rules decide how the operators bind:

- The left operand is found by scanning back over the already-rewritten output, skipping balanced `( )`. For `=`, `op=`, `%`, `^` and `[` it takes only the adjacent primaries. For comparisons, `||`, `&&` and `?` it goes back to the nearest `: ( , ; ? %`.
- The right operand is found by scanning forward over raw input with a per-operator stop set, and is rewritten recursively.

The Rust port works on the token level: the scan-back runs over output tokens and the scan-forward over input tokens. Synthetic tokens carry the operator's byte offset.

To verify it, I wrote an independent oracle in Python (scratchpad, not committed): a char-level port of `preprocessCode`, an emulator for the real bison tables (yytable/yycheck/... copied from nseel-caltab.c), and the function table. Results:
- All 26 parser tests' expected trees were checked against the oracle.
- 190,000 random expressions and token soups were diffed against the oracle. The only disagreements are (a) the documented `;.5` superset below and (b) 9 cases that print a nested Seq differently but mean the same thing.
- All 65 corpus blocks give identical trees in the oracle and the parser.

### Differences from the brief's table (the C wins, per ruling; each has a test)

| brief | ns-eel2 (implemented) | test |
|---|---|---|
| `^` right-assoc | `^` is **left**-assoc: `2^3^2` = `pow(pow(2,3),2)` = 64 | `power_is_left_associative` (renamed from the brief's `power_is_right_associative`) |
| `^` above unary | `^` takes only the primary to its left: `-2^2` = `-(pow(2,2))`; `2^-x*3` = `pow(2,-x)*3`; `a*b^c` = `a*pow(b,c)` | `unary_minus_applies_to_power` |
| `%` with `* /` | `%` → `_mod(l,r)` over the adjacent primaries, so it binds tighter than `*`: `a*b%c` = `a*(b%c)` | `mod_binds_tighter_than_mul` |
| `\|` < `&` < `==` < `<` < `+` | bison: `\|` < `&` < `+ -` < `* /`. So bitwise ops sit **below** `+` but **above** every comparison: `a+b&c` = `(a+b)&c`, `a & b == c` = `(a&b)==c` | `yacc_arithmetic_precedence`, `comparisons` |
| `==` below `<`, both left-assoc | all comparisons share one level and chain to the **right**: `a < b == c` = `a < (b == c)` | `comparisons` |
| `\|\|` below `&&` | `&&` and `\|\|` share one level, left-assoc: `a\|\|b&&c` = `(a\|\|b)&&c` | `and_or_share_precedence` |
| assignment lowest | `=` takes only the primary on its left: `a + b = 3` = `a + (b = 3)`; `-x = 3` = `-(x = 3)` | `assignment_left_operand_is_a_primary` |
| `;` lowest | inside parens, `;` becomes bison's `%` token = exec2 at the `*` level: `(a+1; b)` = `a + (1; b)`. It is **dropped** when the next non-blank char is not alnum/`(`/`_`/`!`/`$`: `(a; -b)` = `a - b` | `semicolon_inside_parens` |

Agreements: `=` and `op=` are right-assoc; `?:` is below `||` and right-nested; unary binds tightest of the bison operators.

### Other pinned semantics (each tested)
- `a ? b` with no `:` is `a ? b : 0`, and an empty branch is 0 (`ternary_forms`).
- `x[i]` = `_mem(x+i)` (megabuf); `x[]`/`x[0]` = `_mem(x)`; `gmem[i]` = `_gmem(i)`, case-insensitive; the closing `]` is optional as in C; `x[1][2]` nests; `2^x[1]` = `_mem(pow(2,x)+1)` (`brackets_index_megabuf`).
- Aliases follow `nseel_lookup` exactly: if, bnot, assign, equal, below, above, megabuf→_mem, gmegabuf→_gmem, int→floor. Every `fnTable1` name is callable, including `_if`, `_set`, `_and`, `_mod`, `_addop`, `_powop` and the rest. Arity is checked exactly (bison FUNCTION1/2/3). Errors are reported at the name's offset for unknown function, wrong arity, or a function name not followed by `(` (`unknown_function_is_error_with_offset`, `wrong_arity_is_error_at_name`, `function_name_without_call_is_error`).
- Canonical lowering: `?:`/if/_if → Ternary. `=`, assign, op=, `^=` → Assign. Comparisons, `&&`, `||` and `%` → Binary. `!`/bnot → Unary(Not). `^` and `pow()` → Call "pow". Unary `+` is dropped: `nseel_asm_uplus` is a no-op that passes the operand's address through, so `assign(+y, 1)` really writes `y` in C, and dropping it keeps that.
- Empty program and `;;` → `Seq([])`. One statement → that expression. Several → `Seq`.

## Deliberate divergences (documented, tested where meaningful)
1. **Non-lvalue assignment targets are an error** (`(a+b)=3`, `5=3`, `sin(x)=3`). ns-eel2 compiles these and writes the value into a scratch slot, so nothing observable happens. Test: `non_lvalue_assignment_is_an_error`. No corpus block does this.
2. **`;` followed by a number written `.5` inside parens** is treated as a sequence. C drops the `;` and then fails with a syntax error, so this only accepts what C rejects. The token has no source text, so a leading `.` cannot be seen.
3. **Nesting bounds**: preprocessor recursion is limited to 200 and parser nesting to 400, giving a CompileError instead of a stack overflow (bison's stack is 5000 states). Test: `deep_nesting_is_an_error_not_a_stack_overflow`, which also checks inputs just under the bounds.
4. `gmem x[i]`, which C reads as `_gmem(i)` (it drops `x`), is mirrored. `$'c'` after `^` (an error in C) parses here. Both are noise.

## TDD evidence
- RED (parser): `cargo test -p md-eval parser` with `parse` stubbed → `test result: FAILED. 1 passed; 25 failed` (only `syntax_errors` passed, trivially).
- RED (lexer): `cargo test -p md-eval exponent` → `exponent_needs_a_mantissa_digit ... FAILED` (left `[Num(0.0), Eof]`, right `[Num(0.0), Ident("e3"), Eof]`).
- GREEN: `cargo test -p md-eval parser` → `26 passed; 0 failed`. `exponent` → `2 passed`. `--test corpus_parse` → `1 passed`.
- Full: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test --workspace` all clean (md-eval lib: 38 passed). The deep-nesting test also passes in `--release`.

## Files changed
- A crates/md-eval/src/ast.rs (157 lines)
- A crates/md-eval/src/preproc.rs (334)
- A crates/md-eval/src/parser.rs (376)
- A crates/md-eval/src/parser_tests.rs (~340)
- A crates/md-eval/tests/corpus_parse.rs (91)
- M crates/md-eval/src/lexer.rs (`.e3` fix + test)
- M crates/md-eval/src/lib.rs

## Self-review
- I read the diff. Synthetic `(`/`)` do not affect the `;`-in-parens counter, matching C (its counter only sees source chars in the main loop). Recursion in the right operand starts its own counter, as C's recursive `preprocessCode` does.
- No `unsafe`, no panics on malformed input. `unwrap_or` is used only where the arity is already checked.

## Concerns / hand-offs to later tasks
1. **Task 1.4 (VM), important:** ns-eel2's `_set` (`nseel_asm_assign`, asm-nseel-x86-msvc.c:575-660) **stores 0 when the value is denormal, ±Inf or NaN** (exponent all-zero or all-one). Every `=` assignment flushes these. The milestone context does not pin this. Whether op= does the same needs checking in the `_op` asm.
2. **Task 1.3 (Vm::var):** variable names compare by `strnicmp(.., NSEEL_MAX_VARIABLE_NAMELEN)` = **16 chars** (ns-eel.h:126). Names that agree in the first 16 chars are the same variable in C. The parser keeps full names; `Vm::var` should truncate to 16 (bytes) to match.
3. **Task 1.4 / 1.5:** `pow` reaches the compiler as `Call{"pow"}` (from both `^` and `pow()`). Map it to `Op::Pow` or `Fn2::Pow`, whichever 1.4 prefers. `exec2`/`exec3` stay calls; `Seq` is the `;` form.
4. **Plan text:** the brief's precedence table and the claim that it "matches nseel-eval.c" are wrong. Later briefs (1.7 props printing ASTs back to source) need the table above. A printer must parenthesize every operand, or round-trips will re-associate (e.g. `^`, `%`, comparisons).
5. The Task 1.1 test `tests/corpus_lex.rs` misses wave/shape code blocks: it splits keys at the last `_`, and keys like `wave_0_per_frame1` have no `_N` suffix. `corpus_parse.rs` covers them (and lexes them), so this is only a stale test.

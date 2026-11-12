### Task 1.2: Parser to AST

**Files:**
- Create: `crates/md-eval/src/ast.rs`, `crates/md-eval/src/parser.rs`

**Interfaces:**
- Produces: `pub enum Expr { Num(f64), Var(String), Call { name: String, args: Vec<Expr> }, Unary(UnOp, Box<Expr>), Binary(BinOp, Box<Expr>, Box<Expr>), Assign(AssignOp, Box<Expr> /* lvalue: Var or Call megabuf/gmegabuf */, Box<Expr>), Ternary(Box<Expr>, Box<Expr>, Box<Expr>), Seq(Vec<Expr>) }`, `pub fn parse(tokens: &[(Tok, usize)]) -> Result<Expr, CompileError>`.
- Precedence, lowest to highest: `;` sequence, assignment (right-assoc), `? :`, `||`, `&&`, `|`, `&`, `== !=`, `< > <= >=`, `+ -`, `* / %`, `^` (right-assoc), unary `- ! +`, call/paren. This matches ns-eel2's yacc grammar in `nseel-eval.c`.
- Aliases resolved here to canonical calls: `if`, `bnot`, `assign`, `equal`, `below`, `above`, `megabuf`->`_mem`, `gmegabuf`->`_gmem`, `int`->`floor`, `abs`, `sqr`, `sqrt`, `pow`, `exp`, `log`, `log10`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `min`, `max`, `sign`, `floor`, `ceil`, `invsqrt`, `sigmoid`, `band`, `bor`, `rand`, `exec2`, `exec3`, `loop`, `while`, `memcpy`, `memset`, `freembuf`.

- [ ] **Step 1:** Write failing tests `power_is_right_associative` (`2^3^2` -> `2^(3^2)`), `assignment_is_right_associative`, `ternary_binds_looser_than_or`, `megabuf_call_is_valid_lvalue` (`megabuf(3) = 1`), `unknown_function_is_error_with_offset`, `empty_statements_are_allowed` (`a=1;;b=2;`).
- [ ] **Step 2:** Run `cargo test -p md-eval parser`. Expected: FAIL.
- [ ] **Step 3:** Implement a Pratt parser in `parser.rs`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(eval): parser`


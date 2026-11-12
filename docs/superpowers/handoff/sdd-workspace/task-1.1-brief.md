### Task 1.1: Lexer

**Files:**
- Create: `crates/md-eval/src/lexer.rs`, `crates/md-eval/src/token.rs`
- Test: `crates/md-eval/src/lexer.rs` (unit tests module)

**Interfaces:**
- Produces: `pub enum Tok { Num(f64), Ident(String /* lowercased */), Op(&'static str), LParen, RParen, Comma, Semi, Eof }`, `pub struct Lexer; impl Lexer { pub fn tokenize(src: &str) -> Result<Vec<(Tok, usize)>, CompileError> }`; operators recognised: `+ - * / % ^ | & = += -= *= /= %= |= &= == != < > <= >= && || !` and the question/colon pair `? :`.

- [ ] **Step 1:** Write failing tests `lexes_numbers_including_hex_and_leading_dot` (`.5`, `$x1F` -> 31.0, `1e3`), `lowercases_identifiers`, `skips_both_comment_styles`, `reports_offset_of_bad_char`.
- [ ] **Step 2:** Run `cargo test -p md-eval lexer`. Expected: FAIL.
- [ ] **Step 3:** Implement `Lexer::tokenize`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(eval): lexer`


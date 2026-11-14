### Task 4.1: Lexer and preprocessor

**Files:**
- Create: `crates/md-hlsl/src/{lexer.rs,preprocess.rs,token.rs}`

**Interfaces:**
- Produces: `pub fn preprocess(src: &str) -> Result<String, TranslateError>` handling `#define NAME value` and `#define NAME(args) body` (object-like and function-like, no `#if`), comment stripping, line continuation, and `//` inside strings not applicable; `pub fn tokenize(src: &str) -> Result<Vec<Token>, TranslateError>` with `Token { kind: TokKind, line: u32, col: u32 }`; `TokKind` covers identifiers, int and float literals (`1`, `1.0`, `1.f`, `.5`, `1e-3`, `0x1F`), all C operators HLSL uses including `?:`, `++`, `--`, compound assignment, and the punctuation `{}()[];,.`.

- [ ] **Step 1:** Write failing tests `expands_object_and_function_macros`, `macro_from_include_fx_time_expands_to_c2_x`, `lexes_float_with_f_suffix`, `reports_line_and_col`.
- [ ] **Step 2:** Run `cargo test -p md-hlsl lexer`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(hlsl): preprocessor and lexer`


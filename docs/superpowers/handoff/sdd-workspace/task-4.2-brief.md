### Task 4.2: Parser and AST

**Files:**
- Create: `crates/md-hlsl/src/{ast.rs,parser.rs}`

**Interfaces:**
- Produces: `pub struct Module { pub decls: Vec<Decl> }`, `Decl::{Global(VarDecl), Function(FnDecl), Struct(StructDecl)}`, `Stmt::{VarDecl, Expr, If, For, While, DoWhile, Return, Block, Break, Continue, Discard}`, `Expr::{Lit, Ident, Index, Member /* also swizzle */, Call, Ctor(TypeName, Vec<Expr>), Unary, Binary, Assign, Ternary, Cast}`, `pub fn parse(tokens: &[Token]) -> Result<Module, TranslateError>`.
- Types parsed: `void bool int uint half float double`, vectors `TN` for N in 2..4, matrices `TRxC` for R,C in 1..4, `sampler sampler2D sampler3D samplerCUBE`, arrays `T name[N]`, qualifiers `const static uniform in out inout`, semantics `: NAME`, register annotations `: register(sN)` ignored.
- Known gap handled here: a missing `;` before `}` at the end of a `shader_body` is accepted (fxc tolerated it); any other missing `;` is an error.

- [ ] **Step 1:** Write failing tests `parses_include_fx_without_error` (the real preamble), `parses_swizzle_chain` (`a.xyz.zyx.x`), `parses_matrix_type_float4x3`, `parses_for_with_decl`, `parses_ternary_nested`, `tolerates_missing_semicolon_before_closing_brace`.
- [ ] **Step 2:** Run `cargo test -p md-hlsl parser`. Expected: FAIL.
- [ ] **Step 3:** Implement a recursive-descent parser with C precedence.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(hlsl): parser`


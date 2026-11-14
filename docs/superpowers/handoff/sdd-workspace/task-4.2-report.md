# Task 4.2 report: Parser and AST

Status: DONE_WITH_CONCERNS (concerns are notes for 4.3/4.6, not defects)
Commit: 3ede38d feat(hlsl): parser

## What was implemented

- `crates/md-hlsl/src/ast.rs` (public `md_hlsl::ast`): `Module`, `Decl::{Global(VarDecl), Function(FnDecl), Struct(StructDecl), ShaderBody(ShaderBody)}`, `Stmt::{VarDecl, Expr, If, For, While, DoWhile, Return, Block, Break, Continue, Discard}`, `Expr::{Lit, Ident, Index, Member, Call, Ctor, Unary, Binary, Assign, Ternary, Cast}`, plus `TypeName/TypeKind/Scalar/SamplerKind`, `Qualifiers`, `Declarator`, `Init` (expr or brace list), `Param`, `Field`, `ForInit`, `UnOp`, `BinOp`, `AssignOp = Option<BinOp>`.
  - Every Expr/Stmt/Decl node carries `pos: Pos { line, col }` (F26). `Expr::pos()` / `Stmt::pos()` are the only exhaustive matches, so adding `Expr::Splat` (4.3) means one new variant and one new arm. Binary/assign/ternary/index/member/postfix nodes take the operator's position; the rest take their first token's.
  - Array sizes are `Vec<Option<Expr>>` (unsized `a[]` is `None`); 4.3 folds them.
  - Empty statement `;` → empty `Stmt::Block`. Comma expressions in `for` clauses: `ForInit::Exprs(Vec<Expr>)` and `step: Vec<Expr>`. There is no general comma operator (as the dispatch ruled).
- `crates/md-hlsl/src/parser.rs` (+ `parser/expr.rs`, `parser/stmt.rs` to keep files under ~600 lines): `pub fn parse(tokens: &[Token]) -> Result<Module, TranslateError>`, exported from `lib.rs`.
  - Types: `void bool int uint dword half float double`, vectors `TN` for N in 2..=4, matrices `TRxC` for R,C in 1..=4, `sampler sampler2D sampler3D samplerCUBE`, `texture`/`texture2D`/`texture3D`/`textureCUBE` (include.fx has `texture PrevFrameImage;`), and declared struct names. Qualifiers `const static uniform in out inout row_major column_major` are recorded; `inline extern shared volatile precise` are accepted and dropped.
  - Declarators: `name[N][]`, `: SEMANTIC`, `: register(...)` / `: packoffset(...)` dropped, `<...>` annotations skipped (F21), `= sampler_state { ... }` skipped with `init: None` (F21), `= expr`, `= { list }`, comma lists.
  - Functions with params (qualifiers, arrays, semantics, defaults, `(void)`), return semantics. Prototypes without a body are an error. Structs `struct S { ... };`. Stray top-level `;` (common `};` after functions) is skipped.
  - Statements: blocks, `if/else`, `for` (decl or expression-list init, optional clauses), `while`, `do/while`, `return`, `break`, `continue`, `discard`, local declarations, expression statements; `[unroll]`/`[loop]`/`[unroll(4)]` attributes skipped.
  - Expressions by precedence climbing with C precedence: assignment (all compound ops incl. `<<= >>=`, right-assoc) < `?:` (right-assoc) < `||` < `&&` < `|` < `^` < `&` < `== !=` < relational < shifts < additive < multiplicative < prefix (`- + ! ~ ++ --`, casts `(T)x` incl. struct casts) < postfix (`[]`, `.member/swizzle`, `++ --`). `float3(...)` is `Ctor`, `name(...)` is `Call`; `(float2(a,b))` is a parenthesized ctor, not a cast. `true/false` → `Lit::Bool`. Reserved words (control keywords, qualifiers, type names) are rejected as names.
  - Missing `;` before `}` is accepted only for a statement directly in a `shader_body` block (so only the last one); `void f() { ret = a }`, `shader_body { if (a) ret = a }` and `shader_body { ret = a ret = b; }` are errors.

### shader_body representation (decided here, binds 4.6)

The C++ (`plugin.cpp` LoadShaderFromMemory ~3607) replaces the first `shader_body` text with `void PS( params )`, inserts the first line after the next `{`, and overwrites the LAST `}` of the whole text with ` <last line>\n}\n`, which also drops anything after that `}`. Mirrored as: top-level ident `shader_body` must be followed by `{`, parses as `Decl::ShaderBody(ShaderBody { pos, body: Block })`, is the last decl, appears at most once; tokens after its closing brace are discarded if they contain no `}` and are an error if they do (the C++ would have wrapped the wrong brace). Everything before it (helper functions, `sampler sampler_foo;`, `float4 texsize_foo;`, `static const` arrays, structs) parses as ordinary decls. **4.6 should parse preamble + defines + raw body and do the `void PS(...)` wrapping at the AST level** (documented on `Decl::ShaderBody`): the missing-`;` tolerance only exists inside a ShaderBody, so a textual rewrite would lose it.

### Untrusted-input bounds

- `MAX_DEPTH = 128` simultaneous recursion (statements, parentheses/args/index/initializer lists via `assign`, prefix operators and casts, right-assoc assignment and ternary chains, nested init lists).
- `MAX_EXPR_HEIGHT = 512` tree height, enforced as nodes are built (each parse returns its height), which bounds iteratively built left-assoc chains (`a+b+...`) and postfix chains (`a.x.x...`, `a[0][0]...`) that later passes will walk recursively.
- Both produce a `TranslateError` containing "nesting too deep" at the current token.
- Stack: debug frames were initially ~12 KiB per nesting level (1.5 MiB at the limit — too close to the 2 MiB test-thread stack). Fixed by boxing the expression in the internal result (`(Box<Expr>, u32)`) and splitting every function on the recursive path into a small "call down" head and a `*_tail` node builder (likewise statement dispatch into per-kind functions). The full deep-nesting test now runs on an explicit **1 MiB** thread in debug (passes at 700 KB, fails at 500 KB); release needs well under 400 KB.

## TDD evidence

RED (stub `parse` returning Err): `cargo test -p md-hlsl --test parser` → `test result: FAILED. 0 passed; 13 failed`.

GREEN: `cargo test -p md-hlsl --test parser` → `test result: ok. 13 passed; 0 failed` — brief tests `parses_include_fx_without_error`, `parses_swizzle_chain`, `parses_matrix_type_float4x3`, `parses_for_with_decl`, `parses_ternary_nested`, `tolerates_missing_semicolon_before_closing_brace`, plus `precedence_follows_c`, `parses_casts_and_constructors`, `parses_globals_functions_and_structs`, `parses_statements`, `shader_body_is_last_and_unique`, `errors_report_position`, `deep_nesting_is_an_error_not_a_stack_overflow` (15 hostile shapes at 20 000 levels each, plus near-limit inputs that must parse, all on a 1 MiB stack).

Corpus: new `every_corpus_shader_parses` in `crates/md-hlsl/tests/corpus.rs` — include.fx + each of the 32 corpus presets' filled warp and comp text (via `md_preset::parse_milk`, which fills MD1 presets from gen_warp/gen_comp) + shipped `warp_ps.fx`/`comp_ps.fx` (66 inputs) preprocess and parse, each yielding exactly one trailing `Decl::ShaderBody`. `cargo test -p md-hlsl --test corpus` → 3 passed. (No 4.6 defines are prepended; they only `#define` identifiers and don't change parseability.)

Note: the brief's `cargo test -p md-hlsl parser` name filter does not select integration tests (F57); use `--test parser`.

Full gate: `cargo build --workspace` ok; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `cargo test --workspace` all ok.

## Files

- Created: `crates/md-hlsl/src/ast.rs` (403 lines), `crates/md-hlsl/src/parser.rs` (551), `crates/md-hlsl/src/parser/expr.rs` (343), `crates/md-hlsl/src/parser/stmt.rs` (191), `crates/md-hlsl/tests/parser.rs`.
- Modified: `crates/md-hlsl/src/lib.rs` (`pub mod ast`, `pub use parser::parse`), `crates/md-hlsl/tests/corpus.rs` (parse test).
- No new dependencies.

## Self-review findings / concerns

1. Two extra files (`parser/expr.rs`, `parser/stmt.rs`) beyond the brief's `{ast.rs,parser.rs}`, for the 600-line rule.
2. 4.6 must wrap `Decl::ShaderBody` at AST level (see above) to keep the missing-`;` tolerance; 4.6 must also offset error lines to body-relative (ledger note from 4.1).
3. 4.3 receives `TypeKind::Texture`, `SamplerKind::Any` (untyped `sampler`, used by presets for `sampler sampler_foo;`), `Scalar::Half/Double`, `Init::List`, unsized arrays, and `Ctor` of any builtin type (incl. scalar `float(x)`); it must handle or reject them.
4. Not supported (error): function prototypes, `vector<T,N>`/`matrix<T,R,C>` template syntax, `float1`, `typedef`, `switch`, general comma operator, local struct declarations, struct-with-trailing-variable `struct S {...} s;`. None appear in the corpus or include.fx.
5. Only 32 corpus presets exist locally (no large external corpus found); constructs common in real presets (helper functions, static const arrays, casts, macro-heavy bodies) were also checked ad hoc.

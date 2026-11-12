# Task 4.3 report: Type checker with HLSL implicit conversions

Status: DONE_WITH_CONCERNS (concerns are design notes for 4.4/4.5/4.6, not defects)
Commit: be28b54 feat(hlsl): type checker with legacy implicit conversions

## What was implemented

Public API (`md_hlsl`): `check(&Module) -> Result<TypedModule, TranslateError>`, `types::{Ty, Scalar, Shape}` (Ty exactly as the brief: `Void, Bool, Int, Float, Vec(Scalar,u8), Mat(Scalar,u8 rows,u8 cols), Sampler2D, Sampler3D, SamplerCube, Array(Box<Ty>,u32), Struct(String)`), `typed::*` (the typed tree), `Intrinsic`, `SigClass`, `Warning`.

### TypedModule shape (decision)
`TypedModule { decls: Vec<TDecl>, warnings: Vec<Warning> }` is a typed mirror of the AST (`TDecl::{Struct,Global,Function}`, `TStmt`, `TExpr { pos, ty: Ty, kind: TExprKind }`), not the `ast::Module` with a side table: every node carries its `Ty`, identifiers are resolved (`Var { name, origin: Local | Param(In/Out/InOut) | Global(Uniform/Private/Const) }`), calls are `Callee::Intrinsic(Intrinsic)` or `Callee::User(unique_name)`, members are split into `Swizzle{comps}` / `MatElems{(r,c)}` / `Field`. Implicit conversions are explicit: element changes are `Cast`, scalar replication is `Splat` (after the element `Cast`), implicit vector truncation is a `Swizzle` + warning, matrix truncation a `Cast` between matrix types (+ warning when implicit). Per F26, `ast::Expr::Splat { pos, ty, expr }` was also added to ast.rs (and `Expr::pos`); the parser never emits it, AST passes may, and the checker accepts it. The emitter consumes the typed tree, whose `TExprKind::Splat` is what it must handle. This reads "AST annotated with Ty" as "typed mirror of the AST"; if the controller wanted 4.4 to walk `ast::Expr` directly, that is a different design (concern 1).

### Pinned conversions (brief), all tested
- scalar → vector splat in arithmetic, assignment and compound assignment (`float3 ret = 0;` → `Splat(Cast<float>(0))`, `ret *= 0.5` keeps the compound with `Splat(0.5)`).
- vector truncation in assignments/initializers/returns/call arguments/binary operands/`?:` branches → `Swizzle` + `Warning { pos, "implicit truncation of vector type (float4 to float3)" }`; widening is an error.
- `int`/`bool` → `float` promotion; `float` → `int` is a `Cast` (truncating); mixed int/float arithmetic is float; `bool` arithmetic promotes to int; `int / int`, `int % int` stay int (truncating); `%` on float = fmod semantics (documented on `TExprKind::Binary`).
- `mul`: `(vecR, matRxC) → vecC`, `(matRxC, vecC) → vecR`, `(matRxC, matCxK) → matRxK`, `(scalar, X)`/`(X, scalar) → X` (scalar NOT splat), `(vecN, vecN) → float` (dot). Inner-dimension mismatch truncates the longer side with a warning (fxc legacy; e.g. `mul(float3, float4x3)`). Types are HLSL's; F24 transposed emission is 4.4's (documented in types.rs and intrinsics.rs).
- swizzle write masks (`v.xz = 1` → lhs `float2`, rhs `Splat`); `half`/`double` → float.

### Ledger/dispatch items
- Non-lvalue targets rejected with position: `a = b ? c : d = e` (Ternary lhs), `(a+1) = 2`, `3++`, repeated-component masks (`.xx`), writes to uniforms (non-static globals), `static const`, `const` locals; out/inout arguments must be lvalues. Valid: locals, params (HLSL in-params are writable copies, 4.4 must copy), statics, swizzles/`_m00` masks without repeats, fields, indexing.
- `TypeKind::Texture`: `texture` globals are dropped from the typed module (only referenced by discarded `sampler_state`); using one in an expression, or as a local/param/field type, is a positioned error.
- Untyped `sampler` globals: dimension from use — `tex3D`/`tex3Dlod` → `Sampler3D`, `texCUBE` → `SamplerCube`, otherwise `Sampler2D`, except unused names containing `noisevol` → `Sampler3D` (pre-pass `typeck/samplers.rs`). Untyped `sampler` params are `sampler2D`. The typed `TGlobal.ty` holds the resolved type (4.5 can read `dims` from it).
- `{…}` initializer lists: flattened HLSL-style and rebuilt per target (vectors/matrices from mixed scalars/vectors, arrays, structs, nested lists); too many/too few/straddling elements are errors. Unsized arrays `a[] = {…}` sized from the list; unsized without a list is an error. Array sizes fold constant ints (literals, arithmetic, casts, `const int` with constant init).
- Constructors of any built-in vector/matrix type, with argument flattening (`float4(v2, 0, 1)`), single scalar → `Splat` (`float3(2)`, also covers 4.7's single-arg ctor idiom), single same-shape arg → element cast, `float(x)` → `Cast`. Matrix ctors accept scalars/vectors whose components sum to R*C (may straddle rows; 4.4 must handle that for WGSL). Matrix args inside ctors and `float4x1`/`float1xC` types are rejected (WGSL has no such types; parser already rejects `float1`).
- Comparisons on vectors yield bool vectors. `if`/`while`/`for`/`do` conditions must be scalar (fxc rejects vector conditions); `?:` with a vector condition is a componentwise select (cond coerced to `boolN`, branches to N components; scalars splat). `&&`/`||` componentwise, both sides evaluated (no short-circuit in fxc).
- Bitwise ops/shifts: int only (float operand → error).
- Compound assignment whose HLSL arithmetic is float but target is int/bool (`i *= 1.5`) is lowered to `i = (int)((float)i * 1.5)` (duplicates the lhs); otherwise the compound op is kept with the rhs converted.
- `for (int i …)` follows fxc legacy scoping: the declaration is hoisted before the loop in the enclosing scope; a later same-scope `for (int i = …)` with the same type becomes the init assignment, with a warning (X3078-like).
- User functions: declaration before use; overloads ranked by conversion cost (exact 0, element 1, splat 2, truncation 3; out/inout exact only), ambiguity is an error; later overloads get unique names `name__ovl<k>` (`TFunction.hlsl_name` keeps the source name); default parameters filled into call args; recursion is an error.
- `Decl::ShaderBody` is an error ("must be wrapped into a function before type checking"): 4.6 wraps at AST level first (as the 4.2 report bound it).

### Intrinsics covered (`intrinsics.rs` module doc has the full table)
All 61 listed names. Componentwise float generics (`T → T`, T = float or floatN, mixed lengths truncate to the shortest with a warning, scalars splat); `abs/min/max/clamp` keep int when all args are int; `sign` returns int of T's shape (HLSL); `length/distance → float` (scalar allowed); `dot → float`, `normalize/reflect/refract` vectors only; `cross` float3; `all/any → bool`; `frexp/modf (T, out T) → T`, `sincos (T, out T, out T) → void` (out args exact type, lvalues); `ldexp (T,T)`; `transpose`; textures return float4 with the sampler argument a variable of the right kind: `tex2D(s2D, float2)`, `tex2Dbias/lod/proj(s2D, float4)`, `tex2Dgrad(s2D, float2, float2, float2)`, `tex3D(s3D, float3)`, `tex3Dlod(s3D, float4)`, `texCUBE(sCUBE, float3)` (type-checked here; 4.4 returns the TranslateError per F26). Not covered (positioned errors): matrix arguments to componentwise intrinsics (`abs(float3x3)` etc.), scalar args to dot/normalize/reflect/refract, the 4-arg `tex2D(s, uv, ddx, ddy)` form. GetBlur*/GetMain/GetPixel/lum are include.fx macros (F21) and arrive expanded.

### Stack bound
The parser allows expression trees 512 high. A plain `match` dispatch cost ~1.9 KiB of debug stack per level (overflowed a 1 MiB thread); `expr` now dispatches through a function pointer and recursive helpers are split into head/`*_tail` (≈1.5 KiB/level for `+` chains, ≈1 KiB for member/unary). `deep_expressions_do_not_overflow` checks a 500-long `+` chain, a 500-long `.x` chain, 60 nested `if`s, 60 nested blocks and 60 nested `-( )` on a 1 MiB thread in debug.

## TDD evidence

RED: with `check` stubbed to `Err("unimplemented")`: `cargo test -p md-hlsl --test typeck` → `test result: FAILED. 0 passed; 14 failed` (brief tests `scalar_splats_into_vector_assignment`, `vector_truncation_warns_not_errors`, `mul_vec_mat_result_type`, `tex2d_returns_float4`, `unknown_identifier_errors_with_position`, `swizzle_write_mask_type` plus `non_lvalue_assignment_targets_are_errors`, `literal_and_mixed_arithmetic_types`, `constructors_casts_and_initializer_lists`, `intrinsic_overloads`, `user_functions_overloads_defaults_and_out_params`, `for_loop_variables_leak_like_fxc`, `untyped_sampler_resolves_by_usage`, `shader_body_must_be_wrapped_first`).

GREEN: `cargo test -p md-hlsl --test typeck` → `test result: ok. 18 passed; 0 failed` (the 14 above + `preset_idioms_check`, `deep_expressions_do_not_overflow` — first run overflowed a 1 MiB stack, fixed as above — `ast_splat_node_is_accepted`, `statement_and_operator_errors`).

Corpus: new `every_corpus_shader_type_checks` in `tests/corpus.rs`: include.fx + the warp/comp defines from plugin.cpp:3493-3501 + each of the 32 corpus presets' warp/comp text and the shipped `warp_ps.fx`/`comp_ps.fx` (66 inputs), with the `shader_body` wrapped at AST level into `void PS(<warp|comp params>) { float3 ret = 0; <body> _return_value = float4(ret.xyz, _vDiffuse.w); }` → all type-check. `cargo test -p md-hlsl --test corpus` → `4 passed`.

Note (F57): the brief's `cargo test -p md-hlsl typeck` name filter matches no test names (integration-test binary names are not part of the test path); use `--test typeck`.

Full gate: `cargo build --workspace` ok; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `cargo doc -p md-hlsl --no-deps` no warnings; `cargo test --workspace` all ok.

## Files
- Created: `crates/md-hlsl/src/types.rs` (158), `src/typed.rs` (309), `src/intrinsics.rs` (262), `src/typeck.rs` (471), `src/typeck/{convert.rs (157), expr.rs (~510), call.rs (321), stmt.rs (322), samplers.rs (120)}`, `tests/typeck.rs`.
- Modified: `src/ast.rs` (`Expr::Splat` + `Expr::pos` arm), `src/lib.rs` (modules + re-exports), `tests/corpus.rs` (type-check test), `tests/parser.rs` (`sx` arm for `Splat`).
- No new dependencies.

## Self-review findings / concerns
1. Extra files beyond the brief's `{types.rs,typeck.rs,intrinsics.rs}`: `typed.rs` and `typeck/*` for the 600-line rule. The typed tree is a mirror, not the ast with a side table (see above); `ast::Expr::Splat` exists per F26 but the emitter will see `TExprKind::Splat`.
2. For 4.4: things the typed tree leaves to the emitter: matrix ctor args straddling rows; componentwise `*` between same-shape matrices (WGSL `*` is matmul) and matrix `Splat`; `Cast` between matrix sizes (top-left block); vector `?:` → `select`; swizzle writes (WGSL only allows single-component writes); swizzles of scalars (`x.xxx`); writes to in-params (WGSL params immutable); `sign` typed int; out/inout args to user functions; `__ovl<k>` names; mixed-length vectors already truncated.
3. For 4.5: untyped sampler dims are in `TGlobal.ty`; preamble samplers that are never referenced still appear as `TGlobal`s (4.5 filters by reference). An untyped sampler passed only to a user function's `sampler3D` param resolves to 2D and fails (not handled; no corpus case).
4. Redeclaring a preamble global in a body (e.g. a second `float4 texsize_noise_lq;` or `sampler sampler_noise_lq;`) is a redefinition error, as in fxc. If real presets do this (the local corpus does not), 4.7 would need a fixup.
5. `uint` is mapped to `int` (fxc emulates both); local `static` is treated as a plain local.
6. fxc-exact behavior I could not verify without fxc and chose conservatively: vector `if` condition = error; `float(v3)`/`float3(v4)` constructors = error; vector↔matrix same-count conversions = error.

## Fix round 1 (review findings + controller rulings)

Commit: see `git log` — `fix(hlsl): scalar dot, benign global redeclaration, writable globals, single-eval compound assign`.

Changes:
1. **Scalar `dot`** (authoring guide documents `dot(float a, float b)`): `SigClass::Dot` now resolves `T` as scalar or vector, result `float`. intrinsics.rs table notes 4.4 lowers scalar dot to `a * b`. Covered in `intrinsic_overloads` (`dot(2.0, 3)` → args `float, float`, result `float`).
2. **Benign global redeclaration** (revised ruling): `Checker::benign_redecl` — a non-static, non-const global redeclared with the same resolved type, no initializer and a compatible semantic (equal ignoring case, or either absent; registers are already dropped by the parser) is a no-op merged into the first declaration; a different type, an initializer, `static`, conflicting semantics, `static` duplicates, or a function of the same name stay `redefinition` errors. Untyped `sampler sampler_noise_lq;` resolves to `sampler2D`, so it merges with include.fx's `sampler2D sampler_noise_lq;`. `texture` redeclarations follow the same rule. Test `benign_global_redeclaration_merges` (include.fx + `float4 texsize_noise_lq;` + `sampler sampler_noise_lq;` → one TGlobal; six error cases).
3. **Writable uniform globals** (ruling: D3DX9_31 legacy compiler): assignments, `++/--` and out/inout arguments targeting non-static globals are allowed unless the global is a sampler or `const`; the checker records them and sets the new `TGlobal.written: bool` (documented in typed.rs: 4.4/4.5 emit a function-private copy initialised from the uniform, or from `init`, at PS start, shared with user functions, e.g. a module `var<private>`). `Storage::Uniform` doc updated; typeck.rs decision list updated. Test `writes_to_uniform_globals_are_flagged` (`_qa.x = 1`, `_qc += 2`, out-arg `h(_qb)` flagged; static and read-only globals not). The old "write to uniform is an error" case in `non_lvalue_assignment_targets_are_errors` was replaced with `const float4 g` (const) and sampler assignment (sampler) errors.
4. **Single-evaluation compound assignment**: compound assignments are never lowered to `=` any more. `TExprKind::Assign` gained `op_ty: Ty` — the type HLSL does the arithmetic in (Binary rules on lhs/rhs types); the rhs is already `op_ty`; the documented emission is `*p = L((O)*p op rhs)` with the lhs evaluated once (address/temporary), `(O)` lhs→op_ty and `L` op_ty→lhs being the implicit conversions (the back-conversion is validated and warned at check time). For `=`, `op_ty` is the lhs type. Test `compound_assignment_evaluates_lhs_once`: `a[i++] *= 1.5` (int array) has exactly one `PostInc`, `op_ty = float`, rhs `float`, node `int`; `float x; x += float3(...)` has `op_ty = float3`.
5. Minors: matrix truncation warning now says "matrix type"; `intrinsic_overloads` asserts types for `any` (arg `bool3`, result `bool`), `max` (float3 args; int `max(1,2)` → `int`), `saturate` (float4 result truncated; float2), `sincos` (`void`), `normalize` (float3).

New test file `crates/md-hlsl/tests/typeck_globals.rs` (fix-round tests; keeps `tests/typeck.rs` under 600 lines).

RED: with the src changes stashed, `cargo test -p md-hlsl --test typeck --test typeck_globals` fails to compile (`Assign` has no field `op_ty` ×3, no field `written` on `TGlobal` ×2) — the new tests pin the new interface. GREEN: `cargo test -p md-hlsl` → typeck 18 passed, typeck_globals 3 passed, corpus 4 passed (incl. `every_corpus_shader_type_checks`), parser 13, preprocess 15, lexer 4. `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --check` clean, `cargo test --workspace` no failures.

Notes for 4.4: `TGlobal.written` copies; `Assign.op_ty` emission; scalar `dot` → `a * b`.

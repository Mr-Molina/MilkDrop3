# Task 4.4 report: WGSL emitter

Status: DONE_WITH_CONCERNS (concerns are interface notes for 4.6 / md-render, not defects)
Commit: 710f53d feat(hlsl): WGSL emitter

## What was implemented

### Public API
- `md_hlsl::emit(m: &TypedModule, kind: ShaderKind, bindings: &Bindings) -> Result<String, TranslateError>`.
  **Signature change (F26):** returns `Result`, not `String` — texCUBE, integer/bool matrices, matrix comparisons, a missing `PS`, a `PS` parameter with an unknown semantic, a uniform redeclared with a type other than its `MdUniforms`/`TexSizes` field, sampler locals/struct fields are positioned `TranslateError`s. Documented on `emit`.
- `md_hlsl::ShaderKind { Warp, Comp }` (A.3 name; defined in emit.rs, re-exported). `kind` only labels the module (`// md-hlsl: Warp shader`): the entry point is mapped by semantic, so the comp `float2 _uv` naturally becomes `in.uv.xy`.
- `md_hlsl::layout` (F29 single source of truth): `MD_UNIFORMS_SIZE = 1920`, `md_uniform_offset(name)`, `md_uniform_kind`, `MD_UNIFORM_FIELDS` (48 fields in contract order), `UniformKind {Vec4, Mat4x3}` with `size()`/`wgsl()`, `md_uniforms_wgsl()` (the struct text), `D3D9_HALF_TEXEL = -0.5` (F29 puts it here), `TEXSIZES_PLACEHOLDER`, `texsizes_size(n)`.

### Output contract
`diagnostic(off, derivative_uniformity);` first line; `struct VsOut {...}` exactly as the M4 context; `struct MdUniforms` (from layout) + `@group(0) @binding(0) var<uniform> U: MdUniforms;`; `struct TexSizes` with one `vec4<f32>` per `Bindings::texsize_names` entry (full names as field names, e.g. `texsize_main`) + `@group(0) @binding(1) var<uniform> T: TexSizes;`; per bound sampler `@group(1) @binding(2*i) var tex_i: texture_2d<f32>|texture_3d<f32>` and `@binding(2*i+1) var samp_i: sampler`; `@fragment fn fs_main(in: VsOut) -> @location(0) vec4<f32>` which initialises private globals then calls `h_PS(in.diffuse, in.uv, in.rad_ang, &md_out)` and returns `md_out`.

### Naming (F28)
Every HLSL identifier (locals, params, statics, consts, functions by unique `name__ovl<k>`, structs, fields) → `h_` prefix. Emitter-invented names start with `md_` (`md_t<N>` temporaries, `md_in_h_x` written-in-param copies, `md_tex_h_s`/`md_samp_h_s` sampler-param pairs, `md_out`). Contract names unprefixed. Uniform references: read-only `_c0`..`rot_rand4` → `U.<name>`; read-only `texsize_*` in the bindings → `T.<name>`.

### Globals
- Written (`TGlobal.written`) MdUniforms/TexSizes globals → `var<private> h_<name>` + `h_<name> = U.<name>;` at the start of fs_main (shared by user functions).
- Other uniforms (body-declared `float4 foo;`, never set by MilkDrop — C++ doesn't call SetDefaults) → `var<private>` from the initializer, else zero.
- `static` → `var<private>`; a const-expression initializer goes in the declaration (`var<private> h_k: f32 = 2.0;`), otherwise it is assigned at the start of fs_main in declaration order (WGSL private initializers must be const-expressions).
- `static const` → `const h_K: T = …;` when the initializer is a const-expression (no calls/matrices/structs/div/rem/shifts), else treated as a static.
- Only globals/functions reachable from `PS` are emitted (`bindings.rs` gained `pub(crate) fn reach`, sharing 4.5's walker — small refactor, behaviour unchanged).

### Emission rules (one test each, F27)
F24 transposed storage: `floatRxC` → `matRxC<f32>`, `mul(v,M)` → `(M * v)`, `mul(M,v)` → `(v * M)`, `mul(A,B)` → `(B * A)`, ctors and `M[i]`/`M._mRC` (`M[r][c]`) unchanged. Brief rules: types, tex2D/bias/lod/3D, saturate (splat bounds), lerp, frac, fmod, atan2, rsqrt, ddx/ddy, log10, sincos (two statements), round/step/smoothstep, sign (`i32(sign(x))`), any/all, discard, int literals in float context (`2.0`), swizzles (scalar swizzle = splat), out/inout → `ptr<function, T>`, statics, consts, `h_` naming. F26: tex2Dgrad, tex2Dproj (`c.xy / c.w`), tex3Dlod, modf/frexp (struct results lowered: `let t = modf(x); ip = t.whole;` → `t.fract`; frexp exponent converted to float), ldexp (`x * exp2(e)`, HLSL's exponent is float), texCUBE → error. Ledger 4.3 notes: compound assign single-evaluation (`a[i++] *= 1.5` → index snapshot, `h_a[md_t0] = i32((f32(h_a[md_t0]) * 1.5));`), written uniforms, scalar dot `a*b`, straddling matrix ctor args (flattened to scalars), componentwise matrix `* / %` (per column), matrix splat, matrix→smaller cast (top-left block), vector `?:` → `select`, swizzle writes (component assignments via a temp), writes to `in` params (copy), `sign` typed int, overload names. WGSL specifics: `for`/`while`/`do-while` → `loop` (+`continuing`, `break if`), loop conditions with side effects rerun per iteration, `&&`/`||` scalar vs `&`/`|` on bool vectors, sampler parameters → texture+sampler params, literal-zero divisors routed through a `var` (naga const-evaluates `5i / 0i` and `1.0/0.0` and rejects them; probed), matrix negation `(-1.0 * M)` (WGSL has none), non-void fall-through gets `return T();`.

Decisions to flag:
1. **fmod → `(a % b)`, not the brief's `(a - b * trunc(a / b))`.** The WGSL spec *defines* float `%` as exactly `e1 - e2 * trunc(e1 / e2)`; `%` evaluates each operand once (the brief's form evaluates `a` and `b` twice — wrong with side effects, costly with texture samples).
2. **Side effects are hoisted** (WGSL assignments are statements): operands before the last side-effecting operand are snapshotted into `let md_t<N>`, so HLSL left-to-right order holds. `&&`/`||` operand side effects are therefore always evaluated — this matches fxc, which (per typed.rs, 4.3) does not short-circuit; pure operands are unaffected either way. Scalar `?:` is `select` (both branches evaluated, as fxc ps_2 does) unless a branch has side effects or the type isn't scalar/vector, in which case it becomes an `if` assigning a temporary.
3. **out/inout args to user functions are copy-in/copy-out temporaries** (`var md_t0: T = cur; h_f(&md_t0); place = md_t0;`) — HLSL semantics, and it works for swizzles, private globals, and pointer params (WGSL ptr<function> can't address those).
4. `round` keeps WGSL half-to-even (HLSL docs: half away from zero). Not lowered; flagged.
5. `TexSizes` is never empty: with no `texsize_*` reference it has one `md_unused: vec4<f32>` field (WGSL forbids empty structs). md-render must bind ≥16 bytes at binding 1 (`layout::texsizes_size(n)`).

### naga dependency
`naga = { workspace = true, features = ["wgsl-in"] }` added as a **dev-dependency** of md-hlsl (emit doesn't need it). Task 4.6's `translate` validates at runtime, so 4.6 should move it to `[dependencies]`. cargo-deny: ok; check-no-native-build: OK. Cargo.lock gained naga's tree (naga, naga-types, codespan-reporting, half, …).

## Layout verification (naga)
`md_uniforms_layout_matches_naga` parses `md_uniforms_wgsl()` + the `U` binding with naga, validates it, and checks: struct span = 1920 = `MD_UNIFORMS_SIZE`; every member's naga offset == `md_uniform_offset`; every member size == `UniformKind::size()` (mat4x3<f32> = 64 bytes, confirmed); plus the brief's pinned offsets (`_c0` 0, `_c13` 208, `_qa` 224, `_qh` 336, `rand_frame` 352, `rand_preset` 368, `rot_s1` 384, `rot_s2` 448, `rot_rand4` 1856).

## TDD evidence
Honest note: I drafted the implementation before the test files (after a naga probe of WGSL edge cases), then wrote the tests. RED was demonstrated by stubbing `emit` to return `Err("unimplemented")` (env-gated, then restored):

`MD_RED=1 cargo test -q --no-fail-fast -p md-hlsl --test emit --test emit_lowering --test emit_corpus`
→ emit: `FAILED. 0 passed; 35 failed`; emit_lowering: `FAILED. 0 passed; 24 failed`; emit_corpus: `FAILED. 1 passed; 3 failed` (the passing one is the layout test, which doesn't call emit).

During GREEN, tests found real issues that were fixed: matrix negation rejected by naga (`Operation Negate can't work with` mat2x2) → `(-1.0 * M)`; deep-expression stack overflow on a 1 MiB debug thread (500-long `+`/`.x` chains needed ~2 MiB) → `expr` became a thin dispatcher with `#[inline(never)]` per-kind methods (now passes at 1 MiB); `else if` chains with hoisting conditions re-emitted the condition (temp numbering gaps) → reuse the hoisted statements. Three expectation fixes in tests (`U.rot_s1[1i]`, snapshot before a user call, `f32(select(2i, 1i, …))` for an int ternary).

GREEN: `cargo test -p md-hlsl --test emit --test emit_lowering --test emit_corpus` → `35 passed`, `24 passed`, `4 passed`.

Full gate: `cargo build --workspace` ok; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `cargo doc -p md-hlsl --no-deps` no warnings; `cargo test --workspace` all ok; `cargo deny check` ok.

## Corpus
`corpus_emission_report` (emit_corpus.rs): the 32 corpus presets' warp + comp shaders plus shipped `warp_ps.fx`/`comp_ps.fx` (66 inputs), wrapped as 4.6 will (shared `tests/common::wrap_preset`), check → resolve_bindings → emit → naga validate: **66/66 validate**. No failures to list. It asserts only that the shipped defaults validate (4.9 owns the 100% triage). `MD_EXTRA_PRESETS` is honoured via md_testkit (no extra pack available here).

`assorted_constructs_validate_with_naga` additionally validates 25 constructs the corpus barely uses (nested assignment, pre/post inc in expressions, structs with arrays, const arrays with dynamic index, static const matrix, matrix ternary, bit ops/shifts, else-if chains with hoisted conditions, return inside loops, inout swizzle args, sincos into components, written texsize, unreachable function referencing an unbound sampler, sampler2D+sampler3D helper params, user uniforms, vector ++, int div/rem, matrix `/=`, `+=`, negate, transpose, do-while with break/continue, frexp/modf/ldexp, bias/lod/proj, misc builtins, int min/max/clamp/abs).

## Files
- Created: `crates/md-hlsl/src/emit.rs` (398, module/globals/types/literals), `emit_stmt.rs` (537, Fx state, functions, fs_main, statements, effect analysis), `emit_expr.rs` (~440, expressions/conversions/operators), `emit_call.rs` (246, user calls + intrinsics + textures), `emit_place.rs` (211, lvalues, assignment, ++/--), `layout.rs` (166); tests `tests/emit.rs` (35), `tests/emit_lowering.rs` (24), `tests/emit_corpus.rs` (4), `tests/common/mod.rs` (shared wrapper + naga helper), `tests/fixtures/default_warp.hlsl` (F25 hand-wrapped default warp; include.fx is prepended from assets/ rather than duplicated).
- Modified: `src/lib.rs` (modules, `emit`, `ShaderKind`, `pub mod layout`), `src/bindings.rs` (`pub(crate) reach()` + `walk()` extracted, no behaviour change), `tests/corpus.rs` (type-check test uses `common::wrap_preset` instead of its inline copy), `Cargo.toml` (naga dev-dep), `Cargo.lock`.

## Self-review findings
- Read the full diff. No leftover probes (`tests/zz_*` removed).
- `is_path` (used to decide whether a string can be re-evaluated) is a textual check on emitted strings; it only accepts identifier paths / `(*h_x)`, never anything with calls or operators, so it errs on the side of an extra `let`.
- Temp numbering can skip numbers (e.g. a nested-if hoist evaluated in a sub-buffer) — cosmetic.
- `has_effects` is called per n-ary node (O(n²) on deep chains; 500² is trivial).

## Concerns / notes for later tasks
1. (4.6) `emit` returns `Result`; `translate` should also run naga and map its error. Move naga to `[dependencies]`.
2. (md-render 5.x) `TexSizes` is at least 16 bytes (placeholder field) — use `layout::texsizes_size(names.len())`. Matrices: WGSL column i (at `offset + 16*i`, 3 floats + pad) = HLSL row i (F24).
3. (4.9) `round` half-to-even vs HLSL half-away-from-zero; `&&`/`||` evaluate both operands' side effects (fxc parity per typed.rs, though the dispatch said "short-circuit semantics preserved" — pure operands make it moot).
4. (4.6) error positions from emit refer to the assembled source; 4.6 offsets them (4.1 note).

## Fix round 1

Commit: see `git log` — `fix(hlsl): sincos by-value arg; emit reachable structs only`.

Changes:
1. **sincos aliasing (Important).** `sincos(x, s, c)` now snapshots `x` (`let md_tN = x;`) before writing `s`, unless `x` is a literal (emit_call.rs; module table updated). `sincos(a, a, b)` → `let md_t0 = h_a; h_a = sin(md_t0); h_b = cos(md_t0);`; `sincos(v.x, v.x, v.y)` likewise. New test `sincos_takes_x_by_value_when_it_aliases_an_output` (both forms); `sincos_becomes_two_statements` updated to the snapshot form.
2. **Reachable structs only.** `used_structs` (emit.rs) collects structs named by reachable globals' types and by reachable functions' return/param/local/expression types (`emit_stmt::visit_types`), closed over struct fields and array elements; others are not emitted. Test `only_reachable_structs_are_emitted` (an unused `struct Bad { int2x2 m; }` — sampler fields are already rejected by the checker — and an unused struct are absent; `Outer` containing `Inner[2]` is emitted).
3. `corpus_emission_report` now asserts `failures.is_empty()` (66/66).
4. emit.rs docs: body-declared uniforms use their initializer, unlike the C++ (no `SetDefaults`; value undefined there); float literal overflow → ±f32::MAX (fxc: inf), NaN → 0.0, int literals saturated to 0..=i32::MAX.

RED (fixes reverted in the working tree: HEAD's emit_call.rs, struct filter disabled): `cargo test -q --no-fail-fast --test emit --test emit_lowering` → emit `FAILED. 34 passed; 2 failed` (sincos_becomes_two_statements, sincos_takes_x_by_value_when_it_aliases_an_output); emit_lowering `FAILED. 24 passed; 1 failed` (only_reachable_structs_are_emitted).
GREEN: `cargo test -q -p md-hlsl` → all binaries ok (emit 36, emit_lowering 25, emit_corpus 4, …); `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `cargo doc -p md-hlsl --no-deps` no warnings; `cargo test --workspace` no failures.

# Task 4.1 report: lexer and preprocessor

## Implemented
- `crates/md-hlsl/src/{error,token,lexer,preprocess,version}.rs`, `lib.rs` exports.
- `preprocess_tokens(src) -> Result<Vec<Token>, TranslateError>` is the primary API (tokens carry ORIGINAL line/col; expansion tokens report the outermost invocation site; comment stripping and `\`-newline joins keep original positions because every char carries its source position). `preprocess(src) -> Result<String,_>` is a thin, lossy convenience (documented).
- `tokenize(src)`: comments/continuations removed, `#` is an error (no directives).
- Preprocessor: object/function-like `#define` (nested parens, multi-line bodies, args spanning lines), `#undef`, `#pragma` and bare `#` ignored, recursion guard (C semantics), arg-count and unterminated-invocation errors. `#include`, `#if*`, `#else`, `#endif`, `#error`, `#line`, `#`/`##` and variadics: TranslateError (not used by include.fx or corpus; grep'd). include.fx `sampler_state { Texture = <PrevFrameImage>; }` and `<`/`>` tokenise as ordinary ops.
- Numbers: `1`, `1.0`, `1.f`, `1.0F/h`, `.5`, `1e-3`, `2.`, `0x1F`, `1u`, `1f`, octal `010`; no suffixes appear in corpus shader text.
- `PsVersion` defined in md-hlsl (F23). `TranslateError` Display + Error.
- F20/F22: all 8 `.fx` files copied verbatim to `assets/hlsl/` (cmp-verified) + `assets/hlsl/README.md` (origin, Nullsoft BSD).
- md-preset and md-testkit added as dev-dependencies only (no new external crates; Cargo.lock only gains path edges).

## TDD
RED (stubs `unimplemented!()`): `cargo test -p md-hlsl` -> lexer tests: `test result: FAILED. 0 passed; 4 failed`.
GREEN: `cargo test -p md-hlsl` -> lexer 4 passed, preprocess 5 passed (includes the 4 brief-named tests `expands_object_and_function_macros`, `macro_from_include_fx_time_expands_to_c2_x`, `lexes_float_with_f_suffix`, `reports_line_and_col`), corpus 2 passed (every corpus preset's warp+comp text with include.fx prepended preprocesses and tokenizes; plus include.fx/warp_ps.fx/comp_ps.fx).
Full: build, `clippy --workspace --all-targets -D warnings`, `fmt --check`, `cargo test --workspace` all clean.

## Notes / concerns
- Tests are integration tests under `crates/md-hlsl/tests/` (public API), so a `lexer` name filter would not match them; use `cargo test -p md-hlsl` (F57).
- Corpus error positions are relative to include.fx + body (include.fx prepended); 4.6 will need to offset or not prepend when reporting body lines.
- A function-like macro whose argument list begins after its own expansion (rescan beyond the replacement) is not supported (not used).
- Ident `Token` has no keyword distinction; the parser (4.2) matches by text. Float literal suffix and int unsignedness are dropped.

## Fix round 1
- `expand` now bounded: `MAX_DEPTH = 128` nesting (rescans + arg pre-expansion) and `MAX_TOKENS = 2_000_000` produced tokens (counted across all levels, per text segment between directives); both return TranslateError ("nested too deeply" / "too large"). Depth cap also removes the quadratic arg re-collection (error occurs at depth 128).
- Hide-sets: internal `PTok {tok, blue}`; an identifier met while its macro is active is painted blue permanently, so arg pre-expansion followed by body rescan no longer re-expands (`#define A A+1`, `F(x) x`, `F(A)` -> `A + 1`).
- New tests: `hide_sets_survive_argument_pre_expansion`, `deep_nesting_is_an_error_not_a_crash` (20000 nested), `exponential_expansion_is_an_error` (30-level doubling).
- `cargo test -p md-hlsl`: 14 passed (preprocess suite 1.6 s debug); clippy workspace -D warnings and fmt --check clean.

## Fix round 2
- One `Limits` budget now created in `preprocess_tokens` and threaded through `flush`/`expand`/`expand_p`; every token examined at any level (incl. pass-through) counts. `MAX_TOKENS` stays 2,000,000: include.fx is ~1,200 tokens, the largest corpus .milk file is 5.7 KB (so a few thousand tokens at most); a test preprocesses include.fx x20 fine.
- Tests: `expansion_budget_is_shared_across_segments` (40 interleaved `D18` + `#define` segments, each 2^18 tokens, asserts "too large"), `budget_is_ample_for_include_fx_and_corpus_sized_input`.
- md-hlsl: 16 tests pass; clippy and fmt clean.

## Fix round 3
- `Limits::charge(n, tok)` now charges allocations before they happen: object-like body copies, `collect_args` totals, and each per-use clone of an expanded argument in function-like substitution.
- New test `argument_substitution_is_charged_before_allocation` (D17 x 300-use macro; errors "too large" quickly, no multi-GB alloc).
- `deep_nesting_is_an_error_not_a_crash` now accepts either "too deeply" or "too large" (nested-arg charging trips the budget first; both are clean errors).
- md-hlsl: 17 tests pass; clippy and fmt clean.

## Fix round 4 (new owner) — commit 93526f5
Changes:
- Finding 1 (memory ∝ identifier length): `MAX_TOKEN_LEN = 256` (pub, `token.rs`, re-exported). Lexer rejects longer identifiers ("identifier too long (max 256 characters)") before allocating text, and longer numeric literals ("numeric literal too long", checked before parsing, hex and decimal). Directive names / `#define` params / `#undef` names obey the same cap (`ident_at` now returns `Result`). Corpus + include.fx max: identifier 32 chars, literal 17. Also `TokKind::Ident(String)` -> `Ident(Arc<str>)`, so every expansion copy is a refcount bump (no text copy). `Token::ident()` still returns `&str`; no consumers outside md-hlsl. Doc comment on `MAX_TOKENS` states the guarantee: memory ≈ MAX_TOKENS × size_of::<PTok>() (40 B, ~80 MB) + lexed source; time linear in MAX_TOKENS × O(MAX_TOKEN_LEN + MAX_DEPTH) plus input. Unit test `ptok_is_small_and_fixed_size` pins size_of::<PTok>() <= 40.
- Finding 2: test `object_macro_chain_hits_the_depth_cap` (A300 -> ... -> x asserts exactly "nested too deeply"; A100 chain still expands to `x`). It passed before the code change (missing coverage, not a bug).
- Finding 3 (time): `Macro.params` is now `Params { count, index: HashMap<String, usize> }` (first duplicate wins, as `position` did); substitution uses `index.get(id)`.

Covering tests (tests/preprocess.rs): `overlong_identifiers_and_literals_are_errors` (256 ok; 257 in text, body, macro name, param, #undef, 300-digit float/int, `tokenize`), `long_identifier_doubling_is_rejected_cheaply` (the 20 KB review probe -> Err "too long"), `object_macro_chain_hits_the_depth_cap`, `parameter_lookup_does_not_scale_with_parameter_count` (20,000 params × 20,000-token body × 20 calls -> Ok 400,000 tokens; asserts result, no wall clock).

RED: `cargo test -p md-hlsl --test preprocess` hung on the params test (killed by `timeout 120`, exit 143); with it and the doubling probe skipped (ulimit -v 2000000): `overlong_identifiers_and_literals_are_errors ... FAILED`, 12 passed / 1 failed.
GREEN: `cargo test -p md-hlsl` -> unit 1, corpus 2, lexer 4, preprocess 15 passed (preprocess suite 1.4 s debug).
Release probes under `ulimit -v 2000000` (peak RSS via VmHWM):
- longid2 20000 17: ERR identifier too long, 0.5 ms, ~3 MB (was Ok at 2.5 GB)
- longid2 256 16: OK 65,536 toks, 32 ms, 5 MB; longid2 256 20: ERR too large, 196 ms, 23 MB
- params: OK 400,000 toks in 231 ms (was 3.2 s), 99 MB (mostly the 1 MB source as Ch vectors)
- chain: ERR nested too deeply; wide / nestwide / unused: ERR too large, <=153 ms, <=57 MB
Commands: `cargo fmt --check` clean; `cargo build --workspace` ok; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace` all ok.
Concerns: none blocking. `lex` still materialises the cleaned source as `Vec<Ch>` (12 B/char), so memory is also linear in input size — expected for any input-proportional cap.

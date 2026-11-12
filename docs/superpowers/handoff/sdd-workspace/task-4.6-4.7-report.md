# Tasks 4.6 + 4.7 report

Status: DONE
Commits: 020b0d3 `feat(hlsl): wrapper assembly and translate entry point`, 23d324c `feat(hlsl): corpus fixups`

## Task 4.6 — wrapper assembly and `translate`

### Implemented
- `crates/md-hlsl/src/wrap.rs` (new): the C++ constants verbatim from plugin.cpp:3493-3505 (warp/comp defines incl. comp `uv_orig _uv.xy` [sic] and `hue_shader`, warp/comp PS params, first/last lines).
  - `assemble(kind, preamble, body)`: preamble + defines + body as text (the C++ pastes the defines right after include.fx; a preamble without a trailing newline gets one). It records the preamble and define line counts.
  - `wrap(&mut Module, kind)`: wraps on the AST (ledger Task 4.2 ruling). It pops the trailing `Decl::ShaderBody` and builds `void PS(<params>) { float3 ret = 0; <body stmts> _return_value = float4(ret.xyz, _vDiffuse.w); }`. The wrapper text is tokenized with every token repositioned to the `shader_body` keyword, so errors in inserted code point at `shader_body`. A missing `shader_body` returns a TranslateError at line 0 (the C++ read past its buffer).
  - `Assembled::map_error`: body lines become body-relative (ledger Task 4.1 note). Preamble errors keep their include.fx line and get the prefix "in the preamble (include.fx): ". Errors in the define lines get "in the shader-type #defines: ". Line 0 (no position) is left unchanged.
- `crates/md-hlsl/src/translate.rs` (new; the file split keeps one responsibility per file, and lib.rs stays re-exports only):
  - `ShaderInput`, `Translated { wgsl, samplers, texsize_names, highest_blur }` and `translate()`. The pipeline is assemble → preprocess_tokens → parse → wrap → check → resolve_bindings → emit → naga validate.
  - A naga failure becomes `TranslateError { line: 0, col: 0, message: "internal translator error: the generated WGSL failed naga validation:\n<naga message>" }` (ledger Task 4.4 note).
  - `parse_wrapped()` is public (the wrapped AST, for tests/diagnostics). `validate_wgsl()` is public too.
- naga moved from dev-dependencies to `[dependencies]` (`wgsl-in`). Cargo.lock is unchanged because naga 30.0.1 was already locked.
- **Decision: `highest_blur`** is added to `Translated`. It is `Bindings.highest_blur`, the maximum `blurN` among the bound samplers' roots (preflight ruling). md-render uses it as `highest_used`.
- **Decision: `ps_version`.** In the C++ it only selects the fxc profile (plugin.cpp:3370-3385). Profiles differ only in instruction/register limits, which WGSL does not have, so `None`/`Ps2_0`/`Ps2_X`/`Ps3_0` produce identical output (tested). `Ps4_0` returns a TranslateError because D3DX9 cannot compile `ps_4_0` (plugin.h:98: "not supported by milkdrop"), so such a preset's shader always failed in MilkDrop.
- Tests switched to the real wrapper:
  - `tests/common/mod.rs::wrap_preset` now calls `parse_wrapped`. Its own defines/params copies were removed, and `naga_validate` delegates to `validate_wgsl`.
  - `tests/bindings.rs::build_typed` uses `parse_wrapped` (its duplicated wrapper was removed).
  - `tests/emit_corpus.rs`: `emitted_module_validates_with_naga` now runs `translate` on `assets/hlsl/warp_ps.fx`. The hand-wrapped `tests/fixtures/default_warp.hlsl` (the F25 interim) was deleted. `corpus_emission_report` now runs every corpus warp/comp plus the defaults through `translate`.
- `assets/hlsl/include.fx`, `warp_ps.fx` and `comp_ps.fx` were already copied in 4.1 (F22).

### TDD
Tests in `crates/md-hlsl/tests/translate.rs`:
- The 5 brief tests: `default_warp_shader_translates`, `default_comp_shader_translates`, `shader_body_missing_is_error`, `comp_defines_hue_shader`, `warp_uv_orig_is_zw_and_comp_uv_orig_is_xy`.
- Additional tests: `error_lines_are_relative_to_the_preset_body`, `preamble_errors_say_so`, `missing_semicolon_before_closing_brace_is_tolerated`, `ps_version_selects_no_feature_and_ps4_is_rejected`, `highest_blur_comes_from_bound_blur_samplers`.

Order: I wrote the implementation before the tests. RED was then shown by stubbing `translate` to `return Err(stub)`:
```
$ cargo test -p md-hlsl --test translate
test result: FAILED. 0 passed; 10 failed
```
GREEN, after restoring the implementation:
```
test result: ok. 10 passed; 0 failed
```
Corpus through `translate`: `cargo test -p md-hlsl --test emit_corpus -- --nocapture` → `corpus emission: 66/66 validate with naga`.

## Task 4.7 — legacy fixups

### Implemented
- `crates/md-hlsl/src/fixups.rs`: `pub fn apply_fixups(src: &str) -> String`. It runs on the preset body before assembly and lexing, inside `translate.rs::assembled`. I modified translate.rs rather than wrap.rs because translate() lives there; that is the F28 "Modify" target in practice. Line structure is preserved, so error lines stay valid.
- Only one real fixup was needed: `strip_line_comments`. It removes `//` comments as `plugin.cpp::StripComments` (plugin.cpp:1342) did before fxc saw the text, and leaves `//` inside `/* */` alone. Without it, the 4.1 preprocessor (C phases: `\`-newline splice before comment removal) let `// ... \` swallow the next line. No corpus preset has this; the doc comment says so and cites the fixture.
- Per-idiom results, one fixture + test each in `tests/fixtures/*.hlsl` / `tests/fixups.rs`:

| Idiom | Result |
|---|---|
| `;` after body `}` | covered by 4.2, passed before any fixup |
| single-arg vector ctor splat | covered by 4.3 (WGSL `vec2<f32>(h__rad_ang.x)`, `vec3<f32>(0.5)`) |
| `tex2D(...).xyz` into `ret`, also as a bare expression statement | covered by 4.2-4.4 |
| float4 → `ret` truncation | covered by 4.3 |
| `#define` redefinition | covered by 4.1: the new body replaces the old. fxc does the same after warning X1519 (from my knowledge of fxc, not verified with a compiler here). |
| `//` ending in `\` | **fixup added** (RED → GREEN) |
| WGSL-keyword identifiers (`loop`, `var`, `fn`, `let`, `vec3`, `override`) | covered by 4.4 `h_` prefix; F28: no fixup |
| comp `uv_orig` | covered by 4.6 defines; F28: no fixup |

- An extra unit test, `apply_fixups_strips_line_comments_like_strip_comments`, covers `//` inside block comments, CRLF, and preserved line count.

### TDD
With `apply_fixups` stubbed to identity:
```
$ cargo test -p md-hlsl --test fixups
test result: FAILED. 7 passed; 2 failed   (line_comment_ending_in_backslash_does_not_continue, apply_fixups_strips_line_comments_like_strip_comments)
```
After implementing: `test result: ok. 9 passed; 0 failed`.

## Verification (both commits)
`cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check` are clean. `cargo test --workspace` has every test result ok (49 result lines, 0 failures). The corpus through translate is 66/66.

## Self-review / concerns
- **Body macros.** The inserted first/last statements are tokenized separately, so body `#define`s of `ret`/`_return_value`/`_vDiffuse` don't apply to them, whereas the C++ pasted them as text. No preset does this; it is documented in wrap.rs.
- **Untested naga failure path.** No input currently makes emit produce invalid WGSL, so the naga-error mapping is untested.
- **Positions inside `parse_wrapped`.** Positions within the module it returns refer to the assembled source. Only the returned errors are rebased; this is documented.
- **Block comments.** StripComments deleted `/* */` with no replacement character (`a/**/b` → `ab`); the lexer turns it into a space. This is pathological, so I did not mirror it.
- **New public surface.** `Translated.highest_blur`, `parse_wrapped` and `validate_wgsl` are additions to A.3.

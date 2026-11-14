# Task 4.5 report: sampler and texsize binding resolution

Implemented `crates/md-hlsl/src/bindings.rs` (+ re-exports in lib.rs) and `crates/md-hlsl/tests/bindings.rs` (16 tests).

API: `resolve_bindings(&TypedModule) -> Result<Bindings, TranslateError>`; `Bindings { samplers: Vec<SamplerBinding>, texsize_names: Vec<String>, highest_blur: u8 }`; `SamplerBinding { name, root, slot, wrap, bilinear, dims }` (A.3); helpers `decode_sampler_name`, `parse_rand_root`, `texsize_root`, `MAX_SAMPLERS`.

Rules mirrored from plugin.cpp CacheParams: root = name minus `sampler_` (case-sensitive); if root len > 3 and root[2]=='_', the 2-char prefix is ALWAYS peeled (unknown prefixes too, e.g. sampler_no_such_x -> root `such_x`, defaults wrap+bilinear), compared case-insensitively: FW/WF, FC/CF, PW/WP, PC/CP. fc_main/pc_main/fw_main/pw_main therefore decode as root `main` with flags. blur1..3 with no recognised prefix: clamp+bilinear. rand roots keep full text (`rand01_smalltiled`); `parse_rand_root` gives (slot 0..15, prefix). dims from TGlobal.ty (Cube -> error if referenced).

Decisions: "referenced" = reachable from function `PS` (all functions if no PS) through user calls and initializers of referenced globals. Slots = declaration order of bound samplers; 17th -> error at its declaration pos. texsize_names = full global names (`texsize_main`), first-reference order, callee visited at first call site; renderer maps via `texsize_root` (C++ maps texsize_randNN by slot; renderer must do the same). highest_blur from bound roots blur1..3.

TDD: honest note, I wrote implementation and tests together, so no RED run was recorded. The tests passed after one fix to my own wrong expectation (sampler_modes warp does not use sampler_main).

Verification: `cargo test -p md-hlsl --test bindings` 16 passed; `cargo test --workspace` all ok; clippy -D warnings and fmt --check clean.
Corpus: every corpus shader (+ default warp/comp) resolves; spot-checks for user_texture, rand_tiles, rand_gallery, noise_volume, sampler_modes, blur3_bloom.

# Task 5.5 report: preset shader pipelines

Status: DONE_WITH_CONCERNS
Commit: `9bda08e feat(render): preset shader pipelines and cache` (not pushed)

## What was implemented

New files in `crates/md-render/src/`:

- `renderer.rs` (F36). Contains `RenderSettings` (the A.3 fields plus `shader_cache_dir: Option<PathBuf>` per F35, `RenderSettings::new(w, h)` with the C++ defaults, and `format()`) and `RenderError { ShaderFallback { which, error }, DefaultShader { which, error } }`. It also has `Renderer::new` (owned `Gpu` per F6; texture size clamped to `1..=max_texture_dimension_2d` per F56) and these parts:
  - the ping-pong `VS[0..2]`, each with a full mip chain
  - six placeholder blur textures
  - the `TextureStore` (noise seeded from `seed`)
  - the `PipelineCache`
  - the `ShaderCache`
  - a Pcg32 for random-texture picks
  - 1×1 white 2D and 3D fallback textures
  - accessors: `vs(i)`, `other_vs(t)`, `blur_texture(i)`, `textures[_mut]`, `pipelines_mut`, `shader_cache`, `last_errors`

  The file also holds `load_preset`, `hlsl_ps_version` (F23) and `blur_texture_sizes`.
- `preset_gpu.rs`: `PresetGpu { warp, comp }` with `highest_blur()`. `ShaderProgram` holds:
  - `pipeline` (Off), `blend_pipeline` (Alpha) and `flip_pipeline` (InvAlpha)
  - `bind_group_0` (U, T, VsParams)
  - `bind_group_1: [BindGroup; 2]` (one per ping-pong texture bound as `main`)
  - the `uniforms`, `vs_params` and `texsizes` buffers
  - `texsize_names`, `texsize_values`, `samplers`, `sources`, `highest_blur`, `is_fallback()`

  Methods: `bind(pass, main, blend)`, `write_uniforms`, `write_vs_params`, `pipeline_for`. It also builds the pipelines, using wgpu error scopes so that a validation error becomes a fallback instead of a panic.
- `preset_bind.rs`: sampler and texsize resolution (`SlotSource`, `sampler_kind`).
- `shader_cache.rs`: `ShaderCache` (memory and disk), `shader_key`, `encode`/`decode`.

Other changes:
- `md-hlsl/src/lib.rs`: added `pub const VERSION`.
- Workspace: added `sha2 = "0.10.9"`.
- md-render now depends on md-preset, sha2 and tracing. Dev-dependencies: md-testkit and tracing-subscriber. `cargo deny check` and `scripts/check-no-native-build.sh` both pass.

### Decisions

- **Shader cache key (F2).** SHA-256 over md-hlsl `VERSION`, kind, ps_version, preamble and body, each length-prefixed. The assembled HLSL is a pure function of these inputs, and md-hlsl's assembly is private.
- **In-memory cache.** It holds `Arc<CachedShader { translated, module }>`. Translate errors are cached too, so a broken preset is not re-translated on every load. The map is flushed when it reaches 512 entries.
- **Disk cache (F35).** Files are `<dir>/<hex>.mdshader` in a small line format:
  - header lines: `md-render shader cache 1`, `md-hlsl <ver>`, `sha256 <of wgsl>`, `highest_blur`
  - then `texsize` and `sampler` lines, then `wgsl` followed by the text
  - entries are written to a temp file and renamed into place

  An entry is ignored and regenerated if it is unreadable or malformed, has the wrong format or version, fails its checksum, or fails naga validation. I did not use serde: md-hlsl types are not Serialize.
- **Sampler resolution** (C++ CacheParams/ApplyShaderParams):
  - `main` → `SlotSource::Main`. When drawing into `VS[t]`, bind with `other_vs(t)`.
  - `blur1..3` → blur textures 1, 3, 5.
  - Noise → `noise2d`/`noise3d`. If the binding's dimensions do not match, the fallback of the shader's dimension is bound.
  - `randNN` → one `RandPicks` per shader, using the renderer's RNG.
  - Anything else → `TextureStore::load`. If that fails, the 1×1 white fallback is bound and a WARN is logged once per name per renderer.
  - `main` and `blurN` are matched case-sensitively, like the C++ `wcscmp`.
- **Sampler state.** Filter and mip filter follow `bilinear` (linear or point). Address mode follows `wrap` (repeat or clamp). There is no anisotropic filtering: the C++ hard-codes `bAniso = false` (milkdropfs.cpp:3837). Samplers come from `PipelineCache::sampler(SamplerKind)`.
- **texsize values.** Each is `(w, h, 1/w, 1/h)` of the named texture: noise, cached disk texture, blur 1/3/5, or the randNN slot's pick. If nothing matches the value is zero, as in the C++; this includes `texsize_main`, which the C++ never registered. That case is logged at DEBUG.
- **Fallback.** A translate or wgpu error records `ShaderFallback` in `last_errors()` (cleared on every load), logs a WARN, and translates `assets/hlsl/{warp,comp}_ps.fx` at Ps2_0. Ps4_0 errors by design and falls back the same way.
- **Pipelines.** The target is the main texture format; triangle list, culling off. The vertex stage is `warp_vs`/`comp_vs.wgsl`.
  - Group 0 is shared: U(0, FS, 1920), T(1, FS), VsParams(2, VS).
  - Group 1 is built per shader: `tex_i`/`samp_i` at (2i, 2i+1), as D2 or D3.
  - Blend modes per the C++: Off for no blend and for pass 0; Alpha for pass 1; InvAlpha for the comp "funky reversal" (milkdropfs.cpp:1105-1108).
- **Texture ages.** `TextureStore::preset_loaded()` is called after every load.
- **F44.** `load_preset` compiles both shaders in one call. The `ShaderLoader` trait belongs to md-core's switcher (Milestone 7), which can implement it for `Renderer`; this is documented in renderer.rs.

## Tests and TDD evidence

`crates/md-render/tests/preset_gpu.rs` contains six GPU tests, run on lavapipe. They load real corpus presets through md-preset:

- `default_warp_preset_builds_pipelines`: `upgraded_default` builds both programs. The warp draw gives main×0.97. PSVERSION 0 gives None, and `max_ps = None` gives None.
- `shader_error_falls_back_and_records`: a body error gives the fallback, `ShaderFallback{Warp, line 3}`, and the fallback renders `main - 0.004`. `comp_ps = 5` (Ps4_0) gives the comp fallback. `last_errors` is cleared per load.
- `sampler_main_binds_other_ping_pong_texture`: VS[0] is blue and VS[1] red. Drawing into each samples the other.
- `cache_hit_skips_translation`:
  - memory: the second load does 0 translations
  - disk: a fresh renderer does 0 translations and gets 2 disk hits
  - garbage entries are regenerated and rewritten; a later renderer then does 0 translations
- `missing_user_texture_binds_fallback_and_logs_once` (F42 / Review Focus #1): `edge/missing_texture.milk` is loaded three times. Exactly one WARN mentions the texture. The comp renders black + white×0.2 = 51.
- `user_noise_and_random_textures_resolve`: `mytex` texsize is 64×64. noisevol binds a 3D texture. `rand_tiles` loads. `blur3_bloom` gives highest_blur ≥ 1. texsize for blur2, noise_hq and noisevol_lq resolves, and `texsize_main` is 0.

Unit tests:
- shader_cache.rs: encode/decode round-trip, rejection of corrupt or version-mismatched entries, names with whitespace not stored, the key covers every input.
- preset_bind.rs: sampler kinds, blur roots are case-sensitive.
- renderer.rs: blur sizes, PsVersion conversion, `other_vs`.

RED/GREEN: I wrote the tests first, but my first run came after the implementation existed (before it, the test file could not compile against the missing API). So I got RED by mutating the code. Each mutation was reverted afterwards, and the command was `cargo test -q -p md-render --test preset_gpu <name>`:

| Mutation | Test | Result |
|---|---|---|
| `other_vs` returns `target` | sampler_main… | FAILED: assertion at preset_gpu.rs:237 |
| Memory lookup skipped | cache_hit… | FAILED: assertion at :262 |
| Disk lookup skipped | cache_hit… | FAILED |
| warn_once without dedupe | missing_user… | FAILED: 3 identical WARNs |
| Fallback slot bound to noise_lq | missing_user… | FAILED: pixel `[28,25,26,255]` |
| Fallback re-translates the preset body | shader_error… | FAILED: `DefaultShader{Warp, line 3}` |

One mutation passed at first: the memory-skip mutation got through the cache test, because disk hits hid it. I then split the memory check into a renderer with no disk cache, and that mutation now fails the test.

GREEN:
- `MD_REQUIRE_GPU=1 cargo test --workspace`: all pass, no warnings.
- `cargo test -p md-render --test preset_gpu`: 6 passed.
- `cargo clippy --workspace --all-targets -D warnings`: clean. `cargo fmt --check`: clean. `cargo deny check`: ok. `check-no-native-build`: OK.

## Self-review findings

- Every file is under 600 lines. Line counts: preset_gpu 424, shader_cache 450, renderer 366, preset_bind 226; the test file is 447.
- C++ references use the F61 `file:line@6f7cd5d` form.
- During the mutation runs a backup mishap left mutations in the tree for a while. I reverted them by hand and checked with grep, and the clean tree passes everything above.

## Concerns

1. **Comp pipeline target format.** Per the brief, the comp pipelines target the *main texture* format, but the C++ composites into the back buffer. Task 5.7 needs to either composite through a main-format texture or build comp pipelines for the surface format. `ShaderProgram::shader()` keeps the module so the rebuild is cheap.
2. **Bind groups reference VS and blur textures.** Task 5.7's `resize` must reload presets or rebuild group 1. `sources` is kept for that.
3. **Cache key is too coarse.** md-hlsl `VERSION` is the workspace version (0.1.0), so translator changes between commits do NOT invalidate disk entries until the version is bumped. Developers should clear the cache dir after translator changes, or a translator revision constant should be added.
4. **Blur placeholder sizes.** They follow the C++ formula (¼, ⅛, 1/16 for blur1..3), not the milestone text (½, ¼, ⅛). Task 6.1 should confirm this.
5. **Fallback colour.** The fallback texture is white, per the brief. The C++ bound NULL, which samples black in D3D9.
6. **PsVersion conversion.** The F23 `From` impl is impossible under the orphan rule, because both types are foreign to md-render. I used the free function `hlsl_ps_version` instead.
7. **Brief interface deviations.** `ShaderCache` is keyed by `[u8; 32]` and stores the full `Translated`, not `HashMap<u64, Arc<ShaderModule>>`. The disk path is `shader_cache_dir/<hex>.mdshader` (F35), not `md_platform::cache_dir()`. `bind_group_1` is a pair indexed by the `main` texture.

## Fix round 1 (commit 97f625d)

1. **The missing-texture fallback is now opaque black (ledger Task 5.5 ruling).**
   - `renderer.rs`: the 2D and 3D fallback textures are created from `[0, 0, 0, 255]`.
   - `preset_bind.rs`: the module docs and the WARN text now say "1x1 black fallback".
   - `tests/preset_gpu.rs` (`missing_user_texture_binds_fallback_and_logs_once`): main is now cleared to grey 0.5, and the comp output must be 128. Black adds 0 × 0.2 = 0. A white fallback would give 179, so the test now tells the two apart; with a black main, black and white would not differ much.
2. **The shader-cache key now changes with the translator.**
   - New `crates/md-hlsl/build.rs` (pure Rust; build-dependency `sha2`). It hashes every file under `crates/md-hlsl/src`, `crates/md-hlsl/Cargo.toml` and `assets/hlsl`, using sorted relative paths with length-prefixed paths and contents. It emits `cargo:rerun-if-changed` for those directories and files, `build.rs` and `Cargo.toml`.
   - md-hlsl exports the result as `TRANSLATOR_FINGERPRINT` (64 hex characters). The unit test `translator_fingerprint_is_a_sha256_hex` checks its shape.
   - `shader_key` now includes the fingerprint.
   - The disk header gains a `translator <fp>` line, and `CACHE_FORMAT` is bumped to `md-render shader cache 2`. `decode` rejects any other fingerprint.
   - Tests:
     - The unit test `corrupt_entries_are_rejected` now covers a zeroed fingerprint.
     - The GPU test `cache_hit_skips_translation` rewrites each entry's fingerprint to zeros. A fresh renderer must then do 2 translations and 0 disk hits, and the regenerated entries must carry the current fingerprint.
3. **Folded-in fixes.**
   - Temporary file names are now `<hex>.tmp<pid>-<counter>`, using an atomic counter.
   - `ShaderCache::new` sweeps `*.tmp*` files older than `STALE_TMP_SECS` (60 s). The new unit test `sweep_removes_only_stale_temporaries` covers this.
   - When wgpu rejects the module from a disk hit, the entry is deleted and the shader is re-translated; the error is no longer cached. This path has no test: I found no input that passes decode and naga validation but is rejected by wgpu.

### Tests and checks

RED was again obtained by mutation, each mutation reverted afterwards:

| Mutation | Test | Result |
|---|---|---|
| Fallback set back to white | missing_user… | FAILED, pixel `[179,179,179,255]` |
| Fingerprint check removed from `decode` | cache_hit… | FAILED at preset_gpu.rs:327 |

GREEN:
- `MD_REQUIRE_GPU=1 cargo test --workspace`: all pass.
- `cargo test -p md-render -p md-hlsl`: all pass.
- `cargo clippy --workspace --all-targets -D warnings`: clean. `cargo fmt --check`: clean.
- `scripts/check-no-native-build.sh`: OK. `cargo deny check`: advisories, bans, licenses and sources all ok.

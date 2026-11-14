# Task 5.4 report: texture store, image loading, noise textures

Status: DONE_WITH_CONCERNS (the concerns are two documented deviations, listed below)
Commit: 7be0a3e feat(render): texture store, image loading, noise textures

## What was implemented

Files (all in crates/md-render):
- `src/noise.rs` (pub mod `md_render::noise`): a port of AddNoiseTex/AddNoiseVol (plugin.cpp:2427-2715).
  - `NOISE_2D` / `NOISE_3D` specs: noise_lq 256/1, noise_lq_lite 32/1, noise_mq 256/4, noise_hq 256/8; noisevol_lq 32/1, noisevol_hq 32/4.
  - `RANGE` is 216 when zoom > 1, else 256. Each channel value is `rand()%RANGE + RANGE/2`.
  - `pack_texel` uses the C++ expression `(v0<<24)|(v1<<16)|(v2<<8)|v3` on u32, so per F33 each byte is `v & 0xFF` and bit 8 ORs into bit 0 of the next channel (B→G→R→A, and A's bit 8 is lost). There is no clamping.
  - Each row gets `size` random swaps, as in the C++.
  - Smoothing port: `fCubicInterpolate`/`dwCubicInterpolate` work per byte, clamp to [0,1] and truncate `f*255`.
    - 2D: lattice rows across X, then every column down Y.
    - 3D: X on the lattice rows of the lattice slices, Y on the lattice slices, then Z everywhere.
    - Both use the C++'s wrapping `(base ± zoom) % size` indexing.
  - `dword_to_rgba` converts the A8R8G8B8 DWORD to Rgba8Unorm bytes.
  - RNG: one `Pcg32` seeded from the store seed drives every texture in spec order. `crand()` = `next_u32() >> 17` (15 bits) keeps rand()'s `% RANGE` bias. The C++'s per-row perf-counter reseeding is not reproduced (A.4).
- `src/image_load.rs`: `load_image(path) -> Result<RgbaImage, ImageError>` with ImageError variants `Io`, `Decode` and `Unsupported`.
  - JPEG, PNG, TGA and BMP decode through `image`; `.dib` decodes as BMP.
  - The format comes from magic bytes first, then the extension. D3DX also sniffs content, so a PNG named .jpg loads. TGA has no magic, so it relies on the extension.
  - DDS goes through ddsfile and only uncompressed formats decode:
    - legacy bitmask RGB(A), 8/16/24/32 bpp;
    - luminance L8, A8L8 and L16;
    - DX10 R8G8B8A8, B8G8R8A8 and B8G8R8X8.
  - FourCC/BC DDS files, volume DDS files and other DXGI formats return `ImageError::Unsupported`. Only the top mip level is used.
- `src/texcache.rs`: `TexCache<T>` plus the public `TextureId`, `MAX_IMAGES = 3000`, `MAX_BYTES = 2e9` and `TEXTURE_OVERHEAD_BYTES = 16384`. This is a generic, GPU-free cache: handles, case-folded keys, ages and the budget.
- `src/textures.rs`: `TEXTURE_EXTS`, `locate_texture`, `scan_texture_names`, `pick_random_texture`, `TextureStore`, `RandPicks` and `TextureError`.
- `tests/textures.rs`: 10 integration tests (4 use the GPU).
- Cargo:
  - workspace `image` features are now png+jpeg+tga+bmp;
  - md-render gains image, ddsfile, rand_pcg, rand_core and thiserror;
  - new crates in the lock: zune-jpeg, zune-core, ddsfile, byteorder, enum-primitive-derive, rand_pcg.
  - `cargo deny check`: advisories, bans, licenses and sources all ok. The "unmatched wrapper android-activity" warning was there before this task.
  - `scripts/check-no-native-build.sh`: OK.

### TextureStore API (as built)
- `new(gpu, seed)` generates and uploads the six noise textures.
- `scan_dir(dir)` sets the textures dir and rescans the random candidates. `set_preset_dir(Option<&Path>)` sets the second search dir.
- `load(gpu, root) -> Result<TextureId, TextureError>` returns `NotFound`, `Image(ImageError)` or `TooLarge` (larger than the device's max_texture_dimension_2d).
- `get(gpu, root) -> Option<&Texture2d>` checks noise names first, then the disk with a full mip chain.
- `texture(id)` returns the texture, or None once it has been evicted.
- `noise2d(name)` / `noise3d(name)` return Option.
- `random_texture_names() -> &[String]`.
- `size_of(root) -> Option<(u32,u32)>` covers noise (volumes report (32,32)) and cached textures. It does not load anything or touch ages.
- `set_budget(max_images, max_bytes)`, `preset_loaded()`, `usage()` and `locate(root)`.
- `RandPicks` holds one shader's `RandTexName[16]`:
  - `resolve_sampler(candidates, root, rng)` picks for `randNN[_prefix]`, filtering candidates by prefix case-insensitively. Other roots pass through unchanged.
  - `texsize_root(root)` maps by slot, whatever the suffix (ledger Task 4.5). It returns None if the slot's pick failed, and plain `randNN` if no sampler used the slot (plugin.cpp:3277-3293).
  - The caller passes `store.random_texture_names()` and the preset's RNG.
- F34: every cached texture sits behind a `TextureId` (ids are never reused). Task 6.8 can add `load_sprite(gpu, path, colorkey_lo, colorkey_hi) -> TextureId` by inserting into the same `TexCache` under a key that includes the path and colorkey. No colorkey is implemented now.

### C++ semantics pinned (sources read)
- **Search order (F32)**, plugin.cpp:2847 and 3158-3171: the loop is extension-major. For each of jpg, dds, png, tga, bmp, dib it tries `textures\root.ext` and then `<presetdir>root.ext`, so `presets/x.jpg` beats `textures/x.dds`. Names match case-insensitively on all OSes (Windows FS semantics). Cache and noise names are case-insensitive too, while the C++ uses `wcscmp` for its cache.
- **Random candidates**, PickRandomTexture (plugin.cpp:2850-2918):
  - Only the textures dir is scanned. Files with a texture extension count, compared case-insensitively.
  - The extension after the last '.' is chopped, and duplicates are kept.
  - The prefix filter is `_wcsnicmp`.
  - The list is sorted case-insensitively, standing in for FindFirstFile order.
- **Budget**, plugin.cpp:3135-3156 and 2772-2843:
  - Before loading an uncached texture, the C++ evicts while the evictable count is `>= max_images` or the bytes are `>= max_bytes`. If nothing is evictable it gives up and loads anyway.
  - Bytes per texture are `w*h*bpp/8 + 16384`, with mips not counted. We always use 32 bpp because everything is decoded to RGBA8.
  - Noise and blur textures are not counted.
  - Age is stamped with `m_nPresetsLoadedTotal` on load or hit. Textures whose age is `>= total-1` are protected (current preset and blend-from preset). `preset_loaded()` mirrors `m_nPresetsLoadedTotal++`.
- **Victim choice (deviation, as the brief asked)**: the C++ evicts the age-weighted "biggest" texture. Its weight grows with recency, which contradicts its own comment, and it is 0/0 when all candidates share an age, in which case nothing is evicted. We evict true LRU instead: oldest age first, ties broken by larger size, then by earlier load. This is documented in texcache.rs.

## TDD evidence

Honest note: I wrote the implementation and the tests in the same pass, so I did not observe RED before the implementation existed. Instead I demonstrated RED by mutation: I injected the wrong behaviour each test guards against, ran the tests, and restored the files from a backup.

Mutations: brief's extension order (jpg,png,tga,bmp,dds,dib); clamped packing (`v.min(255)`); evicting the newest instead of the oldest (`max_by`); `get` panicking on a missing file; noise 2D created without mips.

`MD_REQUIRE_GPU=1 cargo test -p md-render --lib` (with the mutations):
```
test noise::tests::noise_values_carry_bit8_into_next_channel_for_zoom ... FAILED
test texcache::tests::byte_budget_and_rebinding_refreshes_age ... FAILED
test texcache::tests::lru_eviction_respects_budget ... FAILED
test texcache::tests::ties_evict_the_larger_texture ... FAILED
  left: 3372220415  right: 3372297599   (0xC900FF7F clamped vs 0xC9012D7F)
test result: FAILED. 11 passed; 4 failed
```
`MD_REQUIRE_GPU=1 cargo test -p md-render --test textures`:
```
test extension_search_order ... FAILED
test missing_file_returns_none ... FAILED
test store_lru_eviction_respects_budget ... FAILED
test result: FAILED. 7 passed; 3 failed
test noise_lq_is_256_with_9_mips ... FAILED   left: (256, 256, 1) right: (256, 256, 9)
```
GREEN (originals restored):
```
MD_REQUIRE_GPU=1 cargo test -p md-render --lib      -> test result: ok. 15 passed; 0 failed
MD_REQUIRE_GPU=1 cargo test -p md-render --test textures -> test result: ok. 10 passed; 0 failed
MD_REQUIRE_GPU=1 cargo test --workspace             -> all suites ok, 0 failures
cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings -> clean
cargo deny check -> advisories ok, bans ok, licenses ok, sources ok
scripts/check-no-native-build.sh -> OK
```

Brief tests → names:
- `noise_lq_is_256_with_9_mips` (GPU). It also checks the other sizes and mips, that noisevol has 32³ and 1 level, and that the level-0 readback equals the CPU generation (pins RGBA byte order).
- `noise_values_within_range_for_zoom` was renamed to `noise_values_carry_bit8_into_next_channel_for_zoom` (F33). It checks:
  - the exact packing 0xC9012D7F;
  - zoom-4 lattice bytes lie in [0,67]∪[108,255], with more than 1000 wrapped bytes;
  - zoom-1 odd fraction: B ≈ 0.5, G/R/A ≈ 0.75 (from the carry).
- `missing_file_returns_none` (GPU): covers a missing file and a corrupt file.
- `extension_search_order` (CPU): pins `TEXTURE_EXTS` and the extension-major dir order.
- `lru_eviction_respects_budget`: a CPU unit test in texcache.rs, plus `store_lru_eviction_respects_budget` (GPU).
- Extra tests: case-insensitivity, all formats including .dib and a PNG named .jpg, uncompressed DDS versus BC (DXT1 → Unsupported), random names and prefix picks, texsize by slot, cubic clamp, smoothing in 2D and 3D, determinism, specs, and protection of the current and blend-from presets.

## Self-review findings
- Files are under 600 lines: textures.rs 358, noise.rs ~370 (half of it tests), texcache.rs 253, image_load.rs 190.
- `load` evicts (make_room) before it knows whether the file exists. The C++ does the same.
- A texture that is missing or fails to decode is retried from disk on every `get`. There is no negative cache, but callers resolve once per preset load, as the C++ does.
- A DDS file's own mip levels are ignored and the chain is regenerated. D3DX would use the file's mips. This is minor.

## Concerns / deviations for the controller
1. **Noise volumes have no mips (`NOISE_3D_MIPS = false`)**. The milestone-5 context says "all have full mip chains", but AddNoiseVol creates the volumes with `Levels = 1` (plugin.cpp:2573-2575: "DX9 doesn't do auto mipgen w/volume textures"). I followed the C++ for parity. To get chains instead, flip the one constant.
2. **`noise2d`/`noise3d` return `Option<&…>`** instead of the brief's bare `&Texture2d`/`&Texture3d`, so an unknown name cannot panic.
3. **API additions beyond the brief**: `set_preset_dir`, `preset_loaded`, `set_budget`, `usage`, `load`/`texture` with `TextureId`, `locate`, the free functions `locate_texture`/`scan_texture_names`/`pick_random_texture`, and `RandPicks`.
   - Task 5.5 has to call `preset_loaded()` after each preset load so the age protection works.
   - It also has to use one `RandPicks` per shader, with the preset's RNG.
4. **Eviction victim is true LRU (per the brief), not the C++'s age-weighted heuristic.** The heuristic is buggy, as explained above. This only affects performance.

## Fix round 1 (controller ruling amending F32)

Commit: see `git log` — "fix(render): mirror C++ texture candidate loop (last decodable wins, failures fall through)"

Changes:
- Checked the C++ loop at plugin.cpp:3158-3215. The extension is the outer loop (jpg, dds, png, tga, bmp, dib). For each extension the C++ takes `textures\root.ext` if it exists, otherwise `<presetdir>root.ext`, so there is at most one candidate per extension. There is no `break`, so the last successful load wins and a failed load moves on.
- `locate_texture` is replaced by `texture_candidates(dirs, root) -> Vec<PathBuf>`, which returns the candidates in that order. `TextureStore::locate` becomes `TextureStore::candidates`. The module docs are updated.
- `TextureStore::load` decodes every candidate and keeps the last one that decodes and fits `max_texture_dimension_2d`. Only that one is uploaded. If none succeeds, it returns the last error: `Image`, `TooLarge`, or `NotFound` when there are no candidates.
  - A corrupt x.jpg next to a valid x.png now loads the png. Before this fix it returned `TextureError::Image`.
- texture.rs: the `from_rgba8` length asserts for 2D and 3D now compute `w*h*4` (and `*d`) in u64, so the u32 multiply can no longer overflow. Images too large for the device are rejected with the typed `TextureError::TooLarge`; a too-large candidate counts as a failed candidate.

Covering tests (crates/md-render/tests/textures.rs):
- `extension_search_order_last_candidate_wins` (renamed):
  - pins `TEXTURE_EXTS`;
  - pins the exact candidate list across both dirs: preset jpg, textures dds, textures png shadowing the preset png, preset dib;
  - pins the full six-extension order;
  - pins the empty case.
- `store_keeps_last_decodable_candidate` (GPU):
  - corrupt jpg plus valid png loads the png;
  - valid jpg, valid dds and corrupt png loads the dds (last decodable, not first);
  - only-corrupt candidates give `TextureError::Image`.
- `get_loads_with_mips_from_textures_then_preset_dir` is adjusted: the png wins as the last candidate.
- `texture_names_match_case_insensitively` now uses `texture_candidates`.

RED by mutation: I changed the loop to first-wins (`best.get_or_insert`). Running `MD_REQUIRE_GPU=1 cargo test -p md-render --test textures` gave:
```
test get_loads_with_mips_from_textures_then_preset_dir ... FAILED  left: (16, 16, 5) right: (8, 4, 4)
test store_keeps_last_decodable_candidate ... FAILED               left: (4, 4) right: (2, 1)
```
GREEN after restoring:
- `MD_REQUIRE_GPU=1 cargo test -p md-render`: lib 15, gpu 8, layout 9, pipelines 8, textures 11, all ok with 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --check`: clean.

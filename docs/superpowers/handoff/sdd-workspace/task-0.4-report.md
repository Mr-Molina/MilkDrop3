# Task 0.4 report: Preset corpus and reference captures

Status: DONE_WITH_CONCERNS (the concerns are notes for later tasks; nothing in 0.4 is blocked)
Commit: 9b4dc39 test: add preset corpus, test signal, and capture instructions

## What was implemented (per the controller rulings)
1. **Self-authored corpus** (BSD-3), 32 `.milk` files under `tests/presets/{classic,ps2,ps2x,ps3,edge}/`.
   - The MD2 files follow `CState::Export` exactly: header `MILKDROP_PRESET_VERSION=201`/`PSVERSION`/`PSVERSION_WARP`/`PSVERSION_COMP`, `[preset00]`, every key in Export order with Export's formats (`%.3f`/`%.5f`/`%d`), all 4 `wavecode_N_*` and 4 `shapecode_N_*` blocks, `wave_N_init/per_frame/per_pointM`, `shape_N_init/per_frameM`, `per_frame_init_N`, `per_frame_N`, `per_pixel_N`, and `warp_N`/`comp_N` with the leading backtick.
   - The `classic/` files use the MilkDrop 1 layout: no header, `%f` numbers, `bMotionVectorsOn`, no blur keys, so they load as PSVERSION 0.
   - Shaders are MilkDrop-dialect HLSL in the `shader_body { }` form, written against `include.fx` (GetBlur1..3, GetMain/GetPixel, lum, noise samplers, `_qa`..`_qh`, rot_s1, hue_shader).
   - `ps2/upgraded_default` and `ps2x/upgraded_echo` use the exact text from `GenWarpPShaderText`/`GenCompPShaderText`.
   - Coverage: PSVERSION 0/2/3/4, MILKDROP_PRESET_VERSION=200, and mixed PSVERSION_WARP=2/COMP=4. Shader features: tex3D (noisevol_hq and noisevol_lq), tex2Dbias, tex2Dlod, GetBlur1/2/3, sampler_rand00..15 (including the `_smalltiled` suffix and the `fc_`/`pw_` prefixes), the user texture `sampler_mytex` with `texsize_mytex`, and a missing texture (Review Focus #1).
   - Custom waves and shapes: 4 waves and 4 shapes with code, instancing, a textured shape.
   - Expressions: megabuf/gmegabuf, loop/while, exec2/exec3, reg00..reg03, q1..q32, if/equal/above/below/sqr/sqrt/sin/rand/sign/pow, and so on. `edge/eel_syntax_zoo.milk` adds `$x1F`, `$PI/$E/$PHI`, `^`, `^=`, `?:`, `==`/`&&`/`||`/`!`, `a[b]`, memset/memcpy/freembuf, sigmoid/invsqrt/band/bor/bnot/assign, and comments.
   - `edge/crlf_bom_junk.milk` has a UTF-8 BOM, CRLF line endings, and trailing junk after the section (a forum note plus a ^Z byte).
   - `textures/mytex.png` is 64x64 RGB, generated procedurally with Python/zlib (no third-party art).
   - The files were emitted by a throwaway Python generator in the scratchpad that encodes Export's order and formats (not committed). A structural check found 0 problems: no duplicate keys, contiguous line numbering, balanced ()/{}/[] after stripping comments, and `shader_body` present in every shader.
   - `tests/presets/README.md` records origin and license, sources, the PSVERSION value mapping, a preset→features table, file notes, and `MD_EXTRA_PRESETS` usage.
2. **`MD_EXTRA_PRESETS`**:
   - New pure function `corpus_presets_from(root, extra: Option<&Path>)`: recursive walk, case-insensitive `.milk`, canonicalize, sort, dedup.
   - New helpers `presets_dir()` and `EXTRA_PRESETS_ENV`.
   - `corpus_presets()` is now a thin wrapper that reads the env var (an empty value is ignored).
   - The pure function is tested with explicit temp paths; no env mutation in tests.
3. **No LFS.** `.gitattributes` has `*.png binary`, `*.wav binary`, `*.sh text eol=lf`, and `tests/presets/edge/crlf_bom_junk.milk -text`. After the commit, `git ls-files --eol` shows `i/crlf w/crlf attr/-text` for the fixture, and `git show HEAD:` starts with `ef bb bf` and has 237 CR-terminated lines. The PNG and WAV show as `-text`.
4. **Test signal.**
   - `md_testkit::{test_signal, test_signal_sample, TEST_SIGNAL_RATE, TEST_SIGNAL_SECS}` give 20 s, 44.1 kHz, stereo (identical channels), 16-bit. The signal is 0.8·env(2 Hz)·sin(2π·55t) + 0.2·env(0.5 Hz)·sin(2π·4000t), where env(f) = 0.5 − 0.5·cos(2πft) is a raised cosine (starts silent, peaks at full scale).
   - `examples/gen_signal.rs` writes it with `hound`. hound is a dev-dependency of md-testkit, using the existing workspace pin `hound = "3.5.1"`; Cargo.lock is updated.
   - **The WAV is committed** (`tests/golden/test_signal.wav`, 3,528,044 bytes, under 5 MB). Because the generator function is also public, both are available.
5. **Corpus test** in `tests/support/md-testkit/tests/corpus.rs`:
   - `corpus_has_at_least_25_presets` (prints the count: 32).
   - `corpus_covers_every_a4_feature`: PSVERSION 0/2/3/4 using the C++ loader's rule; every needle; each sampler_rand00..15, with an optional fc_/fw_/pc_/pw_ prefix; each q1..q32 as a whole word; an enabled wave with per-point code; an enabled shape with per-frame code; the mytex.png file; and the BOM+CRLF+junk fixture checked at byte level.
   - `extra_dir_presets_are_merged_sorted_and_deduped`.
   - `default_corpus_root_is_tests_presets`.
6. **`tests/golden/manifest.toml`**:
   - `[defaults]` sets 640x480, seed 7, frames [1,60,300,600] and max_dssim 0.02, so Task 6.9 can extend it.
   - 9 `[[preset]]` entries, each with a `covers` list. Blur-heavy entries use 0.05.
   - The entries cover PSVERSION 0/2/3/4, custom shapes and waves, GetBlur1..3, tex2Dbias, tex3D/noisevol_hq, the user texture, and gmegabuf.
   - `tests/golden/windows/README.md` gives the capture procedure:
     - Build `code/MilkDrop3.sln`; install the presets and textures under `plugins\Milkdrop2\`.
     - milk2.ini: `MaxPSVersion=-1`, `fBlendTimeUser=0`, hard cuts off; 60 fps.
     - Play the WAV through the system (WASAPI loopback, no `--wav` in C++); capture frames 1/60/300/600.
     - These are visual references only: the C++ build has no seed control.

## TDD evidence
- RED: `cargo test -p md-testkit --offline`. Output:
  `error[E0432]: unresolved imports md_testkit::corpus_presets_from, md_testkit::presets_dir`
  `error[E0432]: unresolved imports md_testkit::test_signal, ... TEST_SIGNAL_SECS`
- RED (coverage): with the corpus in place, the first run printed `corpus lacks features: ["sampler_rand04", "sampler_rand05"]`. The presets use `sampler_fc_rand04`/`sampler_pw_rand05`, which are valid MilkDrop prefixed names, so I widened the check to accept the fc_/fw_/pc_/pw_ prefixes.
- GREEN: `cargo test -p md-testkit --offline -- --nocapture` printed `corpus presets: 32`. Results: corpus 4 passed; image_diff 9 passed; signal 3 passed (`committed_wav_matches_generator` allows a 1 LSB difference for libm differences between platforms).
- `cargo run -p md-testkit --example gen_signal` printed `wrote .../tests/golden/test_signal.wav (882000 frames, 3528044 bytes)`.
- Full workspace checks all pass:
  - `cargo fmt --check`
  - `cargo build --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`: all ok, 0 failures
  - `scripts/check-no-native-build.sh`: OK
  - `cargo deny check`: advisories, bans, licenses and sources ok. The existing "unmatched wrapper" warning is unchanged.

## Files
- New: `.gitattributes`, `tests/presets/**` (32 .milk files, `textures/mytex.png`, README.md), `tests/golden/test_signal.wav`, `tests/golden/manifest.toml`, `tests/golden/windows/README.md`, `tests/support/md-testkit/{src/signal.rs, examples/gen_signal.rs, tests/corpus.rs, tests/signal.rs}`.
- Modified: `tests/support/md-testkit/{Cargo.toml, src/corpus.rs, src/lib.rs}`, `Cargo.lock`.

## Self-review
- I read the diff. Each file has one job and all are under 600 lines; `tests/corpus.rs` is about 200 lines.
- `corpus_presets()` now returns canonicalized absolute paths, where before it returned `<manifest>/../../presets/...`. That is equivalent, and it is what makes dedup with `MD_EXTRA_PRESETS` correct.
- I cannot compile the HLSL (no fxc/D3DX here) or the EEL (no C++ build on Linux). Validity rests on careful authoring against include.fx, the authoring guide and the ns-eel2 function table, plus the structural check. Tasks 1.9 and 4.9 are the real validators.

## Concerns / notes for later tasks
1. **`$PI`, `$E`, `$PHI` and `$'c'`**: ns-eel2 accepts all of them (nseel-compiler.c:1050-1100), but the plan's Task 1.1 lexer spec lists only `$x` hex. `edge/eel_syntax_zoo.milk` uses `$PI/$E/$PHI`, so Task 1.9 will fail on it until the lexer supports them. This is intended: the global constraint is that every expression the C++ compiles must compile.
2. **BOM in C++**: the C++ loader does not strip a UTF-8 BOM. `_GetLineByName` reads `<BOM>MILKDROP_PRESET_VERSION`, and the preset-list scan's `strncmp` fails, so C++ loads the BOM fixture as a MilkDrop 1 preset. Review Focus #3 requires the Rust parser to parse the file identically to the version without the BOM. This is a deliberate divergence, documented in both READMEs; Task 2.3 should know.
3. **MaxPSVersion default**: the C++ default `MaxPSVersion=3` means ps_2_x, which hides the PSVERSION=4 (ps_3_0) presets from the C++ preset list. The capture README tells the owner to set it to -1.
4. **Corpus scope**: the corpus is self-authored, so real-world quirks will only show up when the corpus tests run with `MD_EXTRA_PRESETS` pointing at the Winamp or projectM pack (documented in `tests/presets/README.md`).
5. **CI LFS flag**: `ci.yml` still uses `actions/checkout` with `lfs: true`. That is harmless now that there is no LFS, and I left it unchanged; dropping it is optional.

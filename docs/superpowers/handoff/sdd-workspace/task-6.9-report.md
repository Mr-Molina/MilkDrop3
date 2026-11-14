# Task 6.9 report: renderer golden suite

Status: DONE_WITH_CONCERNS (provisional baselines; suite is #[ignore]d, 270 s).

## Implemented
- `tests/golden/manifest.toml`: all 33 corpus presets (existing entries' `covers` kept; others from tests/presets/README.md), `driver = "static"` on every entry, defaults 320x240 / seed 7 / frames [1,60,300] / max_dssim 0.02; 0.05 for ps2/blur_glow, ps2x/blur3_bloom, ps3/mixed_versions (the three GetBlur presets).
- `apps/milkdrop/tests/golden_suite.rs`: `manifest_covers_the_whole_corpus` (no GPU: manifest == corpus, driver is static) and `golden_suite` (render via `render_to_images`, every=1, `compare_png`, collects all failures with DSSIM, one assert). Baselines at `tests/golden/rust/<dir>/<name>/frame_<n>.png`.
- `headless.rs`: FrameInput.shapes filled from each enabled shape's static values (one instance each); custom waves stay empty.
- Dev-deps `serde`, `toml` (workspace pins, pure Rust; native-build guard OK). `tests/golden/README.md` doc note. CI step added to `.github/workflows/ci.yml`.
- golden_basic left as is (its own 640x480 default_60.png).

## TDD
RED (no baselines, bless off): `cargo test -p milkdrop --test golden_suite` -> golden_suite FAILED listing all 99 `missing baseline` frames. Also a first RED for the manifest test (StripPrefixError, fixed by using md_testkit::presets_dir()).
BLESS: `MD_BLESS=1 ... -- --include-ignored` -> ok. GREEN (bless off, twice): `2 passed`, 269.8 s.

## Runtime
Suite wall time 265-270 s (lavapipe) > 180 s, so `golden_suite` is `#[ignore]` with a doc comment; run `cargo test -p milkdrop --test golden_suite -- --include-ignored`. Added as an explicit CI step and documented in tests/golden/README.md. The hand-off gate must run it explicitly. Default `cargo test --workspace`: 31 s, all pass.

## Size
tests/golden/rust/ = 4.5 MB (99 suite PNGs + default_60.png).

## Gates
build, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `cargo test --workspace`, check-no-native-build: all clean.

## Frame-60 baselines (each viewed)
- classic/bass_kaleido: orange blob with radial white motion-vector spokes on a grey field. OK.
- classic/drift_tunnel: pale-blue diagonal smeared streak, built-in wave fed back. OK.
- classic/echo_feedback: grey diagonal band, video echo smear. OK.
- classic/orbiting_shapes: white/grey blobs and two arcs (the static shapes) on black. OK.
- classic/q_init_rand: white diagonal smear, additive wave. OK.
- classic/spectral_rings: BLACK. Expected: only a custom wave (empty under static driver), fWaveAlpha 0.001.
- edge/crlf_bom_junk: white diagonal smear. OK.
- edge/eel_syntax_zoo: spiky white ring over grey warped field (wave mode 0). OK.
- edge/missing_texture: grey smear, small white wave at upper left. OK (fallback texture path renders).
- ps2/blur_glow: grey horizontal wave lines, blur glow. OK.
- ps2/borders_motion: red/blue/magenta borders, motion vectors, white smear. OK.
- ps2/megabuf_trails: BLACK. Expected: custom wave only, fWaveAlpha 0.
- ps2/noise_grain: heavy white Lissajous ellipse plus horizontal line with grain. FLAG (mild): blown-out, but consistent with 1.6x comp gain over additive noise; recheck at 7.9.
- ps2/qvar_bank: blue smear (only blue channel; q-driven colours are 0 under static). OK for static.
- ps2/rand_tiles: grey smear with small tiled dots near centre. OK.
- ps2/shader_rings: rainbow concentric rings with centre spokes. OK.
- ps2/upgraded_default: orange looping wave strand over orange haze. OK.
- ps2/user_texture: dim grey ring-shaped wave and textured-shape rings. OK.
- ps2/version200: brown/orange diagonal field with bright core. OK.
- ps2x/blur3_bloom: grey/white horizontal wave lines with bloom. OK.
- ps2x/four_shapes: black with a tiny green triangle at centre (91 lit px). FLAG (explained): shape 2 is textured over the black warp source with alpha 0.9, drawn after shape 0 (red pentagon), so it hides it; comp does ret*ret*2. Matches C++ draw order; only shape 3 (additive green) survives. Instances/equations come in 7.9.
- ps2x/gmegabuf_share: BLACK. Expected: custom waves only.
- ps2x/rand_gallery: grey looping wave strand over dim smear. OK.
- ps2x/tex2dbias_soft: soft white/grey smear. OK.
- ps2x/upgraded_echo: white diamond with grey mirrored wings (echo). OK.
- ps3/all_q32: BLACK. FLAG (explained): comp multiplies by _qe/_qf (q29..q32 sums), all 0 without per-frame equations.
- ps3/four_waves: BLACK. Expected: custom waves only, fWaveAlpha 0.
- ps3/loop_shader: spiky white ring over warped grey field (radial blur). OK.
- ps3/mixed_versions: grey/white smear. OK.
- ps3/noise_volume: grey smear with a saturated white patch on the right. OK.
- ps3/rand_mosaic: grey smear. OK.
- ps3/sampler_modes: two white horizontal wave lines over grey. OK.
- ps3/volume_lq_rot: pale green/cyan tinted smear. OK.

None is mirrored or corrupted. Five are black (4 custom-wave-only, 1 q-dependent comp), so their three frames do not discriminate anything until 7.9. Many presets share the same warp-field look since no equations differentiate them.

## Concerns / notes for controller
- Baselines PROVISIONAL (7.9 flips driver, re-blesses).
- Suite is #[ignore]d; the hand-off gate and CI must pass `--include-ignored`.
- manifest test uses md_testkit::corpus_presets(), so setting MD_EXTRA_PRESETS would make it fail (intended: extra packs are not in the manifest).
- Commit subject says "33-preset corpus" instead of the brief's "40 presets" per the ruling.

## Fix round 1
- ci.yml golden-suite step: `if: runner.os == 'Linux'`, step-level `env` MD_REQUIRE_GPU=1 and WGPU_ADAPTER_NAME=llvmpipe. YAML parses (PyYAML).
- .gitignore: `tests/golden/rust/**/*.diff.png` (verified with git check-ignore).
- tests/golden/README.md: note that bless skips within-threshold drift (delete the PNG or use max_dssim = 0).
- Gate: fmt, clippy -D warnings, `MD_REQUIRE_GPU=1 cargo test --workspace` clean (0 failures). No re-bless.

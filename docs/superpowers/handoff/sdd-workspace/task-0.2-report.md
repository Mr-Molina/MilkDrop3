# Task 0.2 report
Implemented md-testkit: image_diff.rs (own windowed SSIM: 11x11 Gaussian sigma 1.5, K1 .01 K2 .03, luma; dssim = 1/ssim - 1; compare_png / compare_png_with(bless explicit) / bless_enabled / DiffResult with `<golden>.diff.png` on failure), fixed_clock.rs, corpus.rs (tests/presets, empty if absent). No dssim-core (F1).
RED: `cargo test -p md-testkit` failed to compile (compare_png_with, FixedClock undefined).
GREEN: 6 tests pass (identical, shift, different+diff png, missing golden/bless, size mismatch, fixed clock). Full workspace build/clippy -D warnings/fmt/test clean.
Calibration: 64x64 noise-free sinusoid image, 1px shift -> dssim 0.0041 (<0.01). With +-3 hash noise added the shift gave 0.057, so the test image is noise-free (documented in the test).
Deps: image (already pinned) added to md-testkit; Cargo.lock updated (moxcms pulled by image png feature; no cc/cmake/bindgen in build graph).

## Fix round 1
Added tests in tests/support/md-testkit/tests/image_diff.rs pinning SSIM math: `ssim_of_constant_images_matches_analytic_value` (gray means (100,200),(0,255),(50,60),(128,128); ssim == (2*ma*mb+C1)/(ma^2+mb^2+C1) within 1e-9), `dssim_of_constant_images_is_reciprocal_minus_one` (compare_png_with dssim == 1/ssim-1 within 1e-9), `inverted_image_scores_very_low_ssim` (ssim < 0, dssim > 1, fails). No metric change was needed.
Command: `cargo test -p md-testkit` -> 9 passed; clippy -D warnings and fmt --check clean.

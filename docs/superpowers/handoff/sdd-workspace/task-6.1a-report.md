# Task 6.1a report: single-level main textures

Status: DONE. Commit: see `git log -1` (fix(render): single-level main textures as in the C++).

## Implemented
- tex_size.rs `main_textures`: VS[0]/VS[1] created with `mips = false` (1 level at every tex_bits / size mode / after resize, since resize goes through the same function).
- warp.rs: removed `record_mips` call and import; module and `warp_pass` docs corrected.
- frame.rs, blur.rs, comp_fixed.rs, fixed.rs: removed/corrected comments that required main mip regeneration (hook comment for 6.2-6.7, blur_passes doc, "with its mip chain"). frame.rs clear loop is generic over `src.mips` (now 1); left as is.
- `generate_mips`/`record_mips` kept (noise, user textures). `_c12` untouched. Sampler/views: the main `view` covers all (now 1) levels; composite/blur bindings validate (all tests pass).
- tests/warp.rs: the "mips generated after the warp" assertion replaced by "warp target has 1 level".

## TDD
- New `main_textures_are_single_level` (tests/frame.rs): tex_bits 8/16 x Fixed/Auto/AutoPow2, after `new` and after `resize`.
- RED: `MD_REQUIRE_GPU=1 cargo test -p md-render --test frame main_textures` -> FAILED (1 failed).
- GREEN: same command after the change -> passes; full md-render suite all ok.

## Gate (MD_REQUIRE_GPU=1)
cargo build --workspace, clippy --all-targets -D warnings, fmt --check clean; `cargo test --workspace` all ok (no failures).

## Golden
apps/milkdrop golden_basic (`golden_default_preset_frame_60`) PASSES unchanged: default_60.png not changed, no re-bless, no DSSIM to report (the default comp does not sample main minified). No other golden moved.

## Concerns
None. Note: tests' `upload` helpers still call `generate_mips` on main; it is now a no-op there (harmless).

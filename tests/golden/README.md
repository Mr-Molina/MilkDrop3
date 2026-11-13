# Golden images

- `manifest.toml` lists every corpus preset (`tests/presets/`), its frames, size,
  seed, `max_dssim` and `driver`.
- `rust/<dir>/<preset>/frame_<n>.png` are the Rust renderer's own baselines,
  checked by `apps/milkdrop/tests/golden_suite.rs`. They are PROVISIONAL
  (`driver = "static"`): the headless driver runs no equations and draws no
  custom waves; Task 7.9 re-blesses them with `md_core::Visualizer`.
- `windows/` holds visual references from the C++ build (see its README).

The suite takes about 270 s on lavapipe, so `cargo test --workspace` skips it
(`#[ignore]`). Run it with

    MD_REQUIRE_GPU=1 cargo test -p milkdrop --test golden_suite -- --include-ignored

CI has an explicit step for it. Bless missing or changed baselines with
`MD_BLESS=1` on the same command, then view the images before committing.

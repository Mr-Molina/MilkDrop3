### Task 0.1: Workspace skeleton

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `NOTICE`, `crates/md-eval/Cargo.toml`, `crates/md-eval/src/lib.rs` (and the same pair for md-preset, md-analysis, md-audio, md-hlsl, md-text, md-render, md-core, md-platform), `apps/milkdrop/Cargo.toml`, `apps/milkdrop/src/main.rs`, `tests/support/md-testkit/{Cargo.toml,src/lib.rs}`

**Interfaces:**
- Produces: the crate names above, `[workspace.dependencies]` with exact pinned versions chosen at this step (wgpu, naga, winit, cpal, fontdue, fontdb, image, ddsfile, glam, serde, toml, hound, clap, tracing, tracing-subscriber, rand_pcg, rand_core, thiserror, anyhow, dssim-core, libpulse-binding, screencapturekit, dirs), `[workspace.lints]` with `unsafe_code = "forbid"` for every crate except `md-audio` (OS bindings).

- [ ] **Step 1:** Write `Cargo.toml` with the ten members and pinned versions; each member `lib.rs` contains only `#![forbid(unsafe_code)]` (md-audio: `#![deny(unsafe_code)]`) and a doc comment naming its responsibility.
- [ ] **Step 2:** Run `cargo build --workspace`. Expected: success with zero warnings.
- [ ] **Step 3:** Write `NOTICE` carrying the Nullsoft 2005-2013 BSD text from `code/vis_milk2/dxcontext.h` and the Maxim Volskiy 2018 BSD text from `code/LICENSE.txt`.
- [ ] **Step 4:** Commit: `chore: create Rust workspace skeleton`


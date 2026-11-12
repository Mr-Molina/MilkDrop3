# Task 0.1 report: Workspace skeleton

Status: DONE_WITH_CONCERNS. Commit f0545ae "chore: create Rust workspace skeleton" (not pushed).

## Implemented
- Root Cargo.toml: 11 members (9 crates, apps/milkdrop, tests/support/md-testkit), resolver 2, workspace.package (0.1.0, edition 2021, BSD-3-Clause), workspace lints (rust unsafe_code = forbid; clippy all = warn).
- Every member has `publish = false` and a lib.rs with a doc comment plus `#![forbid(unsafe_code)]`. md-audio declares its own `[lints]` (unsafe_code = deny) and `#![deny(unsafe_code)]`, no `lints.workspace`. apps/milkdrop has `fn main() {}`.
- rust-toolchain.toml: channel 1.97.0 (installed), components clippy, rustfmt.
- NOTICE: Nullsoft 2005-2013 BSD text (dxcontext.h lines 4-27) and the full Maxim Volskiy 2018 BSD text (code/LICENSE.txt).
- .gitignore unchanged (already has /target/ and /.superpowers/). Cargo.lock committed.

## [workspace.dependencies] pinned (all verified to resolve in a scratch probe crate)
wgpu 30.0.1, naga 30.0.1, winit 0.30.13, cpal 0.18.2, fontdue 0.9.5, fontdb 0.24.0, image 0.25.10 (default-features off, png), ddsfile 0.6.0, glam 0.34.3, serde 1.0.229 (derive), toml 1.2.1, hound 3.5.1, clap 4.6.7 (derive), tracing 0.1.44, tracing-subscriber 0.3.23, rand_pcg 0.10.2, rand_core 0.10.1, thiserror 2.0.21, anyhow 1.0.104, dirs 7.0.0, libpulse-binding 2.30.1, screencapturekit 11.1.2.
Left out: dssim-core (AGPL, F1), blake3 (F2), and all F10 extras (bytemuck, pollster, proptest, criterion, zbus, objc2, windows, libpulse-simple-binding); later tasks add them.

## No-C-compiler check
Probe crate with all pins (linux target deps libpulse-binding, macos screencapturekit), `cargo tree --target all -e normal,build -i <pkg>`:
- cmake, bindgen: not in graph.
- cc: present only as a build-dependency of wayland-backend (winit) and android-activity. Both invoke C only behind features (wayland-backend `log`, android-activity game-activity); `cargo tree -e features` shows neither enabled. So no C compile occurs, but a naive "cc absent from graph" cargo-deny ban would trip on winit 0.30.13. Concern for the deny.toml task: use a ban with `wrappers`/skip for cc via winit, or accept cc as a Rust-only build dep.
- wgpu/naga 30.0.1 Metal path uses objc2/apple-metal (no objc_exception, no cc). cpal 0.18.2 and screencapturekit 11.1.2 pull no cc. pkg-config appears (alsa-sys, libpulse-sys, khronos-egl) but only locates libs; Linux needs system libasound/libpulse dev packages at build time of those later tasks.
- Probe lived in the scratchpad; the repo does not depend on these yet, so Cargo.lock contains only the 11 members.

## Commands and output
- `cargo build --workspace`: Finished, 0 warnings (checked by touching all lib.rs and grepping "warning": 0).
- `cargo clippy --workspace -- -D warnings`: Finished, clean.
- git status afterwards: clean except pre-existing modification to docs/superpowers/ledger/2026-11-11-rust-conversion.md (not mine, not committed).

## Files changed
Cargo.toml, Cargo.lock, rust-toolchain.toml, NOTICE, crates/md-{eval,preset,analysis,audio,hlsl,text,render,core,platform}/{Cargo.toml,src/lib.rs}, apps/milkdrop/{Cargo.toml,src/main.rs}, tests/support/md-testkit/{Cargo.toml,src/lib.rs}.

## Self-review
- Diff read; trailers match. md-audio clippy lint declared locally to keep parity with the workspace.
- Pinned versions are caret requirements ("30.0.1" means ^30.0.1); Cargo.lock fixes exact versions once members use them. Brief said "exact"; use "=x" if strict pinning is wanted.

## Concerns
- cc via winit/android-activity as above (deny.toml design).
- wgpu/naga 30 and other pins are newer than my training knowledge; API usage in later tasks should be checked against 30.x docs.

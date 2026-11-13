# Hand-off: MilkDrop3 Rust conversion

Written 2026-11-12 17:25 UTC at the hand-off from the first controller session.

## Where things stand

- **Branch:** `rust-experiment`. Everything is committed and pushed, and the working tree is clean.
- **Plan:** `docs/superpowers/plans/2026-11-11-rust-conversion.md`. 70 tasks; finished task steps are ticked `- [x]`.
- **Ledger (source of truth):** `docs/superpowers/ledger/2026-11-11-rust-conversion.md`. It holds one line per task event, every `Ruling:` the controller made, and deferred minors. Read all of it before continuing: later rulings bind later tasks.
- **Pre-flight scan:** `docs/superpowers/ledger/2026-11-11-rust-conversion-preflight.md`. It lists 65 plan defects (F1–F65) with adopted rulings in section 4. Several are amended later in the ledger (F23, F29, F32 and others); the ledger wins.

| Milestone | State |
|---|---|
| 0 Workspace, CI, guard rails (0.1–0.4) | complete |
| 1 `md-eval` (1.1–1.9) | complete |
| 2 `md-preset` (2.1–2.5) | complete |
| 3 `md-analysis`, `md-audio` (3.1–3.6) | complete |
| 4 `md-hlsl` translator (4.1–4.10) | complete |
| 5 `md-text` + render foundation | complete |
| 6 Full renderer (6.1–6.9, plus 6.1a and 6.7a) | complete |
| 7–9 | not started |

**Next task: 5.8** (`render` subcommand and first golden images). Its implementer was cut off by a usage limit before writing any files, so start it fresh. The dispatch prompt used is reproduced below under "Dispatching 5.8".

## Process used (keep it, or the rulings lose their meaning)

The work used the `superpowers:subagent-driven-development` skill. Each task went through these steps:

1. Extract the task brief with the skill's `scripts/task-brief PLAN N`.
2. Dispatch one implementer subagent.
3. Generate a review package with `scripts/review-package PLAN BASE HEAD`.
4. Dispatch one task reviewer subagent, checking spec and quality.
5. Run fix rounds: resume the implementer with the findings, then dispatch a scoped re-review. Rounds 4–5 go to a fresh, stronger implementer.
6. Ledger the result, tick the plan steps with `tick.sh N`, commit and push.

The controller never edits code itself. Every decision is ledgered as `Ruling: what — why — cost if wrong`.

### The workspace

The skill's working directory is `.superpowers/sdd/2026-11-11-rust-conversion/`, which is gitignored. A copy minus the review diffs is committed at `docs/superpowers/handoff/sdd-workspace/`. To restore it:

```bash
mkdir -p .superpowers/sdd/2026-11-11-rust-conversion && cp -r docs/superpowers/handoff/sdd-workspace/. .superpowers/sdd/2026-11-11-rust-conversion/
```

```bash
ln -sf ../../../docs/superpowers/ledger/2026-11-11-rust-conversion.md .superpowers/sdd/2026-11-11-rust-conversion/progress.md
```

The workspace contains:

- **`implementer-rules.md` / `reviewer-rules.md`:** standing instructions that every dispatch points to. They include the commit trailers and two rules: no pushing by subagents, and no `git checkout`/`switch`/`stash` in the repo, which once detached HEAD.
- **`shared-context.md`:** the plan's Global Constraints and sections A.1–A.4, plus controller notes.
- **`milestone-<M>-context.md`:** each milestone's intro, holding the pinned semantics. M4 and M5 carry CONTROLLER NOTE addenda (the F24 matrix mapping; the D3D9 pixel-shift and alpha-op rulings).
- **`global-constraints.md`:** for reviewers.
- **`task-*-brief.md` / `task-*-report.md`:** the brief and the full implementer report for every finished task, useful for the history of a decision.
- **`preflight.md`:** the same as the tracked preflight file.
- **`tick.sh N`:** marks Task N's checkboxes in the plan.

## Environment setup (cloud container)

These were installed during the run. Reinstall them after a container reset:

```bash
apt-get install -y libpulse-dev mesa-vulkan-drivers
```

```bash
rustup target add x86_64-pc-windows-msvc x86_64-apple-darwin
```

```bash
cargo install cargo-deny --locked
```

- **`mesa-vulkan-drivers`** provides lavapipe (`/usr/share/vulkan/icd.d/lvp_icd.json`), so md-render GPU tests run headless. Run them with `MD_REQUIRE_GPU=1` so they fail rather than skip.
- **The cross targets** let `cargo check/clippy -p md-audio --target …` validate the WASAPI and ScreenCaptureKit backends.
- **The gate before every commit:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check` and `scripts/check-no-native-build.sh`.
- **External downloads:** GitHub archives and the API are blocked by the egress proxy. crates.io works.

## Key cross-task facts a new controller must not lose

These are all in the ledger; this is the short list.

### Shaders, matrices and D3D9 emulation

- **Matrices (F24):** HLSL `floatRxC` maps to WGSL `matRxC`, stored transposed, so WGSL columns are HLSL rows. `mul(v,M)` becomes `M*v`. `md_hlsl::layout` is the single source of truth for every uniform offset: `MD_UNIFORMS_SIZE` is 1920 and `D3D9_PIXEL_SHIFT` is 0.5.
- **D3D9 pixel-centre emulation:** every built-in vertex shader shifts clip positions by +0.5 px using `VsParams` at @group(0) @binding(2). md-render builds all vertex data exactly as the C++ does, including its UV biases and the comp grid's −0.5 px offsets.
- **Fixed-function draws:** they use COLOROP MODULATE with ALPHAOP SELECTARG1 DIFFUSE. Sprite mode 4 uses full RGBA modulate (`fixed_tex_modulate_alpha`).
- **Composite:** the comp intermediate and all comp pipelines are Rgba8Unorm (8-bit saturation, as the C++ back buffer was). The warp stays on the main format, which is 16-bit float when `tex_bits` = 16.

### Textures and the shader cache

- **Texture lookup:** the C++ candidate loop has no `break`, so the LAST decodable file wins. The extension order is jpg, dds, png, tga, bmp, dib; for each extension the textures dir is tried, then the preset dir. Names match case-insensitively.
- **Missing textures** bind opaque BLACK, and the miss is logged once.
- **Shader cache:** keyed by sha2 over the inputs plus `md_hlsl::TRANSLATOR_FINGERPRINT`, a build-time hash of md-hlsl's sources and `assets/hlsl`.

### Renderer API for Milestone 7

- **FrameInput:** carries vars, old_vars, rot, old_rot, rand_preset, old_rand_preset, `now`, both preset start times, and `gpu`/`old_gpu`. A blend is running if `old_gpu` is Some. The window size lives only in the renderer (`resize`).
- **Warp mesh:** the renderer owns it. md-core calls `mesh_mut().compute_uvs(...)` before each `begin_frame`, using the ABSOLUTE clock for warp time.
- **Texture store:** call `TextureStore::preset_loaded()` after every preset load, and use one `RandPicks` per shader.

### Notes binding Milestone 6

- `darken_center` belongs in DrawSprites (6.5), not the composite.
- Blur texture sizes follow the C++ formula (1/4, 1/8, 1/16), and 6.1 must confirm this.
- After drawing into the main texture, regenerate its mips before the composite. When blur runs, the no-shader composite's sampler switches to clamp.
- MilkDrop's text manager strips DT_*_ELLIPSIS and DT_NOPREFIX, so 6.6 must do the same.
- The 2D default sampler state is MAG POINT with MIN/MIP LINEAR.

### Expression language and presets

- **Preset corpus:** `tests/presets/` is self-authored (no external pack was reachable). Run `MD_EXTRA_PRESETS=<dir>` against a real Winamp or projectM pack to find real-world gaps.
- **Expression semantics follow ns-eel2's real behaviour, not the plan text:** `^` is left-associative, a value of exactly 1e-5 counts as true, and assigning Inf/NaN stores 0 only through plain `=`.

## Owner-only verification still pending

- **Real audio capture** on a Linux desktop, Windows and macOS. Run:
  ```bash
  cargo test -p md-audio -- --include-ignored
  ```
  On Linux also run:
  ```bash
  cargo run -p md-audio --example list_devices
  ```
- **Windows reference captures** from the C++ build, following `tests/golden/windows/README.md`.
- **Corpus run against a real preset pack** using `MD_EXTRA_PRESETS`.

## Auto-resume

The hourly scheduled task "Resume MilkDrop3 Rust conversion" (trigger `trig_01FM3pmj24AwEMGRxXwKaV7q`) fired into the old session. It is now **disabled**, so it cannot run alongside the new agent. Re-enable it only if the old session should resume the work.

## Dispatching 5.8

Use the brief at `sdd-workspace/task-5.8-brief.md`. Binding notes:

- **App layout (F55):** add `apps/milkdrop/src/lib.rs` exposing `cli`, `render_cmd` and `headless`, and keep `main.rs` thin.
- **Audio (F51):** `WavSource::open(path, loop, fps)`.
- **Clock:** use FixedClock for both the absolute clock and the since-preset clock, so output is deterministic. Use a seeded Pcg32 for all randomness. Add a test that rendering twice gives byte-identical PNGs.
- **Driver scope:** the minimal driver uses only static preset values (`PerFrameVars::from_preset`) plus progress.
- **Golden image:** no Windows captures exist. Bless `tests/golden/rust/default_60.png` with `tests/golden/test_signal.wav` and inspect the PNG visually (agents can view images). Choose `max_dssim` to tolerate cross-device float differences.

## Session tooling added after the hand-off (container-local, not part of the port)

In the last session the owner asked for two Claude Code plugin marketplaces, which were cloned read-only and added. Neither outlives a container reset. To recreate them:

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 https://github.com/Mr-Molina/tokeneconomizer /home/user/mr-molina/tokeneconomizer
```

```bash
claude plugin marketplace add /home/user/mr-molina/tokeneconomizer
```

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 https://github.com/Mr-Molina/rea /home/user/mr-molina/rea
```

```bash
claude plugin marketplace add /home/user/mr-molina/rea
```

```bash
claude plugin install rea@rea
```

`tokeneconomizer` was added as a marketplace only; its plugin is not installed. `rea` 6.8.0 is installed at user scope, and its installed copy ships the prebuilt MCP server.

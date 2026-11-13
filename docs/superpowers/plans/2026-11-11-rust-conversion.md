# MilkDrop3 Rust Conversion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace every line of C and C++ in this repository with a Rust implementation of MilkDrop3 that renders the existing `.milk` preset corpus on Linux, macOS, and Windows with no foreign-language source in the tree.

**Architecture:** A Cargo workspace of small crates migrates the program inward from the edges: the expression language, the preset file model, sound analysis, and the HLSL-to-WGSL translator are pure CPU libraries with exhaustive tests; a wgpu renderer consumes them; a headless core orchestrates frames; a thin winit application wraps the core. The C++ tree stays in place as the reference until the last milestone deletes it.

**Tech Stack:** Rust 2021 (stable, MSRV pinned at milestone 0), wgpu + naga (graphics and WGSL validation), winit (windows and input), cpal / libpulse-binding / screencapturekit (audio capture), fontdue + fontdb (text), image + ddsfile (textures), glam (math), serde + toml (config), hound (WAV), clap, tracing, rand_pcg, dssim-core (image diff).

**Spec:** `docs/superpowers/specs/2026-11-11-cross-platform-port-plan.md` (dependency inventory and feature disposition), plus the two chat answers this plan grew from: the "universal deployment" variant and the Rust candidacy assessment. Where this plan and the spec disagree, this plan wins; it supersedes the C++ port.

## Global Constraints

- **No C or C++ source in the repository at the end of milestone 9.** `code/` is deleted. Crates that bind operating-system libraries (ALSA, PulseAudio, CoreAudio, WASAPI, X11, Wayland) are allowed because no foreign source enters the tree; crates that vendor and compile C or C++ (`*-sys` crates with a bundled source tree, `cc`-built dependencies) are forbidden. Enforce with `cargo deny` and a `cargo tree` check in CI (Task 0.3).
- **Source baseline is the 2023 `code/` snapshot** (MilkDrop 2.25c plus BeatDrop changes). Limits carried over exactly: `MAX_CUSTOM_WAVES = 4`, `MAX_CUSTOM_SHAPES = 4`, `NUM_Q_VAR = 32`, `NUM_WAVEFORM_SAMPLES = 480`, `NUM_FREQUENCIES = 512`, `SAMPLE_SIZE = 576`, `NUM_BLUR_TEX = 6`, `MAX_BIGSTRING_LEN = 32768`, default mesh `64 x 64`, default `fTimeBetweenPresets = 16.0`, `fTimeBetweenPresetsRand = 10.0`, `fBlendTimeAuto = 2.7`, `fBlendTimeUser = 1.7`, `fHardCutLoudnessThresh = 2.5`, `fHardCutHalflife = 60.0`, `max_fps = 60`. The 3.x features in the README are out of scope until the maintainer publishes that source; the crate boundaries leave room for them.
- **Preset compatibility is the product.** Every `.milk` file the C++ parses must parse; every expression it compiles must compile; every shader it compiles must translate and validate under naga. Corpus tests enforce this (Tasks 2.5, 1.9, 4.9).
- **Determinism for tests.** All randomness goes through `rand_pcg::Pcg32` seeded from a `u64`; no `rand::thread_rng()` outside `apps/`. Frame time in tests comes from a fixed-step clock.
- **Strings are UTF-8 `String`/`&str`; paths are `std::path::PathBuf`.** No `wchar_t` equivalents, no `MAX_PATH`.
- **Identifiers in the expression language are case-insensitive**, as in ns-eel2.
- **Rendering target is wgpu with WGSL only.** No GLSL, no SPIR-V passthrough, no HLSL at runtime except as translator input.
- **Licensing:** the result stays BSD 3-Clause; keep the Nullsoft and Maxim Volskiy notices in `NOTICE`. Every dependency must be MIT, Apache-2.0, BSD, Zlib, ISC, MPL-2.0, or Unicode licensed; `cargo deny` blocks anything else.

## Review Focus

Inputs the spec implies but no requirement names; each has a test pinned in the owning task.

1. **A preset whose shader references a texture file that does not exist** must render with the fallback texture and log once, not fail to load (Task 6.8 `missing_texture_binds_fallback`).
2. **An expression that loops forever** (`while(1)`) must be bounded, not hang the render thread (Task 1.6 `while_is_capped_at_1_048_576_iterations`).
3. **A `.milk` file with Windows line endings, a UTF-8 BOM, or trailing garbage after the `[preset00]` section** must parse identically to one without (Task 2.3 `parses_crlf_bom_and_trailing_junk`).
4. **Audio silence or a disconnected device** must keep rendering at full frame rate with zeroed waveform data, not stall (Task 3.6 `source_timeout_yields_silence_frame`).
5. **Window resize to zero or to a size larger than the GPU's maximum texture dimension** must clamp, not panic (Task 8.3 `resize_clamps_to_device_limits`).

---

## Part A: Decisions, layout, and interfaces

### A.1 Workspace layout

```
Cargo.toml                 workspace, shared [workspace.dependencies], lints
rust-toolchain.toml        pinned stable channel
deny.toml                  cargo-deny: licenses, bans, sources
crates/
  md-eval/                 EEL expression language: lexer, parser, compiler, VM
  md-preset/               .milk model, parser, writer, blending, defaults
  md-analysis/             FFT, band energies, attenuation (pure port of fft.cpp + AnalyzeNewSound)
  md-audio/                AudioSource trait, WAV source, silence source, OS loopback backends
  md-hlsl/                 HLSL subset -> WGSL translator, sampler/uniform conventions
  md-text/                 glyph atlas, text layout, DT_* flag emulation
  md-render/               wgpu renderer: resources, pipelines, all draw passes
  md-core/                 Visualizer: frame orchestration, preset management, UI state, commands
  md-platform/             config (TOML + milk2.ini import), paths, logging, key names
apps/
  milkdrop/                winit application; `run` and `render` subcommands
assets/
  shaders/                 WGSL for built-in passes (converted from data/*.fx)
  hlsl/include.fx          preamble pasted before preset shaders (HLSL text, data not code)
  fonts/                   bundled fallback font (OFL or similar)
tests/
  presets/                 corpus (see A.4)
  golden/                  reference PNGs and expression fixtures
  support/                 shared test helpers crate (md-testkit)
docs/
```

Every crate exposes one `lib.rs` with a short module list; files stay under about 600 lines, and a file that grows past that is split by responsibility in the task that touches it.

### A.2 Crate dependency graph

```
md-eval      <- md-preset <- md-core
md-analysis  <-------------- md-core
md-audio     <-------------- apps/milkdrop, md-core (trait only)
md-hlsl      <- md-render  <- md-core
md-text      <- md-render
md-preset    <- md-render   (Preset type only)
md-analysis  <- md-render   (SoundData type only)
md-platform  <-------------- apps/milkdrop, md-core (Config type)
md-core      <- apps/milkdrop
```

`md-core` never touches a window or an OS audio device. It takes an `&wgpu::Device`, an `&wgpu::Queue`, an output `wgpu::TextureView`, and 576-sample audio frames. That is what makes the headless `render` subcommand and the golden tests possible.

### A.3 Public interfaces (the names every task uses)

```rust
// md-eval
pub struct Vm;                              // variables, megabuf, shared gmegabuf
pub struct VarRef(u32);                     // index into Vm's variable table
pub struct Program;                         // compiled bytecode
pub struct GlobalMem;                       // gmegabuf, shared via Rc<RefCell<GlobalMem>>
impl Vm {
    pub fn new(global: Rc<RefCell<GlobalMem>>) -> Vm;
    pub fn var(&mut self, name: &str) -> VarRef;        // case-insensitive; creates at 0.0
    pub fn get(&self, v: VarRef) -> f64;
    pub fn set(&mut self, v: VarRef, x: f64);
    pub fn reset_vars(&mut self);                        // all variables to 0.0, megabuf freed
}
impl Program {
    pub fn compile(vm: &mut Vm, src: &str) -> Result<Program, CompileError>;
    pub fn run(&self, vm: &mut Vm) -> f64;              // value of last statement
}
pub struct CompileError { pub offset: usize, pub message: String }

// md-preset
pub struct Preset { /* every .milk key as a typed field, plus code blocks */ }
pub struct WaveData; pub struct ShapeData;   // 4 each in Preset
pub struct Blendable { /* start, end, start_time, end_time */ }
impl Blendable { pub fn eval(&self, time: f32) -> f32; }
pub fn parse_milk(text: &str) -> Result<Preset, ParseError>;
impl Preset {
    pub fn defaults() -> Preset;
    pub fn to_milk(&self) -> String;
    pub fn start_blend(&mut self, from: &Preset, now: f32, duration: f32);
    pub fn warp_ps_version(&self) -> PsVersion;  // None|Ps2_0|Ps2_X|Ps3_0|Ps4_0 from PSVERSION_WARP/PSVERSION
    pub fn comp_ps_version(&self) -> PsVersion;
}

// md-analysis
pub struct Analyzer;                        // owns FFT tables and running averages
pub struct SoundData { pub imm: [f32;3], pub imm_rel: [f32;3], pub avg: [f32;3], pub avg_rel: [f32;3],
                       pub long_avg: [f32;3], pub med_avg: [f32;3], pub spectrum: [[f32;512];2],
                       pub wave: [[f32;576];2] }
impl Analyzer {
    pub fn new() -> Analyzer;
    pub fn analyze(&mut self, left: &[u8;576], right: &[u8;576], fps: f32) -> &SoundData;
}

// md-audio
pub trait AudioSource: Send {
    fn next_frame(&mut self, left: &mut [u8;576], right: &mut [u8;576], timeout: Duration) -> FrameStatus;
}
pub enum FrameStatus { Data, Silence, Ended }
pub struct WavSource; pub struct SilenceSource;
pub fn default_loopback() -> Result<Box<dyn AudioSource>, AudioError>;   // per-OS backend
pub fn list_devices() -> Result<Vec<DeviceInfo>, AudioError>;

// md-hlsl
pub enum ShaderKind { Warp, Comp }
pub struct ShaderInput<'a> { pub kind: ShaderKind, pub body: &'a str, pub preamble: &'a str, pub ps_version: PsVersion }
pub struct SamplerBinding { pub name: String, pub root: String, pub slot: u32, pub wrap: bool, pub bilinear: bool, pub dims: u8 }
pub struct Translated { pub wgsl: String, pub samplers: Vec<SamplerBinding>, pub texsize_names: Vec<String> }
pub fn translate(input: &ShaderInput) -> Result<Translated, TranslateError>;
pub struct TranslateError { pub line: u32, pub col: u32, pub message: String }

// md-text
pub struct FontSpec { pub face: String, pub size_px: u32, pub bold: bool, pub italic: bool, pub antialiased: bool }
pub struct TextFlags { pub align: Align, pub single_line: bool, pub word_ellipsis: bool, pub no_prefix: bool }
pub struct Atlas;                            // one per FontSpec
pub struct Layout { pub quads: Vec<GlyphQuad>, pub bounds: Rect }
pub fn layout(atlas: &mut Atlas, text: &str, rect: Rect, flags: TextFlags) -> Layout;

// md-render
pub struct Renderer;
pub struct RenderSettings { pub tex_w: u32, pub tex_h: u32, pub grid_x: u32, pub grid_y: u32, pub max_ps: PsVersion, pub anisotropic: bool, pub tex_bits: u8 }
pub struct PresetGpu;                        // compiled warp+comp pipelines, bound textures, texsize uniforms
pub struct FrameInput<'a> { pub time: f32, pub frame: u32, pub fps: f32, pub progress: f32, pub sound: &'a SoundData,
                            pub preset: &'a Preset, pub old_preset: Option<&'a Preset>, pub blend_t: f32,
                            pub q: [f32;32], pub rand_frame: [f32;4], pub rand_preset: [f32;4], pub vert_blend: &'a [VertInfo] }
impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, settings: RenderSettings, seed: u64) -> Result<Renderer, RenderError>;
    pub fn load_preset(&mut self, preset: &Preset) -> Result<PresetGpu, RenderError>;   // uses the Renderer's own TextureStore
    pub fn begin_frame(&mut self, input: &FrameInput) -> FrameCtx;      // runs the passes in Task 5..6 order
    pub fn present(&mut self, ctx: FrameCtx, target: &wgpu::TextureView);
    pub fn resize(&mut self, w: u32, h: u32);
}
pub struct VertInfo { pub rad: f32, pub ang: f32, pub a: f32, pub c: f32 }   // blend mix = clamp(a*t + c, 0, 1)

// md-core
pub struct Visualizer;
pub enum Command { /* one variant per key command in Task 7.6 */ }
impl Visualizer {
    pub fn new(cfg: &md_platform::Config, device: &wgpu::Device, queue: &wgpu::Queue, seed: u64) -> Result<Visualizer, CoreError>;
    pub fn set_preset_dir(&mut self, dir: &Path) -> Result<(), CoreError>;
    pub fn tick(&mut self, left: &[u8;576], right: &[u8;576], now_secs: f64);  // sound analysis, timers, equations
    pub fn command(&mut self, cmd: Command);
    pub fn text_input(&mut self, ch: char);              // UI_LOAD search, UI_SAVEAS typing
    pub fn render(&mut self, target: &wgpu::TextureView);
    pub fn resize(&mut self, w: u32, h: u32);
}

// md-platform
pub struct Config { /* every settings.* key from milk2.ini as a typed field */ }
pub fn config_path() -> PathBuf;                         // XDG / Application Support / AppData
pub fn load_config() -> Result<Config, ConfigError>;     // TOML; missing file => Config::default()
pub fn import_milk2_ini(path: &Path) -> Result<Config, ConfigError>;
pub fn save_config(cfg: &Config) -> Result<(), ConfigError>;
```

### A.4 Decisions the owner must confirm before milestone 1 (defaults chosen so work can start)

| Decision | Default in this plan |
|---|---|
| Preset corpus for tests | The MilkDrop 2 preset pack that shipped with Winamp (BSD) plus the presets the maintainer distributes; stored under `tests/presets/` via Git LFS, or fetched by a CI step from a release asset if size is a concern. The corpus must contain at least one preset per `PSVERSION` value 0, 2, 3 and at least one each using `tex3D`, `tex2Dbias`, `GetBlur1..3`, `sampler_rand00..15`, `noisevol_hq`, and a user texture. |
| Reference images | Captured once on Windows from the C++ build at `tests/golden/windows/` for manual comparison; automated golden tests compare against the Rust renderer's own committed baselines (`tests/golden/rust/`), re-baselined deliberately with a `--bless` flag. Bit-exact parity with D3D9 is not a goal. |
| Minimum platforms | Linux x86_64 and aarch64 (X11 and Wayland), macOS 13+ (arm64, x86_64), Windows 10+ x64. |
| Audio on macOS | ScreenCaptureKit system audio (macOS 13+) through the `screencapturekit` crate; needs the Screen Recording permission. Documented. |
| Audio on Linux | PulseAudio protocol via `libpulse-binding` recording from `<default-sink>.monitor`; works on PipeWire through its PulseAudio server. ALSA-only systems get `SilenceSource` plus a warning. |
| Audio on Windows | cpal WASAPI loopback (verify the pinned cpal version exposes output devices as loopback inputs in Task 3.5; otherwise `windows` crate bindings in the same task). |
| Fonts | `fontdb` scans system fonts and matches the configured face; fallback is the bundled font in `assets/fonts/`. |
| Random source | `rand_pcg::Pcg32`; the C runtime's `rand()%7381` behavior is not reproduced. |
| Desktop mode, Winamp IPC, musikcube ABI, Explorer menus | Not ported. |

---

## Part B: Tasks by milestone

Each task ends with a commit on `rust-experiment`. Commands assume the repository root. "Run" lines give the command and the output that means pass.

## Milestone 0: workspace, CI, and guard rails

### Task 0.1: Workspace skeleton

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `NOTICE`, `crates/md-eval/Cargo.toml`, `crates/md-eval/src/lib.rs` (and the same pair for md-preset, md-analysis, md-audio, md-hlsl, md-text, md-render, md-core, md-platform), `apps/milkdrop/Cargo.toml`, `apps/milkdrop/src/main.rs`, `tests/support/md-testkit/{Cargo.toml,src/lib.rs}`

**Interfaces:**
- Produces: the crate names above, `[workspace.dependencies]` with exact pinned versions chosen at this step (wgpu, naga, winit, cpal, fontdue, fontdb, image, ddsfile, glam, serde, toml, hound, clap, tracing, tracing-subscriber, rand_pcg, rand_core, thiserror, anyhow, dssim-core, libpulse-binding, screencapturekit, dirs), `[workspace.lints]` with `unsafe_code = "forbid"` for every crate except `md-audio` (OS bindings).

- [x] **Step 1:** Write `Cargo.toml` with the ten members and pinned versions; each member `lib.rs` contains only `#![forbid(unsafe_code)]` (md-audio: `#![deny(unsafe_code)]`) and a doc comment naming its responsibility.
- [x] **Step 2:** Run `cargo build --workspace`. Expected: success with zero warnings.
- [x] **Step 3:** Write `NOTICE` carrying the Nullsoft 2005-2013 BSD text from `code/vis_milk2/dxcontext.h` and the Maxim Volskiy 2018 BSD text from `code/LICENSE.txt`.
- [x] **Step 4:** Commit: `chore: create Rust workspace skeleton`

### Task 0.2: Test kit and golden-image comparison

**Files:**
- Create: `tests/support/md-testkit/src/{lib.rs,image_diff.rs,fixed_clock.rs,corpus.rs}`
- Test: `tests/support/md-testkit/tests/image_diff.rs`

**Interfaces:**
- Produces: `pub fn compare_png(actual: &RgbaImage, golden_path: &Path, max_dssim: f64) -> DiffResult`, `pub struct FixedClock { pub fn new(fps: f64) -> Self; pub fn now(&self) -> f64; pub fn advance(&mut self) }`, `pub fn corpus_presets() -> Vec<PathBuf>` (every `*.milk` under `tests/presets/`), `pub fn bless_enabled() -> bool` (env `MD_BLESS=1` rewrites goldens).

- [x] **Step 1:** Write failing tests `identical_images_have_zero_dssim`, `shifted_by_one_pixel_is_under_0_01`, `missing_golden_fails_unless_bless`.
- [x] **Step 2:** Run `cargo test -p md-testkit`. Expected: FAIL, functions undefined.
- [x] **Step 3:** Implement with `dssim-core`; `DiffResult { dssim: f64, passed: bool, diff_png: Option<PathBuf> }` writes a diff image next to the golden on failure.
- [x] **Step 4:** Run `cargo test -p md-testkit`. Expected: PASS.
- [x] **Step 5:** Commit: `test: add image diff and fixed clock test kit`

### Task 0.3: CI and dependency policy

**Files:**
- Create: `.github/workflows/ci.yml`, `deny.toml`, `scripts/check-no-native-build.sh`

- [x] **Step 1:** Write `deny.toml`: allowed licenses from Global Constraints; `[bans]` denies `cc`, `cmake`, `bindgen` as build dependencies anywhere in the graph (`wildcards = "deny"` is not needed; use `deny = [{ name = "cc" }, { name = "cmake" }]` and `multiple-versions = "warn"`).
- [x] **Step 2:** Write `scripts/check-no-native-build.sh`: fails if `cargo tree -e build --workspace` lists `cc` or `cmake`, or if any file matching `*.c`, `*.cc`, `*.cpp`, `*.h`, `*.hpp` exists outside `code/` (the `code/` exemption is removed in Task 9.1).
- [x] **Step 3:** Write `ci.yml`: matrix ubuntu-latest, macos-latest, windows-latest; steps: toolchain from `rust-toolchain.toml`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, `scripts/check-no-native-build.sh`, `cargo test --workspace`. Ubuntu installs `mesa-vulkan-drivers` and sets `WGPU_ADAPTER_NAME=llvmpipe` for GPU tests (used from Milestone 5).
- [x] **Step 4:** Run the script locally: `scripts/check-no-native-build.sh`. Expected: exit 0.
- [x] **Step 5:** Commit: `ci: add matrix build, cargo-deny, and native-code guard`

### Task 0.4: Preset corpus and reference captures

**Files:**
- Create: `tests/presets/README.md`, `.gitattributes` (LFS for `tests/presets/**` and `tests/golden/**`), `tests/golden/windows/README.md`

- [x] **Step 1:** Add the corpus per A.4; record its origin and license in `tests/presets/README.md`.
- [x] **Step 2:** Write `tests/golden/windows/README.md` describing the capture procedure for the owner: C++ build, `--wav tests/golden/test_signal.wav` is not available in C++, so capture by playing the WAV through the system and screenshotting frames 1, 60, 300, 600 for each preset in `tests/golden/manifest.toml`; the 2023 build has no seed control, so these are visual references only.
- [x] **Step 3:** Generate `tests/golden/test_signal.wav`: 20 seconds, 44.1 kHz stereo, a 55 Hz sine at 0.8 amplitude pulsing at 2 Hz plus a 4 kHz sine at 0.2 amplitude pulsing at 0.5 Hz, written with `hound` by `tests/support/md-testkit/examples/gen_signal.rs`.
- [x] **Step 4:** Run `cargo run -p md-testkit --example gen_signal` and `md-testkit::corpus_presets().len() > 0` in a test. Expected: file exists; count printed.
- [x] **Step 5:** Commit: `test: add preset corpus, test signal, and capture instructions`

## Milestone 1: `md-eval`, the expression language

Semantics are those of ns-eel2 as built in `code/ns-eel2/` with `NSEEL_REENTRANT_EXECUTION` and without `NSEEL_EEL1_COMPAT_MODE`. Pinned facts: values are `f64`; identifiers are case-insensitive; `reg00`..`reg99` are 100 global registers shared by every `Vm`; truthiness is `|x| > 0.00001` (`NSEEL_CLOSEFACTOR`); equality is `|a-b| < 0.00001`; `rand(x)` floors `x`, clamps to at least 1, and returns a uniform value in `[0, x)` from a Mersenne Twister seeded with `0x4141f00d`; `megabuf(i)` and `gmegabuf(i)` address a sparse array of 128 blocks of 65536 `f64`, allocated on first write, index outside `[0, 8388608)` reads 0 and ignores writes; `loop(n, body)` iterates at most 1,048,576 times (`NSEEL_LOOPFUNC_SUPPORT_MAXLEN`); `while(body)` repeats while the body's value is true, with the same cap; statements are separated by `;`; comments are `//` to end of line and `/* */`; numbers accept `1`, `1.5`, `.5`, `1e3`, and `$x1F` hex; the value of a program is the value of its last expression.

### Task 1.1: Lexer

**Files:**
- Create: `crates/md-eval/src/lexer.rs`, `crates/md-eval/src/token.rs`
- Test: `crates/md-eval/src/lexer.rs` (unit tests module)

**Interfaces:**
- Produces: `pub enum Tok { Num(f64), Ident(String /* lowercased */), Op(&'static str), LParen, RParen, Comma, Semi, Eof }`, `pub struct Lexer; impl Lexer { pub fn tokenize(src: &str) -> Result<Vec<(Tok, usize)>, CompileError> }`; operators recognised: `+ - * / % ^ | & = += -= *= /= %= |= &= == != < > <= >= && || !` and the question/colon pair `? :`.

- [x] **Step 1:** Write failing tests `lexes_numbers_including_hex_and_leading_dot` (`.5`, `$x1F` -> 31.0, `1e3`), `lowercases_identifiers`, `skips_both_comment_styles`, `reports_offset_of_bad_char`.
- [x] **Step 2:** Run `cargo test -p md-eval lexer`. Expected: FAIL.
- [x] **Step 3:** Implement `Lexer::tokenize`.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(eval): lexer`

### Task 1.2: Parser to AST

**Files:**
- Create: `crates/md-eval/src/ast.rs`, `crates/md-eval/src/parser.rs`

**Interfaces:**
- Produces: `pub enum Expr { Num(f64), Var(String), Call { name: String, args: Vec<Expr> }, Unary(UnOp, Box<Expr>), Binary(BinOp, Box<Expr>, Box<Expr>), Assign(AssignOp, Box<Expr> /* lvalue: Var or Call megabuf/gmegabuf */, Box<Expr>), Ternary(Box<Expr>, Box<Expr>, Box<Expr>), Seq(Vec<Expr>) }`, `pub fn parse(tokens: &[(Tok, usize)]) -> Result<Expr, CompileError>`.
- Precedence, lowest to highest: `;` sequence, assignment (right-assoc), `? :`, `||`, `&&`, `|`, `&`, `== !=`, `< > <= >=`, `+ -`, `* / %`, `^` (right-assoc), unary `- ! +`, call/paren. This matches ns-eel2's yacc grammar in `nseel-eval.c`.
- Aliases resolved here to canonical calls: `if`, `bnot`, `assign`, `equal`, `below`, `above`, `megabuf`->`_mem`, `gmegabuf`->`_gmem`, `int`->`floor`, `abs`, `sqr`, `sqrt`, `pow`, `exp`, `log`, `log10`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `min`, `max`, `sign`, `floor`, `ceil`, `invsqrt`, `sigmoid`, `band`, `bor`, `rand`, `exec2`, `exec3`, `loop`, `while`, `memcpy`, `memset`, `freembuf`.

- [x] **Step 1:** Write failing tests `power_is_right_associative` (`2^3^2` -> `2^(3^2)`), `assignment_is_right_associative`, `ternary_binds_looser_than_or`, `megabuf_call_is_valid_lvalue` (`megabuf(3) = 1`), `unknown_function_is_error_with_offset`, `empty_statements_are_allowed` (`a=1;;b=2;`).
- [x] **Step 2:** Run `cargo test -p md-eval parser`. Expected: FAIL.
- [x] **Step 3:** Implement a Pratt parser in `parser.rs`.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(eval): parser`

### Task 1.3: Variable table and global memory

**Files:**
- Create: `crates/md-eval/src/vm.rs`, `crates/md-eval/src/mem.rs`

**Interfaces:**
- Produces: `Vm`, `VarRef`, `GlobalMem` as in A.3; `pub struct MegaBuf { blocks: Vec<Option<Box<[f64; 65536]>>> /* 128 */ } impl MegaBuf { pub fn get(&self, i: f64) -> f64; pub fn set(&mut self, i: f64, v: f64); pub fn free(&mut self, above_index: f64); pub fn copy(&mut self, dst: f64, src: f64, len: f64); pub fn fill(&mut self, dst: f64, v: f64, len: f64) }` with index = `floor(i + 0.00001)` as in `_asm_megabuf`.
- `reg00`..`reg99` live in `GlobalMem` as `[f64; 100]`; `Vm::var("reg07")` returns a `VarRef` that reads and writes the shared register.

- [x] **Step 1:** Write failing tests `var_lookup_is_case_insensitive`, `new_var_starts_at_zero`, `reg_vars_are_shared_between_vms`, `megabuf_out_of_range_reads_zero_and_ignores_writes`, `megabuf_index_rounds_toward_floor_with_epsilon` (`get(2.99999)` reads slot 3? No: `floor(2.99999 + 0.00001) = 3`; pin this), `reset_vars_zeroes_and_frees`.
- [x] **Step 2:** Run `cargo test -p md-eval vm`. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(eval): variable table and megabuf`

### Task 1.4: Bytecode compiler

**Files:**
- Create: `crates/md-eval/src/bytecode.rs`, `crates/md-eval/src/compiler.rs`

**Interfaces:**
- Produces: `pub enum Op { PushConst(f64), Load(VarRef), Store(VarRef), LoadMem(bool /* global */), StoreMem(bool), Pop, Dup, Add, Sub, Mul, Div, Mod, Pow, BitAnd, BitOr, Eq, Ne, Lt, Gt, Le, Ge, And, Or, Not, Neg, Call1(Fn1), Call2(Fn2), Jump(u32), JumpIfFalse(u32), Loop { body_start: u32, body_end: u32 }, While { body_start: u32, body_end: u32 } }`; `pub fn compile_ast(vm: &mut Vm, ast: &Expr) -> Result<Program, CompileError>`; `Program::compile` from A.3 composes lexer, parser, and this.
- `_if(c,a,b)`, `&&`, `||`, and `? :` compile to jumps so the untaken branch is not evaluated (ns-eel2 evaluates `_if` lazily; `_and`/`_or` likewise).
- Integer semantics: `%`, `&`, `|` convert both operands with truncation toward zero to `i32`; `%` by zero yields 0.

- [x] **Step 1:** Write failing tests `if_does_not_evaluate_untaken_branch` (`if(0, a=1, b=1)` leaves `a` at 0), `mod_by_zero_is_zero`, `bitops_truncate_operands` (`7.9 & 3.2` == 3), `loop_count_is_floored_and_capped`.
- [x] **Step 2:** Run `cargo test -p md-eval compiler`. Expected: FAIL.
- [x] **Step 3:** Implement the compiler and a stack VM `Program::run` in `vm.rs`.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(eval): bytecode compiler and VM`

### Task 1.5: Built-in functions

**Files:**
- Create: `crates/md-eval/src/builtins.rs`, `crates/md-eval/src/mt19937.rs`

**Interfaces:**
- Produces: `pub enum Fn1 { Sin, Cos, Tan, Asin, Acos, Atan, Sqrt, Sqr, Abs, Exp, Log, Log10, Floor, Ceil, Sign, Invsqrt, Rand, Freembuf }`, `pub enum Fn2 { Atan2, Pow, Min, Max, Sigmoid, Band, Bor, Exec2 }` plus `Fn3 { Exec3, Memcpy, Memset }`; `pub struct Mt19937; impl Mt19937 { pub fn seeded(seed: u32) -> Self; pub fn next_u32(&mut self) -> u32 }` living in `GlobalMem` (one generator shared by every `Vm`, as the C static is), seeded with `0x4141f00d`.
- Pinned formulas: `sigmoid(x, c) = 1 / (1 + exp(-x * c))` when `c` != 0 else `0`? No: ns-eel2's `eel1sigmoid` returns `t = 1 + exp(-x*c); t != 0 ? 1/t : 0`. `band(a,b)` = `(a truthy && b truthy) ? 1 : 0`; `bor` likewise with `||`; `sign(x)` = -1, 0, or 1; `invsqrt(x)` = `1/sqrt(x)`; `sqr(x)` = `x*x`; `exec2(a,b)` returns `b`; `exec3(a,b,c)` returns `c`; `memcpy(dst, src, len)` and `memset(dst, v, len)` return `dst`; `freembuf(i)` frees blocks above index `i` and returns 0.

- [x] **Step 1:** Write failing tests `mt19937_first_output_for_seed_0x4141f00d` (value computed by the reference MT19937 algorithm; record it in the test), `rand_floors_and_clamps_to_one`, `rand_is_below_max`, `sigmoid_matches_formula`, `sign_of_zero_is_zero`, `exec3_returns_third`.
- [x] **Step 2:** Run `cargo test -p md-eval builtins`. Expected: FAIL.
- [x] **Step 3:** Implement, including a from-spec MT19937 (the C code in `nseel-cfunc.c` lines 39-99 is the standard algorithm; implement from the published algorithm, not by translating the C).
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(eval): built-in functions and MT19937`

### Task 1.6: Loop bounds and error behavior

**Files:**
- Modify: `crates/md-eval/src/vm.rs`
- Test: `crates/md-eval/tests/bounds.rs`

- [x] **Step 1:** Write failing tests `while_is_capped_at_1_048_576_iterations` (`i=0; while(i=i+1)` finishes and `i` equals the cap), `loop_negative_count_runs_zero_times`, `division_by_zero_yields_inf_not_panic`, `nan_is_falsy`.
- [x] **Step 2:** Run `cargo test -p md-eval --test bounds`. Expected: FAIL.
- [x] **Step 3:** Implement caps; the cap constant is `pub const LOOP_CAP: u32 = 1_048_576` (`NSEEL_LOOPFUNC_SUPPORT_MAXLEN` in `code/ns-eel2/ns-eel.h`).
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(eval): loop caps and non-panicking arithmetic`

### Task 1.7: Fuzz and property tests

**Files:**
- Create: `crates/md-eval/fuzz/` (cargo-fuzz target `compile_and_run`), `crates/md-eval/tests/props.rs`

- [x] **Step 1:** Write a proptest in `props.rs`: for random ASTs printed back to source, `compile` succeeds and `run` terminates within 100 ms.
- [x] **Step 2:** Write the fuzz target: arbitrary bytes -> `Program::compile`; if it compiles, `run` with a 100 ms budget; never panics.
- [x] **Step 3:** Run `cargo test -p md-eval --test props` (PASS) and `cargo +nightly fuzz run compile_and_run -- -max_total_time=60` locally (no crash). CI runs only the proptest.
- [x] **Step 4:** Commit: `test(eval): property and fuzz tests`

### Task 1.8: Performance budget

**Files:**
- Create: `crates/md-eval/benches/per_pixel.rs` (criterion)

- [x] **Step 1:** Write a benchmark running a representative per-pixel program (`zoom = zoom + 0.1*sin(rad*6 + time); rot = rot + 0.02*ang; dx = dx + 0.001*cos(ang*3)` with `rad`, `ang`, `time` set) 4225 times.
- [x] **Step 2:** Run `cargo bench -p md-eval`. Expected: under 2 ms per 4225 evaluations on the CI machine class; if above, add a register-allocated fast path in `vm.rs` (superinstructions `LoadConstMul`, `StoreAddLoad`) before moving on.
- [x] **Step 3:** Commit: `perf(eval): benchmark and fast path`

### Task 1.9: Corpus compile test

**Files:**
- Create: `crates/md-eval/tests/corpus.rs`

- [x] **Step 1:** Write test `every_corpus_expression_compiles`: for each preset in `md_testkit::corpus_presets()`, extract the raw text of keys `per_frame_init_N`, `per_frame_N`, `per_pixel_N`, `wave_I_init_N`, `wave_I_per_frame_N`, `wave_I_per_point_N`, `shape_I_init_N`, `shape_I_per_frame_N` (a minimal regex scan, since `md-preset` does not exist yet), join lines with `\n`, and assert `Program::compile` succeeds; print the failing preset path and `CompileError` otherwise.
- [x] **Step 2:** Run `cargo test -p md-eval --test corpus`. Expected: PASS for 100% of presets. Each failure is a language-feature bug to fix in Tasks 1.1-1.5 before this task is closed.
- [x] **Step 3:** Commit: `test(eval): compile every corpus expression`

## Milestone 2: `md-preset`, the preset model

The `.milk` format is an INI file with a single `[preset00]` section, scalar keys, and numbered code lines. Keys pinned from `state.cpp`: scalars `MILKDROP_PRESET_VERSION`, `PSVERSION`, `PSVERSION_WARP`, `PSVERSION_COMP`, `fRating`, `fGammaAdj`, `fDecay`, `fVideoEchoZoom`, `fVideoEchoAlpha`, `nVideoEchoOrientation`, `nWaveMode`, `bAdditiveWaves`, `bWaveDots`, `bWaveThick`, `bModWaveAlphaByVolume`, `bMaximizeWaveColor`, `bTexWrap`, `bDarkenCenter`, `bRedBlueStereo`, `bBrighten`, `bDarken`, `bSolarize`, `bInvert`, `fWaveAlpha`, `fWaveScale`, `fWaveSmoothing`, `fWaveParam`, `fModWaveAlphaStart`, `fModWaveAlphaEnd`, `fWarpAnimSpeed`, `fWarpScale`, `fZoomExponent`, `fShader`, `zoom`, `rot`, `cx`, `cy`, `dx`, `dy`, `warp`, `sx`, `sy`, `wave_r`, `wave_g`, `wave_b`, `wave_x`, `wave_y`, `ob_size`, `ob_r`, `ob_g`, `ob_b`, `ob_a`, `ib_size`, `ib_r`, `ib_g`, `ib_b`, `ib_a`, `nMotionVectorsX`, `nMotionVectorsY`, `mv_dx`, `mv_dy`, `mv_l`, `mv_r`, `mv_g`, `mv_b`, `mv_a`, `b1n`, `b2n`, `b3n`, `b1x`, `b2x`, `b3x`, `b1ed`; per wave `i` in 0..4 with prefix `wavecode_i_`: `enabled`, `samples`, `sep`, `bSpectrum`, `bUseDots`, `bDrawThick`, `bAdditive`, `scaling`, `smoothing`, `r`, `g`, `b`, `a`; per shape `i` in 0..4 with prefix `shapecode_i_`: `enabled`, `sides`, `additive`, `thickOutline`, `textured`, `num_inst`, `x`, `y`, `rad`, `ang`, `tex_ang`, `tex_zoom`, `r`, `g`, `b`, `a`, `r2`, `g2`, `b2`, `a2`, `border_r`, `border_g`, `border_b`, `border_a`; code blocks numbered from 1 with prefixes `per_frame_init_`, `per_frame_`, `per_pixel_`, `wave_i_init`, `wave_i_per_frame`, `wave_i_per_point`, `shape_i_init`, `shape_i_per_frame`, `warp_`, `comp_`. A line beginning with a backtick has the backtick stripped. Defaults are those `CState::Default()` sets; the implementer reads them from `code/vis_milk2/state.cpp` and records each in `defaults.rs` with the C++ line number in a comment.

### Task 2.1: INI reader

**Files:**
- Create: `crates/md-preset/src/ini.rs`

**Interfaces:**
- Produces: `pub struct Ini { pub fn parse(text: &str) -> Ini; pub fn get(&self, section: &str, key: &str) -> Option<&str>; pub fn get_f32(&self, s: &str, k: &str, default: f32) -> f32; pub fn get_i32(..); pub fn get_bool(..); pub fn code_block(&self, section: &str, prefix: &str) -> String /* joins prefix1, prefix2, ... until the first missing number, with '\n' */ }`.
- Rules: case-insensitive section and key lookup; first occurrence wins; values are not trimmed of interior whitespace but leading and trailing whitespace is removed; a value may contain `=`; lines starting with `;` are comments; `\r\n` and a UTF-8 BOM are accepted.

- [x] **Step 1:** Write failing tests `first_duplicate_key_wins`, `value_may_contain_equals`, `code_block_stops_at_gap` (`per_frame_1`, `per_frame_2`, `per_frame_4` -> two lines), `backtick_prefix_is_stripped`, `parses_crlf_bom_and_trailing_junk`.
- [x] **Step 2:** Run `cargo test -p md-preset ini`. Expected: FAIL.
- [x] **Step 3:** Implement with a `HashMap<String, Vec<(String, String)>>` keyed by lowercased section.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(preset): INI reader`

### Task 2.2: Blendable values

**Files:**
- Create: `crates/md-preset/src/blend.rs`

**Interfaces:**
- Produces: `Blendable` as in A.3 with `pub fn set(&mut self, v: f32)` (no blend), `pub fn blend_to(&mut self, from: f32, to: f32, start: f32, end: f32)`, `pub fn eval(&self, time: f32) -> f32` returning `start + (end-start) * clamp((time-start_time)/(end_time-start_time), 0, 1)`, matching `CBlendableFloat::eval` in `state.cpp`.

- [x] **Step 1:** Write failing tests `eval_before_start_returns_start`, `eval_after_end_returns_end`, `eval_midway_is_linear`, `zero_duration_returns_end`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(preset): blendable floats`

### Task 2.3: Preset model and parser

**Files:**
- Create: `crates/md-preset/src/{preset.rs,wave.rs,shape.rs,defaults.rs,parse.rs}`

**Interfaces:**
- Produces: `Preset`, `WaveData`, `ShapeData`, `parse_milk`, `Preset::defaults`, `PsVersion` as in A.3; code blocks stored as `pub per_frame_init: String`, `pub per_frame: String`, `pub per_pixel: String`, `pub warp_shader: String`, `pub comp_shader: String`, `WaveData::{init, per_frame, per_point}`, `ShapeData::{init, per_frame}`; version rule: `PSVERSION` applies to both shaders unless `PSVERSION_WARP`/`PSVERSION_COMP` are present; `MILKDROP_PRESET_VERSION` below 200 means no shaders (`PsVersion::None`) regardless.
- The blendable preset fields are exactly those `CState` declares as `CBlendableFloat` in `code/vis_milk2/state.h`; list them in `preset.rs` with a comment and a test asserting the count matches the number of `CBlendableFloat` declarations recorded there.

- [x] **Step 1:** Write failing tests `defaults_match_cstate_default` (spot-check 12 values copied from `state.cpp`, e.g. `fDecay = 0.98`, `zoom = 1.0`, `nWaveMode = 0`, `fWaveAlpha = 0.8`), `parses_all_scalar_keys` (a fixture `tests/fixtures/all_keys.milk` setting every key to a distinct value, asserting each field), `parses_four_waves_and_shapes`, `ps_version_precedence`, `parses_crlf_bom_and_trailing_junk` (same fixture with CRLF and BOM gives an equal `Preset`), `missing_keys_keep_defaults`.
- [x] **Step 2:** Run `cargo test -p md-preset parse`. Expected: FAIL.
- [x] **Step 3:** Implement; `Preset` derives `Clone, Debug, PartialEq`.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(preset): model, defaults, and parser`

### Task 2.4: Writer and round trip

**Files:**
- Create: `crates/md-preset/src/write.rs`

**Interfaces:**
- Produces: `Preset::to_milk(&self) -> String` emitting `[preset00]`, then scalars in the order `CState::Export` writes them, floats with `%.3f` for most and `%.6f` for `zoom`, `rot`, `cx`, `cy`, `dx`, `dy`, `warp`, `sx`, `sy` (match `state.cpp` formats; record each in a comment), then code blocks as numbered lines; lines starting with `[` or `;` in code get a backtick prefix.

- [x] **Step 1:** Write failing tests `round_trip_preserves_every_field` (`parse_milk(p.to_milk()) == p` for the all-keys fixture), `code_line_starting_with_bracket_gets_backtick`, `corpus_round_trip_is_stable` (for every corpus preset, `parse(write(parse(x))) == parse(x)`).
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(preset): writer with round-trip tests`

### Task 2.5: Corpus parse test and preset blending

**Files:**
- Create: `crates/md-preset/tests/corpus.rs`, `crates/md-preset/src/transition.rs`

**Interfaces:**
- Produces: `Preset::start_blend(&mut self, from: &Preset, now: f32, duration: f32)` which, for each blendable field, calls `blend_to(from.value, self.value, now, now + duration)`; non-blendable fields switch immediately (as `CState::StartBlendFrom`).

- [x] **Step 1:** Write failing tests `every_corpus_preset_parses_without_error`, `blend_midpoint_is_average_for_blendable_fields`, `non_blendable_fields_switch_immediately`.
- [x] **Step 2:** Run `cargo test -p md-preset`. Expected: FAIL.
- [x] **Step 3:** Implement `transition.rs`.
- [x] **Step 4:** Run the tests. Expected: PASS, corpus at 100%.
- [x] **Step 5:** Commit: `feat(preset): blending and corpus parse test`

## Milestone 3: `md-analysis` and `md-audio`

Pinned facts from `fft.cpp`, `pluginshell.cpp::AnalyzeNewSound`, and `plugin.cpp::DoCustomSoundAnalysis`: input is 576 unsigned bytes per channel; each is converted with `((b ^ 128) - 128) as f32`, giving a signed waveform in `[-128, 127]`; the shell's FFT input is the two-tap average `0.5 * (w[i] + w[i-1])` (with `w[-1] = w[0]`), 576 samples in, 512 bins out, equalize on, envelope power 1.0; the envelope is `0.5 + 0.5 * sin(i * 2π/576 - π/2)`; the equalize table is `-0.02 * ln((512 - i) / 512)`; the bit-reversal and butterfly are the radix-2 FFT `fft.cpp` performs on `NFREQ = 1024` points with the 576 envelope-weighted input samples zero-padded, and the output magnitude scaling is whatever that code produces (the implementer confirms against `time_to_frequency_domain` and pins it in a test). The **preset-facing** values `bass`, `mid`, `treb`, `*_att` come from the plugin's second analysis (`mysound`): an unsmoothed FFT of the left channel only, 512 bins, bands summed over bins `[0,85)`, `[85,170)`, `[170,256)`, `avg` mixed with rate 0.2 (rising) or 0.5 (falling) adjusted from 30 fps, `long_avg` mixed with 0.9 for the first 50 frames then 0.992, `imm_rel = imm / long_avg` (1.0 when `|long_avg| < 0.001`), `avg_rel = avg / long_avg`. `AdjustRateToFPS(rate, fps1, fps)` is `rate ^ (fps1 / fps)`.

### Task 3.1: FFT

**Files:**
- Create: `crates/md-analysis/src/fft.rs`

**Interfaces:**
- Produces: `pub struct Fft; impl Fft { pub fn new(samples_in: usize, samples_out: usize, equalize: bool, envelope_power: f32) -> Fft; pub fn transform(&self, wave: &[f32], spectrum_out: &mut [f32]) }` with the same table construction as `fft.cpp` (implemented from the formulas above, not transliterated).

- [x] **Step 1:** Write failing tests `single_bin_sine_peaks_at_expected_bin` (a 44100/1024*k Hz sine of amplitude 100 over 576 samples, envelope off, equalize off, peaks at bin k), `envelope_table_endpoints_are_zero`, `equalize_table_first_entry_is_zero_and_last_is_positive`, `output_is_deterministic`.
- [x] **Step 2:** Run `cargo test -p md-analysis fft`. Expected: FAIL.
- [x] **Step 3:** Implement with precomputed bit-reversal and cos/sin tables; no external FFT crate (keeps the output shape identical to the C++ including its magnitude scaling).
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(analysis): FFT with envelope and equalize tables`

### Task 3.2: Band analysis

**Files:**
- Create: `crates/md-analysis/src/{analyzer.rs,lib.rs}`

**Interfaces:**
- Produces: `Analyzer`, `SoundData` as in A.3, where `imm`, `avg`, `long_avg`, `imm_rel`, `avg_rel` follow the `mysound` rules above, `spectrum` and `wave` carry the shell's values, and `pub fn adjust_rate_to_fps(rate: f32, fps1: f32, fps: f32) -> f32`.

- [x] **Step 1:** Write failing tests `silence_gives_rel_values_of_one` (after 100 frames of all-zero bytes, `imm_rel == [1,1,1]`), `bass_tone_raises_band_zero_only`, `long_avg_uses_0_9_for_first_50_frames`, `adjust_rate_to_fps_identity_at_reference_fps`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement; frame counter lives in `Analyzer`.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(analysis): band energies and attenuation`

### Task 3.3: `AudioSource` trait, WAV and silence sources

**Files:**
- Create: `crates/md-audio/src/{lib.rs,source.rs,wav.rs,silence.rs,resample.rs}`

**Interfaces:**
- Produces: `AudioSource`, `FrameStatus`, `WavSource::open(path: &Path, loop_playback: bool) -> Result<WavSource, AudioError>`, `SilenceSource`, and `pub fn to_frame(samples_interleaved: &[f32], channels: u16, sample_rate: u32, left: &mut [u8;576], right: &mut [u8;576])` which resamples to 44100 Hz by linear interpolation, takes the most recent 576 samples, converts `[-1,1]` to the signed 8-bit bit pattern `((x * 127.0).round() as i8) as u8` (silence is `0`, which is what `((b ^ 128) - 128)` in the analyzer maps back to `0`), and mixes mono to both channels.

- [x] **Step 1:** Write failing tests `wav_source_yields_576_samples_per_frame`, `to_frame_round_trips_through_analysis_conversion`, `mono_is_duplicated`, `48k_is_resampled_to_44k1`, `ended_after_last_frame_without_loop`.
- [x] **Step 2:** Run `cargo test -p md-audio`. Expected: FAIL.
- [x] **Step 3:** Implement with `hound`.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(audio): source trait, WAV and silence sources`

### Task 3.4: Linux loopback backend

**Files:**
- Create: `crates/md-audio/src/linux_pulse.rs`

**Interfaces:**
- Produces: `#[cfg(target_os = "linux")] pub fn default_loopback() -> Result<Box<dyn AudioSource>, AudioError>` opening a `libpulse_simple_binding::Simple` record stream on `"<default sink>.monitor"` (sink name read via `libpulse_binding` introspection) at 44100 Hz stereo f32; a background thread fills a ring buffer of 8192 frames; `next_frame` waits up to `timeout` on a condvar.
- `list_devices()` lists monitor sources with their descriptions; `Config.audio_device` selects by name.

- [x] **Step 1:** Write test `pulse_backend_falls_back_to_silence_when_no_server` (run with `PULSE_SERVER=unix:/nonexistent`; expect `AudioError::NoServer` and the caller substitutes `SilenceSource`).
- [x] **Step 2:** Run the test. Expected: FAIL.
- [x] **Step 3:** Implement; `unsafe` is not needed (the binding crate is safe).
- [x] **Step 4:** Run the test; manual check on a desktop: `cargo run -p milkdrop -- --list-audio-devices` prints the monitor source.
- [x] **Step 5:** Commit: `feat(audio): PulseAudio/PipeWire monitor capture`

### Task 3.5: Windows and macOS backends

**Files:**
- Create: `crates/md-audio/src/windows_wasapi.rs`, `crates/md-audio/src/macos_sck.rs`

**Interfaces:**
- Windows: `cpal` default output device opened as an input stream (loopback). If the pinned cpal version does not support it, implement with the `windows` crate (`IAudioClient::Initialize` with `AUDCLNT_STREAMFLAGS_LOOPBACK`) in the same file behind `#[allow(unsafe_code)]`; this is the one place `unsafe` is permitted and it is reviewed line by line.
- macOS: `screencapturekit` crate `SCStream` with `captures_audio = true`, audio-only, 44100 Hz stereo; permission denial returns `AudioError::PermissionDenied` with a message naming System Settings > Privacy > Screen Recording.

- [x] **Step 1:** Write test `backend_errors_are_mapped` (each backend's error type converts to `AudioError` with a user-facing message).
- [x] **Step 2:** Implement both backends; CI builds them but cannot capture; mark capture tests `#[ignore]` and run them manually.
- [x] **Step 3:** Run `cargo build -p md-audio` on all three OSes in CI. Expected: success.
- [x] **Step 4:** Commit: `feat(audio): WASAPI loopback and ScreenCaptureKit backends`

### Task 3.6: Timeout and silence behavior

**Files:**
- Modify: `crates/md-audio/src/source.rs`
- Test: `crates/md-audio/tests/timeout.rs`

- [x] **Step 1:** Write failing test `source_timeout_yields_silence_frame`: a `BlockingSource` test double never produces; `next_frame(.., 20ms)` returns `FrameStatus::Silence` within 25 ms and both buffers are all `0`.
- [x] **Step 2:** Run the test. Expected: FAIL.
- [x] **Step 3:** Implement the timeout contract in a `TimeoutSource<S>` wrapper used by the app.
- [x] **Step 4:** Run the test. Expected: PASS.
- [x] **Step 5:** Commit: `feat(audio): timeout wrapper yields silence`

## Milestone 4: `md-hlsl`, the shader translator

The translator accepts the shader language presets actually use: the HLSL subset that `fxc` compiled for `ps_2_0`, `ps_2_a`, `ps_2_b`, and `ps_3_0`, plus the preamble in `assets/hlsl/include.fx` and the wrapper the C++ builds. Input assembly, pinned from `plugin.cpp::LoadShaderFromMemory`: preamble, then type-specific defines (`rad`, `ang`, `uv`, `uv_orig`, and for comp `hue_shader`), then the body with `shader_body` replaced by a function `void PS(<params>)` where warp params are `float4 _vDiffuse : COLOR, float4 _uv : TEXCOORD0, float2 _rad_ang : TEXCOORD1, out float4 _return_value : COLOR0` and comp params use `float2 _uv`; the first statement inserted is `float3 ret = 0;` and the last is `_return_value = float4(ret.xyz, _vDiffuse.w);`. Samplers are declared by the preamble as `sampler2D`/`sampler3D` named `sampler_main`, `sampler_fc_main`, `sampler_pc_main`, `sampler_fw_main`, `sampler_pw_main`, `sampler_blur1..3`, `sampler_noise_lq`, `sampler_noise_lq_lite`, `sampler_noise_mq`, `sampler_noise_hq`, `sampler_noisevol_lq`, `sampler_noisevol_hq`, `sampler_rand00..15`; any other `sampler_XXX` the body declares refers to a user texture file `XXX`, with optional two-letter prefix `FW_`, `FC_`, `PW_`, `PC_` (and the reversed spellings `WF_`, `CF_`, `WP_`, `CP_`) selecting filter (F = bilinear, P = point) and address mode (W = wrap, C = clamp); `main` maps to the previous frame; `blur1..3` default to clamp. Uniforms: `_c0`..`_c13` (`float4`), `_qa`..`_qh` (`float4`), `rand_frame`, `rand_preset` (`float4`), 24 `float4x3` rotation matrices `rot_s1..4`, `rot_d1..4`, `rot_f1..4`, `rot_vf1..4`, `rot_uf1..4`, `rot_rand1..4`, and `texsize_XXX` (`float4`, `.xy` size, `.zw` reciprocal) for every bound texture including `texsize_main`, `texsize_blur1..3`, `texsize_noise_*`.

Output contract (consumed by Milestone 5): one WGSL module with `@fragment fn fs_main(in: VsOut) -> @location(0) vec4<f32>`, `struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) diffuse: vec4<f32>, @location(1) uv: vec4<f32>, @location(2) rad_ang: vec2<f32> }`, uniform block `@group(0) @binding(0) var<uniform> U: MdUniforms` with fields in the order listed above and each `float4x3` emitted as `mat4x3<f32>` whose columns are the HLSL matrix's columns, `@group(0) @binding(1) var<uniform> T: TexSizes` with one `vec4<f32>` per `texsize_*` name in `Translated.texsize_names` order, and per sampler slot `i` in 0..16 `@group(1) @binding(2*i) var tex_i: texture_2d<f32>` (or `texture_3d`) and `@group(1) @binding(2*i+1) var samp_i: sampler`. The module starts with `diagnostic(off, derivative_uniformity);` so `tex2D` inside `if` bodies validates.

### Task 4.1: Lexer and preprocessor

**Files:**
- Create: `crates/md-hlsl/src/{lexer.rs,preprocess.rs,token.rs}`

**Interfaces:**
- Produces: `pub fn preprocess(src: &str) -> Result<String, TranslateError>` handling `#define NAME value` and `#define NAME(args) body` (object-like and function-like, no `#if`), comment stripping, line continuation, and `//` inside strings not applicable; `pub fn tokenize(src: &str) -> Result<Vec<Token>, TranslateError>` with `Token { kind: TokKind, line: u32, col: u32 }`; `TokKind` covers identifiers, int and float literals (`1`, `1.0`, `1.f`, `.5`, `1e-3`, `0x1F`), all C operators HLSL uses including `?:`, `++`, `--`, compound assignment, and the punctuation `{}()[];,.`.

- [x] **Step 1:** Write failing tests `expands_object_and_function_macros`, `macro_from_include_fx_time_expands_to_c2_x`, `lexes_float_with_f_suffix`, `reports_line_and_col`.
- [x] **Step 2:** Run `cargo test -p md-hlsl lexer`. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(hlsl): preprocessor and lexer`

### Task 4.2: Parser and AST

**Files:**
- Create: `crates/md-hlsl/src/{ast.rs,parser.rs}`

**Interfaces:**
- Produces: `pub struct Module { pub decls: Vec<Decl> }`, `Decl::{Global(VarDecl), Function(FnDecl), Struct(StructDecl)}`, `Stmt::{VarDecl, Expr, If, For, While, DoWhile, Return, Block, Break, Continue, Discard}`, `Expr::{Lit, Ident, Index, Member /* also swizzle */, Call, Ctor(TypeName, Vec<Expr>), Unary, Binary, Assign, Ternary, Cast}`, `pub fn parse(tokens: &[Token]) -> Result<Module, TranslateError>`.
- Types parsed: `void bool int uint half float double`, vectors `TN` for N in 2..4, matrices `TRxC` for R,C in 1..4, `sampler sampler2D sampler3D samplerCUBE`, arrays `T name[N]`, qualifiers `const static uniform in out inout`, semantics `: NAME`, register annotations `: register(sN)` ignored.
- Known gap handled here: a missing `;` before `}` at the end of a `shader_body` is accepted (fxc tolerated it); any other missing `;` is an error.

- [x] **Step 1:** Write failing tests `parses_include_fx_without_error` (the real preamble), `parses_swizzle_chain` (`a.xyz.zyx.x`), `parses_matrix_type_float4x3`, `parses_for_with_decl`, `parses_ternary_nested`, `tolerates_missing_semicolon_before_closing_brace`.
- [x] **Step 2:** Run `cargo test -p md-hlsl parser`. Expected: FAIL.
- [x] **Step 3:** Implement a recursive-descent parser with C precedence.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(hlsl): parser`

### Task 4.3: Type checker with HLSL implicit conversions

**Files:**
- Create: `crates/md-hlsl/src/{types.rs,typeck.rs,intrinsics.rs}`

**Interfaces:**
- Produces: `pub enum Ty { Void, Bool, Int, Float, Vec(Scalar, u8), Mat(Scalar, u8 /*rows*/, u8 /*cols*/), Sampler2D, Sampler3D, SamplerCube, Array(Box<Ty>, u32), Struct(String) }`, `pub struct TypedModule` (AST annotated with `Ty` and with every implicit conversion made explicit as `Expr::Cast` or `Expr::Splat`), `pub fn check(m: &Module) -> Result<TypedModule, TranslateError>`.
- Conversion rules pinned (HLSL legacy behavior presets depend on): scalar to vector splat in arithmetic and assignment (`float3 ret = 0;`, `ret *= 0.5`); vector truncation to a shorter vector in assignment and call arguments with a warning collected in `TypedModule.warnings` (`float3 a = float4(...)`); `int` and `bool` promote to `float`; `float` to `int` truncates; mixed `int`/`float` arithmetic is `float`; matrix-vector `mul` overloads: `mul(vecR, matRxC) -> vecC`, `mul(matRxC, vecC) -> vecR`, `mul(scalar, x)`; swizzle write masks; `half` is `float`.
- Intrinsic table (name, overloads, result): `abs acos all any asin atan atan2 ceil clamp cos cosh cross ddx ddy degrees distance dot exp exp2 floor fmod frac frexp fwidth ldexp length lerp log log10 log2 max min modf mul normalize pow radians reflect refract round rsqrt saturate sign sin sincos sinh smoothstep sqrt step tan tanh tex2D tex2Dbias tex2Dlod tex2Dgrad tex2Dproj tex3D tex3Dlod texCUBE transpose trunc`; `tex2D(s, uv)`, `tex2Dbias(s, float4)`, `tex2Dlod(s, float4)`, `tex3D(s, float3)`; `GetBlur1(uv)`, `GetBlur2`, `GetBlur3`, `GetMain(uv)`, `GetPixel(uv)` come from the preamble as ordinary functions.

- [x] **Step 1:** Write failing tests `scalar_splats_into_vector_assignment`, `vector_truncation_warns_not_errors`, `mul_vec_mat_result_type`, `tex2d_returns_float4`, `unknown_identifier_errors_with_position`, `swizzle_write_mask_type`.
- [x] **Step 2:** Run `cargo test -p md-hlsl typeck`. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(hlsl): type checker with legacy implicit conversions`

### Task 4.4: WGSL emitter

**Files:**
- Create: `crates/md-hlsl/src/{emit.rs,emit_expr.rs,layout.rs}`

**Interfaces:**
- Produces: `pub fn emit(m: &TypedModule, kind: ShaderKind, bindings: &Bindings) -> String` where `Bindings { samplers: Vec<SamplerBinding>, texsize_names: Vec<String> }` comes from Task 4.5; the uniform struct layout in `layout.rs` is shared with `md-render` as `pub const MD_UNIFORMS_SIZE: u64 = 1920` and `pub fn md_uniform_offset(name: &str) -> Option<u64>` (`_c0` at 0, `_c13` at 208, `_qa` at 224, `_qh` at 336, `rand_frame` 352, `rand_preset` 368, `rot_s1` 384, each matrix 64 bytes, `rot_rand4` at 1856).
- Emission rules: `floatN` -> `vecN<f32>`, `floatRxC` -> `matCxR<f32>`, `mul(v, M)` -> `(v * M)`, `mul(M, v)` -> `(M * v)`, `tex2D(s_i, uv)` -> `textureSample(tex_i, samp_i, uv)`, `tex2Dbias(s, c)` -> `textureSampleBias(tex, samp, c.xy, c.w)`, `tex2Dlod(s, c)` -> `textureSampleLevel(tex, samp, c.xy, c.w)`, `tex3D` -> `textureSample` on `texture_3d`, `saturate(x)` -> `clamp(x, 0.0, 1.0)` (vector forms splat), `lerp` -> `mix`, `frac` -> `fract`, `fmod(a,b)` -> `(a - b * trunc(a / b))`, `atan2` -> `atan2`, `rsqrt` -> `inverseSqrt`, `ddx/ddy` -> `dpdx/dpdy`, `log10(x)` -> `(log(x) / log(10.0))`, `sincos(x, s, c)` -> two statements, `round` -> `round`, `step`, `smoothstep`, `sign` -> `sign`, `any/all` on vectors of bool, `discard` -> `discard`; integer literals used in float context get `.0`; `float` -> `f32`, `int` -> `i32`, `bool` -> `bool`; swizzles pass through; out parameters become return tuples only for `sincos` (the only out intrinsic) and `inout` user-function parameters become pointers `ptr<function, T>`; `static` globals become module-scope `var<private>`; constants become `const`.
- Naming: every HLSL identifier is prefixed `h_` to avoid WGSL reserved words.

- [x] **Step 1:** Write failing tests that assert exact emitted snippets for each rule above (one test per rule, 24 tests), plus `emitted_module_validates_with_naga` using `naga::front::wgsl::parse_str` and `naga::valid::Validator` on the default warp shader.
- [x] **Step 2:** Run `cargo test -p md-hlsl emit`. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(hlsl): WGSL emitter`

### Task 4.5: Sampler and uniform binding resolution

**Files:**
- Create: `crates/md-hlsl/src/bindings.rs`

**Interfaces:**
- Produces: `pub fn resolve_bindings(m: &TypedModule) -> Result<Bindings, TranslateError>` assigning slots to samplers in order of first declaration (preamble samplers that the body never references are not bound; limit 16, error beyond), decoding the prefix rules into `SamplerBinding { wrap, bilinear, root }`, and collecting `texsize_*` globals the body references into `texsize_names` in order of first reference.

- [x] **Step 1:** Write failing tests `prefix_fc_means_bilinear_clamp`, `reversed_prefix_cf_is_accepted`, `blur_defaults_to_clamp`, `unused_preamble_samplers_are_not_bound`, `seventeenth_sampler_is_error`, `texsize_main_is_collected`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(hlsl): sampler and texsize binding resolution`

### Task 4.6: Wrapper assembly and the public `translate`

**Files:**
- Create: `crates/md-hlsl/src/{wrap.rs,lib.rs}`, `assets/hlsl/include.fx` (copied from `code/resources/Milkdrop2/data/include.fx`)

**Interfaces:**
- Produces: `translate` as in A.3, composing preamble + defines + wrapped body, preprocess, lex, parse, check, bindings, emit, then naga validation; a naga validation failure is returned as `TranslateError` with the naga message, since it indicates a translator bug.

- [x] **Step 1:** Write failing tests `default_warp_shader_translates`, `default_comp_shader_translates`, `shader_body_missing_is_error`, `comp_defines_hue_shader`, `warp_uv_orig_is_zw_and_comp_uv_orig_is_xy`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(hlsl): wrapper assembly and translate entry point`

### Task 4.7: Legacy fixups

**Files:**
- Create: `crates/md-hlsl/src/fixups.rs`, `crates/md-hlsl/tests/fixtures/*.hlsl`

**Interfaces:**
- Produces: `pub fn apply_fixups(src: &str) -> String`, run before lexing, with one rule per known corpus idiom: trailing `;` after `}` of the body, `float2 x = float2(a);` single-arg vector constructors (splat), use of `uv_orig` in comp (alias of `uv`), `tex2D(sampler_main, uv).xyz;` where `ret` is `float3`, assignment of `float4` texture results to `ret` (`ret = tex2D(...)` -> truncation handled by Task 4.3), `#define` redefinition, `//` comments ending with `\` (not a continuation in fxc), and identifiers that are WGSL keywords.

- [x] **Step 1:** Write one test per fixup, each loading a fixture and asserting `translate` succeeds.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement; each fixup is a function with a doc comment citing an example preset from the corpus.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(hlsl): corpus fixups`

### Task 4.8: Built-in WGSL passes

**Files:**
- Create: `assets/shaders/{warp_vs.wgsl, comp_vs.wgsl, blur_vs.wgsl, blur1_ps.wgsl, blur2_ps.wgsl, fixed_color.wgsl, fixed_tex_modulate.wgsl, fixed_tex_select.wgsl, text.wgsl, title.wgsl}`
- Test: `crates/md-hlsl/tests/builtin_wgsl.rs`

**Interfaces:**
- Produces: hand-written WGSL for the eight `data/*.fx` files and for the fixed-function emulation: `fixed_color` (vertex color only), `fixed_tex_modulate` (texture × vertex color), `fixed_tex_select` (texture only); each with `VsIn { pos: vec3, color: vec4, uv: vec4, rad_ang: vec2 }` matching `MYVERTEX`, `WFVERTEX`, `SPRITEVERTEX` through three vertex layouts defined in Task 5.2. `blur1_ps` and `blur2_ps` take `_c0.._c3` from a small uniform block as the `.fx` comments describe.

- [x] **Step 1:** Write test `every_builtin_wgsl_validates` (naga parse and validate each file).
- [x] **Step 2:** Run the test. Expected: FAIL (files missing).
- [x] **Step 3:** Write the shaders, translating the `.fx` math by hand (the blur weights `w[8] = {4.0, 3.8, 3.5, 2.9, 1.9, 1.2, 0.7, 0.3}` and the `d1..d4`, `w_div` derivation from `blur1_ps.fx` comments are copied exactly).
- [x] **Step 4:** Run the test. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): built-in WGSL passes`

### Task 4.9: Corpus translation test and triage report

**Files:**
- Create: `crates/md-hlsl/tests/corpus.rs`, `crates/md-hlsl/examples/triage.rs`

- [x] **Step 1:** Write test `every_corpus_shader_translates_and_validates`: for each preset with `warp_shader` or `comp_shader` non-empty, `translate` succeeds; failures are collected and the test prints a table `preset | shader | line:col | message` before asserting the failure count is 0.
- [x] **Step 2:** Write `triage.rs`: same loop, groups failures by message prefix, prints counts descending. Run `cargo run -p md-hlsl --example triage`.
- [x] **Step 3:** Iterate on Tasks 4.3, 4.4, 4.7 until the corpus test passes at 100%. Each fix gets its own commit naming the preset that exposed it.
- [x] **Step 4:** Run `cargo test -p md-hlsl --test corpus`. Expected: PASS.
- [x] **Step 5:** Commit: `test(hlsl): corpus translation at 100%`

### Task 4.10: Translator fuzzing

**Files:**
- Create: `crates/md-hlsl/fuzz/` target `translate_never_panics`

- [x] **Step 1:** Write the target: arbitrary string -> `translate` for both kinds; must return `Ok` or `Err`, never panic, within 200 ms.
- [x] **Step 2:** Run `cargo +nightly fuzz run translate_never_panics -- -max_total_time=120` locally. Expected: no crashes; fix any found.
- [x] **Step 3:** Commit: `test(hlsl): fuzz target`

## Milestone 5: `md-text` and the `md-render` foundation

Pinned rendering facts from `milkdropfs.cpp` and `plugin.cpp`:

- **Main textures.** Two ping-pong color targets `VS[0]`, `VS[1]` of size `tex_w x tex_h`. Auto size (`tex_size = -1`): the window size; pow2 mode (`-2`): nearest power of two of the larger window dimension; both clamped to the device maximum, scaled by `100 / canvas_stretch`, then rounded up to a multiple of 16. Format is `Rgba8Unorm` for `tex_bits = 8`, `Rgba16Float` for 16 (the `nTexBitsPerCh` option), always with a full mip chain generated after the warp pass (the C++ relies on `D3DUSAGE_AUTOGENMIPMAP`; the comp shader samples `sampler_main` with mips and `_c12` carries `log2(width)`).
- **Warp mesh.** `(grid_x + 1) * (grid_y + 1)` vertices, max grid `192 x 144`; vertex `n = y * (grid_x + 1) + x` at `pos = (x / grid_x * 2 - 1, y / grid_y * 2 - 1)`, `rad` and `ang` per vertex are computed exactly as the mesh initialization block in `plugin.cpp` (`AllocateMyDX9Stuff`, the loop that fills `m_vertinfo[].rad/.ang`) does; the implementer copies that formula with the C++ line number in a comment; per-vertex UV is computed on the CPU each frame exactly as in `ComputeGridAlphaValues`: run the per-pixel program with `x = px * 0.5 * aspect_x + 0.5`, `y = py * -0.5 * aspect_y + 0.5`, `rad`, `ang`, and the per-frame values of `zoom zoomexp rot warp cx cy dx dy sx sy`; then `zoom2 = zoom ^ (zoomexp ^ (rad * 2 - 1))`, `u = px * aspect_x * 0.5 / zoom2 + 0.5`, `v = -py * aspect_y * 0.5 / zoom2 + 0.5`, `u = (u - cx) / sx + cx`, `v = (v - cy) / sy + cy`, the four warp terms with `f[0..4] = 11.68 + 4 cos(wt*1.413 + 10), 8.77 + 3 cos(wt*1.113 + 7), 10.54 + 3 cos(wt*1.233 + 3), 11.49 + 4 cos(wt*0.933 + 5)` where `wt = time * warp_anim_speed` and `ws = 1 / warp_scale`: `u += warp*0.0035*sin(wt*0.333 + ws*(px*f0 - py*f3))`, `v += warp*0.0035*cos(wt*0.375 - ws*(px*f2 + py*f1))`, `u += warp*0.0035*cos(wt*0.753 - ws*(px*f1 - py*f2))`, `v += warp*0.0035*sin(wt*0.825 + ws*(px*f0 + py*f3))`; rotation about `(cx, cy)` by `rot`; then `u -= dx`, `v -= dy`. During a blend the old preset's UVs are mixed with `mix2 = clamp(a * blend_t + c, 0, 1)` and the vertex alpha is `mix2`. Triangles are drawn as two index lists (left half, right half) so that each half's triangles share orientation; the implementer reproduces the index order from `plugin.cpp` where `m_indices_list` is built.
- **Blend patterns** (`RandomizeBlendPattern`): `mixtype = 1 + rand % 3`. Type 1 directional wipe: `ang = rand * 2π`, `band = 0.1 + 0.2 rand`, per vertex `t = ((fx - 0.5) cos ang + (fy - 0.5) sin ang + 0.5 - 0.5) / √2 + 0.5`, `a = (1 + band) / band`, `c = -1/band + t/band`. Type 2 plasma: `band = 0.12 + 0.13 rand`, corner seeds random, midpoint-displacement `GenPlasma(dt = 0.25)` normalized to `[0,1]` then the same `a`, `c`. Type 3 radial: `band = 0.02 + 0.14 rand + 0.34 rand`, `dir = ±1`, `t = sqrt(dx² + dy²) * 1.41421` from the center, `t = 1 - t` when `dir = -1`.
- **Blur chain.** Six textures: sizes halve at each level pair (`blur1` = textures 0,1 at ½, `blur2` = 2,3 at ¼, `blur3` = 4,5 at ⅛ of the main texture, each rounded to even), only `2 * highest_blur_used_this_frame` passes run; weights `w = [4.0, 3.8, 3.5, 2.9, 1.9, 1.2, 0.7, 0.3]`; pass A (horizontal, `blur1_ps`) constants `_c1 = (w0+w1, w2+w3, w4+w5, w6+w7)`, `_c2 = (0 + 2w1/c1.x, 2 + 2w3/c1.y, 4 + 2w5/c1.z, 6 + 2w7/c1.w)`, `_c3 = (scale, bias, 0.5/(sum c1), 0)`; pass B (vertical, `blur2_ps`) `_c1 = (w0+w1+w2+w3, w4+w5+w6+w7)`, `_c2 = (2(w2+w3)/c1.x, 2 + 2(w6+w7)/c1.y)`, `_c3 = (scale, bias, 1/(2(c1.x + c1.y)), 0)` plus `edge_darken = blur1_edge_darken` on the first level; `scale_i = 1 / (blur_max_i - blur_min_i)`, `bias_i = -blur_min_i * scale_i` with the min/max taken from `b1n b1x b2n b2x b3n b3x` after `GetSafeBlurMinMax` (which forces `max > min + 0.001` and keeps level `i+1` inside level `i`).
- **Shader constants** (`ApplyShaderParams`): `_c0 = (aspect_x, aspect_y, 1/aspect_x, 1/aspect_y)`, `_c1 = 0`, `_c2 = (time_since_preset_start wrapped at 10000 s, fps, frame, progress)`, `_c3 = (bass, mid, treb, 0.3333 * treb)` from `imm_rel` and `_c4` the same from `avg_rel` (the C++ in this repository writes `0.3333f*(a, b, c)` with the comma operator, so `.w` is a third of `treb`, not the average; reproduce it, since `include.fx` defines `vol` as `_c3.w` and `vol_att` as `_c4.w` and presets were authored against those values), `_c5 = (blur_max1 - blur_min1, blur_min1, blur_max2 - blur_min2, blur_min2)`, `_c6 = (blur_max3 - blur_min3, blur_min3, blur_min1, blur_max1)`, `_c7 = (tex_w, tex_h, 1/tex_w, 1/tex_h)`, `_c8 = 0.5 + 0.5 cos(time * (0.329, 1.293, 5.070, 20.051) + (1.2, 3.9, 2.5, 5.4))`, `_c9` the same with `sin`, `_c10 = 0.5 + 0.5 cos(time * (0.0050, 0.0085, 0.0133, 0.0217) + (2.7, 5.3, 4.5, 3.8))`, `_c11` with `sin`, `_c12 = (log2(w), log2(w), log2(w), 0)` (the C++ uses width for both; keep it), `_c13 = (blur_min2, blur_max2, blur_min3, blur_max3)`; `_qa.._qh` = `q1..q32` in groups of four; `rand_frame` four uniform randoms per frame; `rand_preset` four per preset; `rot_*` matrices `k = 0..19`: `rotX(base.x + speed.x * time) * rotY(..) * rotZ(..) * translate(xlate)` where at preset load `xlate = rand(-1,1)³`, `base = rand(0, 6.28)³`, `speed = rand(-1,1)³ * 0.9 * (k/8)^3.2`; `rot_rand1..4` (`k = 20..23`) are re-randomized every frame with the same formula at `time = 0`.
- **Composite pass.** A `32 x 24` vertex grid (`FCGSX`, `FCGSY`) covering the screen, each vertex's `diffuse` from four corner hues `shade[i] = normalize_max(0.6 + 0.3 sin(time*30*(0.0143, 0.0107, 0.0129) + (3, 1, 6) + i*(21, 13, 9) + rand_start))` remapped `0.5 + 0.5 x`, bilinearly interpolated; alpha from the warp mesh blend alphas during a transition; blend state `SrcAlpha/InvSrcAlpha` (or inverted on pass 0) when blending, none otherwise. The no-shader path (`ShowToUser_NoShaders`) draws the main texture with video echo (`echo_zoom`, `echo_alpha`, `echo_orient` flips), additive gamma passes (`int(gamma)` extra passes plus a fractional one), and `brighten`, `darken`, `solarize`, `invert`, `darken_center` as further full-screen passes with the blend modes listed in that function; the implementer reads each pass's `SRCBLEND/DESTBLEND` pair from the C++ and records it in a table in `comp_fixed.rs`.
- **Noise textures.** `noise_lq` 256² zoom 1, `noise_lq_lite` 32² zoom 1, `noise_mq` 256² zoom 4, `noise_hq` 256² zoom 8, `noisevol_lq` 32³ zoom 1, `noisevol_hq` 32³ zoom 4; each texel channel is `rand % RANGE + RANGE/2` with `RANGE = 216` when zoom > 1 else 256, then for zoom > 1 the grid is upsampled by `zoom` with bicubic-style smoothing as in `AddNoiseTex` (the implementer ports the smoothing loop and documents it); all have full mip chains.
- **Fixed-function emulation.** Every non-shader draw uses one of three pipelines (`fixed_color`, `fixed_tex_modulate`, `fixed_tex_select`) crossed with blend states `{Off, Alpha(SrcAlpha, InvSrcAlpha), Additive(One, One), Multiply(Zero, SrcColor), Replace(One, Zero), InvAlpha(InvSrcAlpha, SrcAlpha)}` and topology `{TriangleList, TriangleStrip, LineStrip, LineList, PointList}`. `TriangleFan` and `PointList` do not exist in wgpu: fans are converted to lists by the vertex builder; points become two-triangle quads of the configured point size.
- **D3D9 half-pixel convention.** D3D9 maps pixel centers to integer coordinates; wgpu uses half-integer centers. Full-screen passes that sample the main texture with UVs derived from vertex position apply a `(-0.5/w, -0.5/h)` texel offset in the vertex shader (`warp_vs`, `comp_vs`, `blur_vs`) so the feedback loop does not drift. This offset is a named constant `D3D9_HALF_TEXEL` in `layout.rs` and the golden tests in Task 6.9 are the check.

### Task 5.1: Font atlas and layout (`md-text`)

**Files:**
- Create: `crates/md-text/src/{lib.rs,atlas.rs,layout.rs,fonts.rs}`, `assets/fonts/<bundled>.ttf`

**Interfaces:**
- Produces: `Atlas::new(spec: &FontSpec, db: &fontdb::Database) -> Atlas` (falls back to the bundled font when the face is not found), `Atlas::texture_rgba(&self) -> (&[u8], u32, u32)` (dirty-tracked, grows by doubling to 2048 max), `layout` as in A.3 returning glyph quads in pixel space; `Align::{Left, Center, Right}`; `word_ellipsis` truncates at word boundaries and appends `...`; `single_line` ignores `\n`; `no_prefix` keeps `&` literal (otherwise `&&` -> `&` and a single `&` is dropped, as `DrawText` does); the returned `bounds` is what `DT_CALCRECT` would return.

- [x] **Step 1:** Write failing tests `calc_rect_height_equals_line_height_times_lines`, `right_align_ends_at_rect_right`, `word_ellipsis_truncates_at_word`, `ampersand_prefix_rules`, `missing_face_uses_bundled_font`.
- [x] **Step 2:** Run `cargo test -p md-text`. Expected: FAIL.
- [x] **Step 3:** Implement with `fontdue` rasterization and a shelf packer.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(text): glyph atlas and DrawText-compatible layout`

### Task 5.2: GPU context, vertex layouts, and resource types

**Files:**
- Create: `crates/md-render/src/{lib.rs,gpu.rs,vertex.rs,texture.rs,uniforms.rs,layout.rs}`

**Interfaces:**
- Produces: `pub struct Gpu<'a> { device: &'a wgpu::Device, queue: &'a wgpu::Queue }`; `#[repr(C)] pub struct Vertex { pos: [f32;3], color: [u8;4], uv: [f32;4], rad_ang: [f32;2] }` (one layout serves `MYVERTEX`, `WFVERTEX`, `SPRITEVERTEX`; unused fields are zero); `pub struct Texture2d { tex, view, w, h, mips }` with `Texture2d::new_target(gpu, w, h, format, mips)`, `Texture2d::from_rgba8(gpu, w, h, &[u8], mips)`, `Texture3d`; `pub struct MdUniforms` (`bytemuck::Pod`, 1920 bytes, field order per Task 4.4) and `pub fn pack_mat4x3(hlsl_rows: [[f32;3];4]) -> [[f32;4];4]` (columns padded); `pub struct DynamicVertexBuffer` with `write(&[Vertex]) -> wgpu::BufferSlice` ring-allocated per frame; `pub fn generate_mips(gpu, &Texture2d)` using a built-in blit pipeline.
- `pub fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)>` in `gpu.rs` for tests (`WGPU_ADAPTER_NAME` honored; returns `None` when no adapter so tests can `skip`).

- [x] **Step 1:** Write failing tests `uniform_struct_is_1920_bytes`, `mat4x3_packing_puts_hlsl_columns_in_wgsl_columns`, `headless_device_is_available_on_ci` (fails loudly in CI only, via env `MD_REQUIRE_GPU=1`), `mip_generation_halves_dimensions`.
- [x] **Step 2:** Run `cargo test -p md-render gpu`. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS (GPU tests skipped locally when no adapter).
- [x] **Step 5:** Commit: `feat(render): gpu context, vertex layout, uniforms`

### Task 5.3: Pipeline cache and fixed-function emulation

**Files:**
- Create: `crates/md-render/src/{pipelines.rs,fixed.rs,blend.rs}`

**Interfaces:**
- Produces: `pub enum FixedMode { Color, TexModulate, TexSelect }`, `pub enum BlendMode { Off, Alpha, Additive, Multiply, Replace, InvAlpha }`, `pub enum Topo { TriList, TriStrip, LineStrip, LineList, Points }`, `pub struct PipelineKey { mode, blend, topo, target_format }`, `pub struct PipelineCache { pub fn get(&mut self, gpu, key) -> &wgpu::RenderPipeline }`, `pub struct DrawBatch<'a> { pub fn draw(&mut self, key: PipelineKey, verts: &[Vertex], texture: Option<&Texture2d>, sampler: SamplerKind) }` (fan and point conversion happen here), `pub enum SamplerKind { LinearWrap, LinearClamp, PointWrap, PointClamp }`.

- [x] **Step 1:** Write failing GPU tests `color_triangle_renders_expected_pixels` (render a red triangle to a 64x64 target, read back, assert pixel (32,40) is red and (2,2) is black), `additive_blend_saturates`, `fan_conversion_produces_n_minus_2_triangles`, `points_become_quads`.
- [x] **Step 2:** Run `cargo test -p md-render pipelines`. Expected: FAIL.
- [x] **Step 3:** Implement using the WGSL from Task 4.8.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): pipeline cache and fixed-function emulation`

### Task 5.4: Texture store, image loading, and noise textures

**Files:**
- Create: `crates/md-render/src/{textures.rs,image_load.rs,noise.rs}`

**Interfaces:**
- Produces: `pub struct TextureStore { pub fn new(gpu, seed: u64) -> TextureStore; pub fn scan_dir(&mut self, dir: &Path); pub fn get(&mut self, gpu, root: &str) -> Option<&Texture2d> /* loads on demand; tries .jpg .png .tga .bmp .dds in that order as plugin.cpp does */; pub fn noise2d(&self, name: &str) -> &Texture2d; pub fn noise3d(&self, name: &str) -> &Texture3d; pub fn random_texture_names(&self) -> &[String]; pub fn size_of(&self, root: &str) -> Option<(u32,u32)> }`; `pub fn load_image(path: &Path) -> Result<RgbaImage, ImageError>` via `image` for jpg/png/tga/bmp and `ddsfile` for uncompressed DDS (BC-compressed DDS returns `ImageError::Unsupported`); budget `max_images = 3000`, `max_bytes = 2_000_000_000` with least-recently-used eviction as `m_nMaxImages`/`m_nMaxBytes`.
- `rand00..rand15` bind to 16 textures chosen at preset load from `random_texture_names()` with the preset's RNG (as `CacheParams` does for `sampler_randXX`); `texsize_randXX` follows the chosen file.

- [x] **Step 1:** Write failing tests `noise_lq_is_256_with_9_mips`, `noise_values_within_range_for_zoom` (zoom > 1 implies channel in `[108, 324)` clamped to 255 as the C++ does via `DWORD` overflow into the next byte: pin whichever behavior the C++ actually produces by reading `AddNoiseTex`; record it), `missing_file_returns_none`, `extension_search_order`, `lru_eviction_respects_budget`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): texture store, image loading, noise textures`

### Task 5.5: Preset shader pipelines

**Files:**
- Create: `crates/md-render/src/{preset_gpu.rs,shader_cache.rs}`

**Interfaces:**
- Produces: `PresetGpu { warp: Option<ShaderProgram>, comp: Option<ShaderProgram> }`, `ShaderProgram { pipeline, bind_group_1, texsize_values: Vec<[f32;4]>, samplers: Vec<SamplerBinding> }`; `Renderer::load_preset` as in A.3 which, for each of warp and comp with `PsVersion != None`, calls `md_hlsl::translate`, falls back to the built-in default shader (`warp_ps`/`comp_ps` bodies from the `.fx` files) on error while recording `RenderError::ShaderFallback { which, error }` in `Renderer::last_errors()`, creates the `wgpu::ShaderModule` and pipeline (target format of the main texture, no blending for pass 0, `Alpha` for blend pass 1), and resolves every `SamplerBinding` to a texture view: `main` -> the other ping-pong texture, `blur1..3` -> blur textures 1, 3, 5, noise names, `randXX`, else `TextureStore::get(root)` or the fallback 1x1 white texture.
- `ShaderCache`: `HashMap<u64 /* blake3 of assembled HLSL */, Arc<wgpu::ShaderModule>>` in memory plus an on-disk WGSL cache under `md_platform::cache_dir()/shaders/<hash>.wgsl` so translation runs once per preset per machine.

- [x] **Step 1:** Write failing GPU tests `default_warp_preset_builds_pipelines`, `shader_error_falls_back_and_records`, `sampler_main_binds_other_ping_pong_texture`, `cache_hit_skips_translation` (count translator calls via a test hook).
- [x] **Step 2:** Run `cargo test -p md-render preset_gpu`. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): preset shader pipelines and cache`

### Task 5.6: Warp mesh and warp pass

**Files:**
- Create: `crates/md-render/src/{mesh.rs,warp.rs,blendpattern.rs}`

**Interfaces:**
- Produces: `pub struct WarpMesh { pub fn new(grid_x, grid_y, aspect_x, aspect_y) -> WarpMesh; pub fn vert_info(&self) -> &[VertInfo]; pub fn compute_uvs(&mut self, per_frame: &PerFrameVars, per_pixel: Option<&mut PerPixelRunner>, time: f32, blend: Option<(&[VertInfo], f32)>) }` implementing the pinned math; `PerPixelRunner` is a closure type `FnMut(&mut PerPixelVars)` supplied by `md-core` (so `md-render` does not depend on `md-eval`); `pub fn randomize_blend_pattern(rng: &mut Pcg32, grid_x, grid_y, out: &mut [VertInfo])` implementing types 1..3 and `gen_plasma`; `Renderer::warp_pass(&mut self, ctx, preset_gpu, old_preset_gpu, mesh, blend_t)` drawing into `VS[0]` from `VS[1]` (pass 0 old shader, pass 1 new shader with alpha when blending), then generating mips.

- [x] **Step 1:** Write failing tests `uv_of_center_vertex_is_half_half_at_identity`, `zoom_scales_uv_about_center`, `dx_shifts_u_negative`, `rot_rotates_about_cx_cy`, `blend_mix_uses_a_t_plus_c`, `plasma_pattern_is_normalized_to_unit_range`, and GPU test `identity_warp_preserves_image` (upload a checkerboard, warp with identity, read back, DSSIM < 0.001).
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): warp mesh and warp pass`

### Task 5.7: Composite pass and headless frame

**Files:**
- Create: `crates/md-render/src/{comp.rs,comp_fixed.rs,renderer.rs}`

**Interfaces:**
- Produces: `Renderer::new`, `begin_frame`, `present`, `resize` as in A.3 with the frame order: clear `VS[0]` if the preset asked (`m_bNeedRescan`-style first frame), warp pass, blur passes (Task 6.1, stubbed as no-op here), shapes/waves/wave/sprites (Milestone 6, no-ops here), composite pass into `target` with the `32x24` hue grid, swap ping-pong. `comp_fixed.rs` holds the no-shader composite with the echo/gamma/brighten/darken/solarize/invert/darken_center passes and their blend table.

- [x] **Step 1:** Write failing GPU tests `two_frames_of_default_preset_produce_decaying_image` (frame 1 after painting white into VS: mean brightness of frame 2 is lower by about `0.004 * 255` per the default warp shader), `comp_grid_has_768_vertices`, `no_shader_path_applies_gamma_passes` (gamma 2.0 doubles brightness of a 0.25 grey image, clamped).
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): composite pass and frame loop`

### Task 5.8: `render` subcommand and first golden images

**Files:**
- Create: `apps/milkdrop/src/{main.rs,cli.rs,render_cmd.rs}`
- Test: `apps/milkdrop/tests/golden_basic.rs`

**Interfaces:**
- Produces: `milkdrop render --preset <file> --wav <file> --frames <n> --size WxH --seed <u64> --out <dir> [--every <k>]` writing `frame_<n>.png`; uses `md-preset`, `md-analysis`, `md-audio::WavSource`, and a minimal driver (per-frame equations via `md-eval` live in `md-core` from Milestone 7; until then the driver evaluates only the static preset values and `progress`, enough for shader-only presets).
- The driver is `apps/milkdrop/src/headless.rs` and is replaced by `md_core::Visualizer` in Task 7.9.

- [x] **Step 1:** Write failing test `golden_default_preset_frame_60` using `md_testkit::compare_png` against `tests/golden/rust/default_60.png` with `max_dssim = 0.02`; first run with `MD_BLESS=1` creates the golden, reviewer inspects it against `tests/golden/windows/`.
- [x] **Step 2:** Run the test. Expected: FAIL (no binary).
- [x] **Step 3:** Implement the subcommand.
- [x] **Step 4:** Run with bless, inspect, then run without. Expected: PASS.
- [x] **Step 5:** Commit: `feat(app): headless render subcommand and first golden`

## Milestone 6: full renderer

### Task 6.1: Blur chain

**Files:**
- Create: `crates/md-render/src/blur.rs`

**Interfaces:**
- Produces: `Renderer::blur_passes(&mut self, ctx, preset: &Preset, highest_used: u8)` per the pinned constants; `highest_used` comes from scanning the preset's shader text for `GetBlur3`/`sampler_blur3` (3), `blur2` (2), `blur1` (1), else 0, computed in `load_preset` and stored on `PresetGpu`.

- [x] **Step 1:** Write failing GPU tests `blur_texture_sizes_halve_per_level_and_are_even`, `blur_of_constant_image_is_constant_after_scale_bias`, `no_passes_when_unused`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): blur chain`

### Task 6.2: Built-in waveform

**Files:**
- Create: `crates/md-render/src/wave.rs`

**Interfaces:**
- Produces: `pub fn build_wave(mode: u32, sound: &SoundData, p: &WaveParams, aspect: (f32,f32), time: f32, out: &mut Vec<Vertex>) -> Topo` for modes 0..8 (circle, x-y spectrum, centered spiro, derivative, explosive hash, two spectrum lines, line, double line, spectrum line) with flags `256`, `512`, `1024`, `2048` as in `DrawWave`'s `switch`; `WaveParams { r, g, b, a, x, y, mystery, use_dots, thick, additive, maximize_color, mod_alpha_by_volume, mod_alpha_start, mod_alpha_end, scale, smoothing }`; alpha modulation by volume `alpha *= (vol - start) / (end - start)` clamped, `maximize_color` scales rgb so the max channel is 1; smoothing `wave[i] = wave[i] * (1 - s) + wave[i-1] * s` after scaling by `scale / 128`; `thick` draws four offset copies as the C++ `draw fat dots` loop; `use_dots` draws points.
- `Renderer::draw_wave(ctx, ...)` draws with `FixedMode::Color` and `BlendMode::Additive` when `additive` else `Alpha`.

- [x] **Step 1:** Write failing tests `mode_0_circle_vertex_count_is_samples_plus_one`, `mode_6_line_spans_width`, `alpha_modulation_clamps`, `maximize_color_scales_max_channel_to_one`, `thick_quadruples_vertices`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement from the C++ geometry per mode (the implementer transcribes each mode's vertex formulas into Rust with the C++ line reference in a comment).
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): built-in waveforms`

### Task 6.3: Custom shapes

**Files:**
- Create: `crates/md-render/src/shapes.rs`

**Interfaces:**
- Produces: `pub struct ShapeFrameVars { x, y, rad, ang, tex_ang, tex_zoom, r, g, b, a, r2, g2, b2, a2, border_r, border_g, border_b, border_a, sides, textured, additive, thick, instance, num_inst }` filled by `md-core` after running the shape's per-frame program for each instance; `pub fn build_shape(v: &ShapeFrameVars, aspect, out_fill: &mut Vec<Vertex>, out_border: &mut Vec<Vertex>)`: sides clamped to `[3, 100]`, center vertex color `(r,g,b,a)`, ring color `(r2,g2,b2,a2)`, ring vertex `j` at angle `j/sides * 2π + ang + π/4` scaled by `rad` and aspect; textured UVs `0.5 + 0.5 * cos/sin(t*2π + tex_ang + π/4) / tex_zoom * aspect_y`; border as a line strip with `thick` four-offset copies; fill drawn as a fan (converted) with `TexModulate` on `VS[1]` when textured else `Color`.

- [x] **Step 1:** Write failing tests `sides_clamped_to_100`, `fan_has_sides_plus_2_vertices`, `textured_uv_center_is_half_half`, `border_closes_the_loop`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): custom shapes`

### Task 6.4: Custom waves

**Files:**
- Create: `crates/md-render/src/custom_waves.rs`

**Interfaces:**
- Produces: `pub struct CustomWaveFrame { samples: u32 /* clamped to 512 */, sep, scaling, smoothing, r, g, b, a, spectrum: bool, use_dots, thick, additive }`, `pub fn custom_wave_inputs(f: &CustomWaveFrame, sound: &SoundData) -> Vec<(f32 /* value1 */, f32 /* value2 */, f32 /* sample 0..1 */)>` computing the smoothed, scaled source values: `mult = (spectrum ? 0.15 : 0.004) * scaling * wave_scale`, left/right offsets by `sep`, two-pass smoothing as the C++; then `md-core` runs the per-point program for each sample producing `(x, y, r, g, b, a)` and `build_custom_wave(points, use_dots, thick, out)`.

- [x] **Step 1:** Write failing tests `samples_clamped_to_512`, `spectrum_uses_0_15_multiplier`, `sep_offsets_right_channel`, `thick_quadruples`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): custom waves`

### Task 6.5: Motion vectors, borders, darken-center

**Files:**
- Create: `crates/md-render/src/overlays.rs`

**Interfaces:**
- Produces: `pub fn build_motion_vectors(mv: &MotionVectorVars, mesh: &WarpMesh, aspect, out: &mut Vec<Vertex>)` (grid of `nMotionVectorsX x nMotionVectorsY` lines, each traced backwards through the warp UVs with `ReversePropagatePoint`'s bilinear lookup, length `mv_l`, offset by `mv_dx`, `mv_dy`, color `mv_r/g/b/a`, clipped at the edges), `pub fn build_borders(ob: Border, ib: Border, out)` (outer border `ob_size` fraction of the screen, inner border `ib_size`), `pub fn build_darken_center(out)` (the center-darkening fan from the `darken_center` block in `milkdropfs.cpp`; vertex count, colors, and alphas copied from the C++ with a line reference).

- [x] **Step 1:** Write failing tests `mv_count_matches_grid`, `mv_zero_length_emits_nothing`, `outer_border_is_four_quads`, `darken_center_fan_vertex_count_matches_cpp`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): motion vectors, borders, darken center`

### Task 6.6: Text rendering pass

**Files:**
- Create: `crates/md-render/src/text_pass.rs`

**Interfaces:**
- Produces: `pub struct TextQueue { pub fn draw(&mut self, font: FontId, text: &str, rect: Rect, flags: TextFlags, color: [u8;4], dark_box: Option<[u8;4]>) -> Rect /* calc rect */; pub fn draw_box(&mut self, rect, color) }` and `Renderer::flush_text(ctx, &mut TextQueue)` which rasterizes queued items in order, boxes before text, with `CTextManager`'s behavior of caching: the queue is drawn to the screen each frame (the C++'s text-surface caching is an optimization the Rust version does not need); fonts are the six `FontSpec`s from config (`simple`, `decorative`, `help`, `playlist`, `extra1`, `extra2`) created through `md-text`.

- [x] **Step 1:** Write failing GPU test `text_pass_draws_visible_glyphs` (draw "MilkDrop" white on black, assert some pixels are non-black inside the calc rect and none outside), `dark_box_precedes_text`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): text pass`

### Task 6.7: Song title texture and animation

**Files:**
- Create: `crates/md-render/src/title.rs`

**Interfaces:**
- Produces: `Renderer::render_title_texture(&mut self, text: &str)` (renders the title with the `songtitle` font at double size into a 1024x256 (or device-limited) texture as `RenderStringToTitleTexture`), `Renderer::draw_title_anim(ctx, progress: f32, seed)` reproducing `ShowSongTitleAnim`: a `32x8`-ish mesh distorted by the per-preset random values over `progress`, drawn additively with the title texture; parameters copied from the C++ into constants with line references.

- [x] **Step 1:** Write failing GPU test `title_anim_at_progress_half_is_visible`.
- [x] **Step 2:** Run the test. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the test. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): song title texture and animation`

### Task 6.8: Image sprites

**Files:**
- Create: `crates/md-render/src/sprites.rs`

**Interfaces:**
- Produces: `pub struct SpriteFrame { x, y, sx, sy, repeatx, repeaty, rot, flipx, flipy, r, g, b, a, blendmode: u8, burn: bool, texture: TextureId }` (values clamped as `DrawUserSprites` does: `x,y` in `[-1000, 1000]` after `*2-1`, `sx,sy` in `[-1000, 1000]`, `repeat` in `[0.01, 100]`), `Renderer::draw_sprites(ctx, &[SpriteFrame])` with blend modes 0 = alpha, 1 = additive, 2 = multiply (`Zero, SrcColor`), 3 = `InvAlpha` ... pinned from the C++ `switch (blendmode)`; `burn` draws into `VS[0]` instead of the screen; the `colorkey` option becomes alpha at load time in `TextureStore`.

- [x] **Step 1:** Write failing tests `clamps_match_cpp`, `blendmode_table`, and GPU test `missing_texture_binds_fallback` (a `SpriteFrame` whose texture failed to load draws the 1x1 white fallback tinted by `r,g,b,a` and logs exactly once per name).
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(render): image sprites`

### Task 6.9: Renderer golden suite

**Files:**
- Create: `tests/golden/manifest.toml`, `apps/milkdrop/tests/golden_suite.rs`

- [x] **Step 1:** Write `manifest.toml`: 40 presets chosen per A.4 coverage, each with frames `[1, 60, 300]`, size `640x480`, seed `7`, `max_dssim` per entry (default `0.02`, `0.05` for blur-heavy presets).
- [x] **Step 2:** Write test `golden_suite` iterating the manifest through the `render` subcommand's library entry point (`render_cmd::render_to_images`) and `compare_png`.
- [x] **Step 3:** Run with `MD_BLESS=1`, inspect every image against the Windows captures, commit the goldens; run without bless. Expected: PASS.
- [x] **Step 4:** Commit: `test(render): golden suite over 40 presets`

## Milestone 7: `md-core`, the visualizer

Pinned behavior from `plugin.cpp` and `milkdropfs.cpp`:

- **Per-frame flow** (`RenderFrame`): advance time and fps; if `next_preset_time < 0` set it to `now + blend_time_auto + time_between_presets + time_between_presets_rand * rand`; if `now > next_preset_time` and no load in progress and not locked, `load_random_preset(blend_time_auto)`; hard cuts: when enabled and not locked, if `bass + mid + treb (imm_rel) > hard_cut_thresh * 3` then `load_random_preset(0)` and `hard_cut_thresh *= 2`, otherwise decay `thresh = (thresh - loudness_thresh) * exp(-1.3863 / (halflife * fps)) + loudness_thresh` with `thresh` starting at `2 * loudness_thresh`; random song titles and custom messages spawn by the Poisson-like `GetNumToSpawn` rule when their intervals are positive; `load_preset_tick` advances a pending load (tick 2 and 5 compile warp and comp shaders, tick 8 swaps states, randomizes the blend pattern, calls `start_blend`, resets preset timers); run per-frame equations; compute mesh UVs; render.
- **Per-frame equations** (`RunPerFrameEquations`): load `time fps frame progress bass mid treb bass_att mid_att treb_att meshx meshy pixelsx pixelsy aspectx aspecty monitor` and every preset scalar into the per-frame `Vm`; run `per_frame_init` once at preset load and capture `q1..q32` and `monitor`; each frame, set `q` to the captured init values, run `per_frame`, then copy `q` and the output scalars to the per-pixel `Vm`; clamp `echo_zoom` to `[0.001, 1000]`; when blending, mix the old and new states' output variables with `mix = cosine_interp(blend_progress)` for the comp-related variables listed at `milkdropfs.cpp` lines 700-750 (`zoom`, `rot`, `cx`, `cy`, `warp`, `sx`, `sy`, `dx`, `dy`, `decay`, `wave_*`, `ob_*`, `ib_*`, `mv_*`, `echo_*`, `gamma`, `b1n..b3x`, `blur1_edge_darken`, flags `wave_mode` and the boolean toggles switch at progress 0.5).
- **Progressive load**: `LoadPreset` with `blend_time > 0` parses into `new_state`, compiles its expressions immediately, and sets `loading = 1`; ticks run once per frame as above; a load started while one is in progress is ignored.
- **Preset list** (`UpdatePresetList`): directories first (marked `*`), then files sorted case-insensitively; `..` entry when not at the root; ratings read from each file's `fRating` (only when `bInstaScan` is on; otherwise progressively, `m_nRatingReadProgress` files per frame); random selection is uniform unless ratings are enabled and the cumulative rating sum is above 0.1, then by the rating CDF with a binary search; sequential mode steps `current + 1` wrapping to the first file; history ring of 66 entries supports Backspace (previous) and re-stepping forward.
- **UI modes**: `Regular, Menu, Load, LoadDel, LoadRename, SaveAs, SaveOverwrite, EditMenuString, ChangeDir, ImportWave, ExportWave, ImportShape, ExportShape, UpgradePixelShader, Mashup`; the menu tree is the one `menu.cpp` builds (bool, int, float, logfloat, blendable, logblendable, string, uimode item types) with `MilkMenu` navigation per the tooltip "ESC: exit, Left Arrow: back, Right Arrow: select, UP/DOWN: change sel".
- **Help text** is `code/vis_milk2/text1.bin` (ported verbatim to `assets/help.txt`).

### Task 7.1: State: per-frame and per-pixel evaluation contexts

**Files:**
- Create: `crates/md-core/src/{lib.rs,state.rs,vars.rs}`

**Interfaces:**
- Produces: `pub struct PresetState { pub preset: Preset, pub pf: Vm, pub pv: Vm, pub pf_program: Option<Program>, pf_init: Option<Program>, pv_program: Option<Program>, pub waves: [WaveState; 4], pub shapes: [ShapeState; 4], pub q_init: [f64; 32], pub monitor_init: f64, pub rand_preset: [f32;4], pub rot: [RotParams; 24], pub blend_progress: f32 }`, `PresetState::from_preset(preset: Preset, global: Rc<RefCell<GlobalMem>>, rng: &mut Pcg32) -> Result<PresetState, CoreError>` which registers every variable name from the pinned list in `vars.rs` (the `regvar` list captured from the C++: `time fps frame progress bass mid treb bass_att mid_att treb_att meshx meshy pixelsx pixelsy aspectx aspecty monitor zoom zoomexp rot warp cx cy dx dy sx sy decay wave_a wave_r wave_g wave_b wave_x wave_y wave_mystery wave_mode wave_usedots wave_thick wave_additive wave_brighten ob_size ob_r ob_g ob_b ob_a ib_size ib_r ib_g ib_b ib_a mv_x mv_y mv_dx mv_dy mv_l mv_r mv_g mv_b mv_a echo_zoom echo_alpha echo_orient darken_center wrap invert brighten darken solarize gamma blur1_min blur1_max blur2_min blur2_max blur3_min blur3_max blur1_edge_darken q1..q32` for per-frame; per-pixel adds `x y rad ang` and drops the output-only ones; waves add `sample value1 value2 x y r g b a`; shapes add `instance num_inst sides textured additive thick thickoutline x y rad ang tex_ang tex_zoom r g b a r2 g2 b2 a2 border_r border_g border_b border_a`), compiles all programs, and runs `per_frame_init`.

- [x] **Step 1:** Write failing tests `registers_all_variables` (every name in `vars.rs` resolves), `init_captures_q_and_monitor`, `compile_error_in_per_frame_is_reported_not_fatal` (state still loads, `pf_program` is `None`, error recorded).
- [x] **Step 2:** Run `cargo test -p md-core state`. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(core): preset state and evaluation contexts`

### Task 7.2: Per-frame equations and blending of outputs

**Files:**
- Create: `crates/md-core/src/{per_frame.rs,per_pixel.rs}`

**Interfaces:**
- Produces: `pub fn run_per_frame(state: &mut PresetState, inputs: &FrameInputs) -> PerFrameVars` (loads inputs, resets `q`, runs, clamps, copies to `pv`), `pub fn blend_outputs(new: &mut PerFrameVars, old: &PerFrameVars, progress: f32)`, `pub fn per_pixel_runner(state: &mut PresetState) -> impl FnMut(&mut PerPixelVars)` for `WarpMesh::compute_uvs`; `FrameInputs { time, fps, frame, progress, sound: &SoundData, mesh: (u32,u32), pixels: (u32,u32), aspect: (f32,f32) }`.

- [x] **Step 1:** Write failing tests `q_resets_to_init_each_frame`, `per_frame_can_modify_zoom`, `echo_zoom_is_clamped`, `blend_at_half_is_midpoint_for_floats`, `booleans_switch_at_half`.
- [x] **Step 2:** Run the tests. Expected: FAIL.
- [x] **Step 3:** Implement.
- [x] **Step 4:** Run the tests. Expected: PASS.
- [x] **Step 5:** Commit: `feat(core): per-frame equations and output blending`

### Task 7.3: Waves and shapes evaluation

**Files:**
- Create: `crates/md-core/src/{waves.rs,shapes.rs}`

**Interfaces:**
- Produces: `pub fn eval_custom_waves(state: &mut PresetState, sound: &SoundData, inputs: &FrameInputs) -> Vec<CustomWaveDraw>` (per enabled wave: run per-frame, compute inputs via `md_render::custom_wave_inputs`, run per-point for each sample, collect points) and `pub fn eval_custom_shapes(..) -> Vec<ShapeFrameVars>` (per enabled shape, per instance `0..num_inst`, run per-frame with `instance` set).

- [ ] **Step 1:** Write failing tests `disabled_wave_produces_nothing`, `per_point_sees_sample_in_unit_range`, `shape_instances_see_instance_index`, `shape_per_frame_can_change_color`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): custom wave and shape evaluation`

### Task 7.4: Preset library and selection

**Files:**
- Create: `crates/md-core/src/{library.rs,history.rs}`

**Interfaces:**
- Produces: `pub struct PresetLibrary { pub fn scan(dir: &Path, insta_scan_ratings: bool) -> PresetLibrary; pub fn entries(&self) -> &[Entry]; pub fn dir_count(&self) -> usize; pub fn pick_random(&self, rng: &mut Pcg32, use_ratings: bool) -> Option<usize>; pub fn next_sequential(&self, current: usize) -> usize; pub fn read_more_ratings(&mut self, n: usize); pub fn find_prefix(&self, typed: &str, from: usize) -> Option<usize> }`, `Entry { name: String, is_dir: bool, rating: f32, rating_cum: f32 }`; `pub struct History { ring of 66 } ` with `push`, `back() -> Option<&Path>`, `forward() -> Option<&Path>`.

- [ ] **Step 1:** Write failing tests `dirs_sort_before_files_and_are_starred`, `sort_is_case_insensitive`, `rating_cdf_prefers_high_ratings` (rating 5 vs 1, 10000 picks, ratio within 4.5..5.5), `uniform_when_ratings_disabled`, `sequential_wraps_past_dirs`, `history_back_then_forward_returns_same`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement with `std::fs::read_dir`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): preset library, selection, history`

### Task 7.5: Preset switching, blending, hard cuts, timers

**Files:**
- Create: `crates/md-core/src/{switcher.rs,timing.rs}`

**Interfaces:**
- Produces: `pub struct Switcher { current: PresetState, old: Option<PresetState>, pending: Option<(PresetState, f32 /* blend */, u8 /* tick */)>, blend_pattern: Vec<VertInfo>, next_preset_time: f32, hard_cut_thresh: f32, locked_by_user: bool, locked_by_code: bool }` with `load(&mut self, path, blend_time, now)`, `tick(&mut self, now, renderer: &mut Renderer)` (implements the 8-tick schedule and the shader compiles at ticks 2 and 5 through `Renderer::load_preset`), `update_timers(&mut self, now, fps, sound, cfg) -> Vec<SwitchEvent>` (auto-advance, hard cut), `blend_t(now) -> f32`, `progress(now) -> f32`.

- [ ] **Step 1:** Write failing tests `auto_advance_after_interval` (fixed clock, no audio), `hard_cut_triggers_and_doubles_threshold`, `threshold_decays_toward_loudness_thresh`, `load_during_pending_is_ignored`, `tick_8_swaps_and_starts_blend`, `locked_preset_never_advances`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): preset switching, blending, hard cuts`

### Task 7.6: Commands and key bindings

**Files:**
- Create: `crates/md-core/src/{commands.rs,keymap.rs}`

**Interfaces:**
- Produces: `pub enum Command { ToggleHelp, ToggleFullscreen, ToggleStretch, NextPresetSoft, NextPresetHard, PrevPreset, ToggleLock, ToggleRandomOrder, OpenLoadMenu, ToggleMenu, SaveAs, RandomMashup, ToggleShaderLocks, CycleMaxFps, CycleTimeBetweenPresets, TogglePresetName, ToggleFps, ToggleRating, ToggleAlwaysOnTop, TogglePause, ToggleSongTitle, ToggleDebugInfo, ShowShaderHelp, RateUp, RateDown, MenuUp, MenuDown, MenuLeft, MenuRight, MenuEscape, MenuEnter, PageUp, PageDown, Home, End, Delete, Backspace, LaunchCustomMessage(u8), LaunchSongTitle, KillSprites, KillTexts, LaunchSprite(u8), MashupSlot(u8), IncreaseValue, DecreaseValue, TypeChar(char) }` (one variant per key case found in `plugin.cpp` and `pluginshell.cpp` key handlers; the implementer maps each `case` to a variant and lists the mapping in `keymap.rs` as `pub const BINDINGS: &[(KeyChord, Command)]`), `pub struct KeyChord { key: Key, shift: bool, ctrl: bool, alt: bool }` where `Key` is a platform-neutral enum in `md-core` (winit mapping lives in the app).
- Every `Command` has a doc comment quoting the help text line from `assets/help.txt` or the C++ comment.

- [ ] **Step 1:** Write failing tests `every_help_text_key_has_a_binding` (a hand-written list in the test of the 24 key chords named in `assets/help.txt`, each asserted to appear in `BINDINGS`), `digits_launch_custom_messages`, `shift_digits_launch_sprites`, `bindings_are_unique`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): command enum and key bindings`

### Task 7.7: UI state machine, menus, overlays

**Files:**
- Create: `crates/md-core/src/ui/{mod.rs,mode.rs,menu.rs,menu_tree.rs,load_screen.rs,save_screen.rs,overlays.rs}`

**Interfaces:**
- Produces: `pub enum UiMode { .. }` (the 15 modes), `pub struct Ui { mode, menu: MenuTree, load: LoadScreen, save: SaveScreen, toasts: Vec<Toast>, show_fps, show_preset_name, show_rating, show_help, show_debug }`, `Ui::handle(&mut self, cmd: Command, ctx: &mut UiCtx) -> Vec<UiAction>` (`UiAction::{LoadPreset(PathBuf), SavePreset(PathBuf), SetValue(..), ChangeDir(PathBuf), ..}`), `Ui::draw(&self, q: &mut TextQueue, layout: &ScreenLayout)` emitting the same overlays the C++ draws: preset name lower-left, fps upper-right, rating, "Press F1 for help", help screen, the load screen with `MAX_PRESETS_PER_PAGE = 128` paging and type-to-search, the menu with tooltips and `[value]` editing, error toasts with durations, the debug info block.
- `MenuTree` is built from a declarative table in `menu_tree.rs` mirroring `menu.cpp`'s `AddChildMenu`/`AddItem` calls (each item: label, type, target field, min, max, step, tooltip string from the former `.rc` table).

- [ ] **Step 1:** Write failing tests `escape_from_menu_returns_to_regular`, `load_screen_search_jumps_to_prefix`, `menu_float_item_increments_by_step`, `menu_tree_item_count_matches_cpp` (count recorded from `menu.cpp`), `save_overwrite_requires_confirmation`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): UI modes, menus, overlays`

### Task 7.8: Custom messages and sprites

**Files:**
- Create: `crates/md-core/src/{messages.rs,sprites.rs}`

**Interfaces:**
- Produces: `pub struct CustomMessages { fonts: [MsgFont; 16], messages: [Option<CustomMessage>; 100] }` loaded from `milk2_msg.ini` (`[font00..15]` keys `face bold ital r g b`; `[message00..99]` keys `text font size x y randx randy growth time fade r g b`) with `launch(n, now)` and `active(&self, now) -> Vec<TextDraw>` implementing the grow/fade timeline; `pub struct Sprites { slots: [Option<SpriteSlot>; 16] }` loaded from `milk2_img.ini` (`[img00..99]` keys `img colorkey colorkey_lo colorkey_hi init_N per_frame_N`) with `launch(n, textures, now, frame)` (evicting the oldest slot by start frame when full) and `tick(&mut self, now, frame, sound) -> Vec<SpriteFrame>` running each sprite's per-frame program with the variables `time frame fps progress bass bass_att mid mid_att treb treb_att x y sx sy repeatx repeaty rot flipx flipy r g b a blendmode done burn`; a sprite whose `done` becomes true is killed.

- [ ] **Step 1:** Write failing tests `message_defaults_match_cpp` (size 50, x 0.5, y 0.5, growth 1.0, face "arial"), `message_fades_after_time`, `sprite_slot_eviction_picks_oldest`, `sprite_done_kills_it`, `sprite_init_runs_once`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): custom messages and sprites`

### Task 7.9: `Visualizer` facade and headless integration

**Files:**
- Create: `crates/md-core/src/visualizer.rs`
- Modify: `apps/milkdrop/src/render_cmd.rs` (use `Visualizer`; delete `headless.rs`)

**Interfaces:**
- Produces: `Visualizer` as in A.3, owning `Renderer`, `Switcher`, `PresetLibrary`, `Analyzer`, `Ui`, `CustomMessages`, `Sprites`, `TextQueue`, `Pcg32`; `tick` performs the pinned per-frame flow up to and including equations; `render` performs mesh UVs, draw passes in the order `warp, blur, motion vectors, shapes, waves, wave, sprites (burn), title anim, composite, sprites (screen), text`.

- [ ] **Step 1:** Write failing tests `headless_300_frames_of_corpus_sample_do_not_panic` (10 presets, fixed clock, WAV source), `render_subcommand_uses_visualizer` (golden `default_60` still passes).
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests and the Task 6.9 golden suite (re-bless entries whose change is explained by per-frame equations now running; reviewer approves each).
- [ ] **Step 5:** Commit: `feat(core): visualizer facade; headless renderer uses it`

### Task 7.10: Preset save, rating, and mash-up

**Files:**
- Create: `crates/md-core/src/{save.rs,mashup.rs}`

**Interfaces:**
- Produces: `Visualizer::save_preset_as(&mut self, name: &str, overwrite: bool) -> Result<PathBuf, CoreError>` (writes `to_milk()` of the current state's live values, as `SavePresetAs`), `set_rating(delta: i32)` (clamps 0..5, rewrites `fRating` in the file and the library entry), `random_mashup(rng)` (`MASH_SLOTS = 5`: picks a random preset per slot of `general, waves, shapes, warp shader, comp shader` and composes a new `Preset` as `A` key / `UI_MASHUP` does).

- [ ] **Step 1:** Write failing tests `save_writes_parseable_file`, `rating_is_clamped_and_persisted`, `mashup_takes_each_slot_from_a_corpus_preset`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): save, rating, mash-up`

## Milestone 8: `md-platform` and the desktop application

### Task 8.1: Config model, TOML persistence, `milk2.ini` import

**Files:**
- Create: `crates/md-platform/src/{lib.rs,config.rs,paths.rs,ini_import.rs,log.rs}`

**Interfaces:**
- Produces: `Config` with one typed field per `settings.*` key listed in the spec's inventory (`max_fps_w`, `max_fps_fs`, `start_fullscreen`, `always_on_top`, `fTimeBetweenPresets`, ... every key from the grep in the spec except the Direct3D adapter and dual-head keys, which are dropped), six `FontSpec`s, `preset_dir`, `audio_device: Option<String>`, `tex_size`, `tex_bits`, `grid`, `canvas_stretch`, `max_ps_version`; `config_path()` = `dirs::config_dir()/milkdrop3/config.toml`; `cache_dir()`; `data_dir()` (presets, textures, `milk2_msg.ini`, `milk2_img.ini` default location; first run copies nothing and points at the executable's directory if a `presets/` folder exists there, matching the portable layout); `import_milk2_ini` maps every key by name.

- [ ] **Step 1:** Write failing tests `default_config_matches_cpp_defaults`, `toml_round_trip`, `ini_import_maps_known_keys` (fixture `milk2.ini` with 20 keys), `missing_config_yields_defaults`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement; logging via `tracing-subscriber` to stderr and to `cache_dir()/milkdrop.log`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(platform): config, paths, ini import, logging`

### Task 8.2: Window, surface, event loop

**Files:**
- Create: `apps/milkdrop/src/{app.rs,window.rs,keys.rs}`

**Interfaces:**
- Produces: `winit` `ApplicationHandler` creating one window with the icon from `assets/icon.png`, a `wgpu::Surface` with `PresentMode::AutoVsync` (or `Immediate` when `allow_page_tearing`), the render loop driven by `RedrawRequested` with a frame limiter to `max_fps` using `std::thread::sleep` plus a short spin as the C++ `EnforceMaxFPS`; `keys.rs` maps `winit::keyboard::Key` and modifiers to `md_core::KeyChord` and routes text input (`Key::Character`) to `Command::TypeChar` when the UI is in a typing mode.

- [ ] **Step 1:** Write failing tests `winit_key_to_chord_mapping_covers_all_bindings` (every `KeyChord` in `md_core::BINDINGS` is producible from some winit key), `shift_a_and_a_are_distinct`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement; manual check: `cargo run -p milkdrop` shows the default preset and reacts to keys.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): window, surface, event loop, key mapping`

### Task 8.3: Fullscreen, stretch, topmost, resize

**Files:**
- Create: `apps/milkdrop/src/display.rs`

**Interfaces:**
- Produces: `toggle_fullscreen` (borderless on the current monitor), `toggle_stretch` (window positioned over the union of all monitors, decorations off), `set_always_on_top`, `on_resize(w, h)` -> `Visualizer::resize` with clamping to `[1, device.limits().max_texture_dimension_2d]`.

- [ ] **Step 1:** Write failing test `resize_clamps_to_device_limits` (headless: `resize(0, 0)` becomes `(1, 1)`; `resize(1 << 20, 10)` clamps to the limit; no panic, a frame still renders).
- [ ] **Step 2:** Run the test. Expected: FAIL.
- [ ] **Step 3:** Implement; manual check on a two-monitor Linux desktop and on macOS.
- [ ] **Step 4:** Run the test. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): fullscreen, multi-monitor stretch, topmost`

### Task 8.4: Audio wiring and device selection

**Files:**
- Create: `apps/milkdrop/src/audio.rs`

**Interfaces:**
- Produces: `select_source(cfg: &Config, cli: &Cli) -> Box<dyn AudioSource>`: `--wav` > `--audio-device <name>` > `cfg.audio_device` > `default_loopback()` > `SilenceSource` with a visible toast "No audio source: <error>"; `--list-audio-devices` prints `list_devices()` and exits; the source is wrapped in `TimeoutSource` with a 50 ms timeout.

- [ ] **Step 1:** Write failing test `selection_precedence`.
- [ ] **Step 2:** Run the test. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the test. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): audio source selection`

### Task 8.5: Media keys and track title

**Files:**
- Create: `apps/milkdrop/src/media/{mod.rs,mpris.rs,macos.rs,windows.rs}`

**Interfaces:**
- Produces: `pub trait MediaControl { fn next(&self); fn previous(&self); fn now_playing(&self) -> Option<String> }`; Linux via `zbus` MPRIS (`org.mpris.MediaPlayer2.Player` on the first active player), macOS via `objc2` `MPRemoteCommandCenter`/`MPNowPlayingInfoCenter` bindings (pure Rust objc2 crates), Windows via the `windows` crate `GlobalSystemMediaTransportControlsSessionManager`; `Ctrl+Right/Left` map to `next/previous`; the title feeds `Visualizer::set_track_title`.

- [ ] **Step 1:** Write failing test `title_change_triggers_song_title_anim_when_enabled` (headless, fake `MediaControl`).
- [ ] **Step 2:** Run the test. Expected: FAIL.
- [ ] **Step 3:** Implement; each backend is `#[cfg(target_os)]`-gated and returns a `NullMedia` when unavailable.
- [ ] **Step 4:** Run the test. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(app): media keys and now-playing title`

### Task 8.6: Packaging and release artifacts

**Files:**
- Create: `.github/workflows/release.yml`, `packaging/linux/{milkdrop3.desktop,AppImageBuilder.yml}`, `packaging/flatpak/org.milkdrop3.MilkDrop3.yml`, `packaging/macos/{Info.plist,bundle.sh}`, `packaging/windows/installer.nsi`

- [ ] **Step 1:** Write `release.yml` on tag `v*`: build release binaries on the matrix, run the golden suite on Linux (llvmpipe), produce AppImage, `.app` zip, and the NSIS installer; attach to the release.
- [ ] **Step 2:** Run the workflow on a `v0.0.1-rc1` tag. Expected: three artifacts attached.
- [ ] **Step 3:** Commit: `ci: release packaging for linux, macos, windows`

## Milestone 9: removal of the C++ tree and documentation

### Task 9.1: Delete `code/`, migrate assets, close the guard

**Files:**
- Delete: `code/` (entire tree), `linux/milkdrop3-launcher.sh`
- Move: `code/resources/Milkdrop2/docs/*` -> `docs/milkdrop2/`, `code/vis_milk2/text1.bin` -> `assets/help.txt` (converted to LF), `code/vis_milk2/plugin_icon.ico` -> `assets/icon.png` (converted), `code/resources/Milkdrop2/data/include.fx` already at `assets/hlsl/include.fx`
- Modify: `scripts/check-no-native-build.sh` (remove the `code/` exemption), `.github/workflows/ci.yml`

- [ ] **Step 1:** Grep for any remaining references to `code/` in the workspace, tests, and docs; fix them.
- [ ] **Step 2:** Run `scripts/check-no-native-build.sh` and `cargo test --workspace`. Expected: exit 0, all tests pass, `find . -name '*.c' -o -name '*.cpp' -o -name '*.h'` prints nothing.
- [ ] **Step 3:** Commit: `chore: remove C++ sources; the repository is Rust only`

### Task 9.2: Documentation

**Files:**
- Modify: `README.md` (build, run, audio setup per OS, keyboard reference from `assets/help.txt`, "what is and is not ported" table from the spec's feature disposition, note on the 2023 source baseline), `linux/README.md` (replace Wine guide with native instructions and a legacy appendix), `docs/ARCHITECTURE.md` (crate map from A.1 and A.2, the frame flow, the shader pipeline), `CONTRIBUTING.md` (how to add a preset to the golden suite, how to bless, how to run the triage example)

- [ ] **Step 1:** Write the documents.
- [ ] **Step 2:** Run `cargo doc --workspace --no-deps` with `#![deny(missing_docs)]` on every public crate. Expected: success.
- [ ] **Step 3:** Commit: `docs: README, architecture, contributing for the Rust tree`

### Task 9.3: Final verification

- [ ] **Step 1:** Run on each OS: `cargo run -p milkdrop --release` for ten minutes with system audio and a 500-preset directory; record crashes and shader fallbacks from the log.
- [ ] **Step 2:** Run `cargo run -p md-hlsl --example triage` and `cargo test --workspace --release`. Expected: zero translator failures, all tests pass.
- [ ] **Step 3:** Tag `v0.1.0` and run the release workflow.
- [ ] **Step 4:** Commit any fixes found with messages naming the preset or OS.

---

## Appendix: effort and sequencing

| Milestone | Tasks | Estimate (one engineer) | Depends on |
|---|---|---|---|
| 0 Workspace and guard rails | 4 | 1 week | owner decisions in A.4 |
| 1 `md-eval` | 9 | 3 weeks | 0 |
| 2 `md-preset` | 5 | 2 weeks | 1 |
| 3 `md-analysis`, `md-audio` | 6 | 2 to 3 weeks | 0 |
| 4 `md-hlsl` | 10 | 6 to 8 weeks | 0 (corpus) |
| 5 render foundation | 8 | 5 to 6 weeks | 2, 4 |
| 6 full renderer | 9 | 5 to 6 weeks | 5 |
| 7 `md-core` | 10 | 5 to 6 weeks | 1, 2, 3, 6 |
| 8 app and platform | 6 | 3 to 4 weeks | 7 |
| 9 removal and docs | 3 | 1 week | 8 |

Milestones 1, 3, and 4 are independent and can run in parallel across three engineers; the critical path is 0 → 4 → 5 → 6 → 7 → 8 → 9 at roughly 26 to 32 weeks. Total single-engineer effort is 33 to 42 weeks.

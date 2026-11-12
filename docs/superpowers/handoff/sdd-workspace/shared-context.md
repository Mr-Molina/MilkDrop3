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

> CONTROLLER NOTE (ledger Task 4.8 rulings supersede plan text): D3D9_HALF_TEXEL is replaced by md_hlsl::layout::D3D9_PIXEL_SHIFT (0.5); built-in VS shift clip-space POSITIONS by +0.5 px (VsParams{texel_size,pixel_shift} at (0,2)); md-render builds all vertex data exactly as the C++; fixed_tex_modulate alpha = vertex alpha; fixed_tex_modulate_alpha for sprite mode 4. See assets/shaders/README.md.

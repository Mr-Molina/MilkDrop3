# MilkDrop3: plan to remove all Windows dependencies

Date: 2026-11-11
Status: proposal, awaiting owner decisions (see section 3)

## 1. Summary and recommendation

MilkDrop3 is a Win32 / Direct3D 9 application. Every layer touches Windows:
the window and input loop, the renderer, the shader compiler, the text and
image loaders, the audio capture, the expression JIT, the config and preset
file I/O, the string resources, the timers and threads, and the build system.
There is no portable core to extract. The port is a rewrite of the platform
layer around the existing visualizer logic, not a sequence of small fixes.

Recommended approach: **port in place, behind two small interfaces**.

- A platform abstraction (files, config, time, threads, strings, logging,
  window, input, audio capture).
- A render hardware interface (textures, render targets, blend and sampler
  state, dynamic draws, shader programs with named uniforms) sized to what
  `milkdropfs.cpp` actually uses, with an OpenGL 3.3 core backend first.

Proposed replacement stack (all cross-platform, permissively licensed unless
noted):

| Concern | Today | Replacement |
|---|---|---|
| Build | VS2015 `.sln`/`.vcxproj`, Win32 only, DirectX SDK June 2010 | CMake, vcpkg or FetchContent for deps, GitHub Actions matrix |
| Window, input, monitors, DPI | Win32 (`CreateWindowW`, `WndProc`, `VK_*`, `GetSystemMetrics`) | SDL3 |
| Graphics | Direct3D 9 fixed function + HLSL ps_2_0/ps_3_0 via D3DX | OpenGL 3.3 core (later: Vulkan or Metal via the same RHI) |
| Preset shader compiler | `D3DXCompileShader` at runtime | glslang HLSL front end to SPIR-V, SPIRV-Cross to GLSL (fallback: projectM's `hlslparser`) |
| Built-in shaders (`data/*.fx`) | HLSL | hand-converted GLSL, shipped alongside |
| Shader constants | `ID3DXConstantTable` | uniform lookup by name through the RHI |
| Math | D3DX matrix/vector | GLM |
| Text | GDI `CreateFontW` + `ID3DXFont` | stb_truetype glyph atlas (or FreeType) drawn through the RHI |
| Images | `D3DXCreateTextureFromFileEx` | stb_image (jpg, png, tga, bmp) plus a small DDS loader |
| Audio capture | WASAPI loopback | SDL3 audio recording from a monitor source (PipeWire/PulseAudio), CoreAudio tap on macOS, WASAPI on Windows, behind one interface |
| Expression language | ns-eel2 with x86 inline asm | projectm-eval (portable C, EEL-compatible, no JIT) |
| Config and presets | `GetPrivateProfile*` on `milk2.ini` and `.milk` files | one portable INI reader/writer module |
| Filesystem | `FindFirstFileW`, `GetFileAttributesW`, `MAX_PATH`, backslashes, UTF-16 | `std::filesystem`, UTF-8 `std::string` |
| Timing, threads | `timeGetTime`, `QueryPerformanceCounter`, `Sleep`, `_beginthreadex`, `CreateEvent` | `std::chrono`, `std::thread`, `std::condition_variable` |
| String resources | `.rc` STRINGTABLE (593 strings) via `LoadStringW` | generated C++ table (`strings.h`), same IDs |
| Open URL | `ShellExecute` | `SDL_OpenURL` |
| Host plugin ABI | musikcube `IPcmVisualizer` exports, Winamp IPC | dropped; standalone executable only |

Rough effort for one experienced engineer, with the existing source as the
base: 4 to 7 months to a first Linux build that runs the preset corpus, with
the shader translator and rendering fidelity dominating. Details in section 6.

## 2. Inventory of Windows dependencies in this repository

Total source is about 43,600 lines. Counts below are call sites found by
grep, not lines, and are approximate.

### 2.1 Graphics (Direct3D 9 and D3DX)

Files: `milkdropfs.cpp` (4,712 lines), `plugin.cpp` (8,859), `pluginshell.cpp`
(2,563), `support.cpp`, `dxcontext.cpp`, `textmgr.cpp`, `texmgr.cpp`, and the
headers that expose D3D types in class members (`plugin.h`, `state.h`,
`support.h`).

| D3D9 usage | Call sites | Notes |
|---|---|---|
| `SetRenderState` | 181 | alpha blend, cull, z, fill mode, shade mode, lighting |
| `SetTextureStageState` | 69 | fixed-function color/alpha ops: MODULATE, SELECTARG1, DISABLE |
| `SetTexture` | 64 | |
| `SetSamplerState` | 63 | filters and wrap/clamp |
| `DrawPrimitiveUP` | 38 | immediate-mode vertex arrays: strips, lists, fans, line strips, point lists |
| `Clear` | 25 | |
| `SetFVF` | 22 | three vertex layouts: `MYVERTEX`, `WFVERTEX`, `SPRITEVERTEX` |
| `SetRenderTarget` / `GetRenderTarget` | 19 | ping-pong VS textures, blur chain, title texture |
| `CreateTexture` / `CreateVolumeTexture` | 9 | formats X8R8G8B8, A8R8G8B8, R5G6B5, `D3DUSAGE_AUTOGENMIPMAP` |
| `SetTransform` | 5 | fixed-function world/view/projection, including a 3D "VJ" camera path |
| `D3DXCompileShader` | 2 | runtime HLSL compile of preset warp/comp shaders, profiles ps_2_0, ps_2_a/b, ps_3_0, vs_1_1 |
| `D3DXCreateFontW` | 3 | GDI-backed text |
| `D3DXCreateTextureFromFile*` | 3 | sprites and preset textures |
| D3DX math (`D3DXMatrix*`, `D3DXVECTOR*`) | about 30 | `support.cpp`, `state.h` |

The fixed-function pipeline is used for all non-shader drawing: waveforms,
custom shapes and waves, borders, motion vectors, sprites, text quads, and the
ps_0_0 fallback path for presets with no shaders.

### 2.2 Preset shaders (the hard part)

Presets embed HLSL fragments written by thousands of authors over 15 years.
`plugin.cpp` wraps them with `data/include.fx` and a `shader_body` macro,
compiles them with `D3DXCompileShader`, and binds constants by name through
`ID3DXConstantTable` (`_c0`..`_c13`, `_qa`..`_qh`, `rot_*`, `sampler_*`,
`texsize_*`). On the target platform there is no HLSL compiler for the
ps_2_0/ps_3_0 dialect. A translator to GLSL is required and must tolerate the
sloppy HLSL the corpus contains. See section 7.

### 2.3 Audio capture (`code/audio/`, 1,100 lines)

Pure WASAPI: `IMMDeviceEnumerator`, `IAudioClient` in
`AUDCLNT_STREAMFLAGS_LOOPBACK` mode, `IAudioCaptureClient`,
`AvSetMmThreadCharacteristics`, COM init, Win32 events and threads, and a WAV
writer using `mmio*`. `audiobuf.cpp` converts to the 576-sample 8-bit L/R
frames the visualizer consumes. Only `GetAudioBuf` is needed by the rest of
the program.

### 2.4 Window, input, and process (`Milkdrop2PcmVisualizer.cpp`, `pluginshell.cpp`)

- Win32 window class, `WndProc`, message pump, `_beginthreadex` render thread.
- Fullscreen, borderless, always-on-top, and multi-monitor stretch via
  `SetWindowLong`, `SetWindowPos`, `MonitorFromWindow`, `GetSystemMetrics`.
- DPI awareness through `ShellScalingApi.h`; `DwmApi.h` included.
- Keyboard handling through `VK_*` codes in `WM_KEYDOWN`/`WM_CHAR`: 59 sites in
  `plugin.cpp`, 22 in `menu.cpp`, 17 in `pluginshell.cpp`, 13 in the entry file.
- "Desktop mode" that parents the window to the Explorer desktop
  (`FindWindow("Program Manager")`, `SHELLDLL_DefView`, `SysListView32`) and a
  registry read for icon size. Windows-only by nature.
- Winamp IPC constants and a `WM_COMMAND` path; musikcube SDK plugin exports
  (`GetPlugin`, `GetPcmVisualizer`, `GetPlaybackRemote`), with
  `..\..\musikcube\src` as an include path.
- `utility.cpp` carries dead Explorer context-menu and PIDL code
  (`DoExplorerMenu`, `ExecutePidl`), `ShellExecuteEx` for URLs, and
  `DownloadDirectX` / `MissingDirectX` dialogs.

### 2.5 Expression JIT (`code/ns-eel2/`, about 9,000 lines)

Nullsoft EEL2 with hand-written x86 machine code glue: `asm-nseel-x86-msvc.c`
(2,463 lines of MSVC inline asm), `asm-nseel-x86-gcc.c`, a PowerPC variant,
and a prebuilt `asm-nseel-x64-macho.o`. The 64-bit path calls an external
`win64_callcode` that is not in the tree. It sets the x87 rounding mode with
`_controlfp`, allocates executable memory with `VirtualProtect`/`mprotect`,
and has no ARM64 support. The project only builds 32-bit today.

### 2.6 Config, presets, and files

- About 240 `GetPrivateProfile*`/`WritePrivateProfile*` calls across `plugin.cpp`,
  `pluginshell.cpp`, `dxcontext.cpp`, `utility.cpp`, and `state.cpp`.
  `state.cpp` already has a custom fast INI reader for `.milk` files but still
  calls `GetPrivateProfileString` once and writes presets with `_wfopen`.
- Directory enumeration with `FindFirstFileW` in two places (texture list and
  preset list), existence checks with `GetFileAttributesW` (about 12),
  `GetModuleFileNameW` to locate the install directory, and backslash paths
  built with `swprintf` (`presets\`, `textures\`, `data\include.fx`, hard-coded
  `c:\program files\winamp\` fallbacks).
- Strings are UTF-16 `wchar_t` throughout (about 900 sites), converted with
  `MultiByteToWideChar` in `AutoWide.h` / `AutoCharFn.h`. Microsoft CRT
  variants are used widely: `_wfopen`, `_stricmp`, `_wcsicmp`, `lstrcpy*`,
  `swprintf` in its non-conforming form, `sprintf_s`, `__int64`.

### 2.7 Resources and build

- `plugin.rc`: 593 `STRINGTABLE` entries, the icon, `VERSIONINFO`, and
  `text1.bin` as a custom resource. No dialogs remain (the config panel is
  gone). Loaded through `wasabi.cpp` with `LoadStringW`.
- `plugin.vcxproj` is Win32-only, toolset v142, links `d3d9.lib d3dx9.lib
  Shlwapi.lib`, and references the June 2010 DirectX SDK. Older `plugin.dsp`
  and `plugin.vcproj` files are stale duplicates.
- `vis_milk2/Debug/` contains two compiled `.exe` files (1.9 MB) checked into
  source control.
- `linux/` holds a Wine guide and launcher, which the port supersedes.

### 2.8 Timing, threads, and CRT

`timeBeginPeriod`, `timeGetTime`, `QueryPerformanceCounter`, `Sleep` in a
frame limiter, `_beginthreadex`, `CreateEvent`/`WaitForSingleObject`,
`InterlockedIncrement`, `GetPentiumTimeRaw` (RDTSC), `CheckForMMX`/`CheckForSSE`
with `__cpuid`, `_CRTDBG_MAP_ALLOC`, `#pragma comment(lib, ...)`. Some
pointer arithmetic casts pointers to `DWORD` (`textmgr.cpp`), which breaks on
64-bit.

## 3. Critical caveat: this source does not match the shipped MilkDrop 3.37

The `code/` tree was committed once, on 2023-10-03, and never updated. The
README describes releases through 3.37 (September 2026). Checks against the
source:

| README feature | Present in `code/`? |
|---|---|
| 16 custom waves and shapes | No, `MAX_CUSTOM_WAVES` and `MAX_CUSTOM_SHAPES` are 4 |
| q1..q64 variables | No, `NUM_Q_VAR` is 32 |
| `.milk2` double presets, `.shape` / `.wave` export | No |
| Beat-driven hardcut modes (F8) | No |
| Media-key player control (Ctrl+arrows) | No |
| Shader cache, ps_4_0 auto-upgrade, "no DirectX files needed" | No |
| New transitions, color randomizer, filter key | No |

What is here is Maxim Volskiy's BeatDrop (MilkDrop 2.25c plus a musikcube
host shim) with a WASAPI capture layer. Porting this repository therefore
produces a cross-platform MilkDrop 2.25c-class visualizer, not MilkDrop 3.37.

Decisions the owner must make before work starts:

1. **Source baseline.** Publish the current 3.37 source into this repository
   first, or accept porting the 2023 snapshot and re-adding 3.x features later
   from the maintainer's private tree. The plan below assumes the current
   source is published first; otherwise add a phase to rebuild the 3.x
   features and extend the estimates.
2. **Target platforms.** Assumed: Linux x86_64 and aarch64, macOS (Apple
   Silicon and Intel), Windows x64 through the same build. 32-bit Windows is
   dropped.
3. **Graphics API.** Assumed: OpenGL 3.3 core first because it is the fastest
   path to a running build on every target and matches what the shader
   translator emits. Vulkan and Metal remain possible later through the RHI.
4. **Fidelity bar.** Assumed: pixel-close, not pixel-identical, with a
   screenshot regression suite that flags divergence per preset.
5. **Audio on Linux.** Assumed: capture the default output's monitor source
   through PipeWire (PulseAudio compatible). No device picker UI in the first
   release; a config key selects a device by name.
6. **Licensing.** The code is BSD 3-Clause. SDL3 (zlib), glslang (BSD/Apache),
   SPIRV-Cross (Apache 2), GLM (MIT), stb (MIT), and projectm-eval (check its
   license before adopting; it is published by the projectM project) are
   compatible. If any LGPL component is chosen, dynamic linking is required.

## 4. Target architecture

```
+----------------------------------------------------------------+
| Visualizer core (unchanged logic)                              |
|  plugin.cpp  milkdropfs.cpp  state.cpp  menu.cpp  fft.cpp      |
|  pluginshell.cpp (reduced to frame orchestration)              |
+------------------------+---------------------------------------+
| RHI (render interface) | Platform layer                        |
|  gl33/ backend         |  fs, ini, strings, time, threads, log |
|  shader translator     |  window+input (SDL3)                  |
|  text atlas, images    |  audio capture (SDL3 / per-OS taps)   |
|                        |  eval (projectm-eval)                 |
+------------------------+---------------------------------------+
| SDL3  OpenGL  glslang  SPIRV-Cross  GLM  stb  projectm-eval    |
+----------------------------------------------------------------+
```

Design rules:

- The core never includes `<windows.h>`, `<d3d9.h>`, or `<d3dx9.h>`. Compile
  with a header that `#error`s if either is pulled in.
- The core uses UTF-8 `std::string` and `std::filesystem::path`. The
  `wchar_t` overloads disappear; `AutoWide.h` / `AutoCharFn.h` are deleted.
- The RHI is deliberately small: it exposes the handful of state and draw
  operations the histogram in section 2.1 shows, nothing more. Fixed-function
  color/alpha ops become four or five tiny built-in GLSL programs selected by
  a state key (texture modulate, texture select, color only, additive variants).
- The platform layer has one Windows backend too, so the Windows build moves
  onto the new code rather than keeping two codebases.

## 5. Feature disposition

| Feature | Disposition |
|---|---|
| Preset rendering, transitions, blending, waves, shapes, sprites, menus, help overlay, preset browser, search | Keep, ported through RHI |
| Fullscreen, borderless, always-on-top, multi-monitor stretch (Alt+Shift) | Keep via SDL3; stretch uses the union of display bounds |
| FPS limiter | Keep, reimplemented on `std::chrono` with sleep plus spin |
| Song title overlay | Keep; title comes from config or MPRIS (Linux) / none, since there is no host player |
| Audio input | Keep, new backends |
| Desktop mode (window behind desktop icons) | Drop (X11/Wayland have no equivalent that matches); document |
| Winamp IPC, musikcube plugin exports | Drop |
| Explorer context menu, PIDL helpers, DirectX download prompts, MMX/SSE checks, RDTSC timer | Delete |
| Media keys (Ctrl+arrows) once present | Linux: MPRIS D-Bus; macOS: media key events; Windows: `APPCOMMAND` |
| Registry read for desktop icon size | Delete with desktop mode |
| `text1.bin` custom resource | Ship as a data file |
| Wine guide in `linux/` | Replace with native build and run instructions |

## 6. Phased plan

Each phase ends with a green build and a demonstrable exit criterion. Phases
1 through 4 are platform-neutral and keep the Windows build working, so the
project is never in a half-ported state for long.

### Phase 0: baseline and decisions (1 week)

- Resolve the six decisions in section 3.
- Delete `vis_milk2/Debug/*.exe`, `plugin.dsp`, `plugin.vcproj`, and the
  `asm-nseel-x64-macho.o` blob; add a `.gitignore`.
- Record a reference capture set on Windows with the current binary: 40 to 60
  presets covering ps_0_0, ps_2_0, ps_3_0, blur, sprites, custom shapes, and
  3D "VJ" mode, each rendered for N frames from a fixed WAV file and a fixed
  random seed. These become the fidelity oracle.
- Exit: decisions recorded in this document; reference images committed to a
  separate assets repository or release attachment.

### Phase 1: CMake and compiler hygiene (1 to 2 weeks)

- Replace `.sln`/`.vcxproj` with CMake targets: `milkdrop_core`, `milkdrop_eval`,
  `milkdrop_platform`, `milkdrop_rhi_gl`, `milkdrop` (executable), `tests`.
- Build on Windows x64 with MSVC and clang-cl, warnings as errors for new
  files. Fix 64-bit casts (`textmgr.cpp`, `SetWindowLong` sites).
- Move the 593 strings out of `plugin.rc` into a generated header; replace
  `wasabiApiLangString` with a function over that table.
- Add GitHub Actions for Windows now, Linux and macOS as later phases land.
- Exit: Windows x64 build from CMake, behavior unchanged.

### Phase 2: platform layer, non-graphics (3 to 4 weeks)

- `platform/ini`: reader and writer compatible with `milk2.ini` and `.milk`
  files (sections, repeated keys, the `[preset00]` header). Port the roughly 240 call
  sites. Replace the one `GetPrivateProfileString` in `state.cpp`.
- `platform/fs`: install dir discovery, enumerate presets and textures
  recursively, exists/mkdir, path joins. Replace `FindFirstFileW`,
  `GetFileAttributesW`, `GetModuleFileNameW`, hard-coded `c:\` fallbacks.
- `platform/str`: convert the tree to UTF-8; remove `_wfopen`, `_wcsicmp`,
  `lstrcpy`, non-conforming `swprintf`, `sprintf_s`. Provide `fopen_utf8` that
  handles Windows internally.
- `platform/time` and threads: `std::chrono::steady_clock`, `std::thread`,
  `std::condition_variable` for the capture-started/stop events.
- `platform/log`: replace `OutputDebugString`/`ERR`/`LOG` wide-string loggers.
- Exit: `grep -r "windows.h" core/` returns only the audio and window files
  still pending; Windows build still green and still rendering identically.

### Phase 3: expression engine (2 to 3 weeks)

- Replace `ns-eel2` with projectm-eval behind the existing `NSEEL_*` call
  shape (`VM_alloc`, `regvar`, `code_compile`, `code_execute`, `code_free`).
  The 208 `NSEEL_VM_regvar` sites stay.
- Honor the same semantics: megabuf/gmegabuf, `if`/`loop`/`while`, the
  chop-toward-zero rounding the x87 path set with `_controlfp` (projectm-eval
  defines its own behavior; verify against reference presets that depend on
  `int()` and `%`).
- Tests: a corpus of per-frame and per-vertex expressions extracted from the
  preset set, executed against recorded outputs from the x86 JIT.
- Exit: expression tests pass on x86_64 and aarch64; no inline assembly left.

### Phase 4: audio capture (2 weeks)

- Interface: `IAudioSource` with `start()`, `stop()`, `read(left, right, 576)`.
- Backends: SDL3 recording device opened on the monitor source (Linux
  PipeWire/PulseAudio), WASAPI loopback (Windows, retaining the existing code
  behind the interface), CoreAudio process tap (macOS 14.2+) or a
  virtual-device note for older macOS.
- Keep `audiobuf.cpp` conversion logic; delete the WAV writer and `mmio` code.
- Exit: visualizer reacts to system audio on all three OSes; a `--wav` flag
  plays a file into the pipeline for deterministic tests.

### Phase 5: window, input, and main loop (2 to 3 weeks)

- Replace `Milkdrop2PcmVisualizer.cpp` with an SDL3 `main`: window with GL
  context, event loop on the main thread, render on the same thread.
- Key translation table from `VK_*` and `WM_CHAR` semantics to SDL keycodes and
  text input; MilkDrop distinguishes `a` from `A`, so text-input events drive
  the letter commands and keydown events drive function keys.
- Fullscreen, borderless, topmost, multi-monitor stretch, DPI scale, icon.
- Delete `PluginShellDesktopWndProc`, `PluginShellVJModeWndProc` window
  plumbing (keep VJ rendering if still wanted), and all `HWND` members.
- Exit: window opens on Linux and macOS with a cleared frame and working
  keyboard commands; render thread model simplified.

### Phase 6: render interface and OpenGL backend (8 to 12 weeks)

This is the largest phase. Suggested order:

1. **RHI definition** from the section 2.1 histogram: `Texture2D`,
   `RenderTarget`, `BlendState`, `SamplerState`, `DrawDynamic(topology,
   layout, vertices)`, `Program` with `setUniform(name, ...)` and
   `bindTexture(name, tex)`, `clear`, `setViewport`, `setTransform`.
2. **Fixed-function emulation**: built-in GLSL programs for the color/alpha
   op combinations found in the code; D3D9 half-pixel offset handled in the
   vertex stage so UV sampling matches the reference captures.
3. **Built-in shaders**: hand-convert the eight `data/*.fx` files to GLSL 330.
4. **Preset shader translator** (section 7).
5. **Text**: stb_truetype atlas per font slot (`m_fontinfo[]`), queued-text
   behavior of `CTextManager` preserved, `DT_*` flags mapped to a small
   layout routine (right align, single line, word ellipsis, calc rect).
6. **Images**: stb_image plus DDS; mip generation on upload; `texsize_*`
   constants populated as before.
7. **Port `milkdropfs.cpp` and `plugin.cpp` draw code** to the RHI, file by
   file, validating each against the Phase 0 captures.
- Exit: the Phase 0 reference set renders on Linux with per-preset image
  diffs under an agreed threshold; ps_0_0, ps_2_0, and ps_3_0 presets all
  load.

### Phase 7: feature re-homing and cleanup (2 weeks)

- MPRIS/media-key integration, `SDL_OpenURL`, remove dead `utility.cpp` code,
  drop desktop mode with a README note, delete `wasabi.*`, `dxcontext.*`,
  `AutoWide.h`, `AutoCharFn.h`, `resource.h` remnants.
- Exit: zero includes of Windows headers outside `platform/win/` and
  `audio/wasapi/`.

### Phase 8: packaging and CI (2 weeks)

- Linux: AppImage and Flatpak manifests; macOS: `.app` bundle with
  notarization notes; Windows: installer from the same CMake build.
- GitHub Actions matrix building all three and running the expression and
  screenshot tests on a software GL renderer (Mesa llvmpipe) for Linux.
- Replace `linux/README.md` with native instructions; keep the Wine guide as
  a legacy appendix or delete it.
- Exit: tagged release with three artifacts produced by CI.

### Phase 9: fidelity and performance hardening (ongoing, 3 to 4 weeks initial)

- Run the full public preset collection through the translator; triage
  failures by category and fix the translator rather than presets.
- Profile against the Windows build; the translated shaders and the lack of
  the EEL JIT are the expected hot spots. If per-vertex expression cost is a
  problem on large meshes, add a cached bytecode fast path in projectm-eval
  or batch per-vertex evaluation.

## 7. Preset shader translation

Inputs: preset `warp` and `comp` HLSL bodies, the `include.fx` preamble, the
`shader_body` wrapper, sampler declarations generated by `plugin.cpp` for
`sampler_XXX` textures, and the ps_2_0 / ps_2_x / ps_3_0 version hint.

Pipeline:

1. Assemble the same source string the D3D path compiles today (preamble,
   generated sampler declarations, body wrapped into
   `float4 main(float2 uv : TEXCOORD0, ...) : COLOR0`).
2. Compile with glslang's HLSL front end to SPIR-V. It accepts the legacy
   `tex2D`, `tex2Dbias`, `tex2Dlod`, `tex3D`, `texCUBE` intrinsics and
   `sampler2D` combined samplers, which is what presets use.
3. Run SPIRV-Cross to GLSL 330 (or GLSL ES 300 and MSL later), with
   combined-sampler emission and explicit uniform names preserved so
   `setUniform("_c2")` keeps working.
4. On failure, apply the same ladder the current code uses (ps_2_a, then
   ps_2_b, then ps_3_0), then fall back to the built-in default shader, and
   log the preset name and error for the triage list.

Known hazards to budget for:

- Loose HLSL in the corpus: implicit scalar-to-vector widening, assigning to
  swizzles, missing semicolons that fxc tolerated, `float3 x = 0;`. The
  translator needs a preprocessing pass that applies the fixups projectM
  accumulated over years; its `hlslparser` directory is a useful checklist
  even if it is not adopted wholesale.
- D3D9 conventions: half-pixel offset, `tex2Dbias` sign, `D3DTADDRESS_WRAP`
  on non-power-of-two textures, point sampling of the noise textures,
  Y-flip of render targets. Each must be matched or the blur and warp feedback
  loops drift visibly.
- Precision: ps_2_0 presets assume 24-bit float minimums and some rely on
  `half` behaving as float. Emit `highp` everywhere.
- Uniform packing: `_c0`..`_c13`, `_qa`..`_qh`, 24 `rot_*` float4x3 matrices.
  Map to a single uniform block to cut per-draw overhead.
- Compile latency: D3DX compiled on preset switch; glslang plus SPIRV-Cross
  is slower. Compile on a worker thread with a shader cache keyed by source
  hash (this also restores the 3.3x "shader cache" feature).

## 8. Validation strategy

- **Expression tests** (Phase 3): recorded input/output pairs from the x86
  JIT for several thousand expressions.
- **Translator corpus test** (Phases 6 and 9): every preset in the public
  collection must compile; the failure list is tracked as an issue and must
  shrink release over release.
- **Screenshot regression** (Phases 6 and 9): deterministic run with a fixed
  WAV and seed; perceptual diff per preset against the Windows captures;
  thresholds per category since blur-heavy presets accumulate drift.
- **Manual checklist**: every keyboard shortcut in the README, fullscreen and
  monitor transitions, preset browser and search, saving a preset, loading a
  user texture.

## 9. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Source in repo is not 3.37 (section 3) | Port delivers an older product | Owner publishes current source first |
| Shader translator rejects a meaningful share of presets | User-visible regressions | Corpus test from day one; preprocessing fixups; keep projectM translator as plan B |
| Visual drift in feedback loops | Presets look different after seconds | Match D3D9 sampling conventions explicitly; screenshot oracle at multiple frame counts |
| Loss of EEL JIT performance | Lower FPS on dense meshes | Benchmark early; evaluator fast path if needed |
| Audio capture permissions and device naming differ per OS and desktop | "No sound" reports | Clear device selection config, `--list-audio-devices`, documented defaults |
| Wayland quirks: topmost, exact positioning, multi-monitor spanning | Some window features degrade | Document; X11 fallback via SDL hint |
| Single-maintainer bandwidth | Long calendar time | Phase ordering keeps Windows build shippable throughout |

## 10. Alternatives considered

- **Build MilkDrop3's features on libprojectM instead of porting.** projectM
  already runs on every target and has an HLSL translator and a portable
  evaluator. The cost is reimplementing the MilkDrop3 UI and features on a
  different renderer whose output is not identical to MilkDrop's, plus the
  LGPL boundary. Worth revisiting if the 3.37 source cannot be published,
  since in that case the features have to be rebuilt anyway.
- **Native D3D9 shims (DXVK-native plus Wine's d3dx9).** Keeps the code
  untouched but is not a removal of Windows dependencies, and the d3dx9
  compiler and font code have no supported native build.
- **Keep Wine (status quo).** Already documented in `linux/`; menu text is
  broken and audio routing is manual. Not a conversion.
- **Vulkan or WebGPU first.** Better long-term, but doubles Phase 6 risk.
  The RHI leaves the door open once the OpenGL backend is validated.

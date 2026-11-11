# Windows reference captures (C++ build)

These images are **visual references only**, captured once by the owner from the
2023 C++ build in `code/`. Automated golden tests compare the Rust renderer
against its own baselines in `tests/golden/rust/`; the images here are what a
human compares those baselines against when blessing them. Bit-exact parity with
Direct3D 9 is not a goal.

The C++ build has no seed control (its `rand()` and the random-texture choice
are not seedable) and no `--wav` input, so captures are not reproducible frame
for frame. Expect differences in anything driven by `rand()`, `rand_frame`,
`rand_preset`, `rot_*` matrices and `randNN` textures.

## What to capture

Every preset listed in `tests/golden/manifest.toml` (`[[preset]]` entries; paths
are relative to `tests/presets/`), at frames **1, 60, 300 and 600** after the
preset is loaded, at the manifest's size (640x480 client area).

Save as `tests/golden/windows/<preset path without .milk, '/' replaced by '__'>/frame_<NNNN>.png`,
for example `tests/golden/windows/ps2__blur_glow/frame_0060.png`.

## Procedure

1. **Build the C++ app.** Open `code/MilkDrop3.sln` in Visual Studio (2019 or
   later, with the DirectX 9 / D3DX9 SDK headers the project expects), select
   `Release | Win32`, and build. The executable expects its data under
   `<exe dir>\plugins\Milkdrop2\`.
2. **Install the corpus.** Copy the `.milk` files from the subdirectories of
   `tests/presets/` that the manifest names into
   `<exe dir>\plugins\Milkdrop2\presets\` (flat; the subdirectory is only part of
   the capture name), and copy `tests/presets/textures/mytex.png` into
   `<exe dir>\plugins\Milkdrop2\textures\`.
3. **Configure `milk2.ini`** (`[settings]` section; create it by running the app
   once and quitting):
   - `MaxPSVersion=-1` (auto). The default `3` caps presets at ps_2_x and hides
     the `PSVERSION=4` (ps_3_0) presets from the list.
   - `fBlendTimeUser=0` so a manually loaded preset hard-cuts (no blend frames).
   - `bHardCutsDisabled=1` and a large `fTimeBetweenPresets` (e.g. `9999`) so
     nothing switches presets during a capture; or lock the preset (Scroll Lock).
   - Window client area 640x480 (resize the window; check with a screenshot).
   - FPS limit 60 (F2 cycles 60/90/120 in MilkDrop3), so frame N is at
     roughly N/60 s.
4. **Play the test signal through the system.** `--wav tests/golden/test_signal.wav`
   is not available in C++: the app captures the default output device via
   WASAPI loopback. Play `tests/golden/test_signal.wav` (20 s, 44.1 kHz stereo;
   55 Hz sine at 0.8 pulsing at 2 Hz plus 4 kHz sine at 0.2 pulsing at 0.5 Hz;
   regenerate with `cargo run -p md-testkit --example gen_signal`) in a media
   player on loop, at 100% system and player volume, on the default output
   device.
5. **Capture.** For each manifest preset: open the load menu (`L`), select the
   preset, and screenshot the client area at frames 1, 60, 300 and 600 after the
   load (about 0.02 s, 1 s, 5 s and 10 s). A screen recorder at 60 fps with
   frames extracted afterwards (e.g. `ffmpeg -i rec.mp4 -vf "select=eq(n\,60)" ...`)
   is the most practical way to hit the frame numbers; MilkDrop's own frame
   counter is shown in the `n` debug overlay if you need to align the recording.
   Turn the overlay off for the captured frames.
6. **Commit** the PNGs under `tests/golden/windows/` with a note in the commit
   message of the GPU, driver, Windows version and app build used.

## Known C++ quirks to keep in mind when comparing

- `edge/crlf_bom_junk.milk` (not in the manifest) loads as a MilkDrop 1 preset in
  the C++ build because the UTF-8 BOM hides `MILKDROP_PRESET_VERSION`; the Rust
  parser strips the BOM.
- Presets using `randNN` textures pick a random file from `textures\` at load;
  with only `mytex.png` installed they all get that image.

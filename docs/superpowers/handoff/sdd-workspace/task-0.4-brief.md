### Task 0.4: Preset corpus and reference captures

**Files:**
- Create: `tests/presets/README.md`, `.gitattributes` (LFS for `tests/presets/**` and `tests/golden/**`), `tests/golden/windows/README.md`

- [ ] **Step 1:** Add the corpus per A.4; record its origin and license in `tests/presets/README.md`.
- [ ] **Step 2:** Write `tests/golden/windows/README.md` describing the capture procedure for the owner: C++ build, `--wav tests/golden/test_signal.wav` is not available in C++, so capture by playing the WAV through the system and screenshotting frames 1, 60, 300, 600 for each preset in `tests/golden/manifest.toml`; the 2023 build has no seed control, so these are visual references only.
- [ ] **Step 3:** Generate `tests/golden/test_signal.wav`: 20 seconds, 44.1 kHz stereo, a 55 Hz sine at 0.8 amplitude pulsing at 2 Hz plus a 4 kHz sine at 0.2 amplitude pulsing at 0.5 Hz, written with `hound` by `tests/support/md-testkit/examples/gen_signal.rs`.
- [ ] **Step 4:** Run `cargo run -p md-testkit --example gen_signal` and `md-testkit::corpus_presets().len() > 0` in a test. Expected: file exists; count printed.
- [ ] **Step 5:** Commit: `test: add preset corpus, test signal, and capture instructions`

## Milestone 1: `md-eval`, the expression language

Semantics are those of ns-eel2 as built in `code/ns-eel2/` with `NSEEL_REENTRANT_EXECUTION` and without `NSEEL_EEL1_COMPAT_MODE`. Pinned facts: values are `f64`; identifiers are case-insensitive; `reg00`..`reg99` are 100 global registers shared by every `Vm`; truthiness is `|x| > 0.00001` (`NSEEL_CLOSEFACTOR`); equality is `|a-b| < 0.00001`; `rand(x)` floors `x`, clamps to at least 1, and returns a uniform value in `[0, x)` from a Mersenne Twister seeded with `0x4141f00d`; `megabuf(i)` and `gmegabuf(i)` address a sparse array of 128 blocks of 65536 `f64`, allocated on first write, index outside `[0, 8388608)` reads 0 and ignores writes; `loop(n, body)` iterates at most 1,048,576 times (`NSEEL_LOOPFUNC_SUPPORT_MAXLEN`); `while(body)` repeats while the body's value is true, with the same cap; statements are separated by `;`; comments are `//` to end of line and `/* */`; numbers accept `1`, `1.5`, `.5`, `1e3`, and `$x1F` hex; the value of a program is the value of its last expression.


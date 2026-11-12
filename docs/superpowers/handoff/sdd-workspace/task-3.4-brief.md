### Task 3.4: Linux loopback backend

**Files:**
- Create: `crates/md-audio/src/linux_pulse.rs`

**Interfaces:**
- Produces: `#[cfg(target_os = "linux")] pub fn default_loopback() -> Result<Box<dyn AudioSource>, AudioError>` opening a `libpulse_simple_binding::Simple` record stream on `"<default sink>.monitor"` (sink name read via `libpulse_binding` introspection) at 44100 Hz stereo f32; a background thread fills a ring buffer of 8192 frames; `next_frame` waits up to `timeout` on a condvar.
- `list_devices()` lists monitor sources with their descriptions; `Config.audio_device` selects by name.

- [ ] **Step 1:** Write test `pulse_backend_falls_back_to_silence_when_no_server` (run with `PULSE_SERVER=unix:/nonexistent`; expect `AudioError::NoServer` and the caller substitutes `SilenceSource`).
- [ ] **Step 2:** Run the test. Expected: FAIL.
- [ ] **Step 3:** Implement; `unsafe` is not needed (the binding crate is safe).
- [ ] **Step 4:** Run the test; manual check on a desktop: `cargo run -p milkdrop -- --list-audio-devices` prints the monitor source.
- [ ] **Step 5:** Commit: `feat(audio): PulseAudio/PipeWire monitor capture`


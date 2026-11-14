### Task 3.5: Windows and macOS backends

**Files:**
- Create: `crates/md-audio/src/windows_wasapi.rs`, `crates/md-audio/src/macos_sck.rs`

**Interfaces:**
- Windows: `cpal` default output device opened as an input stream (loopback). If the pinned cpal version does not support it, implement with the `windows` crate (`IAudioClient::Initialize` with `AUDCLNT_STREAMFLAGS_LOOPBACK`) in the same file behind `#[allow(unsafe_code)]`; this is the one place `unsafe` is permitted and it is reviewed line by line.
- macOS: `screencapturekit` crate `SCStream` with `captures_audio = true`, audio-only, 44100 Hz stereo; permission denial returns `AudioError::PermissionDenied` with a message naming System Settings > Privacy > Screen Recording.

- [ ] **Step 1:** Write test `backend_errors_are_mapped` (each backend's error type converts to `AudioError` with a user-facing message).
- [ ] **Step 2:** Implement both backends; CI builds them but cannot capture; mark capture tests `#[ignore]` and run them manually.
- [ ] **Step 3:** Run `cargo build -p md-audio` on all three OSes in CI. Expected: success.
- [ ] **Step 4:** Commit: `feat(audio): WASAPI loopback and ScreenCaptureKit backends`


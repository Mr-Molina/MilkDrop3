# Task 3.5 report: Windows and macOS loopback backends

Status: DONE_WITH_CONCERNS (macOS and Windows capture has not been run on real hardware here; see Concerns)
Commit: ad9be3b feat(audio): WASAPI loopback and ScreenCaptureKit backends

## What was implemented

- **Windows: `crates/md-audio/src/windows_wasapi.rs`, using cpal (no `unsafe`).** I checked this against the cpal 0.18.2 source. `host/wasapi/device.rs::build_input_stream_raw_inner` ORs in `AUDCLNT_STREAMFLAGS_LOOPBACK` when the device's data flow is `eRender`, and `host/wasapi/mod.rs` documents that an output device used as an input is looped back transparently. One catch: `default_input_config()` returns an error on a render endpoint, so the code calls `default_output_config()`, which gives the mix format, and passes that to `build_input_stream`. Supported formats are f32, i16 and i32; any other format returns `Backend`. Rerouting the default device (`DeviceChanged`) is ignored. `DeviceNotAvailable`, `StreamInvalidated` and `HostUnavailable` mark the ring as failed, after which `next_frame` returns `Ended`.
  - `list_devices` lists the output endpoints. `name` is the cpal `DeviceId` string (`wasapi:...`) and `is_default` marks the default output.
  - `open_device` accepts a `DeviceId` string or the friendly name. If the name refers to a capture endpoint, that endpoint is recorded directly.
  - cpal is declared only under `[target.'cfg(windows)'.dependencies]` (F8).
- **macOS: `crates/md-audio/src/macos_sck.rs`, using objc2-screen-capture-kit 0.3.2, objc2-core-media, objc2-core-audio-types, objc2-foundation, block2 and dispatch2 (F4).**
  - It runs an audio-only `SCStream`. The content filter has to name a display, so it uses the main display with 2x2 video at 1 fps and attaches no screen output. Audio is 48 kHz stereo (F53), `excludesCurrentProcessAudio` is on, and `to_frame` resamples to 44.1 kHz.
  - An objc2 `define_class!` output/delegate object receives the sample buffers on a serial dispatch queue. It decodes them with `CMBlockBufferCopyDataBytes`, handling planar and interleaved f32 according to the ASBD flags, and pushes them into the ring. `stream:didStopWithError:` marks the ring as failed.
  - The SCK error codes `UserDeclined` (-3801) and `MissingEntitlements` (-3803) map to `PermissionDenied`, with a message naming "System Settings > Privacy & Security > Screen Recording".
  - On macOS older than 13 it returns `Backend` (checked with `available!`) instead of aborting on missing selectors.
  - There is one device, `"system"`. `open_device` accepts `"system"` or `"default"` and returns `DeviceNotFound` for anything else; this is documented in the module docs.
  - `unsafe` is confined to 8 item-scoped `#[allow(unsafe_code)]` attributes, and every `unsafe` block has a SAFETY comment. There is one `unsafe impl Send` on a wrapper that moves the `SCShareableContent` snapshot from the SCK completion queue to the capture thread.
- **Shared ring: `crates/md-audio/src/capture_ring.rs` (new).** This is the Pulse design pulled out into one place:
  - An 8192-frame ring that tracks the format; a change of channels or rate clears it.
  - A condvar `next_frame` that honours its timeout. It returns `Silence` with zeroed buffers when the timeout passes and `Ended` after a failure. The tail it reads is sized for the resampling window (`ceil(575*rate/44100)+2` frames), which fixes the 576-frame tail being too short at 48 kHz.
  - `spawn_capture(name, open_timeout, open, run)`: the backend opens its stream on a detached thread, and the wait for that open is bounded at 10 s. `RingSource::drop` only sets the stop flag, so it never blocks.
  - `decode_f32_le` handles both interleaved and planar input.
- **Linux Pulse backend changed (`linux_pulse.rs`).** It now uses `capture_ring`, so all three backends share one ring/Drop design and the module is 147 lines shorter.
  - **Bug fix found by the new manual test:** `libpulse-simple-binding` returns *positive* error codes, while `Code::try_from(PAErr)` expects negative ones. Every `Simple::new` error, such as "No such entity" for a missing source, was therefore falling through to `Backend`. `map_err` now normalises the sign, so a missing source correctly gives `DeviceNotFound`.
- **`backend_errors.rs` (new, compiled on every OS).** `map_sck_error(domain, code, description)` and `map_wasapi_failure(WasapiFailure, detail, device)` are portable. `impl From<cpal::ErrorKind> for WasapiFailure` and `map_cpal_error` are behind `cfg(windows)`. The test `backend_errors_are_mapped` checks the portable functions on every OS, plus a `cfg(windows)` block that maps real `cpal::Error` values.
- **`loopback.rs`.** It dispatches to `linux_pulse`, `windows_wasapi` or `macos_sck`. The stub (now private) remains only for other OSes.
- **`tests/loopback_capture.rs`.** Two `#[ignore]`d real-capture tests that run on every OS: the default loopback delivers non-silent data and its drop takes under 100 ms; and the default device from `list_devices` opens while a missing name gives `DeviceNotFound`.
- **`examples/list_devices.rs`.** Wording changed to be backend-neutral.
- **Workspace pins.** Removed `screencapturekit` (it was unused, and F4 rules it out). Added `objc2 0.6.5`, `objc2-foundation 0.3.2`, `objc2-screen-capture-kit 0.3.2`, `objc2-core-media 0.3.2`, `objc2-core-audio-types 0.3.2`, `block2 0.6.2` and `dispatch2 0.3.1`, each with default features off where that applies and only the features needed.

## TDD evidence

RED: I wrote `backend_errors_are_mapped` and 11 `capture_ring` tests against stubs (`todo!()` / an empty `Backend`), then ran `cargo test -p md-audio --lib`:
```
test backend_errors::tests::backend_errors_are_mapped ... FAILED
test capture_ring::tests::decodes_interleaved_and_planar_f32 ... FAILED
test capture_ring::tests::failure_ends_the_source_and_stop_rejects_pushes ... FAILED
test capture_ring::tests::empty_ring_times_out_with_zeroed_silence ... FAILED
test capture_ring::tests::format_change_discards_older_samples ... FAILED
test capture_ring::tests::fresh_audio_is_delivered_once_from_the_newest_samples ... FAILED
test capture_ring::tests::ring_keeps_at_most_ring_frames ... FAILED
test capture_ring::tests::spawned_capture_streams_and_drop_never_blocks ... FAILED
test capture_ring::tests::tail_covers_the_resampling_window_at_48k ... FAILED
test capture_ring::tests::spawn_capture_gives_up_on_a_hanging_open ... FAILED
test capture_ring::tests::spawn_capture_reports_open_errors ... FAILED
test capture_ring::tests::waiting_reader_wakes_on_push ... FAILED
```
GREEN, `cargo test -p md-audio`:
```
test result: ok. 28 passed; 0 failed   (lib, includes backend_errors_are_mapped + 11 capture_ring tests)
test result: ok. 0 passed; 0 failed; 2 ignored   (loopback_capture)
test result: ok. 1 passed   (pulse_no_server)
test result: ok. 7 passed   (timeout)
```
**Real capture on Linux, through the refactored Pulse backend.** I installed PulseAudio locally, loaded a null sink and played a tone with `paplay`, then ran `cargo test -p md-audio --test loopback_capture -- --ignored`, three runs in a row:
```
test result: ok. 2 passed; 0 failed   (x3)
```
The first version of the second test failed with `Backend("PulseAudio error: No such entity")`, which is how the sign bug above was found; after the fix it passes. The first version of the frame-count assertion (>10 frames in 2 s) also flaked, because a null sink under a high-latency client sends audio in large blocks. The test now stops once it has 3 non-silent frames, within a 5 s limit.

## Verification

- `cargo build --workspace`: OK.
- `cargo clippy --workspace --all-targets -- -D warnings`: OK.
- `cargo fmt --check`: OK.
- `cargo test --workspace`: 37 result lines, all ok, 0 failed.
- `cargo check -p md-audio --target x86_64-pc-windows-msvc --all-targets` and the same clippy with `-D warnings`: OK.
- `cargo check -p md-audio --target x86_64-apple-darwin --all-targets` and the same clippy with `-D warnings`: OK. No dependency build script needed the macOS SDK. The objc2 crates have no C build step and `libc` builds cross without trouble. Linking was not attempted, since there is no Apple or MSVC linker here; CI on macOS and Windows will link and run the tests.
- Removing the `#[allow(unsafe_code)]` lines from `macos_sck.rs` produces 16 `unsafe_code` errors, which confirms that the crate-level `deny` is active and each allow is needed.
- `scripts/check-no-native-build.sh`: OK.
- `cargo tree --workspace --target all -e normal,build -i {cc,cmake,bindgen}`: no matches for any of them (cpal 0.18's CoreAudio path uses objc2, not coreaudio-sys/bindgen).
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`. There are two new *warnings* for duplicate versions (`jni-sys` and `thiserror`/`thiserror-impl` 1.x, both pulled in by cpal's target-specific dependencies); these are warnings only (`multiple-versions = "warn"`).
- CI (`.github/workflows/ci.yml`) already builds, lints and tests on ubuntu, macos and windows, so no CI edit was needed for Step 3.

## Files changed

- `Cargo.toml`, `Cargo.lock`
- `crates/md-audio/Cargo.toml`
- `crates/md-audio/src/{lib.rs, loopback.rs, linux_pulse.rs}`
- New: `crates/md-audio/src/{backend_errors.rs, capture_ring.rs, windows_wasapi.rs, macos_sck.rs}`
- New: `crates/md-audio/tests/loopback_capture.rs`
- `crates/md-audio/examples/list_devices.rs`

The largest file is `capture_ring.rs` at about 450 lines, of which about 190 are tests.

## Self-review findings (fixed before commit)

- An empty push used to set `fresh` and replay old samples. It is now a no-op.
- `abs()` could overflow on `i32::MIN` in the Pulse error mapping. It now uses `saturating_abs`.
- Added the macOS-13 availability guard.
- The `dead_code` allows are kept per item and gated per OS, because each OS uses a different subset of the ring API.

## Concerns

1. **macOS planar layout assumption.** `push_sample_buffer` reads the whole `CMBlockBuffer` with `CMBlockBufferCopyDataBytes` and treats non-interleaved audio as contiguous per-channel planes. That is the layout CoreMedia uses for SCK's float buffers, but I could not verify it here. The fallback would be `CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer`, which needs more unsafe code. It needs one manual run on a Mac: `cargo test -p md-audio --test loopback_capture -- --ignored` while audio plays, with the terminal granted Screen Recording.
2. **macOS from a bare CLI or test process.** SCK has been reported to need a CoreGraphics connection (some CLI tools hit `CGS_REQUIRE_INIT` asserts when no NSApplication exists). The app will have winit/NSApplication, so this mainly affects the ignored test run from `cargo test`. It is untested here.
3. **Windows capture not run.** It compiles and lints for the msvc target only. Manual check on Windows: the same ignored test while audio plays. WASAPI loopback delivers no packets while the endpoint is silent, so the source reports `Silence` then, which is expected.
4. **The Linux Pulse backend was refactored**, although it had already been reviewed in 3.4, to share `capture_ring`. It also carries the error-sign fix. Its behaviour is otherwise unchanged and was verified against a real PulseAudio server; the reviewer should look at the diff.
5. **Plan text is now out of date.** `shared-context.md` (A.4 row "Audio on macOS") still names the `screencapturekit` crate; F4 supersedes it. I did not edit the plan or ledger docs.
6. **Two new duplicate-version warnings from cargo-deny**, both from cpal's dependencies (see Verification). They are warnings, not errors.

## Fix round 1 (commit 5937cc6)

Changes:
1. **(Important) SCStream is now always stopped.** `Capture` has `impl Drop`, which calls `stopCaptureWithCompletionHandler`. The completion handler holds clones of the stream and the output, so neither is freed before the stop finishes. Every path out of `start` after `startCapture` drops the `Capture`: success leading to `run`, an error from the start handler, and a timeout.
   - In the case the review flagged, `start` succeeds after `spawn_capture` has stopped waiting, so `ready_tx.send` fails and `run` is skipped. `spawn_capture` then drops the guard, and the Drop impl stops the stream. A comment in `capture_ring.rs` documents this.
   - `shareable_content` and the start wait now share a single deadline of `OPEN_TIMEOUT - 500 ms`, set when `start` begins. Before this fix the two waits could take up to 2 x 10 s; now this thread normally gives up before the caller does.
   - `run` now just calls `wait_for_stop()` and then `drop(capture)`.
2. **Planar buffers must be exactly the expected size.** For planar audio, `push_sample_buffer` now requires `data_length() == frames * channels * 4`; interleaved audio still only needs at least that many bytes.
3. **Empty buffers are skipped.** A buffer with `frames == 0` returns early (pattern `Ok(frames @ 1..)`), so `copy_data_bytes` is never called with length 0.
4. **WASAPI friendly-name lookup uses render endpoints only.** It now searches `host.output_devices()`, so a headset microphone with the same name as the speakers is never picked. Lookup by `DeviceId` is unchanged; the module docs are updated.
5. **SAFETY comments now state the cross-thread facts.** They cover:
   - the `define_class!` header: methods are called on SCK dispatch queues, and the only ivar is an `Arc<CaptureRing>`, which is Send + Sync;
   - `addStreamOutput`: the serial queue runs on libdispatch threads, and the ring is protected by a Mutex and Condvar;
   - `SendContent`: the value is moved from the SCK queue to the capture thread, never shared;
   - every completion handler: it runs on another thread and captures only `Send` values;
   - `Capture::drop`: the handler releases its `Retained` handles on SCK's thread, and retain/release are atomic.

Covering test: I added `capture_ring::tests::late_open_drops_its_guard_without_running`. It opens a stream that finishes after `spawn_capture` has timed out, then checks that `run` is not called and that the guard is dropped. This was already how `spawn_capture` behaved, so the test passed when first run; there was no RED step. It guards the property the new `Capture::drop` depends on. The macOS-specific stop path cannot run on this Linux machine.

Commands and output:
- `cargo test -p md-audio`: lib 29 passed; loopback_capture 2 ignored; pulse_no_server 1 passed; timeout 7 passed; 0 failed.
- `cargo check` and `cargo clippy -p md-audio --all-targets -- -D warnings` for `x86_64-apple-darwin` and `x86_64-pc-windows-msvc`: both Finished, no warnings.
- `cargo clippy --workspace --all-targets -- -D warnings`: OK.
- `cargo fmt --check`: OK.

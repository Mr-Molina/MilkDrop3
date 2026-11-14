# Task 3.4 report: Linux PulseAudio/PipeWire monitor capture

Commit 0919af9 `feat(audio): PulseAudio/PipeWire monitor capture`.

Implemented crates/md-audio/src/linux_pulse.rs (list_devices, default_loopback, open_device; capture thread with 8192-frame ring, condvar next_frame honouring timeout, Drop stops and joins), loopback.rs now uses it on Linux (stub test cfg-gated to non-Linux), examples/list_devices.rs, tests/pulse_no_server.rs (own binary; PULSE_SERVER=unix:/nonexistent, PULSE_CLIENTCONFIG with autospawn=no).
Deps: libpulse-binding 2.30.1 (existing pin), libpulse-simple-binding 2.29.0 (newest published; 2.30.1 does not exist), both under cfg(target_os="linux") of md-audio.

TDD: RED: first run of pulse_no_server returned PermissionDenied (libpulse reports an unreachable socket as PA_ERR_ACCESS), so the test failed. GREEN: connect errors before authorization map to NoServer; `cargo test -p md-audio --test pulse_no_server` ok, all three entry points return NoServer in <10s.
Checks: clippy -D warnings, fmt --check, full cargo test --workspace, check-no-native-build.sh OK, cargo deny check ok (one pre-existing "unmatched wrapper" warning for android-activity).
`cargo run -p md-audio --example list_devices` here prints "no audio server is reachable" (exit 1), expected without a server.

Concerns: real capture and open-by-name on an actual server untested (no server). open_device probes with an introspection connect first because the simple API cannot distinguish no-server errors. Simple::read has no timeout, so Drop is prompt only while the server delivers data or errors (reads are ~10 ms).

## Fix round 1
- Drop of PulseSource now sets stop, notifies, and detaches the capture thread (no join); documented. The thread owns the Simple and an Arc of shared state.
- Server: `context` now declared before `mainloop` (comment explains). Locals in Server::connect already drop context before mainloop (declared after). No other struct holds both; Operation handles are local and dropped before the Server.
- Probing/listing connects with NOAUTOSPAWN.
- Covering: `cargo test -p md-audio` (incl. pulse_no_server) ok; clippy -D warnings, fmt --check ok.

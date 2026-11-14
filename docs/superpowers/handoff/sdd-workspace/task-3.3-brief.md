### Task 3.3: `AudioSource` trait, WAV and silence sources

**Files:**
- Create: `crates/md-audio/src/{lib.rs,source.rs,wav.rs,silence.rs,resample.rs}`

**Interfaces:**
- Produces: `AudioSource`, `FrameStatus`, `WavSource::open(path: &Path, loop_playback: bool) -> Result<WavSource, AudioError>`, `SilenceSource`, and `pub fn to_frame(samples_interleaved: &[f32], channels: u16, sample_rate: u32, left: &mut [u8;576], right: &mut [u8;576])` which resamples to 44100 Hz by linear interpolation, takes the most recent 576 samples, converts `[-1,1]` to the signed 8-bit bit pattern `((x * 127.0).round() as i8) as u8` (silence is `0`, which is what `((b ^ 128) - 128)` in the analyzer maps back to `0`), and mixes mono to both channels.

- [ ] **Step 1:** Write failing tests `wav_source_yields_576_samples_per_frame`, `to_frame_round_trips_through_analysis_conversion`, `mono_is_duplicated`, `48k_is_resampled_to_44k1`, `ended_after_last_frame_without_loop`.
- [ ] **Step 2:** Run `cargo test -p md-audio`. Expected: FAIL.
- [ ] **Step 3:** Implement with `hound`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(audio): source trait, WAV and silence sources`


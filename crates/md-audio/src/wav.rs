//! WAV file source with deterministic frame pacing.

use crate::{to_frame, AudioError, AudioSource, FrameStatus};
use std::io::Read;
use std::path::Path;
use std::time::Duration;

const OUT_RATE: f64 = 44_100.0;

/// Plays a WAV file one analysis frame per `next_frame` call, ignoring wall-clock time.
///
/// Pacing and windowing (deterministic): the playhead starts at source sample 0. Each
/// call advances it by `round(sample_rate / fps)` source samples per channel (never
/// fewer than one) and returns [`FrameStatus::Data`] holding the most recent 576 samples, resampled
/// to 44.1 kHz by [`to_frame`], ending at the new playhead (the window covers source
/// samples before the playhead; samples before the start of the file read as 0).
/// Without looping the playhead stops at the end of the file; the call whose playhead
/// reaches the end is the last `Data` frame and every later call returns
/// [`FrameStatus::Ended`] with zeroed buffers. With looping the file wraps seamlessly
/// and never ends. A file with no samples ends immediately.
pub struct WavSource {
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
    frames: u64,
    step: u64,
    looping: bool,
    playhead: u64,
    scratch: Vec<f32>,
}

impl WavSource {
    /// Opens `path`; `fps` is the render rate used to pace the playhead.
    pub fn open(path: &Path, loop_playback: bool, fps: f32) -> Result<WavSource, AudioError> {
        let file = std::io::BufReader::new(std::fs::File::open(path)?);
        Self::from_reader(file, loop_playback, fps)
    }

    /// Like [`WavSource::open`] but reads from any reader.
    pub fn from_reader<R: Read>(
        reader: R,
        loop_playback: bool,
        fps: f32,
    ) -> Result<WavSource, AudioError> {
        if !(fps.is_finite() && fps > 0.0) {
            return Err(AudioError::InvalidArgument(format!(
                "fps must be > 0, got {fps}"
            )));
        }
        let mut r = hound::WavReader::new(reader)?;
        let spec = r.spec();
        if spec.channels == 0 || spec.sample_rate == 0 {
            return Err(AudioError::InvalidArgument(
                "WAV has zero channels or rate".into(),
            ));
        }
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
            hound::SampleFormat::Int => {
                let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
                r.samples::<i32>()
                    .map(|s| s.map(|v| v as f32 / scale))
                    .collect::<Result<_, _>>()?
            }
        };
        let frames = (samples.len() / spec.channels as usize) as u64;
        let step = ((f64::from(spec.sample_rate) / f64::from(fps)).round() as u64).max(1);
        Ok(WavSource {
            samples,
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            frames,
            step,
            looping: loop_playback,
            playhead: 0,
            scratch: Vec::new(),
        })
    }

    /// Source samples (per channel) the playhead advances each frame.
    pub fn step(&self) -> u64 {
        self.step
    }
}

impl AudioSource for WavSource {
    fn next_frame(
        &mut self,
        left: &mut [u8; 576],
        right: &mut [u8; 576],
        _timeout: Duration,
    ) -> FrameStatus {
        if self.frames == 0 || (!self.looping && self.playhead >= self.frames) {
            left.fill(0);
            right.fill(0);
            return FrameStatus::Ended;
        }
        self.playhead += self.step;
        if !self.looping {
            self.playhead = self.playhead.min(self.frames);
        }
        let ratio = f64::from(self.sample_rate) / OUT_RATE;
        let need = (575.0 * ratio).ceil() as u64 + 2;
        let start = self.playhead as i64 - need as i64;
        let ch = self.channels as usize;
        self.scratch.clear();
        for a in start..self.playhead as i64 {
            if a < 0 {
                self.scratch.extend(std::iter::repeat_n(0.0, ch));
            } else {
                let f = (a as u64 % self.frames) as usize;
                self.scratch
                    .extend_from_slice(&self.samples[f * ch..(f + 1) * ch]);
            }
        }
        to_frame(&self.scratch, self.channels, self.sample_rate, left, right);
        FrameStatus::Data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn wav_bytes(rate: u32, channels: u16, frames: usize) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(&mut buf, spec).unwrap();
            for i in 0..frames {
                let v = ((i as f32 * 0.03).sin() * 20000.0) as i16;
                for _ in 0..channels {
                    w.write_sample(v).unwrap();
                }
            }
            w.finalize().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn wav_source_yields_576_samples_per_frame() {
        let bytes = wav_bytes(44_100, 1, 44_100);
        let mut s = WavSource::from_reader(Cursor::new(bytes.clone()), false, 30.0).unwrap();
        assert_eq!(s.step(), 1470);
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        assert_eq!(
            s.next_frame(&mut l, &mut r, Duration::ZERO),
            FrameStatus::Data
        );
        assert_eq!(l, r);
        assert!(l.iter().any(|&b| b != 0));
        // Window ends at playhead 1470: last sample is source sample 1469.
        let expect = (((1469.0f32 * 0.03).sin() * 20000.0) as i16 as f32 / 32768.0 * 127.0).round();
        assert_eq!(l[575] as i8 as f32, expect);
        // Deterministic.
        let mut s2 = WavSource::from_reader(Cursor::new(bytes), false, 30.0).unwrap();
        let (mut l2, mut r2) = ([0u8; 576], [0u8; 576]);
        s2.next_frame(&mut l2, &mut r2, Duration::ZERO);
        assert_eq!(l, l2);
    }

    #[test]
    fn ended_after_last_frame_without_loop() {
        // 3000 samples at step 1470: playheads 1470, 2940, 3000 -> 3 Data then Ended.
        let bytes = wav_bytes(44_100, 1, 3000);
        let mut s = WavSource::from_reader(Cursor::new(bytes), false, 30.0).unwrap();
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        for _ in 0..3 {
            assert_eq!(
                s.next_frame(&mut l, &mut r, Duration::ZERO),
                FrameStatus::Data
            );
        }
        l.fill(9);
        assert_eq!(
            s.next_frame(&mut l, &mut r, Duration::ZERO),
            FrameStatus::Ended
        );
        assert!(l.iter().chain(r.iter()).all(|&b| b == 0));
        assert_eq!(
            s.next_frame(&mut l, &mut r, Duration::ZERO),
            FrameStatus::Ended
        );
    }

    #[test]
    fn loops_forever_when_looping() {
        let bytes = wav_bytes(44_100, 2, 2000);
        let mut s = WavSource::from_reader(Cursor::new(bytes), true, 30.0).unwrap();
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        for _ in 0..20 {
            assert_eq!(
                s.next_frame(&mut l, &mut r, Duration::ZERO),
                FrameStatus::Data
            );
        }
    }

    #[test]
    fn rejects_bad_fps_and_garbage() {
        assert!(matches!(
            WavSource::from_reader(Cursor::new(wav_bytes(44_100, 1, 10)), false, 0.0),
            Err(AudioError::InvalidArgument(_))
        ));
        assert!(matches!(
            WavSource::from_reader(Cursor::new(vec![1, 2, 3]), false, 30.0),
            Err(AudioError::Wav(_))
        ));
        assert!(matches!(
            WavSource::open(Path::new("/nonexistent/x.wav"), false, 30.0),
            Err(AudioError::Io(_))
        ));
    }

    fn write_wav<T: hound::Sample + Copy>(
        spec: hound::WavSpec,
        frames: usize,
        value: T,
    ) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(&mut buf, spec).unwrap();
            for _ in 0..frames * spec.channels as usize {
                w.write_sample(value).unwrap();
            }
            w.finalize().unwrap();
        }
        buf.into_inner()
    }

    fn first_frame(bytes: Vec<u8>) -> ([u8; 576], [u8; 576]) {
        let mut s = WavSource::from_reader(Cursor::new(bytes), false, 30.0).unwrap();
        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        assert_eq!(
            s.next_frame(&mut l, &mut r, Duration::ZERO),
            FrameStatus::Data
        );
        (l, r)
    }

    fn spec(channels: u16, bits: u16, fmt: hound::SampleFormat) -> hound::WavSpec {
        hound::WavSpec {
            channels,
            sample_rate: 44_100,
            bits_per_sample: bits,
            sample_format: fmt,
        }
    }

    #[test]
    fn all_sample_formats_decode_to_half_scale() {
        use hound::SampleFormat::{Float, Int};
        // Each file holds a constant of half full-scale: expect round(0.5 * 127) = 64.
        let cases: Vec<Vec<u8>> = vec![
            write_wav(spec(1, 8, Int), 3000, 64i8),
            write_wav(spec(1, 16, Int), 3000, 16384i16),
            write_wav(spec(1, 24, Int), 3000, 1i32 << 22),
            write_wav(spec(1, 32, Int), 3000, 1i32 << 30),
            write_wav(spec(1, 32, Float), 3000, 0.5f32),
        ];
        for (i, bytes) in cases.into_iter().enumerate() {
            let (l, r) = first_frame(bytes);
            assert!(
                l.iter().all(|&b| b as i8 == 64),
                "case {i}: {}",
                l[575] as i8
            );
            assert_eq!(l, r);
        }
    }

    #[test]
    fn wav_shorter_than_one_window() {
        let (l, r) = first_frame(write_wav(
            spec(1, 16, hound::SampleFormat::Int),
            100,
            16384i16,
        ));
        // Only 100 samples exist: the older 476 outputs are silence.
        assert!(l[..476].iter().all(|&b| b == 0));
        assert!(l[476..].iter().all(|&b| b as i8 == 64));
        assert_eq!(l, r);
    }

    #[test]
    fn more_than_two_channels_use_first_two() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w =
                hound::WavWriter::new(&mut buf, spec(3, 16, hound::SampleFormat::Int)).unwrap();
            for _ in 0..3000 {
                for v in [16384i16, -16384, 32767] {
                    w.write_sample(v).unwrap();
                }
            }
            w.finalize().unwrap();
        }
        let (l, r) = first_frame(buf.into_inner());
        assert!(l.iter().all(|&b| b as i8 == 64));
        assert!(r.iter().all(|&b| b as i8 == -64));
    }

    #[test]
    fn step_rounds_rate_over_fps() {
        let s = WavSource::from_reader(Cursor::new(wav_bytes(48_000, 1, 10)), false, 60.0).unwrap();
        assert_eq!(s.step(), 800);
    }
}

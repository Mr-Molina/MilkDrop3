//! Wave alignment: port of `CPluginShell::AlignWaves` (pluginshell.cpp:2342-2488@6f7cd5d).
//!
//! Each frame, the first `NUM_WAVEFORM_SAMPLES` (480) samples of the new 576-sample
//! wave are matched against the previous frame's window by a coarse-to-fine weighted
//! least-absolute-error search over offsets `0..96`; the new wave is then shifted left by
//! the best offset and samples `480..576` are zeroed (only when the offset is non-zero).
//! The previous frame is remembered *unshifted*, together with its offset.

const WAVE: usize = 576;
/// `NUM_WAVEFORM_SAMPLES` (defines.h:188@6f7cd5d).
pub(crate) const NUM_WAVEFORM_SAMPLES: usize = 480;
/// `floor(log2(576 - 480))`, capped at `MAX_OCTAVES = 10`.
const OCTAVES: usize = 6;

pub(crate) struct WaveAligner {
    old: [[f32; WAVE]; 2],
    prev_offset: [usize; 2],
    /// Samples and search space per octave (576/96 halved each octave).
    spls: [usize; OCTAVES],
    space: [usize; OCTAVES],
    /// Pyramid weights per octave and their first/last non-zero index (inclusive).
    weight: Vec<Vec<f32>>,
    first_nz: [usize; OCTAVES],
    last_nz: [usize; OCTAVES],
}

impl WaveAligner {
    pub(crate) fn new() -> WaveAligner {
        let mut spls = [0; OCTAVES];
        let mut space = [0; OCTAVES];
        spls[0] = WAVE;
        space[0] = WAVE - NUM_WAVEFORM_SAMPLES;
        for o in 1..OCTAVES {
            spls[o] = spls[o - 1] / 2;
            space[o] = space[o - 1] / 2;
        }
        let mut weight = Vec::with_capacity(OCTAVES);
        let mut first_nz = [0; OCTAVES];
        let mut last_nz = [0; OCTAVES];
        for o in 0..OCTAVES {
            let compare = spls[o] - space[o];
            let w: Vec<f32> = (0..compare)
                .map(|n| {
                    let pyramid = if n < compare / 2 {
                        (n * 2) as f32 / compare as f32
                    } else {
                        ((compare - 1 - n) * 2) as f32 / compare as f32
                    };
                    ((pyramid - 0.8) * 5.0 + 0.8).clamp(0.0, 1.0)
                })
                .collect();
            first_nz[o] = w.iter().position(|x| *x != 0.0).unwrap_or(compare);
            last_nz[o] = w.iter().rposition(|x| *x != 0.0).unwrap_or(0);
            weight.push(w);
        }
        WaveAligner {
            old: [[0.0; WAVE]; 2],
            prev_offset: [0; 2],
            spls,
            space,
            weight,
            first_nz,
            last_nz,
        }
    }

    /// Aligns both channels of `wave` in place.
    pub(crate) fn align(&mut self, wave: &mut [[f32; WAVE]; 2]) {
        let mut offset = [0usize; 2];
        for (ch, off) in offset.iter_mut().enumerate() {
            *off = self.best_offset(&wave[ch], ch);
        }
        self.old = *wave;
        self.prev_offset = offset;
        for (w, &off) in wave.iter_mut().zip(&offset) {
            if off > 0 {
                w.copy_within(off..off + NUM_WAVEFORM_SAMPLES, 0);
                w[NUM_WAVEFORM_SAMPLES..].fill(0.0);
            }
        }
    }

    fn best_offset(&self, new: &[f32; WAVE], ch: usize) -> usize {
        // Mip pyramids; level 0 of `old` holds only the 480 compared samples (the C++
        // leaves the rest uninitialised and never reads it through a non-zero weight).
        let mut tnew = vec![new.to_vec()];
        let mut first_old = vec![0f32; WAVE];
        let p = self.prev_offset[ch];
        first_old[..NUM_WAVEFORM_SAMPLES]
            .copy_from_slice(&self.old[ch][p..p + NUM_WAVEFORM_SAMPLES]);
        let mut told = vec![first_old];
        for o in 1..OCTAVES {
            let half = |prev: &[f32]| -> Vec<f32> {
                (0..self.spls[o])
                    .map(|n| 0.5 * (prev[n * 2] + prev[n * 2 + 1]))
                    .collect()
            };
            let (n, d) = (half(&tnew[o - 1]), half(&told[o - 1]));
            tnew.push(n);
            told.push(d);
        }
        let (mut n1, mut n2) = (0usize, self.space[OCTAVES - 1]);
        let mut result = 0;
        for o in (0..OCTAVES).rev() {
            let w = &self.weight[o];
            let mut lowest: Option<(usize, f32)> = None;
            for n in n1..n2 {
                let mut err = 0f32;
                for i in self.first_nz[o]..=self.last_nz[o] {
                    err += ((tnew[o][i + n] - told[o][i]) * w[i]).abs();
                }
                if lowest.is_none_or(|(_, e)| err < e) {
                    lowest = Some((n, err));
                }
            }
            // The range is never empty: the top octave searches 0..3, and below it
            // best < space[o] gives 2*best + 1 < space[o-1] = 2*space[o], so n1 < n2.
            let best = lowest.map_or(0, |(n, _)| n);
            if o > 0 {
                n1 = (best * 2).saturating_sub(1);
                n2 = (best * 2 + 3).min(self.space[o - 1]);
            } else {
                result = best;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A smooth, non-periodic (over the window) signal, so the coarse octaves still
    /// correlate; white noise loses the match after 32x decimation and the
    /// coarse-to-fine search (like the C++) can then settle on a wrong offset.
    fn smooth(len: usize, phase: f32) -> Vec<f32> {
        (0..len)
            .map(|i| {
                let t = i as f32 + phase;
                60.0 * (t * 0.031).sin()
                    + 40.0 * (t * 0.0137 + 1.0).sin()
                    + 20.0 * (t * 0.071 + 2.0).sin()
            })
            .collect()
    }

    #[test]
    fn tables_match_the_cpp_geometry() {
        let a = WaveAligner::new();
        assert_eq!(a.spls, [576, 288, 144, 72, 36, 18]);
        assert_eq!(a.space, [96, 48, 24, 12, 6, 3]);
        assert_eq!(OCTAVES, (96f32.ln() / 2f32.ln()).floor() as usize);
        // weight = clamp((pyramid - 0.8) * 5 + 0.8): zero while pyramid <= 0.64, one from 0.8.
        let w0 = &a.weight[0];
        assert_eq!(w0.len(), 480);
        assert_eq!(a.first_nz[0], 154);
        assert_eq!(a.last_nz[0], 325);
        assert_eq!(w0[153], 0.0);
        assert!(w0[154] > 0.0);
        assert_eq!(w0[240], 1.0);
    }

    #[test]
    fn finds_a_known_shift_against_the_previous_frame() {
        let sig = smooth(800, 0.0);
        for d in [0usize, 1, 7, 20, 63, 95] {
            let mut a = WaveAligner::new();
            a.old[0].copy_from_slice(&sig[100..676]);
            a.prev_offset[0] = 0;
            // new[i + d] == old[i]
            let mut wave = [[0f32; WAVE]; 2];
            wave[0].copy_from_slice(&sig[100 - d..676 - d]);
            let raw = wave[0];
            a.align(&mut wave);
            assert_eq!(a.prev_offset[0], d, "d={d}");
            assert_eq!(a.old[0], raw, "history keeps the unshifted wave");
            assert_eq!(wave[0][..480], sig[100..580], "d={d}");
            if d > 0 {
                assert!(wave[0][480..].iter().all(|v| *v == 0.0));
            } else {
                assert_eq!(wave[0], raw);
            }
        }
    }

    #[test]
    fn uses_the_previous_offset_into_the_history() {
        let sig = smooth(800, 17.0);
        let mut a = WaveAligner::new();
        a.old[1].copy_from_slice(&sig[0..576]);
        a.prev_offset[1] = 30;
        // old window = sig[30..510]; new[i + 12] == sig[30 + i] -> new starts at sig[18].
        let mut wave = [[0f32; WAVE]; 2];
        wave[1].copy_from_slice(&sig[18..594]);
        a.align(&mut wave);
        assert_eq!(a.prev_offset[1], 12);
        assert_eq!(wave[1][..480], sig[30..510]);
    }
}

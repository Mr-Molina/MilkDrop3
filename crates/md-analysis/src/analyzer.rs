//! Per-frame sound analysis: the shell's `AnalyzeNewSound` + `AlignWaves`
//! (pluginshell.cpp:1653-1754, 2342-2488@6f7cd5d) followed by the plugin's
//! `DoCustomSoundAnalysis` (plugin.cpp:8720-8780@6f7cd5d), whose `mysound` values are
//! what presets see as `bass`, `mid`, `treb` and `*_att` (milkdropfs.cpp:483-488@6f7cd5d).

use crate::align::WaveAligner;
use crate::fft::Fft;

/// One frame of analysis results.
///
/// * `imm`, `imm_rel`, `avg`, `avg_rel`, `long_avg`: the plugin's `mysound` (preset-facing).
/// * `med_avg`: the shell's left-channel `m_sound.med_avg` (no consumer in the C++; kept
///   for interface completeness, see the Task 3.2 report).
/// * `spectrum`: the shell's `m_sound.fSpectrum` (custom waves with `bSpectrum`).
/// * `wave`: the shell's `m_sound.fWaveform` after `AlignWaves` (custom waves, the main
///   wave via `mysound.fWave`, and the input of the `mysound` FFT).
#[derive(Debug, Clone, PartialEq)]
pub struct SoundData {
    pub imm: [f32; 3],
    pub imm_rel: [f32; 3],
    pub avg: [f32; 3],
    pub avg_rel: [f32; 3],
    pub long_avg: [f32; 3],
    pub med_avg: [f32; 3],
    pub spectrum: [[f32; 512]; 2],
    pub wave: [[f32; 576]; 2],
}

impl Default for SoundData {
    fn default() -> SoundData {
        SoundData {
            imm: [0.0; 3],
            imm_rel: [0.0; 3],
            avg: [0.0; 3],
            avg_rel: [0.0; 3],
            long_avg: [0.0; 3],
            med_avg: [1.0; 3],
            spectrum: [[0.0; 512]; 2],
            wave: [[0.0; 576]; 2],
        }
    }
}

/// `AdjustRateToFPS` (utility.cpp:80-90@6f7cd5d): the per-frame decay rate at `fps`
/// equivalent to `rate` at `fps1`, i.e. `(rate^fps1)^(1/fps)` evaluated in two f32 steps.
pub fn adjust_rate_to_fps(rate: f32, fps1: f32, fps: f32) -> f32 {
    rate.powf(fps1).powf(1.0 / fps)
}

/// Bins summed into bass/mid/treble by `DoCustomSoundAnalysis`: `512*i/6 .. 512*(i+1)/6`.
const PRESET_BANDS: [(usize, usize); 3] = [(0, 85), (85, 170), (170, 256)];
/// Shell band normalisers (pluginshell.cpp:1718-1725@6f7cd5d).
const SHELL_BAND_NORM: [f32; 3] = [0.326_781_56, 0.380_873_77, 0.199_888_93];

/// Owns the FFT tables, the wave-alignment history and the running averages.
pub struct Analyzer {
    shell_fft: Fft,
    preset_fft: Fft,
    aligner: WaveAligner,
    shell_bands: [(usize, usize); 3],
    /// `GetFrame()` at the next `DoCustomSoundAnalysis` call (starts at 0).
    frame: u64,
    data: SoundData,
}

impl Default for Analyzer {
    fn default() -> Analyzer {
        Analyzer::new()
    }
}

impl Analyzer {
    pub fn new() -> Analyzer {
        Analyzer {
            // m_fftobj.Init(576, NUM_FREQUENCIES): defaults bEqualize=1, envelope 1.0.
            shell_fft: Fft::new(576, 512, true, 1.0),
            // myfft.Init(576, MY_FFT_SAMPLES, -1): the -1 is bEqualize (truthy) and the
            // envelope power keeps its default 1.0 (plugin.cpp:1063@6f7cd5d).
            preset_fft: Fft::new(576, 512, true, 1.0),
            aligner: WaveAligner::new(),
            shell_bands: shell_band_ranges(),
            frame: 0,
            data: SoundData::default(),
        }
    }

    /// Analyzes one frame of 576 unsigned 8-bit samples per channel at the current `fps`.
    pub fn analyze(&mut self, left: &[u8; 576], right: &[u8; 576], fps: f32) -> &SoundData {
        let d = &mut self.data;
        // AnalyzeNewSound: convert, damp into the FFT, spectrum (pre-alignment).
        for (ch, bytes) in [left, right].into_iter().enumerate() {
            for (w, &b) in d.wave[ch].iter_mut().zip(bytes) {
                *w = (((b ^ 128) as i32) - 128) as f32;
            }
            let w = &d.wave[ch];
            let mut damped = [0f32; 576];
            let mut old_i = 0;
            for (i, t) in damped.iter_mut().enumerate() {
                *t = 0.5 * (w[i] + w[old_i]);
                old_i = i;
            }
            self.shell_fft.transform(&damped, &mut d.spectrum[ch]);
        }
        // The shell's own band averages; only med_avg (left channel) is exposed.
        let med_mix = adjust_rate_to_fps(0.91, 14.0, fps);
        for (i, &(start, end)) in self.shell_bands.iter().enumerate() {
            let sum = d.spectrum[0][start..end].iter().fold(0f32, |a, v| a + v);
            let imm = sum / (end - start) as f32 / SHELL_BAND_NORM[i];
            d.med_avg[i] = d.med_avg[i] * med_mix + imm * (1.0 - med_mix);
        }
        // AlignWaves (runs before DrawAndDisplay -> DoCustomSoundAnalysis).
        self.aligner.align(&mut d.wave);
        // DoCustomSoundAnalysis: un-damped FFT of the aligned left channel.
        let mut spec = [0f32; 512];
        self.preset_fft.transform(&d.wave[0], &mut spec);
        let rise = adjust_rate_to_fps(0.2, 30.0, fps);
        let fall = adjust_rate_to_fps(0.5, 30.0, fps);
        let long_rate = adjust_rate_to_fps(if self.frame < 50 { 0.9 } else { 0.992 }, 30.0, fps);
        for (i, &(start, end)) in PRESET_BANDS.iter().enumerate() {
            let imm = spec[start..end].iter().fold(0f32, |a, v| a + v);
            d.imm[i] = imm;
            let rate = if imm > d.avg[i] { rise } else { fall };
            d.avg[i] = d.avg[i] * rate + imm * (1.0 - rate);
            d.long_avg[i] = d.long_avg[i] * long_rate + imm * (1.0 - long_rate);
            let tiny = d.long_avg[i].abs() < 0.001;
            d.imm_rel[i] = if tiny { 1.0 } else { imm / d.long_avg[i] };
            d.avg_rel[i] = if tiny { 1.0 } else { d.avg[i] / d.long_avg[i] };
        }
        self.frame += 1;
        &self.data
    }
}

/// The shell's pitch-spaced band bins (pluginshell.cpp:1690-1713@6f7cd5d): band `i` spans
/// `512 * 200 * m^i / 11025 .. 512 * 200 * m^(i+1) / 11025` with `m^3 = 11025 / 200`.
fn shell_band_ranges() -> [(usize, usize); 3] {
    let (min_freq, max_freq) = (200.0f32, 11025.0f32);
    let net_octaves = (max_freq / min_freq).ln() / 2.0f32.ln();
    let mult = 2.0f32.powf(net_octaves / 3.0);
    let edge = |i: i32| (512.0f32 * min_freq * mult.powf(i as f32) / 11025.0) as i32;
    std::array::from_fn(|i| {
        let start = edge(i as i32).max(0) as usize;
        let end = edge(i as i32 + 1).min(512) as usize;
        (start, end)
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn shell_band_ranges_are_pitch_spaced() {
        assert_eq!(super::shell_band_ranges(), [(9, 35), (35, 134), (134, 512)]);
    }
}

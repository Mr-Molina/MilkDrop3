//! Custom waves (`CPlugin::DrawCustomWaves`, code/vis_milk2/milkdropfs.cpp
//! 2496-2680): the source values of a wave's per-point program and the
//! vertices of the finished wave. The GPU side is
//! [`crate::custom_waves_draw`].

use crate::cast::{c_int, color_byte};
use crate::fixed::Topo;
use crate::vertex::Vertex;
use crate::wave::{smooth_wave, NUM_WAVEFORM_SAMPLES};
use md_analysis::SoundData;

/// A custom wave's inputs to the sample setup (2548-2584), after its
/// per-frame program ran (`LoadCustomWavePerFrameEvallibVars`, then
/// `m_pf_codehandle`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CustomWaveFrame {
    /// `var_pf_samples` (a `double`, read with `(int)`, 2548).
    pub samples: f64,
    /// `m_wave[i].sep` (a preset constant).
    pub sep: i32,
    /// `m_wave[i].scaling`.
    pub scaling: f32,
    /// `m_wave[i].smoothing`.
    pub smoothing: f32,
    /// `m_fWaveScale.eval(-1)`: the unblended value of the wave's own
    /// preset (the old preset's for an old wave).
    pub wave_scale: f32,
    /// `m_wave[i].bSpectrum`.
    pub spectrum: bool,
    /// `m_wave[i].bUseDots` (one sample is enough to draw dots).
    pub use_dots: bool,
}

/// The per-point program's input per sample: `(value1, value2, sample)`.
pub type CustomWaveInput = (f32, f32, f32);

/// The smoothed, scaled source values of one custom wave.
///
/// `n` samples are `(int)samples`, capped (F41) at 512 for a spectrum and
/// `NUM_WAVEFORM_SAMPLES` (480) otherwise, `max_samples` of 2520. The C++
/// caps the per-frame value at 512 only (2548-2549); a waveform of 481..512
/// samples then reads before the start of `fWaveform` (`j0 < 0`), which is
/// undefined there, so the cap of 2520 is applied to it. No samples are
/// returned when `n < 2` (`n < 1` for dots, 2551). Reads that still fall
/// outside the sound data (a large `sep`) see the two channels as one
/// contiguous array, as `float[2][N]` is, and zero beyond it.
///
/// The values are the forward-and-backward smoothed (2563-2578) source data
/// times `mult` (2555, 2580-2584); `sample` is `j / (n - 1)` (2592-2595).
/// md-core runs the per-point program on each (2598-2619).
pub fn custom_wave_inputs(f: &CustomWaveFrame, sound: &SoundData) -> Vec<CustomWaveInput> {
    let max_samples: i32 = if f.spectrum {
        512
    } else {
        NUM_WAVEFORM_SAMPLES as i32
    };
    let n = c_int(f.samples).min(512).min(max_samples);
    if !(n >= 2 || (f.use_dots && n >= 1)) {
        return Vec::new();
    }
    let nu = n as usize;
    let mult = (if f.spectrum { 0.15f32 } else { 0.004 }) * f.scaling * f.wave_scale;
    let (j0, j1) = if f.spectrum {
        (0, 0)
    } else {
        (
            (max_samples - n) / 2 - f.sep / 2,
            (max_samples - n) / 2 + f.sep / 2,
        )
    };
    let t = if f.spectrum {
        (512 - f.sep) as f32 / n as f32
    } else {
        1.0
    };
    let mix1 = (f.smoothing * 0.98).powf(0.5);
    let mix2 = 1.0 - mix1;
    let fetch = |ch: usize, idx: i64| {
        if f.spectrum {
            fetch_flat(&sound.spectrum, ch, idx)
        } else {
            fetch_flat(&sound.wave, ch, idx)
        }
    };
    let at = |j: usize| c_int(f64::from(j as f32 * t)) as i64;
    let mut temp = [vec![0.0f32; nu], vec![0.0f32; nu]];
    temp[0][0] = fetch(0, i64::from(j0));
    temp[1][0] = fetch(1, i64::from(j1));
    for j in 1..nu {
        temp[0][j] = fetch(0, at(j) + i64::from(j0)) * mix2 + temp[0][j - 1] * mix1;
        temp[1][j] = fetch(1, at(j) + i64::from(j1)) * mix2 + temp[1][j - 1] * mix1;
    }
    // Smooth again, backwards (2574-2578).
    for j in (0..nu.saturating_sub(1)).rev() {
        for ch in &mut temp {
            ch[j] = ch[j] * mix2 + ch[j + 1] * mix1;
        }
    }
    let j_mult = 1.0 / (nu as f32 - 1.0);
    (0..nu)
        .map(|j| (temp[0][j] * mult, temp[1][j] * mult, j as f32 * j_mult))
        .collect()
}

/// `data[ch][idx]` as C reads a `float[2][N]`: `idx` may run into the
/// other channel; outside the whole array it reads 0.
fn fetch_flat<const N: usize>(data: &[[f32; N]; 2], ch: usize, idx: i64) -> f32 {
    let flat = (ch * N) as i64 + idx;
    if (0..2 * N as i64).contains(&flat) {
        data[flat as usize / N][flat as usize % N]
    } else {
        0.0
    }
}

/// One point after the per-point program (`var_pp_x..a`, 2612-2619).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CustomWavePoint {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

/// A custom wave ready to draw (F38; filled by md-core).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomWaveDraw {
    pub points: Vec<CustomWavePoint>,
    pub use_dots: bool,
    pub thick: bool,
    pub additive: bool,
}

/// What the geometry depends on besides the wave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CustomWaveView {
    /// `(m_fAspectX, m_fAspectY)`.
    pub aspect: (f32, f32),
    /// `m_nTexSizeX/Y`.
    pub tex: (u32, u32),
    /// `alpha_mult`.
    pub alpha_mult: f32,
}

/// What [`build_custom_wave`] produced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CustomWaveGeometry {
    pub topo: Topo,
    pub verts_per_copy: usize,
    pub copies: usize,
    pub point_size: f32,
}

/// Append the wave's vertices, `verts_per_copy` per copy: the points of
/// 2612-2619 (clip-space with the C++'s flipped y; colour bytes
/// `(int)(c * 255) & 0xFF`, alpha times `alpha_mult`), tessellated with
/// `SmoothWave` unless `use_dots` (2639-2643; each in-between point keeps
/// the colour of the point before it, 2487), then, for a thick line, four
/// copies offset by `+x`, `+y`, `-x` cumulatively (2654-2665). A line wave
/// with fewer than two points draws nothing (`copies` 0). `point_size` is
/// `D3DRS_POINTSIZE` for dots (2650): 2 from a 1024-wide texture, plus 1
/// when thick.
pub fn build_custom_wave(
    w: &CustomWaveDraw,
    view: &CustomWaveView,
    out: &mut Vec<Vertex>,
) -> CustomWaveGeometry {
    let none = CustomWaveGeometry {
        topo: Topo::LineStrip,
        verts_per_copy: 0,
        copies: 0,
        point_size: 1.0,
    };
    if w.points.is_empty() || (!w.use_dots && w.points.len() < 2) {
        return none;
    }
    let (inv_ax, inv_ay) = (1.0 / view.aspect.0, 1.0 / view.aspect.1);
    let verts: Vec<Vertex> = w
        .points
        .iter()
        .map(|p| Vertex {
            pos: [
                (p.x * 2.0 - 1.0) as f32 * inv_ax,
                (p.y * -2.0 + 1.0) as f32 * inv_ay,
                0.0,
            ],
            color: [
                color_byte(p.r, 1.0),
                color_byte(p.g, 1.0),
                color_byte(p.b, 1.0),
                color_byte(p.a, view.alpha_mult),
            ],
            ..Vertex::default()
        })
        .collect();
    let base: Vec<Vertex> = if w.use_dots {
        verts
    } else {
        let pos: Vec<[f32; 2]> = verts.iter().map(|v| [v.pos[0], v.pos[1]]).collect();
        smooth_wave(&pos)
            .into_iter()
            .enumerate()
            .map(|(k, p)| Vertex {
                pos: [p[0], p[1], 0.0],
                color: verts[k / 2].color,
                ..Vertex::default()
            })
            .collect()
    };
    if w.use_dots {
        let size = if view.tex.0 >= 1024 { 2.0 } else { 1.0 } + if w.thick { 1.0 } else { 0.0 };
        out.extend_from_slice(&base);
        return CustomWaveGeometry {
            topo: Topo::Points,
            verts_per_copy: base.len(),
            copies: 1,
            point_size: size,
        };
    }
    let copies = if w.thick { 4 } else { 1 };
    let x_inc = 2.0 / view.tex.0 as f32;
    let y_inc = 2.0 / view.tex.1 as f32;
    let mut off = [0.0f32; 2];
    for it in 0..copies {
        match it {
            1 => off[0] += x_inc,
            2 => off[1] += y_inc,
            3 => off[0] -= x_inc,
            _ => {}
        }
        out.extend(base.iter().map(|v| Vertex {
            pos: [v.pos[0] + off[0], v.pos[1] + off[1], v.pos[2]],
            ..*v
        }));
    }
    CustomWaveGeometry {
        topo: Topo::LineStrip,
        verts_per_copy: base.len(),
        copies,
        point_size: 1.0,
    }
}

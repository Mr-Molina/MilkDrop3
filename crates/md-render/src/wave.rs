//! The built-in waveform (`CPlugin::DrawWave`, code/vis_milk2/milkdropfs.cpp:2682-3293@6f7cd5d).
//!
//! F40: the wave modes are `0..NUM_WAVES` = 0..=7 (md_defines.h:51; the
//! C++ takes `(int)wave_mode % NUM_WAVES`, milkdropfs.cpp:2770); the
//! `case 8` spectrum line in the switch (2905-3086) is unreachable and
//! not ported. The modes, from the switch's comments:
//!
//! | mode | shape | C++ |
//! |---|---|---|
//! | 0 | circular wave | 2815-2859 |
//! | 1 | x-y oscillation going round in a spiral, in time | 2861-2884 |
//! | 2 | centred spiro, constant alpha | 2886-2915 |
//! | 3 | centred spiro, alpha tied to treble | 2916-2945 |
//! | 4 | horizontal "script", left channel, with momentum | 2946-3003 |
//! | 5 | weird explosive complex-number thing | 3005-3035 |
//! | 6 | angle-adjustable left channel | 3037-3170 |
//! | 7 | the same with both channels, `wave_y` the separation | 3037-3170 |
//!
//! F38: [`WaveParams`] is built from the post-equation
//! [`PerFrameVars`] (`var_pf_wave_*`), the C++'s inputs.
//!
//! Positions are the C++'s pre-projection `x, y` (y is negated just before
//! drawing, 3221-3230, "to stay consistent with the pre-VMS milkdrop"); the
//! fixed vertex shader applies the ortho projection (support.cpp:179,
//! clip `y = -y`, Task 6.7a) and the D3D9 half-pixel shift (`VsParams`,
//! ledger Task 4.8), so neither is applied here.

use crate::comp::cosine_interp;
use crate::comp_fixed::rgba_01;
use crate::fixed::Topo;
use crate::frame_vars::PerFrameVars;
use crate::vertex::Vertex;
use md_analysis::SoundData;
use std::ops::Range;

/// `NUM_WAVES` (md_defines.h:51).
pub const NUM_WAVES: i32 = 8;
/// `NUM_WAVEFORM_SAMPLES` (defines.h:188): the valid samples per channel.
pub const NUM_WAVEFORM_SAMPLES: usize = 480;
const WAVE_LEN: usize = 576;

/// The wave's inputs (F38): post-equation `wave_*` values plus the preset's
/// scale/smoothing and the volume alpha modulation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveParams {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    pub x: f32,
    pub y: f32,
    /// `wave_mystery` (`fWaveParam`).
    pub mystery: f32,
    pub use_dots: bool,
    pub thick: bool,
    pub additive: bool,
    /// `wave_brighten` (maximize colour).
    pub maximize_color: bool,
    pub mod_alpha_by_volume: bool,
    pub mod_alpha_start: f32,
    pub mod_alpha_end: f32,
    /// `m_fWaveScale` (divided by 128 when applied).
    pub scale: f32,
    /// `m_fWaveSmoothing`.
    pub smoothing: f32,
}

impl WaveParams {
    /// The values `DrawWave` reads (`(float)(*var_pf_wave_*)`).
    pub fn from_vars(v: &PerFrameVars) -> WaveParams {
        WaveParams {
            r: v.wave_r as f32,
            g: v.wave_g as f32,
            b: v.wave_b as f32,
            a: v.wave_a as f32,
            x: v.wave_x as f32,
            y: v.wave_y as f32,
            mystery: v.wave_mystery as f32,
            use_dots: v.wave_usedots != 0.0,
            thick: v.wave_thick != 0.0,
            additive: v.wave_additive != 0.0,
            maximize_color: v.wave_brighten != 0.0,
            mod_alpha_by_volume: v.mod_wave_alpha_by_volume,
            mod_alpha_start: v.mod_wave_alpha_start,
            mod_alpha_end: v.mod_wave_alpha_end,
            scale: v.wave_scale,
            smoothing: v.wave_smoothing,
        }
    }
}

/// A running preset blend (`m_bBlending`): the old preset's wave mode
/// (`m_nOldWaveMode`, set from the old preset's `m_nWaveMode`,
/// state.cpp:712) and `m_fBlendProgress`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveBlend {
    pub old_mode: i32,
    pub progress: f32,
}

/// The frame's geometry inputs: `(m_fAspectX, m_fAspectY)`, `GetTime()` and
/// the main texture's size (`m_nTexSizeX/Y`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveView {
    pub aspect: (f32, f32),
    pub time: f32,
    pub tex: (u32, u32),
}

/// What [`build_wave`] produced, in `out` order.
#[derive(Clone, Debug, PartialEq)]
pub struct WaveGeometry {
    /// [`Topo::LineStrip`], or [`Topo::Points`] for dots.
    pub topo: Topo,
    /// One `DrawPrimitiveUP` each: per fat-copy, one range, or two when the
    /// wave has a break (mode 7's two channels).
    pub draws: Vec<Range<usize>>,
    /// `wave_additive`: blend `SRCALPHA, ONE` instead of `SRCALPHA,
    /// INVSRCALPHA` (2716-2717).
    pub additive: bool,
}

/// The per-frame scale and smoothing of the sample data
/// (milkdropfs.cpp:850-859): `wave[0] *= scale`, then for `i >= 1` the
/// recursive `wave[i] = wave[i]*scale*(1-s) + wave[i-1]*s`, where
/// `wave[i-1]` is the already smoothed value and `scale` is
/// `wave_scale / 128`.
pub fn scaled_wave(
    wave: &[[f32; WAVE_LEN]; 2],
    wave_scale: f32,
    smoothing: f32,
) -> [[f32; WAVE_LEN]; 2] {
    let scale = wave_scale / 128.0;
    let mix2 = smoothing;
    let mix1 = scale * (1.0 - mix2);
    let mut out = *wave;
    for ch in &mut out {
        ch[0] *= scale;
        for i in 1..WAVE_LEN {
            ch[i] = ch[i] * mix1 + ch[i - 1] * mix2;
        }
    }
    out
}

/// `SmoothWave` (milkdropfs.cpp:2466-2494): a better-than-linear
/// tessellation, `2n - 1` points for `n >= 1` (`n = 0` gives none; the C++
/// would read out of bounds).
pub fn smooth_wave(vi: &[[f32; 2]]) -> Vec<[f32; 2]> {
    const C1: f32 = -0.15;
    const C2: f32 = 1.15;
    const C3: f32 = 1.15;
    const C4: f32 = -0.15;
    let inv_sum = 1.0 / (C1 + C2 + C3 + C4);
    let n = vi.len();
    if n == 0 {
        return Vec::new();
    }
    let mut vo = Vec::with_capacity(2 * n);
    let mut i_below = 0;
    let mut i_above2 = 1;
    for i in 0..n - 1 {
        let i_above = i_above2;
        i_above2 = (n - 1).min(i + 2);
        vo.push(vi[i]);
        let f = |k: usize| {
            (C1 * vi[i_below][k] + C2 * vi[i][k] + C3 * vi[i_above][k] + C4 * vi[i_above2][k])
                * inv_sum
        };
        vo.push([f(0), f(1)]);
        i_below = i;
    }
    vo.push(vi[n - 1]);
    vo
}

/// One mode's raw points before blending and tessellation.
#[derive(Clone, Debug, PartialEq)]
struct RawWave {
    pts: Vec<[f32; 2]>,
    /// `nBreak`: index where the solid line breaks.
    brk: Option<usize>,
    pub alpha: f32,
}

struct Ctx<'a> {
    fl: &'a [f32; WAVE_LEN],
    fr: &'a [f32; WAVE_LEN],
    p: &'a WaveParams,
    pos: (f32, f32),
    param: f32,
    ax: f32,
    ay: f32,
    time: f32,
    tex_w: i32,
    vol: f32,
    treble: f32,
    blending: bool,
}

impl Ctx<'_> {
    /// The volume modulation and clamp shared by every mode (e.g.
    /// milkdropfs.cpp:2830-2833). A NaN (zero-width range) draws nothing.
    fn finish_alpha(&self, mut alpha: f32) -> f32 {
        if self.p.mod_alpha_by_volume {
            alpha *= (self.vol - self.p.mod_alpha_start)
                / (self.p.mod_alpha_end - self.p.mod_alpha_start);
        }
        if alpha.is_nan() {
            return 0.0;
        }
        alpha.clamp(0.0, 1.0)
    }

    /// The `switch (m_nTexSizeX)` alpha factors of modes 2 and 5
    /// (2893-2899, 3012-3018); any other width leaves alpha unchanged.
    fn size_factor(&self) -> f32 {
        match self.tex_w {
            256 => 0.07,
            512 => 0.09,
            1024 => 0.11,
            2048 => 0.13,
            _ => 1.0,
        }
    }
}

fn mode_param(wave: i32, p: f32) -> f32 {
    // 2791-2801: wrap into -1..1 for modes 0, 1 and 4.
    if matches!(wave, 0 | 1 | 4) && !(-1.0..=1.0).contains(&p) {
        let mut q = p * 0.5 + 0.5;
        q -= q.floor();
        q = q.abs();
        return q * 2.0 - 1.0;
    }
    p
}

/// The raw points of one wave mode (`switch (wave)`, 2806-3086).
// The C++ literals 6.28, 3.1416 and 1.57 are kept verbatim (not TAU/PI).
#[allow(clippy::approx_constant)]
fn raw_wave(wave: i32, c: &Ctx) -> RawWave {
    let (fl, fr) = (c.fl, c.fr);
    let (ax, ay) = (c.ax, c.ay);
    let (px, py) = c.pos;
    let wp = mode_param(wave, c.param);
    let mut nv = NUM_WAVEFORM_SAMPLES as i32;
    let mut brk = None;
    let mut alpha = c.p.a;
    let mut pts: Vec<[f32; 2]> = Vec::new();
    let usize_ = |i: i32| i as usize;
    match wave {
        0 => {
            nv /= 2;
            let off = (NUM_WAVEFORM_SAMPLES as i32 - nv) / 2;
            alpha = c.finish_alpha(alpha);
            let inv = 1.0 / (nv - 1) as f32;
            for i in 0..nv {
                let mut rad = 0.5 + 0.4 * fr[usize_(i + off)] + wp;
                let ang = i as f32 * inv * 6.28 + c.time * 0.2;
                if i < nv / 10 {
                    let mut mix = i as f32 / (nv as f32 * 0.1);
                    mix = 0.5 - 0.5 * (mix * 3.1416).cos();
                    let rad2 = 0.5 + 0.4 * fr[usize_(i + nv + off)] + wp;
                    rad = rad2 * (1.0 - mix) + rad * mix;
                }
                pts.push([rad * ang.cos() * ay + px, rad * ang.sin() * ax + py]);
            }
            // Dupe the first vertex to close the circle; skipped while
            // blending (2852-2857).
            if !c.blending {
                pts.push(pts[0]);
            }
        }
        1 => {
            alpha *= 1.25;
            alpha = c.finish_alpha(alpha);
            nv /= 2;
            for i in 0..nv {
                let i = usize_(i);
                let rad = 0.53 + 0.43 * fr[i] + wp;
                let ang = fl[i + 32] * 1.57 + c.time * 2.3;
                pts.push([rad * ang.cos() * ay + px, rad * ang.sin() * ax + py]);
            }
        }
        2 | 3 => {
            if wave == 2 {
                alpha *= c.size_factor();
            } else {
                alpha = match c.tex_w {
                    256 => 0.075,
                    512 => 0.150,
                    1024 => 0.220,
                    2048 => 0.330,
                    _ => alpha,
                };
                alpha *= 1.3;
                alpha *= c.treble.powf(2.0);
            }
            alpha = c.finish_alpha(alpha);
            for i in 0..nv as usize {
                pts.push([fr[i] * ay + px, fl[i + 32] * ax + py]);
            }
        }
        4 => {
            nv = nv.min(c.tex_w / 3);
            let off = (NUM_WAVEFORM_SAMPLES as i32 - nv) / 2;
            alpha = c.finish_alpha(alpha);
            let w1 = 0.45 + 0.5 * (wp * 0.5 + 0.5);
            let w2 = 1.0 - w1;
            let inv = 1.0 / nv as f32;
            for i in 0..nv {
                let k = usize_(i);
                let mut x = -1.0 + 2.0 * (i as f32 * inv) + px;
                let mut y = fl[k + usize_(off)] * 0.47 + py;
                x += fr[k + 25 + usize_(off)] * 0.44;
                if i > 1 {
                    let (a, b) = (pts[k - 1], pts[k - 2]);
                    x = x * w2 + w1 * (a[0] * 2.0 - b[0]);
                    y = y * w2 + w1 * (a[1] * 2.0 - b[1]);
                }
                pts.push([x, y]);
            }
        }
        5 => {
            alpha *= c.size_factor();
            alpha = c.finish_alpha(alpha);
            let (cos_rot, sin_rot) = ((c.time * 0.3).cos(), (c.time * 0.3).sin());
            for i in 0..nv as usize {
                let x0 = fr[i] * fl[i + 32] + fl[i] * fr[i + 32];
                let y0 = fr[i] * fr[i] - fl[i + 32] * fl[i + 32];
                pts.push([
                    (x0 * cos_rot - y0 * sin_rot) * ay + px,
                    (x0 * sin_rot + y0 * cos_rot) * ax + py,
                ]);
            }
        }
        6 | 7 => {
            nv /= 2;
            nv = nv.min(c.tex_w / 3);
            let off = usize_((NUM_WAVEFORM_SAMPLES as i32 - nv) / 2);
            alpha = c.finish_alpha(alpha);
            let ang = 1.57 * wp;
            let (dx, dy) = (ang.cos(), ang.sin());
            let mut ex = [
                px * (ang + 1.57).cos() - dx * 3.0,
                px * (ang + 1.57).cos() + dx * 3.0,
            ];
            let mut ey = [
                px * (ang + 1.57).sin() - dy * 3.0,
                px * (ang + 1.57).sin() + dy * 3.0,
            ];
            // Clip both end points against the screen +/- 1.1 (3095-3150).
            for i in 0..2 {
                for j in 0..4 {
                    let (arr, lim) = (
                        if j < 2 { ex } else { ey },
                        if j % 2 == 0 { 1.1 } else { -1.1 },
                    );
                    let outside = if j % 2 == 0 {
                        arr[i] > lim
                    } else {
                        arr[i] < lim
                    };
                    if outside {
                        let t = (lim - arr[1 - i]) / (arr[i] - arr[1 - i]);
                        let (dxx, dyy) = (ex[i] - ex[1 - i], ey[i] - ey[1 - i]);
                        ex[i] = ex[1 - i] + dxx * t;
                        ey[i] = ey[1 - i] + dyy * t;
                    }
                }
            }
            let dx = (ex[1] - ex[0]) / nv as f32;
            let dy = (ey[1] - ey[0]) / nv as f32;
            let ang2 = dy.atan2(dx);
            let (pdx, pdy) = ((ang2 + 1.57).cos(), (ang2 + 1.57).sin());
            if wave == 6 {
                for i in 0..nv as usize {
                    let s = fl[i + off];
                    pts.push([
                        ex[0] + dx * i as f32 + pdx * 0.25 * s,
                        ey[0] + dy * i as f32 + pdy * 0.25 * s,
                    ]);
                }
            } else {
                let sep = (py * 0.5 + 0.5).powf(2.0);
                for i in 0..nv as usize {
                    let s = 0.25 * fl[i + off] + sep;
                    pts.push([
                        ex[0] + dx * i as f32 + pdx * s,
                        ey[0] + dy * i as f32 + pdy * s,
                    ]);
                }
                for i in 0..nv as usize {
                    let s = 0.25 * fr[i + off] - sep;
                    pts.push([
                        ex[0] + dx * i as f32 + pdx * s,
                        ey[0] + dy * i as f32 + pdy * s,
                    ]);
                }
                brk = Some(nv as usize);
            }
        }
        _ => {
            // No case: the C++ leaves 480 zeroed vertices and the alpha
            // untouched (2784, 2804).
            pts = vec![[0.0, 0.0]; nv as usize];
        }
    }
    RawWave { pts, brk, alpha }
}

/// Build the built-in wave's vertices into `out` (appended; the caller
/// clears it) and say how to draw them. `mode` is the post-modulo mode
/// (`(int)wave_mode % NUM_WAVES`); `blend` is `Some` while a preset blend
/// runs, then the old mode's wave is drawn as well when it differs
/// (`its = 2`, 2771-2772) and the two are mixed as 3185-3230 does:
///
/// * `mix = CosineInterp(progress)`, `mix2 = 1 - mix`;
/// * each of the new wave's points `i` is `v1[i]*mix + v2(i*m)*mix2` with
///   `m = (n2 - 1) / n1`, `v2` linearly interpolated at the fractional
///   index (and where `v2`'s break falls, the new wave breaks too);
/// * alpha is `alpha1*mix + alpha2*mix2`.
///
pub fn build_wave(
    mode: i32,
    blend: Option<WaveBlend>,
    sound: &SoundData,
    p: &WaveParams,
    view: &WaveView,
    out: &mut Vec<Vertex>,
) -> WaveGeometry {
    let WaveView { aspect, time, tex } = *view;
    let wave = scaled_wave(&sound.wave, p.scale, p.smoothing);
    let ctx = Ctx {
        fl: &wave[0],
        fr: &wave[1],
        p,
        pos: (p.x * 2.0 - 1.0, p.y * 2.0 - 1.0),
        param: p.mystery,
        ax: aspect.0,
        ay: aspect.1,
        time,
        tex_w: tex.0 as i32,
        vol: (sound.imm_rel[0] + sound.imm_rel[1] + sound.imm_rel[2]) * 0.333,
        treble: sound.imm[2],
        blending: blend.is_some(),
    };
    let topo = if p.use_dots {
        Topo::Points
    } else {
        Topo::LineStrip
    };
    let empty = WaveGeometry {
        topo,
        draws: Vec::new(),
        additive: p.additive,
    };

    let mut cur = raw_wave(mode, &ctx);
    let old = blend
        .filter(|b| b.old_mode != mode)
        .map(|b| raw_wave(b.old_mode, &ctx));
    if let (Some(b), Some(old)) = (blend, &old) {
        let mix = cosine_interp(b.progress);
        let mix2 = 1.0 - mix;
        let m = (old.pts.len() as f32 - 1.0) / cur.pts.len() as f32;
        for i in 0..cur.pts.len() {
            let f_idx = i as f32 * m;
            let n_idx = f_idx as usize;
            let t = f_idx - n_idx as f32;
            let (x, y);
            if old.brk.is_some_and(|k| n_idx as i64 == k as i64 - 1) {
                x = old.pts[n_idx][0];
                y = old.pts[n_idx][1];
                cur.brk = Some(i + 1);
            } else {
                x = old.pts[n_idx][0] * (1.0 - t) + old.pts[n_idx + 1][0] * t;
                y = old.pts[n_idx][1] * (1.0 - t) + old.pts[n_idx + 1][1] * t;
            }
            cur.pts[i][0] = cur.pts[i][0] * mix + x * mix2;
            cur.pts[i][1] = cur.pts[i][1] * mix + y * mix2;
        }
        cur.alpha = cur.alpha * mix + old.alpha * (1.0 - mix);
    }

    // Colour (2738-2759), alpha, and the y flip (3221-3230).
    let [mut cr, mut cg, mut cb] = [p.r, p.g, p.b].map(|x| x.clamp(0.0, 1.0));
    if p.maximize_color {
        let max = cr.max(cg).max(cb);
        if max > 0.01 {
            cr /= max;
            cg /= max;
            cb /= max;
        }
    }
    if cur.alpha < 0.004 {
        return empty;
    }
    let color = rgba_01(cr, cg, cb, cur.alpha);
    for pt in &mut cur.pts {
        pt[1] = -pt[1];
    }

    // Tessellate once (3190-3211).
    let (pts, brk) = match cur.brk {
        None => (smooth_wave(&cur.pts), None),
        Some(k) => {
            let k = k.min(cur.pts.len());
            let mut a = smooth_wave(&cur.pts[..k]);
            let nb = a.len();
            a.extend(smooth_wave(&cur.pts[k..]));
            (a, Some(nb))
        }
    };

    // Draw copies (3213-3262): fat copies are cumulative offsets.
    let fat = (p.thick || p.use_dots) && tex.0 >= 512;
    let (x_inc, y_inc) = (2.0 / tex.0.max(1) as f32, 2.0 / tex.1.max(1) as f32);
    let mut pts = pts;
    let mut draws = Vec::new();
    for it in 0..if fat { 4 } else { 1 } {
        match it {
            1 => pts.iter_mut().for_each(|v| v[0] += x_inc),
            2 => pts.iter_mut().for_each(|v| v[1] += y_inc),
            3 => pts.iter_mut().for_each(|v| v[0] -= x_inc),
            _ => {}
        }
        let base = out.len();
        out.extend(pts.iter().map(|v| Vertex {
            pos: [v[0], v[1], 0.0],
            color,
            ..Vertex::default()
        }));
        match brk {
            None => draws.push(base..out.len()),
            Some(k) => {
                draws.push(base..base + k);
                draws.push(base + k..out.len());
            }
        }
    }
    WaveGeometry {
        topo,
        draws,
        additive: p.additive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> WaveParams {
        WaveParams {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.8,
            x: 0.5,
            y: 0.5,
            mystery: 0.0,
            use_dots: false,
            thick: false,
            additive: false,
            maximize_color: false,
            mod_alpha_by_volume: false,
            mod_alpha_start: 0.5,
            mod_alpha_end: 1.5,
            scale: 128.0,
            smoothing: 0.0,
        }
    }

    #[test]
    fn smoothing_is_recursive_after_scaling() {
        let mut w = [[0.0f32; WAVE_LEN]; 2];
        w[0][0] = 128.0;
        w[0][1] = 128.0;
        let s = scaled_wave(&w, 128.0, 0.5);
        // scale = 1: w0 = 128; w1 = 128*0.5 + 128*0.5 = 128; w2 = 0 + 64.
        assert_eq!(s[0][0], 128.0);
        assert_eq!(s[0][1], 128.0);
        assert_eq!(s[0][2], 64.0);
        assert_eq!(s[0][3], 32.0);
    }

    #[test]
    fn smooth_wave_doubles_points() {
        let v = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]];
        let s = smooth_wave(&v);
        assert_eq!(s.len(), 5);
        assert_eq!(s[0], v[0]);
        assert_eq!(s[4], v[2]);
        assert!(smooth_wave(&[]).is_empty());
    }

    #[test]
    fn mystery_wraps_for_modes_0_1_4_only() {
        assert_eq!(mode_param(0, 0.5), 0.5);
        assert!((mode_param(0, 1.5) - -0.5).abs() < 1e-6);
        assert_eq!(mode_param(6, 1.5), 1.5);
    }

    #[test]
    fn p_defaults_build() {
        let mut out = Vec::new();
        let g = build_wave(
            2,
            None,
            &SoundData::default(),
            &params(),
            &WaveView {
                aspect: (1.0, 1.0),
                time: 0.0,
                tex: (64, 64),
            },
            &mut out,
        );
        assert_eq!(g.draws.len(), 1);
        assert_eq!(out.len(), 959);
    }
}

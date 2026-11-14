//! `.milk` writer: a port of `CState::Export` (state.cpp:878-988@6f7cd5d),
//! `CWave::Export` (state.cpp:990-1023@6f7cd5d) and `CShape::Export`
//! (state.cpp:1025-1069@6f7cd5d), with `WriteCode` (state.cpp:851-876@6f7cd5d).
//!
//! Every key is written in Export's order with Export's printf format; each
//! format is recorded next to its key below. `%.3f`/`%.5f` lose digits, so
//! `parse_milk(p.to_milk()) == p` holds only for values representable in
//! that precision; one write/parse round trip reaches a fixed point.
//!
//! Layout (exactly as Export):
//! 1. Only when `m_nMaxPSVersion = max(warp_ps, comp_ps) > 0`
//!    (state.cpp:884, 1448@6f7cd5d), BEFORE the section header:
//!    `MILKDROP_PRESET_VERSION=201` (`CUR_MILKDROP_PRESET_VERSION`,
//!    state.h:236@6f7cd5d), `PSVERSION=<max>`, `PSVERSION_WARP`,
//!    `PSVERSION_COMP`.
//! 2. `[preset00]`, the general/wave/motion scalars, the 4 custom waves
//!    (scalars then code), the 4 custom shapes (scalars then code).
//! 3. `per_frame_init_`, `per_frame_`, `per_pixel_` code, then `warp_` code
//!    only when `warp_ps >= MD2_PS_2_0` (2) and `comp_` code only when
//!    `comp_ps >= 2` (state.cpp:980-983@6f7cd5d).
//!
//! Code lines: `WriteCode` splits the buffer on the line terminator and
//! writes `<prefix><n>=<line>`. Warp/comp lines are ALL prefixed with a
//! backtick (`bPrependApostrophe = true`), whatever they start with; EEL
//! lines never are. The flat reader keys on the text before the first `=`,
//! so a raw EEL line starting with `[` or `;` still reads back intact. The
//! one C++ loss mirrored here: an EEL line whose stored text starts with a
//! backtick (file had two) loses it on the next read.

use std::fmt::Write;

use crate::preset::Preset;
use crate::shape::ShapeData;
use crate::wave::WaveData;

/// `CUR_MILKDROP_PRESET_VERSION` (state.h:236@6f7cd5d).
const CUR_MILKDROP_PRESET_VERSION: i32 = 201;
/// `MD2_PS_2_0` (plugin.h:95@6f7cd5d).
const MD2_PS_2_0: i32 = 2;

/// C `printf("%.<prec>f", (double)v)`. Rust's fixed-precision formatting of
/// an `f64` rounds the exact binary value like glibc (ties to even, sign
/// kept on values that round to zero); only the non-finite spellings
/// differ, so they are spelled as glibc does.
fn c_fixed(v: f32, prec: usize) -> String {
    if v.is_nan() {
        if v.is_sign_negative() { "-nan" } else { "nan" }.to_string()
    } else if v.is_infinite() {
        if v < 0.0 { "-inf" } else { "inf" }.to_string()
    } else {
        format!("{:.prec$}", f64::from(v))
    }
}

struct Out(String);

impl Out {
    /// `"%s=%.3f\n"`.
    fn f3(&mut self, key: &str, v: f32) {
        self.line(key, &c_fixed(v, 3));
    }
    /// `"%s=%.5f\n"`.
    fn f5(&mut self, key: &str, v: f32) {
        self.line(key, &c_fixed(v, 5));
    }
    /// `"%s=%d\n"`.
    fn d(&mut self, key: &str, v: i32) {
        self.line(key, &v.to_string());
    }
    /// `"%s=%d\n"` of a C++ `bool` (0/1).
    fn b(&mut self, key: &str, v: bool) {
        self.d(key, i32::from(v));
    }
    fn line(&mut self, key: &str, value: &str) {
        let _ = writeln!(self.0, "{key}={value}");
    }

    /// `WriteCode`. The C++ buffer ends every line with a terminator, so a
    /// buffer of N lines writes N lines; the Rust text joins lines with
    /// `'\n'`, so `split('\n')` recovers them. Empty text is ambiguous (no
    /// line, or one empty line): EEL blocks write nothing (an empty C++
    /// buffer, as after `Default`), shader blocks write one empty line,
    /// because after `Import` a shader buffer is never empty -- `parse_milk`
    /// generates text when no line was read -- so empty text means a lone
    /// `warp_1=` was read.
    fn code(&mut self, prefix: &str, text: &str, backtick: bool) {
        if text.is_empty() && !backtick {
            return;
        }
        let tick = if backtick { "`" } else { "" };
        for (n, line) in text.split('\n').enumerate() {
            let _ = writeln!(self.0, "{prefix}{}={tick}{line}", n + 1);
        }
    }
}

impl Preset {
    /// Serialize as `.milk` text, as `CState::Export` writes it.
    pub fn to_milk(&self) -> String {
        let mut o = Out(String::new());
        let max_ps = self.warp_ps.max(self.comp_ps);
        if max_ps > 0 {
            o.d("MILKDROP_PRESET_VERSION", CUR_MILKDROP_PRESET_VERSION);
            o.d("PSVERSION", max_ps);
            o.d("PSVERSION_WARP", self.warp_ps);
            o.d("PSVERSION_COMP", self.comp_ps);
        }
        o.0.push_str("[preset00]\n");

        o.f3("fRating", self.rating); // %.3f
        o.f3("fGammaAdj", self.gamma_adj.value()); // %.3f
        o.f3("fDecay", self.decay.value()); // %.3f
        o.f3("fVideoEchoZoom", self.video_echo_zoom.value()); // %.3f
        o.f3("fVideoEchoAlpha", self.video_echo_alpha.value()); // %.3f
        o.d("nVideoEchoOrientation", self.video_echo_orientation); // %d

        o.d("nWaveMode", self.wave_mode); // %d
        o.b("bAdditiveWaves", self.additive_waves); // %d
        o.b("bWaveDots", self.wave_dots); // %d
        o.b("bWaveThick", self.wave_thick); // %d
        o.b("bModWaveAlphaByVolume", self.mod_wave_alpha_by_volume); // %d
        o.b("bMaximizeWaveColor", self.maximize_wave_color); // %d
        o.b("bTexWrap", self.tex_wrap); // %d
        o.b("bDarkenCenter", self.darken_center); // %d
        o.b("bRedBlueStereo", self.red_blue_stereo); // %d
        o.b("bBrighten", self.brighten); // %d
        o.b("bDarken", self.darken); // %d
        o.b("bSolarize", self.solarize); // %d
        o.b("bInvert", self.invert); // %d

        o.f3("fWaveAlpha", self.wave_alpha.value()); // %.3f
        o.f3("fWaveScale", self.wave_scale.value()); // %.3f
        o.f3("fWaveSmoothing", self.wave_smoothing.value()); // %.3f
        o.f3("fWaveParam", self.wave_param.value()); // %.3f
        o.f3("fModWaveAlphaStart", self.mod_wave_alpha_start.value()); // %.3f
        o.f3("fModWaveAlphaEnd", self.mod_wave_alpha_end.value()); // %.3f
        o.f3("fWarpAnimSpeed", self.warp_anim_speed); // %.3f
        o.f3("fWarpScale", self.warp_scale.value()); // %.3f
        o.f5("fZoomExponent", self.zoom_exponent.value()); // %.5f
        o.f3("fShader", self.shader.value()); // %.3f

        o.f5("zoom", self.zoom.value()); // %.5f
        o.f5("rot", self.rot.value()); // %.5f
        o.f3("cx", self.cx.value()); // %.3f
        o.f3("cy", self.cy.value()); // %.3f
        o.f5("dx", self.dx.value()); // %.5f
        o.f5("dy", self.dy.value()); // %.5f
        o.f5("warp", self.warp.value()); // %.5f
        o.f5("sx", self.sx.value()); // %.5f
        o.f5("sy", self.sy.value()); // %.5f
        o.f3("wave_r", self.wave_r.value()); // %.3f
        o.f3("wave_g", self.wave_g.value()); // %.3f
        o.f3("wave_b", self.wave_b.value()); // %.3f
        o.f3("wave_x", self.wave_x.value()); // %.3f
        o.f3("wave_y", self.wave_y.value()); // %.3f

        o.f3("ob_size", self.ob_size.value()); // %.3f
        o.f3("ob_r", self.ob_r.value()); // %.3f
        o.f3("ob_g", self.ob_g.value()); // %.3f
        o.f3("ob_b", self.ob_b.value()); // %.3f
        o.f3("ob_a", self.ob_a.value()); // %.3f
        o.f3("ib_size", self.ib_size.value()); // %.3f
        o.f3("ib_r", self.ib_r.value()); // %.3f
        o.f3("ib_g", self.ib_g.value()); // %.3f
        o.f3("ib_b", self.ib_b.value()); // %.3f
        o.f3("ib_a", self.ib_a.value()); // %.3f
        o.f3("nMotionVectorsX", self.mv_x.value()); // %.3f
        o.f3("nMotionVectorsY", self.mv_y.value()); // %.3f
        o.f3("mv_dx", self.mv_dx.value()); // %.3f
        o.f3("mv_dy", self.mv_dy.value()); // %.3f
        o.f3("mv_l", self.mv_l.value()); // %.3f
        o.f3("mv_r", self.mv_r.value()); // %.3f
        o.f3("mv_g", self.mv_g.value()); // %.3f
        o.f3("mv_b", self.mv_b.value()); // %.3f
        o.f3("mv_a", self.mv_a.value()); // %.3f
        o.f3("b1n", self.blur1_min.value()); // %.3f
        o.f3("b2n", self.blur2_min.value()); // %.3f
        o.f3("b3n", self.blur3_min.value()); // %.3f
        o.f3("b1x", self.blur1_max.value()); // %.3f
        o.f3("b2x", self.blur2_max.value()); // %.3f
        o.f3("b3x", self.blur3_max.value()); // %.3f
        o.f3("b1ed", self.blur1_edge_darken.value()); // %.3f

        for (i, w) in self.waves.iter().enumerate() {
            write_wave(&mut o, i, w);
        }
        for (i, s) in self.shapes.iter().enumerate() {
            write_shape(&mut o, i, s);
        }

        o.code("per_frame_init_", &self.per_frame_init, false);
        o.code("per_frame_", &self.per_frame, false);
        o.code("per_pixel_", &self.per_pixel, false);
        if self.warp_ps >= MD2_PS_2_0 {
            o.code("warp_", &self.warp_shader, true);
        }
        if self.comp_ps >= MD2_PS_2_0 {
            o.code("comp_", &self.comp_shader, true);
        }
        o.0
    }
}

/// `CWave::Export` (state.cpp:990-1023@6f7cd5d).
fn write_wave(o: &mut Out, i: usize, w: &WaveData) {
    let k = |name: &str| format!("wavecode_{i}_{name}");
    o.d(&k("enabled"), w.enabled); // %d
    o.d(&k("samples"), w.samples); // %d
    o.d(&k("sep"), w.sep); // %d
    o.d(&k("bSpectrum"), w.spectrum); // %d
    o.d(&k("bUseDots"), w.use_dots); // %d
    o.d(&k("bDrawThick"), w.draw_thick); // %d
    o.d(&k("bAdditive"), w.additive); // %d
    o.f5(&k("scaling"), w.scaling); // %.5f
    o.f5(&k("smoothing"), w.smoothing); // %.5f
    o.f3(&k("r"), w.r); // %.3f
    o.f3(&k("g"), w.g); // %.3f
    o.f3(&k("b"), w.b); // %.3f
    o.f3(&k("a"), w.a); // %.3f
    o.code(&format!("wave_{i}_init"), &w.init, false);
    o.code(&format!("wave_{i}_per_frame"), &w.per_frame, false);
    o.code(&format!("wave_{i}_per_point"), &w.per_point, false);
}

/// `CShape::Export` (state.cpp:1025-1069@6f7cd5d). No per-point code.
fn write_shape(o: &mut Out, i: usize, s: &ShapeData) {
    let k = |name: &str| format!("shapecode_{i}_{name}");
    o.d(&k("enabled"), s.enabled); // %d
    o.d(&k("sides"), s.sides); // %d
    o.d(&k("additive"), s.additive); // %d
    o.d(&k("thickOutline"), s.thick_outline); // %d
    o.d(&k("textured"), s.textured); // %d
    o.d(&k("num_inst"), s.instances); // %d
    o.f3(&k("x"), s.x); // %.3f
    o.f3(&k("y"), s.y); // %.3f
    o.f5(&k("rad"), s.rad); // %.5f
    o.f5(&k("ang"), s.ang); // %.5f
    o.f5(&k("tex_ang"), s.tex_ang); // %.5f
    o.f5(&k("tex_zoom"), s.tex_zoom); // %.5f
    o.f3(&k("r"), s.r); // %.3f
    o.f3(&k("g"), s.g); // %.3f
    o.f3(&k("b"), s.b); // %.3f
    o.f3(&k("a"), s.a); // %.3f
    o.f3(&k("r2"), s.r2); // %.3f
    o.f3(&k("g2"), s.g2); // %.3f
    o.f3(&k("b2"), s.b2); // %.3f
    o.f3(&k("a2"), s.a2); // %.3f
    o.f3(&k("border_r"), s.border_r); // %.3f
    o.f3(&k("border_g"), s.border_g); // %.3f
    o.f3(&k("border_b"), s.border_b); // %.3f
    o.f3(&k("border_a"), s.border_a); // %.3f
    o.code(&format!("shape_{i}_init"), &s.init, false);
    o.code(&format!("shape_{i}_per_frame"), &s.per_frame, false);
}

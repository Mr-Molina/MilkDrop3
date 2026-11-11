//! `.milk` parser: a port of `CState::Import` (state.cpp:1263-1456@6f7cd5d)
//! for `ApplyFlags == STATE_ALL`.
//!
//! Import first calls `Default(STATE_ALL)` and then reads every key with the
//! CURRENT value as its fallback, so a missing key keeps its default -- with
//! two C++ quirks that are mirrored here because they are observable:
//!
//! * `wave_r`, `wave_g`, `wave_b`, `wave_x`, `wave_y` fall back to
//!   `m_fRot.eval(-1)` (state.cpp:1368-1372@6f7cd5d), and `rot` is only read later,
//!   so a missing `wave_r` parses as the DEFAULT rot, `0.0`, not `1.0`.
//! * `mv_a` falls back to `bMotionVectorsOn != 0 ? 1 : 0` (state.cpp:1381@6f7cd5d),
//!   so a file with neither key gets `mv_a = 0.0`, not `1.0`.
//!
//! Lookups use the section-blind flat model ([`Ini::flat_get`]), as C++'s
//! `GetFast*` do. A leading UTF-8 BOM is stripped (intentional divergence).

use std::fmt;

use crate::gen_shader::{gen_comp_shader, gen_warp_shader, CompShaderParams};
use crate::ini::Ini;
use crate::preset::Preset;

/// Error from [`parse_milk`].
///
/// C++ `Import` fails only when the file cannot be opened; any readable text
/// loads (unknown lines are ignored), so `parse_milk` currently never
/// returns this. It exists so callers handle failure without an API change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Human-readable reason.
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse `.milk` text into a [`Preset`], as `CState::Import` would.
///
/// Shader versions (state.cpp:1309-1327@6f7cd5d): `MILKDROP_PRESET_VERSION` (default
/// 100) `< 200` means no shaders (warp = comp = 0) whatever the other keys
/// say; `== 200` reads `PSVERSION` (default 2) for both; `> 200` reads
/// `PSVERSION_WARP` and `PSVERSION_COMP` (each default 2) and IGNORES
/// `PSVERSION`.
///
/// Empty shader text is filled with [`gen_warp_shader`] /
/// [`gen_comp_shader`] exactly where Import does (state.cpp:1432-1434,
/// 1442-1444@6f7cd5d): whenever `ReadCode` read no line, regardless of the version.
pub fn parse_milk(text: &str) -> Result<Preset, ParseError> {
    let ini = Ini::parse(text);
    let mut p = Preset::defaults();

    let version = ini.flat_i32("MILKDROP_PRESET_VERSION", 100);
    let (warp_ps, comp_ps) = match version {
        v if v < 200 => (0, 0),
        200 => {
            let v = ini.flat_i32("PSVERSION", 2);
            (v, v)
        }
        _ => (
            ini.flat_i32("PSVERSION_WARP", 2),
            ini.flat_i32("PSVERSION_COMP", 2),
        ),
    };

    read_general(&ini, &mut p);
    read_wave(&ini, &mut p);
    read_motion(&ini, &mut p);

    // warp shader (state.cpp:1429-1436@6f7cd5d)
    p.warp_shader = ini.code_block("warp_");
    if !ini.has_code("warp_") {
        p.warp_shader = gen_warp_shader(p.decay.value(), p.tex_wrap);
    }
    p.warp_ps = warp_ps;

    // comp shader (state.cpp:1439-1446@6f7cd5d)
    p.comp_shader = ini.code_block("comp_");
    if !ini.has_code("comp_") {
        p.comp_shader = gen_comp_shader(&CompShaderParams {
            gamma: p.gamma_adj.value(),
            echo_alpha: p.video_echo_alpha.value(),
            echo_zoom: p.video_echo_zoom.value(),
            echo_orient: p.video_echo_orientation,
            hue_shader: p.shader.value(),
            brighten: p.brighten,
            darken: p.darken,
            solarize: p.solarize,
            invert: p.invert,
        });
    }
    p.comp_ps = comp_ps;

    Ok(p)
}

/// `GetFastFloat(key, current)` into a blendable (`operator=`).
macro_rules! float {
    ($ini:expr, $key:literal, $field:expr) => {
        $field.set($ini.flat_f32($key, $field.value()))
    };
}

/// `GetFastInt(key, current) != 0`.
fn flag(ini: &Ini, key: &str, cur: bool) -> bool {
    ini.flat_i32(key, i32::from(cur)) != 0
}

/// STATE_GENERAL block (state.cpp:1330-1351@6f7cd5d).
fn read_general(ini: &Ini, p: &mut Preset) {
    p.rating = ini.flat_f32("fRating", p.rating);
    float!(ini, "fDecay", p.decay);
    float!(ini, "fGammaAdj", p.gamma_adj);
    float!(ini, "fVideoEchoZoom", p.video_echo_zoom);
    float!(ini, "fVideoEchoAlpha", p.video_echo_alpha);
    p.video_echo_orientation = ini.flat_i32("nVideoEchoOrientation", p.video_echo_orientation);
    p.red_blue_stereo = flag(ini, "bRedBlueStereo", p.red_blue_stereo);
    p.brighten = flag(ini, "bBrighten", p.brighten);
    p.darken = flag(ini, "bDarken", p.darken);
    p.solarize = flag(ini, "bSolarize", p.solarize);
    p.invert = flag(ini, "bInvert", p.invert);
    float!(ini, "fShader", p.shader);
    float!(ini, "b1n", p.blur1_min);
    float!(ini, "b2n", p.blur2_min);
    float!(ini, "b3n", p.blur3_min);
    float!(ini, "b1x", p.blur1_max);
    float!(ini, "b2x", p.blur2_max);
    float!(ini, "b3x", p.blur3_max);
    float!(ini, "b1ed", p.blur1_edge_darken);
}

/// STATE_WAVE block (state.cpp:1354-1391@6f7cd5d), including custom waves/shapes.
fn read_wave(ini: &Ini, p: &mut Preset) {
    p.wave_mode = ini.flat_i32("nWaveMode", p.wave_mode);
    p.additive_waves = flag(ini, "bAdditiveWaves", p.additive_waves);
    p.wave_dots = flag(ini, "bWaveDots", p.wave_dots);
    p.wave_thick = flag(ini, "bWaveThick", p.wave_thick);
    p.mod_wave_alpha_by_volume = flag(ini, "bModWaveAlphaByVolume", p.mod_wave_alpha_by_volume);
    p.maximize_wave_color = flag(ini, "bMaximizeWaveColor", p.maximize_wave_color);
    float!(ini, "fWaveAlpha", p.wave_alpha);
    float!(ini, "fWaveScale", p.wave_scale);
    float!(ini, "fWaveSmoothing", p.wave_smoothing);
    float!(ini, "fWaveParam", p.wave_param);
    float!(ini, "fModWaveAlphaStart", p.mod_wave_alpha_start);
    float!(ini, "fModWaveAlphaEnd", p.mod_wave_alpha_end);
    // C++ quirk: the fallback is m_fRot (still its default), not the wave's own.
    let rot = p.rot.value();
    p.wave_r.set(ini.flat_f32("wave_r", rot));
    p.wave_g.set(ini.flat_f32("wave_g", rot));
    p.wave_b.set(ini.flat_f32("wave_b", rot));
    p.wave_x.set(ini.flat_f32("wave_x", rot));
    p.wave_y.set(ini.flat_f32("wave_y", rot));
    float!(ini, "nMotionVectorsX", p.mv_x);
    float!(ini, "nMotionVectorsY", p.mv_y);
    float!(ini, "mv_dx", p.mv_dx);
    float!(ini, "mv_dy", p.mv_dy);
    float!(ini, "mv_l", p.mv_l);
    float!(ini, "mv_r", p.mv_r);
    float!(ini, "mv_g", p.mv_g);
    float!(ini, "mv_b", p.mv_b);
    // C++ quirk: legacy bMotionVectorsOn (default false) seeds mv_a's fallback.
    let on = ini.flat_i32("bMotionVectorsOn", 0) != 0;
    p.mv_a.set(if on { 1.0 } else { 0.0 });
    float!(ini, "mv_a", p.mv_a);
    for (i, w) in p.waves.iter_mut().enumerate() {
        w.import(ini, i);
    }
    for (i, s) in p.shapes.iter_mut().enumerate() {
        s.import(ini, i);
    }
}

/// STATE_MOTION block (state.cpp:1394-1426@6f7cd5d), including the EEL code.
fn read_motion(ini: &Ini, p: &mut Preset) {
    float!(ini, "zoom", p.zoom);
    float!(ini, "rot", p.rot);
    float!(ini, "cx", p.cx);
    float!(ini, "cy", p.cy);
    float!(ini, "dx", p.dx);
    float!(ini, "dy", p.dy);
    float!(ini, "warp", p.warp);
    float!(ini, "sx", p.sx);
    float!(ini, "sy", p.sy);
    p.tex_wrap = flag(ini, "bTexWrap", p.tex_wrap);
    p.darken_center = flag(ini, "bDarkenCenter", p.darken_center);
    p.warp_anim_speed = ini.flat_f32("fWarpAnimSpeed", p.warp_anim_speed);
    float!(ini, "fWarpScale", p.warp_scale);
    float!(ini, "fZoomExponent", p.zoom_exponent);
    float!(ini, "ob_size", p.ob_size);
    float!(ini, "ob_r", p.ob_r);
    float!(ini, "ob_g", p.ob_g);
    float!(ini, "ob_b", p.ob_b);
    float!(ini, "ob_a", p.ob_a);
    float!(ini, "ib_size", p.ib_size);
    float!(ini, "ib_r", p.ib_r);
    float!(ini, "ib_g", p.ib_g);
    float!(ini, "ib_b", p.ib_b);
    float!(ini, "ib_a", p.ib_a);
    p.per_frame_init = ini.code_block("per_frame_init_");
    p.per_frame = ini.code_block("per_frame_");
    p.per_pixel = ini.code_block("per_pixel_");
}

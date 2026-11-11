//! The preset model: every `.milk` key as a typed field, plus code blocks.
//!
//! Field types follow `CState` (state.h): `CBlendableFloat` members are
//! [`Blendable`], plain `float`/`int`/`bool` members are `f32`/`i32`/`bool`.
//!
//! Code blocks hold the text exactly as `ReadCode` assembles it (lines joined
//! with `'\n'`, one leading backtick stripped, comments kept), so the writer
//! can reproduce them. C++ applies `StripLinefeedCharsAndComments` only when
//! compiling (`RecompileExpressions`, state.cpp:1665ff), never at load; the
//! `*_eel` accessors give that stripped form, which is what md-eval compiles.

use crate::blend::Blendable;
use crate::ini::strip_code;
use crate::shape::ShapeData;
use crate::wave::WaveData;

/// `MAX_CUSTOM_WAVES` (md_defines.h).
pub const MAX_CUSTOM_WAVES: usize = 4;
/// `MAX_CUSTOM_SHAPES` (md_defines.h).
pub const MAX_CUSTOM_SHAPES: usize = 4;

/// Pixel-shader model a preset asks for (`MD2_PS_*`, plugin.h:94-98).
///
/// Variant names follow the A.3 interface (`Ps2_X` mirrors `MD2_PS_2_X`).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsVersion {
    /// `0`: MilkDrop 1 era, no preset shaders.
    None,
    /// `2`: ps_2_0.
    Ps2_0,
    /// `3`: ps_2_x (ps_2_a, then ps_2_b).
    Ps2_X,
    /// `4`: ps_3_0.
    Ps3_0,
    /// `5`: ps_4_0 ("not supported by milkdrop").
    Ps4_0,
}

impl PsVersion {
    /// Map a stored `m_n*PSVersion` value. `<= 0` is `None` (C++ tests
    /// `> 0` for "uses a shader", milkdropfs.cpp:862). Values the C++ switch
    /// does not know (`1`, `>= 6`) hit `default: assert(0)` and a `ps_0_0`
    /// profile that never compiles; here they clamp to the nearest model.
    pub fn from_file_value(v: i32) -> PsVersion {
        match v {
            i32::MIN..=0 => PsVersion::None,
            1 | 2 => PsVersion::Ps2_0,
            3 => PsVersion::Ps2_X,
            4 => PsVersion::Ps3_0,
            _ => PsVersion::Ps4_0,
        }
    }
}

/// A MilkDrop preset (`CState`'s persistent part).
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// `m_nWarpPSVersion`, as resolved by `Import` from the header keys (see
    /// [`crate::parse_milk`]). Raw so the writer can echo it.
    pub warp_ps: i32,
    /// `m_nCompPSVersion`.
    pub comp_ps: i32,

    /// `fRating`.
    pub rating: f32,
    /// `fGammaAdj`.
    pub gamma_adj: Blendable,
    /// `fDecay`.
    pub decay: Blendable,
    /// `fVideoEchoZoom`.
    pub video_echo_zoom: Blendable,
    /// `fVideoEchoAlpha`.
    pub video_echo_alpha: Blendable,
    /// `nVideoEchoOrientation` (0..3).
    pub video_echo_orientation: i32,
    /// `nWaveMode`.
    pub wave_mode: i32,
    /// `bAdditiveWaves`.
    pub additive_waves: bool,
    /// `bWaveDots`.
    pub wave_dots: bool,
    /// `bWaveThick`.
    pub wave_thick: bool,
    /// `bModWaveAlphaByVolume`.
    pub mod_wave_alpha_by_volume: bool,
    /// `bMaximizeWaveColor`.
    pub maximize_wave_color: bool,
    /// `bTexWrap`.
    pub tex_wrap: bool,
    /// `bDarkenCenter`.
    pub darken_center: bool,
    /// `bRedBlueStereo`.
    pub red_blue_stereo: bool,
    /// `bBrighten`.
    pub brighten: bool,
    /// `bDarken`.
    pub darken: bool,
    /// `bSolarize`.
    pub solarize: bool,
    /// `bInvert`.
    pub invert: bool,
    /// `fWaveAlpha`.
    pub wave_alpha: Blendable,
    /// `fWaveScale`.
    pub wave_scale: Blendable,
    /// `fWaveSmoothing`.
    pub wave_smoothing: Blendable,
    /// `fWaveParam`.
    pub wave_param: Blendable,
    /// `fModWaveAlphaStart`.
    pub mod_wave_alpha_start: Blendable,
    /// `fModWaveAlphaEnd`.
    pub mod_wave_alpha_end: Blendable,
    /// `fWarpAnimSpeed` (plain float in `CState`, not blendable).
    pub warp_anim_speed: f32,
    /// `fWarpScale`.
    pub warp_scale: Blendable,
    /// `fZoomExponent`.
    pub zoom_exponent: Blendable,
    /// `fShader`.
    pub shader: Blendable,

    /// `zoom`.
    pub zoom: Blendable,
    /// `rot`.
    pub rot: Blendable,
    /// `cx`.
    pub cx: Blendable,
    /// `cy`.
    pub cy: Blendable,
    /// `dx`.
    pub dx: Blendable,
    /// `dy`.
    pub dy: Blendable,
    /// `warp`.
    pub warp: Blendable,
    /// `sx`.
    pub sx: Blendable,
    /// `sy`.
    pub sy: Blendable,
    /// `wave_r`.
    pub wave_r: Blendable,
    /// `wave_g`.
    pub wave_g: Blendable,
    /// `wave_b`.
    pub wave_b: Blendable,
    /// `wave_x`.
    pub wave_x: Blendable,
    /// `wave_y`.
    pub wave_y: Blendable,
    /// `ob_size`.
    pub ob_size: Blendable,
    /// `ob_r`.
    pub ob_r: Blendable,
    /// `ob_g`.
    pub ob_g: Blendable,
    /// `ob_b`.
    pub ob_b: Blendable,
    /// `ob_a`.
    pub ob_a: Blendable,
    /// `ib_size`.
    pub ib_size: Blendable,
    /// `ib_r`.
    pub ib_r: Blendable,
    /// `ib_g`.
    pub ib_g: Blendable,
    /// `ib_b`.
    pub ib_b: Blendable,
    /// `ib_a`.
    pub ib_a: Blendable,
    /// `nMotionVectorsX` (`m_fMvX`, read as a float).
    pub mv_x: Blendable,
    /// `nMotionVectorsY` (`m_fMvY`, read as a float).
    pub mv_y: Blendable,
    /// `mv_dx`.
    pub mv_dx: Blendable,
    /// `mv_dy`.
    pub mv_dy: Blendable,
    /// `mv_l`.
    pub mv_l: Blendable,
    /// `mv_r`.
    pub mv_r: Blendable,
    /// `mv_g`.
    pub mv_g: Blendable,
    /// `mv_b`.
    pub mv_b: Blendable,
    /// `mv_a` (legacy `bMotionVectorsOn` seeds its fallback).
    pub mv_a: Blendable,
    /// `b1n`.
    pub blur1_min: Blendable,
    /// `b2n`.
    pub blur2_min: Blendable,
    /// `b3n`.
    pub blur3_min: Blendable,
    /// `b1x`.
    pub blur1_max: Blendable,
    /// `b2x`.
    pub blur2_max: Blendable,
    /// `b3x`.
    pub blur3_max: Blendable,
    /// `b1ed`.
    pub blur1_edge_darken: Blendable,

    /// `wavecode_0_*` .. `wavecode_3_*` and their code.
    pub waves: [WaveData; MAX_CUSTOM_WAVES],
    /// `shapecode_0_*` .. `shapecode_3_*` and their code.
    pub shapes: [ShapeData; MAX_CUSTOM_SHAPES],

    /// `per_frame_init_<N>` lines.
    pub per_frame_init: String,
    /// `per_frame_<N>` lines.
    pub per_frame: String,
    /// `per_pixel_<N>` lines.
    pub per_pixel: String,
    /// `warp_<N>` lines, or the generated default when the file has none.
    pub warp_shader: String,
    /// `comp_<N>` lines, or the generated default when the file has none.
    pub comp_shader: String,
}

/// Expands `$m!(name, field)` once per blendable field, in `CState`
/// declaration order.
///
/// These are exactly the members `CState` declares as `CBlendableFloat` in
/// `code/vis_milk2/state.h:264-351` (the commented-out `m_fPlate*` block at
/// state.h:302-307 excluded). Task 2.5 tests that each one interpolates
/// under `start_blend`.
macro_rules! for_each_blendable {
    ($m:ident) => {
        $m! {
            gamma_adj,
            video_echo_zoom,
            video_echo_alpha,
            decay,
            wave_alpha,
            wave_scale,
            wave_smoothing,
            wave_param,
            mod_wave_alpha_start,
            mod_wave_alpha_end,
            warp_scale,
            zoom_exponent,
            shader,
            zoom,
            rot,
            cx,
            cy,
            dx,
            dy,
            warp,
            sx,
            sy,
            wave_r,
            wave_g,
            wave_b,
            wave_x,
            wave_y,
            ob_size,
            ob_r,
            ob_g,
            ob_b,
            ob_a,
            ib_size,
            ib_r,
            ib_g,
            ib_b,
            ib_a,
            mv_x,
            mv_y,
            mv_dx,
            mv_dy,
            mv_l,
            mv_r,
            mv_g,
            mv_b,
            mv_a,
            blur1_min,
            blur2_min,
            blur3_min,
            blur1_max,
            blur2_max,
            blur3_max,
            blur1_edge_darken
        }
    };
}

macro_rules! blendable_accessors {
    ($($f:ident),*) => {
        /// Every blendable field with its Rust field name, in `CState`
        /// declaration order (see `for_each_blendable` in preset.rs).
        pub fn blendables(&self) -> Vec<(&'static str, &Blendable)> {
            vec![$((stringify!($f), &self.$f)),*]
        }

        /// Mutable form of [`Preset::blendables`].
        pub fn blendables_mut(&mut self) -> Vec<(&'static str, &mut Blendable)> {
            vec![$((stringify!($f), &mut self.$f)),*]
        }
    };
}

impl Preset {
    for_each_blendable!(blendable_accessors);

    /// Shader model of the warp shader (from `warp_ps`).
    pub fn warp_ps_version(&self) -> PsVersion {
        PsVersion::from_file_value(self.warp_ps)
    }

    /// Shader model of the composite shader (from `comp_ps`).
    pub fn comp_ps_version(&self) -> PsVersion {
        PsVersion::from_file_value(self.comp_ps)
    }

    /// `per_frame_init` as md-eval compiles it (`strip_code`).
    pub fn per_frame_init_eel(&self) -> String {
        strip_code(&self.per_frame_init)
    }

    /// `per_frame` as md-eval compiles it (`strip_code`).
    pub fn per_frame_eel(&self) -> String {
        strip_code(&self.per_frame)
    }

    /// `per_pixel` as md-eval compiles it (`strip_code`).
    pub fn per_pixel_eel(&self) -> String {
        strip_code(&self.per_pixel)
    }
}

impl WaveData {
    /// `init` as md-eval compiles it.
    pub fn init_eel(&self) -> String {
        strip_code(&self.init)
    }
    /// `per_frame` as md-eval compiles it.
    pub fn per_frame_eel(&self) -> String {
        strip_code(&self.per_frame)
    }
    /// `per_point` as md-eval compiles it.
    pub fn per_point_eel(&self) -> String {
        strip_code(&self.per_point)
    }
}

impl ShapeData {
    /// `init` as md-eval compiles it.
    pub fn init_eel(&self) -> String {
        strip_code(&self.init)
    }
    /// `per_frame` as md-eval compiles it.
    pub fn per_frame_eel(&self) -> String {
        strip_code(&self.per_frame)
    }
}

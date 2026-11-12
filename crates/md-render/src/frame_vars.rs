//! [`PerFrameVars`]: the post-equation per-frame values the renderer reads
//! (F37: md-render defines it, md-core 7.2 fills it; F38: `FrameInput`
//! carries `vars`/`old_vars`). Kept apart from `mesh.rs` (which F37 names)
//! only for file size; `crate::mesh` is where it is documented as used.
//!
//! The fields are `CState`'s `var_pf_*` variables (code/vis_milk2/state.h:389-424)
//! — the values the C++ renderer reads through `*m_pState->var_pf_*` after
//! `RunPerFrameEquations` (milkdropfs.cpp:640-750: q copied to the
//! per-pixel VM, `gamma` and `echo_zoom` clamped, non-motion values blended
//! during a transition) — plus the preset scalars the renderer reads
//! straight from `CState` because no per-frame variable shadows them.
//! EEL values stay `f64` so the per-pixel program sees exactly what the
//! per-frame program left (the C++ copies doubles); the renderer narrows
//! to `f32` where the C++ casts. Later tasks may add fields.

use md_preset::Preset;

/// The per-frame variables of one preset after its per-frame equations.
#[derive(Clone, Debug, PartialEq)]
pub struct PerFrameVars {
    // --- read-only inputs (LoadPerFrameEvallibVars, milkdropfs.cpp:481-497) ---
    /// `time`: `GetTime() - m_fStartTime`.
    pub time: f64,
    pub fps: f64,
    pub frame: f64,
    /// `progress` through the current preset.
    pub progress: f64,
    /// `imm_rel[0..3]`.
    pub bass: f64,
    pub mid: f64,
    pub treb: f64,
    /// `avg_rel[0..3]`.
    pub bass_att: f64,
    pub mid_att: f64,
    pub treb_att: f64,
    pub meshx: f64,
    pub meshy: f64,
    pub pixelsx: f64,
    pub pixelsy: f64,
    /// `m_fInvAspectX/Y` (sic: the C++ stores the inverse aspect).
    pub aspectx: f64,
    pub aspecty: f64,

    // --- motion (warp mesh; blended through the mesh UVs, not here) ---
    pub zoom: f64,
    pub zoomexp: f64,
    pub rot: f64,
    pub warp: f64,
    pub cx: f64,
    pub cy: f64,
    pub dx: f64,
    pub dy: f64,
    pub sx: f64,
    pub sy: f64,

    // --- everything else (blended in RunPerFrameEquations when blending) ---
    pub decay: f64,
    pub wave_a: f64,
    pub wave_r: f64,
    pub wave_g: f64,
    pub wave_b: f64,
    pub wave_x: f64,
    pub wave_y: f64,
    pub wave_mystery: f64,
    pub wave_mode: f64,
    pub wave_usedots: f64,
    pub wave_thick: f64,
    pub wave_additive: f64,
    pub wave_brighten: f64,
    pub ob_size: f64,
    pub ob_r: f64,
    pub ob_g: f64,
    pub ob_b: f64,
    pub ob_a: f64,
    pub ib_size: f64,
    pub ib_r: f64,
    pub ib_g: f64,
    pub ib_b: f64,
    pub ib_a: f64,
    pub mv_x: f64,
    pub mv_y: f64,
    pub mv_dx: f64,
    pub mv_dy: f64,
    pub mv_l: f64,
    pub mv_r: f64,
    pub mv_g: f64,
    pub mv_b: f64,
    pub mv_a: f64,
    pub echo_zoom: f64,
    pub echo_alpha: f64,
    pub echo_orient: f64,
    pub darken_center: f64,
    pub gamma: f64,
    /// Texture wrap for the fixed-function warp (`> m_fSnapPoint` = wrap).
    pub wrap: f64,
    pub invert: f64,
    pub brighten: f64,
    pub darken: f64,
    pub solarize: f64,
    /// `b1n`, `b2n`, `b3n` / `b1x`, `b2x`, `b3x` / `b1ed`.
    pub blur1_min: f64,
    pub blur2_min: f64,
    pub blur3_min: f64,
    pub blur1_max: f64,
    pub blur2_max: f64,
    pub blur3_max: f64,
    pub blur1_edge_darken: f64,
    pub monitor: f64,
    /// `q1..q32` after the per-frame code (the shaders' `_qa.._qh`).
    pub q: [f64; 32],

    // --- preset scalars with no per-frame variable (read from CState) ---
    /// `m_fWarpAnimSpeed`.
    pub warp_anim_speed: f32,
    /// `m_fWarpScale.eval(GetTime())`.
    pub warp_scale: f32,
    /// `m_fWaveScale`, `m_fWaveSmoothing` (evaluated now).
    pub wave_scale: f32,
    pub wave_smoothing: f32,
    /// `m_bModWaveAlphaByVolume`, `m_fModWaveAlphaStart/End` (evaluated now).
    pub mod_wave_alpha_by_volume: bool,
    pub mod_wave_alpha_start: f32,
    pub mod_wave_alpha_end: f32,
    /// `m_bRedBlueStereo`.
    pub red_blue_stereo: bool,
    /// `m_fShader` (hue shader amount, evaluated now).
    pub shader: f32,
}

impl Default for PerFrameVars {
    /// [`PerFrameVars::from_preset`] of [`Preset::defaults`] at time 0.
    fn default() -> Self {
        PerFrameVars::from_preset(&Preset::defaults(), 0.0)
    }
}

impl PerFrameVars {
    /// The values `LoadPerFrameEvallibVars` (milkdropfs.cpp:460-554) loads
    /// before the per-frame code runs: motion values evaluated at time
    /// `-1` (no blend), the others at `now` (`GetTime()`). The read-only
    /// inputs (`time`, `fps`, `frame`, `progress`, the six band values,
    /// `meshx`..`aspecty`) and `q`/`monitor` (restored from the init code)
    /// are left 0 for md-core to set.
    /// With no per-frame code this is the frame's result.
    pub fn from_preset(p: &Preset, now: f32) -> PerFrameVars {
        let m = |b: &md_preset::Blendable| f64::from(b.eval(-1.0));
        let e = |b: &md_preset::Blendable| f64::from(b.eval(now));
        let flag = |b: bool| if b { 1.0 } else { 0.0 };
        PerFrameVars {
            time: 0.0,
            fps: 0.0,
            frame: 0.0,
            progress: 0.0,
            bass: 0.0,
            mid: 0.0,
            treb: 0.0,
            bass_att: 0.0,
            mid_att: 0.0,
            treb_att: 0.0,
            meshx: 0.0,
            meshy: 0.0,
            pixelsx: 0.0,
            pixelsy: 0.0,
            aspectx: 0.0,
            aspecty: 0.0,
            zoom: m(&p.zoom),
            zoomexp: m(&p.zoom_exponent),
            rot: m(&p.rot),
            warp: m(&p.warp),
            cx: m(&p.cx),
            cy: m(&p.cy),
            dx: m(&p.dx),
            dy: m(&p.dy),
            sx: m(&p.sx),
            sy: m(&p.sy),
            decay: e(&p.decay),
            wave_a: e(&p.wave_alpha),
            wave_r: e(&p.wave_r),
            wave_g: e(&p.wave_g),
            wave_b: e(&p.wave_b),
            wave_x: e(&p.wave_x),
            wave_y: e(&p.wave_y),
            wave_mystery: e(&p.wave_param),
            wave_mode: f64::from(p.wave_mode),
            wave_usedots: flag(p.wave_dots),
            wave_thick: flag(p.wave_thick),
            wave_additive: flag(p.additive_waves),
            wave_brighten: flag(p.maximize_wave_color),
            ob_size: e(&p.ob_size),
            ob_r: e(&p.ob_r),
            ob_g: e(&p.ob_g),
            ob_b: e(&p.ob_b),
            ob_a: e(&p.ob_a),
            ib_size: e(&p.ib_size),
            ib_r: e(&p.ib_r),
            ib_g: e(&p.ib_g),
            ib_b: e(&p.ib_b),
            ib_a: e(&p.ib_a),
            mv_x: e(&p.mv_x),
            mv_y: e(&p.mv_y),
            mv_dx: e(&p.mv_dx),
            mv_dy: e(&p.mv_dy),
            mv_l: e(&p.mv_l),
            mv_r: e(&p.mv_r),
            mv_g: e(&p.mv_g),
            mv_b: e(&p.mv_b),
            mv_a: e(&p.mv_a),
            echo_zoom: e(&p.video_echo_zoom),
            echo_alpha: e(&p.video_echo_alpha),
            echo_orient: f64::from(p.video_echo_orientation),
            darken_center: flag(p.darken_center),
            gamma: e(&p.gamma_adj),
            wrap: flag(p.tex_wrap),
            invert: flag(p.invert),
            brighten: flag(p.brighten),
            darken: flag(p.darken),
            solarize: flag(p.solarize),
            blur1_min: e(&p.blur1_min),
            blur2_min: e(&p.blur2_min),
            blur3_min: e(&p.blur3_min),
            blur1_max: e(&p.blur1_max),
            blur2_max: e(&p.blur2_max),
            blur3_max: e(&p.blur3_max),
            blur1_edge_darken: e(&p.blur1_edge_darken),
            monitor: 0.0,
            q: [0.0; 32],
            warp_anim_speed: p.warp_anim_speed,
            warp_scale: p.warp_scale.eval(now),
            wave_scale: p.wave_scale.eval(now),
            wave_smoothing: p.wave_smoothing.eval(now),
            mod_wave_alpha_by_volume: p.mod_wave_alpha_by_volume,
            mod_wave_alpha_start: p.mod_wave_alpha_start.eval(now),
            mod_wave_alpha_end: p.mod_wave_alpha_end.eval(now),
            red_blue_stereo: p.red_blue_stereo,
            shader: p.shader.eval(now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_preset_reads_the_preset_values() {
        let mut p = Preset::defaults();
        p.zoom = md_preset::Blendable::new(1.25);
        p.decay = md_preset::Blendable::new(0.9);
        p.tex_wrap = false;
        p.wave_mode = 5;
        let v = PerFrameVars::from_preset(&p, 3.0);
        assert_eq!(v.zoom, 1.25);
        assert_eq!(v.decay, f64::from(0.9f32));
        assert_eq!(v.wrap, 0.0);
        assert_eq!(v.wave_mode, 5.0);
        assert_eq!(v.warp_anim_speed, p.warp_anim_speed);
    }
}

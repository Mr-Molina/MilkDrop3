//! `RunPerFrameEquations` (code/vis_milk2/milkdropfs.cpp:556-749) and the
//! `LoadPerFrameEvallibVars` it starts with (460-554), for one preset.
//!
//! The C++ runs the function once per frame for the current preset and,
//! while a transition is in progress (`m_bBlending`), a second time for
//! the OLD preset (`rep` 1, `pState = m_pOldState`, 628-631) with the same
//! plugin-global inputs, then mixes the outputs into the new preset's
//! variables (691-748). Here that is: [`run_per_frame`] on each state with
//! the same [`FrameInputs`], then [`blend_outputs`] on the new result.
//! The old result is left unmixed: the warp mesh needs its own motion
//! values (`ComputeGridAlphaValues` reads `pOldState->var_pf_zoom` ...,
//! milkdropfs.cpp:1722-1737), so the caller passes it to
//! `WarpMesh::compute_uvs(&old_vars, old_runner, now, Some((pattern,
//! blend_progress)))` after the new preset's own call.
//!
//! Departures from the task brief (the C++ wins): [`FrameInputs`] gains
//! `now` (the absolute clock, `GetTime()`: the non-motion preset values are
//! evaluated at it, 497-553) and `blend_outputs` gains `snap`
//! (`m_fSnapPoint`, 586-620, which the caller derives with
//! `md_render::snap_point`). The milestone bullet lists `wave_mode`, `sx`,
//! `sy`, `dx`, `dy`, `zoom`, `rot`, `cx`, `cy`, `warp` among the blended
//! values; the C++ does not blend them here (`wave_mode` is "exempt",
//! 705; motion is blended through the mesh UVs).

use crate::state::PresetState;
use md_render::{cosine_interp, PerFrameVars, SoundData};

/// The plugin-global values `LoadPerFrameEvallibVars` reads.
pub struct FrameInputs<'a> {
    /// `GetTime()`.
    pub now: f32,
    /// `GetTime() - m_fStartTime` (`time`, 481).
    pub time: f32,
    /// `GetFps()`.
    pub fps: f32,
    /// `GetFrame()`.
    pub frame: u32,
    /// [`preset_progress`] of the CURRENT preset's timers; the old preset
    /// sees the same value (494 uses the globals for either state).
    pub progress: f32,
    /// `mysound`: `bass mid treb` are `imm_rel` (483-485), the `_att`
    /// values `avg_rel` (486-488).
    pub sound: &'a SoundData,
    /// `m_nGridX`, `m_nGridY` (`meshx`, `meshy`).
    pub mesh: (u32, u32),
    /// `GetWidth()`, `GetHeight()`.
    pub pixels: (u32, u32),
    /// `m_fInvAspectX/Y` (NOT `m_fAspectX/Y`, 544-545; plugin.cpp:2037).
    pub aspect: (f32, f32),
}

/// `progress` (milkdropfs.cpp:494): `(GetTime() - m_fPresetStartTime) /
/// (m_fNextPresetTime - m_fPresetStartTime)`, in `f32` as the C++ computes
/// it (the same value `apps/milkdrop/src/headless.rs` forms from its fixed
/// clocks). Infinite or NaN when the preset has no duration, as in C++.
pub fn preset_progress(now: f32, preset_start: f32, next_preset: f32) -> f32 {
    (now - preset_start) / (next_preset - preset_start)
}

/// Run the per-frame code of `state` and return the variables the renderer
/// reads.
///
/// Mirrors `LoadPerFrameEvallibVars` (every variable is written each frame:
/// motion values at time -1, the rest at `now`, `q` and `monitor` from the
/// init capture), the once-per-frame per-pixel inputs (640-655), the code
/// run (664), the `monitor` save (670), the `q` copy to the per-pixel
/// `Vm` (673-674) and the range checks (677-678): `gamma` to `[0, 8]` and
/// `echo_zoom` to `[0.001, 1000]`. As in C++ `min`/`max`, a NaN passes
/// through. Nothing else is clamped here.
pub fn run_per_frame(state: &mut PresetState, inputs: &FrameInputs) -> PerFrameVars {
    let mut v = PerFrameVars::from_preset(&state.preset, inputs.now);
    let s = inputs.sound;
    v.time = f64::from(inputs.time);
    v.fps = f64::from(inputs.fps);
    v.frame = f64::from(inputs.frame);
    v.progress = f64::from(inputs.progress);
    v.bass = f64::from(s.imm_rel[0]);
    v.mid = f64::from(s.imm_rel[1]);
    v.treb = f64::from(s.imm_rel[2]);
    v.bass_att = f64::from(s.avg_rel[0]);
    v.mid_att = f64::from(s.avg_rel[1]);
    v.treb_att = f64::from(s.avg_rel[2]);
    v.meshx = f64::from(inputs.mesh.0);
    v.meshy = f64::from(inputs.mesh.1);
    v.pixelsx = f64::from(inputs.pixels.0);
    v.pixelsy = f64::from(inputs.pixels.1);
    v.aspectx = f64::from(inputs.aspect.0);
    v.aspecty = f64::from(inputs.aspect.1);
    v.q = state.q_init;
    v.monitor = state.monitor_init;
    state.pf_refs.load(&mut state.pf, &v);

    let (pv, r) = (&mut state.pv, &state.pv_refs);
    macro_rules! copy {
        ($($f:ident),* $(,)?) => { $( pv.set(r.$f, v.$f); )* };
    }
    copy!(
        time, fps, frame, progress, bass, mid, treb, bass_att, mid_att, treb_att, meshx, meshy,
        pixelsx, pixelsy, aspectx, aspecty
    );

    if let Some(prog) = &state.pf_program {
        prog.run(&mut state.pf);
    }
    state.monitor_init = state.pf.get(state.pf_refs.monitor);
    state.pf_refs.store(&state.pf, &mut v);
    for (r, q) in state.pv_refs.q.iter().zip(&v.q) {
        state.pv.set(*r, *q);
    }
    v.gamma = clamp(v.gamma, 0.0, 8.0);
    v.echo_zoom = clamp(v.echo_zoom, 0.001, 1000.0);
    v
}

/// `max(lo, min(hi, x))` with the C++ macros' NaN behavior.
fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    let m = if hi < x { hi } else { x };
    if lo > m {
        lo
    } else {
        m
    }
}

/// Mix the old preset's per-frame results into the new one's
/// (milkdropfs.cpp:691-748), for a transition at `progress`
/// (`m_fBlendProgress`) with the boolean snap point `snap`
/// (`m_fSnapPoint`; `md_render::snap_point`).
///
/// `mix = CosineInterp(progress)`. Mixed as `mix * new + (1 - mix) * old`:
/// `decay`, `wave_a/r/g/b/x/y/mystery`, `ob_*`, `ib_*`, `mv_*`,
/// `echo_zoom`, `echo_alpha`, `gamma`, `blur1..3_min/max` and
/// `blur1_edge_darken`. Taken from the old preset while `mix < snap`, else
/// the new one: `echo_orient`, `wave_usedots/thick/additive/brighten`,
/// `darken_center`, `wrap`, `invert`, `brighten`, `darken`, `solarize`.
/// Everything else (`wave_mode`, the motion values, inputs, `q`, `monitor`
/// and the preset scalars) stays the new preset's. Call it only while
/// blending.
pub fn blend_outputs(new: &mut PerFrameVars, old: &PerFrameVars, progress: f32, snap: f32) {
    let mix = f64::from(cosine_interp(progress));
    let mix2 = 1.0 - mix;
    let old_side = mix < f64::from(snap);
    macro_rules! mixed {
        ($($f:ident),* $(,)?) => { $( new.$f = mix * new.$f + mix2 * old.$f; )* };
    }
    macro_rules! snapped {
        ($($f:ident),* $(,)?) => { $( if old_side { new.$f = old.$f; } )* };
    }
    mixed!(
        decay,
        wave_a,
        wave_r,
        wave_g,
        wave_b,
        wave_x,
        wave_y,
        wave_mystery,
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
        echo_zoom,
        echo_alpha,
        gamma,
        blur1_min,
        blur2_min,
        blur3_min,
        blur1_max,
        blur2_max,
        blur3_max,
        blur1_edge_darken
    );
    snapped!(
        echo_orient,
        wave_usedots,
        wave_thick,
        wave_additive,
        wave_brighten,
        darken_center,
        wrap,
        invert,
        brighten,
        darken,
        solarize
    );
}

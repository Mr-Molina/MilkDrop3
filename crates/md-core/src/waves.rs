//! Custom wave evaluation: the per-frame and per-point programs of
//! `CPlugin::DrawCustomWaves` (code/vis_milk2/milkdropfs.cpp:2496-2680,
//! the evaluation part, 2517-2622) with `LoadCustomWavePerFrameEvallibVars`
//! (2442-2463).
//!
//! What the C++ does, and so what this does, per enabled wave:
//! 1. load the per-frame `Vm` (2442-2463): the plugin inputs, `q1..q32`
//!    from the PRESET's per-frame result (`*pState->var_pf_q`, so the
//!    preset's per-frame program must have run first), `t1..t8` from the
//!    values captured after the init code (every frame, 2457), and the
//!    wave's `r g b a samples`;
//! 2. copy the read-only inputs to the per-point `Vm` once (2530-2539),
//!    run the per-frame program (2541; with no program it is a no-op,
//!    nseel-compiler.c:1665) and copy `q` and `t` it left to the per-point
//!    `Vm` (2543-2546). `t` is not saved back (2624-2633 are commented
//!    out), so each frame starts from the init capture again;
//! 3. take `samples` AFTER the program: `min(512, (int)samples)` (2548-2549);
//!    the sample window and smoothing are `md_render::custom_wave_inputs`;
//! 4. per point set `sample value1 value2`, `x = 0.5f + value1`,
//!    `y = 0.5f + value2` and `r g b a` from the per-frame result (2598-2606),
//!    run the per-point program (2609) and read `x y r g b a` (2612-2619). Only
//!    those nine are reset per point; `q`, `t` and user variables carry
//!    from point to point (and the per-point `Vm` keeps them across frames
//!    apart from the per-frame copy of `q`/`t`).
//!
//! A wave with fewer than 2 samples (1 for dots) draws nothing and is left
//! out of the result; the per-frame program still ran (2541 precedes the
//! test at 2551). A program that failed to compile is a no-op, as in C++.
//!
//! Departures from the brief: no separate `sound` argument (it is
//! `inputs.sound`); waves that produce no points are omitted instead of
//! returned empty.
//!
//! For task 7.9: call [`crate::run_per_frame`] and then this on the new
//! state every frame, and, while blending, on the OLD state as well (same
//! `inputs`) into `FrameInput::old_custom_waves`. Alpha scaling by
//! `blend_t` and `1 - blend_t` is the renderer's.

use crate::per_frame::FrameInputs;
use crate::state::PresetState;
use md_render::{custom_wave_inputs, CustomWaveDraw, CustomWaveFrame, CustomWavePoint};

/// Evaluate every enabled custom wave of `state` for this frame.
pub fn eval_custom_waves(state: &mut PresetState, inputs: &FrameInputs) -> Vec<CustomWaveDraw> {
    let wave_scale = state.preset.wave_scale.eval(-1.0);
    let q: [f64; 32] = std::array::from_fn(|i| state.pf.get(state.pf_refs.q[i]));
    let mut out = Vec::new();
    for (data, w) in state.preset.waves.iter().zip(state.waves.iter_mut()) {
        if data.enabled == 0 {
            continue;
        }
        let s = inputs.sound;
        let common = [
            f64::from(inputs.time),
            f64::from(inputs.fps),
            f64::from(inputs.frame),
            f64::from(inputs.progress),
            f64::from(s.imm_rel[0]),
            f64::from(s.imm_rel[1]),
            f64::from(s.imm_rel[2]),
            f64::from(s.avg_rel[0]),
            f64::from(s.avg_rel[1]),
            f64::from(s.avg_rel[2]),
        ];
        let (f, p) = (&w.pf_refs, &w.pp_refs);
        let f_vars = [
            f.time, f.fps, f.frame, f.progress, f.bass, f.mid, f.treb, f.bass_att, f.mid_att,
            f.treb_att,
        ];
        let p_vars = [
            p.time, p.fps, p.frame, p.progress, p.bass, p.mid, p.treb, p.bass_att, p.mid_att,
            p.treb_att,
        ];
        for (v, x) in f_vars.iter().zip(common) {
            w.pf.set(*v, x);
        }
        for (v, x) in f.q.iter().zip(q) {
            w.pf.set(*v, x);
        }
        for (v, x) in f.t.iter().zip(w.t_init) {
            w.pf.set(*v, x);
        }
        w.pf.set(f.r, f64::from(data.r));
        w.pf.set(f.g, f64::from(data.g));
        w.pf.set(f.b, f64::from(data.b));
        w.pf.set(f.a, f64::from(data.a));
        w.pf.set(f.samples, f64::from(data.samples));

        // 2530-2539: the per-point read-only inputs, from the per-frame
        // values as loaded (before the program runs).
        for (v, x) in p_vars.iter().zip(common) {
            w.pp.set(*v, x);
        }
        if let Some(prog) = &w.pf_program {
            prog.run(&mut w.pf);
        }
        for (pv, fv) in p.q.iter().zip(&f.q) {
            w.pp.set(*pv, w.pf.get(*fv));
        }
        for (pv, fv) in p.t.iter().zip(&f.t) {
            w.pp.set(*pv, w.pf.get(*fv));
        }

        let frame = CustomWaveFrame {
            samples: w.pf.get(f.samples),
            sep: data.sep,
            scaling: data.scaling,
            smoothing: data.smoothing,
            wave_scale,
            spectrum: data.spectrum != 0,
            use_dots: data.use_dots != 0,
        };
        let src = custom_wave_inputs(&frame, s);
        if src.is_empty() {
            continue;
        }
        let (r, g, b, a) = (w.pf.get(f.r), w.pf.get(f.g), w.pf.get(f.b), w.pf.get(f.a));
        let mut points = Vec::with_capacity(src.len());
        for (value1, value2, sample) in src {
            w.pp.set(p.sample, f64::from(sample));
            w.pp.set(p.value1, f64::from(value1));
            w.pp.set(p.value2, f64::from(value2));
            // `0.5f + value1` is a float sum (2601-2602).
            w.pp.set(p.x, f64::from(0.5f32 + value1));
            w.pp.set(p.y, f64::from(0.5f32 + value2));
            w.pp.set(p.r, r);
            w.pp.set(p.g, g);
            w.pp.set(p.b, b);
            w.pp.set(p.a, a);
            if let Some(prog) = &w.pp_program {
                prog.run(&mut w.pp);
            }
            points.push(CustomWavePoint {
                x: w.pp.get(p.x),
                y: w.pp.get(p.y),
                r: w.pp.get(p.r),
                g: w.pp.get(p.g),
                b: w.pp.get(p.b),
                a: w.pp.get(p.a),
            });
        }
        out.push(CustomWaveDraw {
            points,
            use_dots: data.use_dots != 0,
            thick: data.draw_thick != 0,
            additive: data.additive != 0,
        });
    }
    out
}

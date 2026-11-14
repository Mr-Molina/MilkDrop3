//! Custom shape evaluation: the per-instance per-frame program of
//! `CPlugin::DrawCustomShapes` (code/vis_milk2/milkdropfs.cpp:2215-2398,
//! the evaluation at 2264-2273) with `LoadCustomShapePerFrameEvallibVars`
//! (2400-2440).
//!
//! For each enabled shape and each `instance` in `0..instances` (2264) the
//! C++ reloads EVERY per-frame variable (2400-2440): the plugin inputs,
//! `q1..q32` from the preset's per-frame result (`*pState->var_pf_q`, so
//! the preset's per-frame program must have run first), `t1..t8` from the
//! init capture (2415; the save-back at 2276-2286 is commented out, so
//! `t` restarts for every instance and frame), the shape's own `x y rad ang
//! tex_zoom tex_ang sides additive textured r g b a r2 g2 b2 a2 border_*`,
//! `num_inst` (`var_pf_instances`) and `instance`. It then runs the
//! per-frame program (2270-2273; no program is a no-op, nseel-compiler.c:
//! 1665) and the draw reads the variables. User variables and `megabuf`
//! persist from instance to instance and frame to frame in the shape's `Vm`.
//!
//! The values are returned as the `double`s the program left; the `(int)`
//! casts of `sides`, `additive`, `textured` and `thick` and the `sides`
//! clamp (2288-2290) happen where they are used, in
//! `md_render::build_shape`, via its `c_int`. Instance count: the loop
//! `instance < instances` (2264), so 0 or negative gives nothing; the
//! program cannot change it. Shapes come out in the C++ order, shape 0..4,
//! instances ascending.
//!
//! Departures from the brief: no separate `sound` argument (it is
//! `inputs.sound`).
//!
//! For task 7.9: call [`crate::run_per_frame`] then this on the new state
//! every frame into `FrameInput::shapes` and, while blending, on the OLD
//! state with the same `inputs` into `FrameInput::old_shapes`; the renderer
//! scales alpha by `blend_t` / `1 - blend_t`.

use crate::per_frame::FrameInputs;
use crate::state::PresetState;
use md_render::ShapeFrameVars;

/// Evaluate every instance of every enabled custom shape of `state`.
pub fn eval_custom_shapes(state: &mut PresetState, inputs: &FrameInputs) -> Vec<ShapeFrameVars> {
    let q: [f64; 32] = std::array::from_fn(|i| state.pf.get(state.pf_refs.q[i]));
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
    let mut out = Vec::new();
    for (data, sh) in state.preset.shapes.iter().zip(state.shapes.iter_mut()) {
        if data.enabled == 0 {
            continue;
        }
        let r = &sh.pf_refs;
        let common_vars = [
            r.time, r.fps, r.frame, r.progress, r.bass, r.mid, r.treb, r.bass_att, r.mid_att,
            r.treb_att,
        ];
        let f = |x: f32| f64::from(x);
        let own = [
            (r.x, f(data.x)),
            (r.y, f(data.y)),
            (r.rad, f(data.rad)),
            (r.ang, f(data.ang)),
            (r.tex_zoom, f(data.tex_zoom)),
            (r.tex_ang, f(data.tex_ang)),
            (r.sides, f64::from(data.sides)),
            (r.additive, f64::from(data.additive)),
            (r.textured, f64::from(data.textured)),
            (r.num_inst, f64::from(data.instances)),
            (r.thick, f64::from(data.thick_outline)),
            (r.r, f(data.r)),
            (r.g, f(data.g)),
            (r.b, f(data.b)),
            (r.a, f(data.a)),
            (r.r2, f(data.r2)),
            (r.g2, f(data.g2)),
            (r.b2, f(data.b2)),
            (r.a2, f(data.a2)),
            (r.border_r, f(data.border_r)),
            (r.border_g, f(data.border_g)),
            (r.border_b, f(data.border_b)),
            (r.border_a, f(data.border_a)),
        ];
        for instance in 0..data.instances.max(0) {
            for (v, x) in common_vars.iter().zip(common) {
                sh.pf.set(*v, x);
            }
            for (v, x) in r.q.iter().zip(q) {
                sh.pf.set(*v, x);
            }
            for (v, x) in r.t.iter().zip(sh.t_init) {
                sh.pf.set(*v, x);
            }
            for (v, x) in own {
                sh.pf.set(v, x);
            }
            sh.pf.set(r.instance, f64::from(instance));
            if let Some(prog) = &sh.pf_program {
                prog.run(&mut sh.pf);
            }
            let g = |v| sh.pf.get(v);
            out.push(ShapeFrameVars {
                x: g(r.x),
                y: g(r.y),
                rad: g(r.rad),
                ang: g(r.ang),
                tex_ang: g(r.tex_ang),
                tex_zoom: g(r.tex_zoom),
                r: g(r.r),
                g: g(r.g),
                b: g(r.b),
                a: g(r.a),
                r2: g(r.r2),
                g2: g(r.g2),
                b2: g(r.b2),
                a2: g(r.a2),
                border_r: g(r.border_r),
                border_g: g(r.border_g),
                border_b: g(r.border_b),
                border_a: g(r.border_a),
                sides: g(r.sides),
                textured: g(r.textured),
                additive: g(r.additive),
                thick: g(r.thick),
                instance: instance as u32,
                num_inst: data.instances as u32,
            });
        }
    }
    out
}

//! Custom waves and shapes: each carries its own `Vm`s and programs, with
//! their init code run at load as `CState::RecompileExpressions` does
//! (state.cpp:1728-1790 waves (init, per-frame, per-point), 1800-1850 shapes (init, per-frame)).

use crate::error::ProgramError;
use crate::state::{compile_section, InitInputs};
use crate::vars::{ShapeFrameRefs, WaveFrameRefs, WavePointRefs};
use md_eval::{GlobalMem, Program, Vm};
use md_preset::{ShapeData, WaveData};
use std::cell::RefCell;
use std::rc::Rc;

/// One custom wave: a per-frame `Vm` (init and per-frame code) and a
/// per-point `Vm` (`m_pf_eel` and `m_pp_eel`).
pub struct WaveState {
    pub pf: Vm,
    pub pp: Vm,
    pub pf_refs: WaveFrameRefs,
    pub pp_refs: WavePointRefs,
    pub pf_program: Option<Program>,
    pub pp_program: Option<Program>,
    /// `t_values_after_init_code`: `t1..t8` after the init code, restored
    /// into the per-frame (and per-point) `Vm` every frame
    /// (milkdropfs.cpp:2457). 0 when there is no init code or it fails.
    pub t_init: [f64; 8],
}

/// One custom shape: a per-frame `Vm` (`m_pf_eel`; the C++ per-point code
/// is commented out, state.cpp:1852-1869).
pub struct ShapeState {
    pub pf: Vm,
    pub pf_refs: ShapeFrameRefs,
    pub pf_program: Option<Program>,
    /// `t_values_after_init_code`, as for [`WaveState::t_init`].
    pub t_init: [f64; 8],
}

fn load_inputs_wave(vm: &mut Vm, r: &WaveFrameRefs, i: &InitInputs) {
    vm.set(r.time, i.time);
    vm.set(r.frame, i.frame);
    vm.set(r.fps, i.fps);
    vm.set(r.progress, i.progress);
    vm.set(r.bass, i.imm_rel[0]);
    vm.set(r.mid, i.imm_rel[1]);
    vm.set(r.treb, i.imm_rel[2]);
    vm.set(r.bass_att, i.avg_rel[0]);
    vm.set(r.mid_att, i.avg_rel[1]);
    vm.set(r.treb_att, i.avg_rel[2]);
}

impl WaveState {
    /// Register the variables, run the init code (when it compiles) with
    /// the `LoadCustomWavePerFrameEvallibVars` values (milkdropfs.cpp:
    /// 2442-2463: inputs, `q` = the preset's after-init values, `t` = 0,
    /// the wave's `r g b a samples`), capture `t1..t8`, then compile the
    /// per-frame and per-point code.
    pub(crate) fn load(
        index: usize,
        data: &WaveData,
        global: &Rc<RefCell<GlobalMem>>,
        q_init: &[f64; 32],
        loaded: Option<&InitInputs>,
        errors: &mut Vec<ProgramError>,
    ) -> WaveState {
        let mut pf = Vm::new(global.clone());
        let mut pp = Vm::new(global.clone());
        let pf_refs = WaveFrameRefs::register(&mut pf);
        let pp_refs = WavePointRefs::register(&mut pp);
        let mut t_init = [0.0; 8];

        let init = data.init_eel();
        if !init.trim().is_empty() {
            let name = format!("wave{index}_init");
            if let Some(prog) = compile_section(&mut pf, &init, &name, errors) {
                if let Some(inputs) = loaded {
                    load_inputs_wave(&mut pf, &pf_refs, inputs);
                    for (r, x) in pf_refs.q.iter().zip(q_init) {
                        pf.set(*r, *x);
                    }
                    pf.set(pf_refs.r, f64::from(data.r));
                    pf.set(pf_refs.g, f64::from(data.g));
                    pf.set(pf_refs.b, f64::from(data.b));
                    pf.set(pf_refs.a, f64::from(data.a));
                    pf.set(pf_refs.samples, f64::from(data.samples));
                }
                prog.run(&mut pf);
                for (x, r) in t_init.iter_mut().zip(&pf_refs.t) {
                    *x = pf.get(*r);
                }
            }
        }
        let pf_program = compile_section(
            &mut pf,
            &data.per_frame_eel(),
            &format!("wave{index}_per_frame"),
            errors,
        );
        let pp_program = compile_section(
            &mut pp,
            &data.per_point_eel(),
            &format!("wave{index}_per_point"),
            errors,
        );
        WaveState {
            pf,
            pp,
            pf_refs,
            pp_refs,
            pf_program,
            pp_program,
            t_init,
        }
    }
}

impl ShapeState {
    /// As [`WaveState::load`], with `LoadCustomShapePerFrameEvallibVars`
    /// (milkdropfs.cpp:2400-2440) for instance 0.
    pub(crate) fn load(
        index: usize,
        data: &ShapeData,
        global: &Rc<RefCell<GlobalMem>>,
        q_init: &[f64; 32],
        loaded: Option<&InitInputs>,
        errors: &mut Vec<ProgramError>,
    ) -> ShapeState {
        let mut pf = Vm::new(global.clone());
        let pf_refs = ShapeFrameRefs::register(&mut pf);
        let mut t_init = [0.0; 8];

        let init = data.init_eel();
        if !init.trim().is_empty() {
            let name = format!("shape{index}_init");
            if let Some(prog) = compile_section(&mut pf, &init, &name, errors) {
                if let Some(inputs) = loaded {
                    let r = &pf_refs;
                    let f = |x: f32| f64::from(x);
                    let sets = [
                        (r.time, inputs.time),
                        (r.frame, inputs.frame),
                        (r.fps, inputs.fps),
                        (r.progress, inputs.progress),
                        (r.bass, inputs.imm_rel[0]),
                        (r.mid, inputs.imm_rel[1]),
                        (r.treb, inputs.imm_rel[2]),
                        (r.bass_att, inputs.avg_rel[0]),
                        (r.mid_att, inputs.avg_rel[1]),
                        (r.treb_att, inputs.avg_rel[2]),
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
                        (r.instance, 0.0),
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
                    for (v, x) in sets {
                        pf.set(v, x);
                    }
                    for (v, x) in pf_refs.q.iter().zip(q_init) {
                        pf.set(*v, *x);
                    }
                }
                prog.run(&mut pf);
                for (x, v) in t_init.iter_mut().zip(&pf_refs.t) {
                    *x = pf.get(*v);
                }
            }
        }
        let pf_program = compile_section(
            &mut pf,
            &data.per_frame_eel(),
            &format!("shape{index}_per_frame"),
            errors,
        );
        ShapeState {
            pf,
            pf_refs,
            pf_program,
            t_init,
        }
    }
}

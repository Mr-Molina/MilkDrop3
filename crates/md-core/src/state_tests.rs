//! Tests for `PresetState::from_preset` (Task 7.1).

use crate::vars::{
    per_frame_names, per_pixel_names, shape_frame_names, wave_frame_names, wave_point_names,
};
use crate::{InitInputs, InitMode, PresetState};
use md_eval::{GlobalMem, Vm};
use md_preset::Preset;
use rand_core::SeedableRng;
use rand_pcg::Pcg32;
use std::cell::RefCell;
use std::rc::Rc;

fn global() -> Rc<RefCell<GlobalMem>> {
    Rc::new(RefCell::new(GlobalMem::new()))
}

fn load(p: Preset) -> PresetState {
    PresetState::from_preset(p, global(), &mut Pcg32::seed_from_u64(1)).unwrap()
}

/// Every name resolves to an already-registered variable: asking the Vm
/// again for it allocates nothing (registered names take indices 0..n).
fn assert_registered(vm: &mut Vm, names: &[String]) {
    let mut seen = std::collections::HashSet::new();
    for n in names {
        let upper = n.to_ascii_uppercase();
        let i = vm.var(n).index();
        assert!(i < names.len() as u32, "{n} was not registered");
        assert_eq!(vm.var(&upper).index(), i, "{n} is case-insensitive");
        assert!(seen.insert(i), "{n} duplicated");
    }
}

#[test]
fn registers_all_variables() {
    let mut s = load(Preset::defaults());
    // Counts verified against state.cpp:267-511 (see the task report).
    let pf = per_frame_names();
    let pv = per_pixel_names();
    assert_eq!(pf.len(), 76 + 32);
    assert_eq!(pv.len(), 30 + 32);
    assert_eq!(wave_frame_names().len(), 15 + 32 + 8);
    assert_eq!(wave_point_names().len(), 19 + 32 + 8);
    assert_eq!(shape_frame_names().len(), 34 + 32 + 8);
    assert_registered(&mut s.pf, &pf);
    assert_registered(&mut s.pv, &pv);
    for w in &mut s.waves {
        assert_registered(&mut w.pf, &wave_frame_names());
        assert_registered(&mut w.pp, &wave_point_names());
    }
    for sh in &mut s.shapes {
        assert_registered(&mut sh.pf, &shape_frame_names());
    }
    // Names the C++ does not register (the brief listed some of them).
    let sh = shape_frame_names();
    assert!(!sh.iter().any(|n| n == "thickoutline"));
    assert!(sh.iter().any(|n| n == "thick") && sh.iter().any(|n| n == "num_inst"));
    assert!(!pv
        .iter()
        .any(|n| n == "monitor" || n == "wave_a" || n == "echo_zoom"));
    assert!(pf.iter().any(|n| n == "monitor") && pf.iter().any(|n| n == "q32"));
}

#[test]
fn per_frame_refs_round_trip_every_variable() {
    let mut s = load(Preset::defaults());
    let names = per_frame_names();
    for (i, n) in names.iter().enumerate() {
        let v = s.pf.var(n);
        s.pf.set(v, i as f64 + 1.0);
    }
    let mut vars = md_render::PerFrameVars::default();
    s.pf_refs.store(&s.pf, &mut vars);
    let mut other = Vm::new(global());
    let refs = crate::vars::PerFrameRefs::register(&mut other);
    refs.load(&mut other, &vars);
    for (i, n) in names.iter().enumerate() {
        let v = other.var(n);
        assert_eq!(other.get(v), i as f64 + 1.0, "{n}");
    }
}

#[test]
fn init_captures_q_and_monitor() {
    let mut p = Preset::defaults();
    p.per_frame_init = "q1 = 5; q32 = 7; monitor = 3; zoom = 99;".into();
    let s = load(p);
    assert_eq!(s.q_init[0], 5.0);
    assert_eq!(s.q_init[31], 7.0);
    assert_eq!(s.q_init[1], 0.0);
    assert_eq!(s.monitor_init, 3.0);
    assert!(s.errors.is_empty());
}

#[test]
fn init_user_variables_and_registers_persist() {
    let g = global();
    let mut p = Preset::defaults();
    p.per_frame_init = "foo = 7; reg05 = 42;".into();
    p.per_frame = "q2 = foo;".into();
    let mut s = PresetState::from_preset(p, g.clone(), &mut Pcg32::seed_from_u64(1)).unwrap();
    let prog = s.pf_program.take().unwrap();
    prog.run(&mut s.pf);
    let q2 = s.pf.var("q2");
    assert_eq!(s.pf.get(q2), 7.0);
    assert_eq!(g.borrow().regs[5], 42.0);
}

fn load_mode(p: Preset, mode: &InitMode) -> PresetState {
    PresetState::from_preset_with(p, global(), &mut Pcg32::seed_from_u64(1), mode).unwrap()
}

fn reading_preset() -> Preset {
    let mut p = Preset::defaults();
    p.per_frame_init = "q1 = time + meshx; q2 = decay; q3 = bass;".into();
    p.waves[1].init = "t1 = q1; t2 = r; t3 = time;".into();
    p.waves[1].r = 0.5;
    p.shapes[2].sides = 7;
    p.shapes[2].init = "t1 = sides; t2 = q2; t3 = bass;".into();
    p
}

#[test]
fn instant_init_sees_the_preset_values_and_inputs() {
    let p = reading_preset();
    let decay = f64::from(p.decay.eval(0.0));
    let inputs = InitInputs {
        time: 2.5,
        meshx: 64.0,
        imm_rel: [1.5, 0.0, 0.0],
        ..InitInputs::default()
    };
    let s = load_mode(p, &InitMode::Instant(inputs));
    assert_eq!(s.q_init[..3], [66.5, decay, 1.5]);
    assert_eq!(s.waves[1].t_init[..3], [66.5, 0.5, 2.5]);
    assert_eq!(s.shapes[2].t_init[..3], [7.0, decay, 1.5]);
}

#[test]
fn blended_init_sees_nothing_loaded() {
    let s = load_mode(reading_preset(), &InitMode::Blended);
    assert_eq!(s.q_init[..3], [0.0; 3]);
    assert_eq!(s.waves[1].t_init[..3], [0.0; 3]);
    assert_eq!(s.shapes[2].t_init[..3], [0.0; 3]);
    // The decay really is non-zero, so the zeros are the mode, not the preset.
    assert!(reading_preset().decay.eval(0.0) > 0.0);
}

#[test]
fn compile_error_in_per_frame_is_reported_not_fatal() {
    let mut p = Preset::defaults();
    p.per_frame_init = "q1 = 4;".into();
    p.per_frame = "zoom = ;".into();
    p.per_pixel = "zoom = zoom + 1;".into();
    let s = load(p);
    assert!(s.pf_program.is_none());
    assert!(s.pv_program.is_some());
    assert_eq!(s.q_init[0], 4.0);
    assert_eq!(s.errors.len(), 1);
    assert_eq!(s.errors[0].section, "per_frame");
    assert!(!s.errors[0].error.message.is_empty());
}

#[test]
fn init_compile_error_zeroes_q_and_is_reported() {
    let mut p = Preset::defaults();
    p.per_frame_init = "q1 = ;".into();
    p.waves[0].init = "t1 = ;".into();
    p.shapes[1].init = "t1 = (;".into();
    let s = load(p);
    assert_eq!(s.q_init, [0.0; 32]);
    let sections: Vec<&str> = s.errors.iter().map(|e| e.section.as_str()).collect();
    assert_eq!(sections, ["per_frame_init", "wave0_init", "shape1_init"]);
    assert_eq!(s.waves[0].t_init, [0.0; 8]);
}

#[test]
fn empty_and_comment_only_code_is_not_compiled() {
    let mut p = Preset::defaults();
    p.per_frame = "// nothing here".into();
    p.per_pixel = "   ".into();
    let s = load(p);
    assert!(s.pf_program.is_none() && s.pv_program.is_none());
    assert!(s.errors.is_empty());
}

#[test]
fn wave_and_shape_init_capture_t1_to_t8() {
    let mut p = Preset::defaults();
    p.per_frame_init = "q1 = 10;".into();
    p.waves[2].r = 0.25;
    p.waves[2].samples = 300;
    p.waves[2].init = "t1 = 3; t8 = q1 + 1; t3 = r; t4 = samples;".into();
    p.waves[2].per_point = "x = x + t1;".into();
    p.shapes[3].instances = 5;
    p.shapes[3].sides = 6;
    p.shapes[3].init = "t2 = instance + num_inst * 10; t5 = sides; t6 = q1;".into();
    p.shapes[3].per_frame = "t2 = t2 + 1;".into();
    let s = load(p);
    let w = &s.waves[2];
    assert_eq!(w.t_init, [3.0, 0.0, 0.25, 300.0, 0.0, 0.0, 0.0, 11.0]);
    assert!(w.pp_program.is_some());
    assert!(w.pf_program.is_none());
    assert_eq!(s.waves[0].t_init, [0.0; 8]);
    let sh = &s.shapes[3];
    assert_eq!(sh.t_init, [0.0, 50.0, 0.0, 0.0, 6.0, 10.0, 0.0, 0.0]);
    assert!(sh.pf_program.is_some());
    assert!(s.errors.is_empty());
}

#[test]
fn rand_preset_and_rot_are_deterministic_for_a_seed() {
    let a = load(Preset::defaults());
    let b = load(Preset::defaults());
    assert_eq!(a.rand_preset, b.rand_preset);
    assert_eq!(a.rot, b.rot);
    let c = PresetState::from_preset(Preset::defaults(), global(), &mut Pcg32::seed_from_u64(2))
        .unwrap();
    assert_ne!(a.rand_preset, c.rand_preset);
    assert_ne!(a.rot, c.rot);
    let (rp, rot) = md_render::randomize_preset_vars(&mut Pcg32::seed_from_u64(1));
    assert_eq!(a.rand_preset, rp);
    assert_eq!(a.rot, rot);
    assert!(a.rand_preset.iter().all(|x| (0.0..=1.0).contains(x)));
    assert_eq!(a.blend_progress, 0.0);
}

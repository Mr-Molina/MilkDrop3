//! Tests for `run_per_frame`, `blend_outputs` and `per_pixel_runner`
//! (Task 7.2).

use crate::{blend_outputs, per_pixel_runner, run_per_frame, FrameInputs, PresetState};
use md_eval::GlobalMem;
use md_preset::Preset;
use md_render::{cosine_interp, PerFrameVars, PerPixelVars, SoundData};
use rand_core::SeedableRng;
use rand_pcg::Pcg32;
use std::cell::RefCell;
use std::rc::Rc;

fn load(init: &str, frame: &str, pixel: &str) -> PresetState {
    let mut p = Preset::defaults();
    p.per_frame_init = init.into();
    p.per_frame = frame.into();
    p.per_pixel = pixel.into();
    let g = Rc::new(RefCell::new(GlobalMem::new()));
    PresetState::from_preset(p, g, &mut Pcg32::seed_from_u64(1)).unwrap()
}

fn inputs(sound: &SoundData) -> FrameInputs<'_> {
    FrameInputs {
        now: 12.0,
        time: 2.5,
        fps: 30.0,
        frame: 77,
        progress: 0.125,
        sound,
        mesh: (48, 36),
        pixels: (1280, 720),
        aspect: (1.0, 1.75),
    }
}

#[test]
fn q_resets_to_init_each_frame() {
    let mut s = load(
        "q1 = 5; monitor = 2;",
        "q1 = q1 + 1; zoom = q1; monitor = monitor + 1;",
        "",
    );
    let snd = SoundData::default();
    for _ in 0..3 {
        let v = run_per_frame(&mut s, &inputs(&snd));
        assert_eq!(v.zoom, 6.0);
        assert_eq!(v.q[0], 6.0);
    }
    // `monitor` is saved back after every run (milkdropfs.cpp:670).
    assert_eq!(s.monitor_init, 5.0);
    assert_eq!(s.q_init[0], 5.0);
}

#[test]
fn per_frame_can_modify_zoom() {
    let mut s = load("", "zoom = zoom * 2; rot = 0.5; wave_mode = 3;", "");
    let v = run_per_frame(&mut s, &inputs(&SoundData::default()));
    assert_eq!(v.zoom, 2.0 * f64::from(Preset::defaults().zoom.eval(-1.0)));
    assert_eq!(v.rot, 0.5);
    assert_eq!(v.wave_mode, 3.0);
    // Preset scalars with no variable survive.
    assert_eq!(v.warp_anim_speed, Preset::defaults().warp_anim_speed);
}

#[test]
fn echo_zoom_is_clamped() {
    let mut s = load("", "echo_zoom = 5000; gamma = -3;", "");
    let v = run_per_frame(&mut s, &inputs(&SoundData::default()));
    assert_eq!(v.echo_zoom, 1000.0);
    assert_eq!(v.gamma, 0.0);
    let mut s = load("", "echo_zoom = -1; gamma = 99;", "");
    let v = run_per_frame(&mut s, &inputs(&SoundData::default()));
    assert_eq!(v.echo_zoom, 0.001);
    assert_eq!(v.gamma, 8.0);
}

#[test]
fn inputs_reach_the_per_frame_vm() {
    let snd = SoundData {
        imm_rel: [1.0, 2.0, 3.0],
        avg_rel: [4.0, 5.0, 6.0],
        imm: [7.0, 8.0, 9.0],
        avg: [10.0, 11.0, 12.0],
        ..SoundData::default()
    };
    let code = "monitor = bass; q1 = mid; q2 = treb; q3 = bass_att; q4 = mid_att; \
        q5 = treb_att; q6 = meshx; q7 = meshy; q8 = pixelsx; q9 = pixelsy; q10 = aspectx; \
        q11 = aspecty; q12 = progress; q13 = frame; q14 = fps; q15 = time;";
    let mut s = load("", code, "");
    let v = run_per_frame(&mut s, &inputs(&snd));
    assert_eq!(v.monitor, 1.0);
    let want = [
        2.0, 3.0, 4.0, 5.0, 6.0, 48.0, 36.0, 1280.0, 720.0, 1.0, 1.75, 0.125, 77.0, 30.0, 2.5,
    ];
    assert_eq!(&v.q[..15], &want[..]);
    assert_eq!(
        (v.bass, v.treb_att, v.frame, v.progress),
        (1.0, 6.0, 77.0, 0.125)
    );
    // Motion values and the others come from the preset (eval(-1) / eval(now)).
    assert_eq!(v.decay, f64::from(Preset::defaults().decay.eval(12.0)));
}

#[test]
fn per_pixel_code_changes_a_vertexs_motion() {
    let mut s = load(
        "",
        "q1 = 0.25; zoom = 1.5;",
        "zoom = zoom + x; rot = rad * 0.5 + ang; dx = q1; dy = bass + time + aspecty; \
         cx = meshx; sy = sy * 2;",
    );
    let snd = SoundData {
        imm_rel: [0.5, 0.0, 0.0],
        ..SoundData::default()
    };
    let vars = run_per_frame(&mut s, &inputs(&snd));
    let mut run = per_pixel_runner(&mut s);
    let mut v = PerPixelVars {
        x: 0.3,
        y: 0.4,
        rad: 0.6,
        ang: 1.0,
        zoom: vars.zoom,
        zoomexp: vars.zoomexp,
        rot: vars.rot,
        warp: vars.warp,
        cx: vars.cx,
        cy: vars.cy,
        dx: vars.dx,
        dy: vars.dy,
        sx: vars.sx,
        sy: vars.sy,
    };
    run(&mut v);
    assert_eq!(v.zoom, 1.5 + 0.3);
    assert_eq!(v.rot, 0.6 * 0.5 + 1.0);
    assert_eq!(v.dx, 0.25);
    assert_eq!(v.dy, 0.5 + 2.5 + 1.75);
    assert_eq!(v.cx, 48.0);
    assert_eq!(v.sy, vars.sy * 2.0);
    // Each call restarts from the vertex's own values.
    let mut w = v;
    w.x = 0.0;
    w.zoom = 1.5;
    w.sy = 1.0;
    run(&mut w);
    assert_eq!((w.zoom, w.sy), (1.5, 2.0));
}

#[test]
fn per_pixel_runner_without_code_leaves_the_vertex_alone() {
    let mut s = load("", "", "");
    assert!(s.pv_program.is_none());
    let mut run = per_pixel_runner(&mut s);
    let mut v = PerPixelVars {
        zoom: 1.1,
        ..PerPixelVars::default()
    };
    run(&mut v);
    assert_eq!(v.zoom, 1.1);
}

macro_rules! fields {
    ($m:ident) => {
        $m!(
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
        )
    };
}

macro_rules! snapped {
    ($m:ident) => {
        $m!(
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
        )
    };
}

macro_rules! set_all {
    ($v:expr, $x:expr; $($f:ident),*) => { $( $v.$f = $x; )* };
}

fn pair(
    mixed_new: f64,
    mixed_old: f64,
    snap_new: f64,
    snap_old: f64,
) -> (PerFrameVars, PerFrameVars) {
    let (mut n, mut o) = (PerFrameVars::default(), PerFrameVars::default());
    macro_rules! go {
        ($($f:ident),*) => {{ set_all!(n, mixed_new; $($f),*); set_all!(o, mixed_old; $($f),*); }};
    }
    fields!(go);
    macro_rules! gs {
        ($($f:ident),*) => {{ set_all!(n, snap_new; $($f),*); set_all!(o, snap_old; $($f),*); }};
    }
    snapped!(gs);
    // Fields the C++ does not touch here.
    n.zoom = 1.25;
    o.zoom = 3.0;
    n.sx = 0.5;
    o.sx = 2.0;
    n.wave_mode = 6.0;
    o.wave_mode = 1.0;
    n.q[3] = 8.0;
    o.q[3] = 9.0;
    n.monitor = 4.0;
    o.monitor = 5.0;
    n.bass = 0.7;
    o.bass = 0.1;
    (n, o)
}

#[test]
fn blend_at_half_is_midpoint_for_floats() {
    let (mut n, o) = pair(10.0, 20.0, 1.0, 0.0);
    blend_outputs(&mut n, &o, 0.5, 0.5);
    let mix = f64::from(cosine_interp(0.5));
    assert!((mix - 0.5).abs() < 1e-6);
    macro_rules! check {
        ($($f:ident),*) => {$(
            assert!((n.$f - 15.0).abs() < 1e-5, "{} = {}", stringify!($f), n.$f);
        )*};
    }
    fields!(check);
}

#[test]
fn blend_mixes_with_the_cosine_of_progress() {
    let (mut n, o) = pair(10.0, 20.0, 1.0, 0.0);
    blend_outputs(&mut n, &o, 0.3, 0.5);
    let mix = f64::from(cosine_interp(0.3));
    assert!(mix > 0.1 && mix < 0.3);
    macro_rules! check {
        ($($f:ident),*) => {$(
            assert_eq!(n.$f, mix * 10.0 + (1.0 - mix) * 20.0, "{}", stringify!($f));
        )*};
    }
    fields!(check);
}

#[test]
fn booleans_switch_at_half() {
    macro_rules! expect {
        ($n:expr, $want:expr; $($f:ident),*) => {$(
            assert_eq!($n.$f, $want, "{}", stringify!($f));
        )*};
    }
    let (mut n, o) = pair(10.0, 20.0, 1.0, 0.0);
    blend_outputs(&mut n, &o, 0.49, 0.5);
    macro_rules! old { ($($f:ident),*) => { expect!(n, 0.0; $($f),*) }; }
    snapped!(old);
    let (mut n, o) = pair(10.0, 20.0, 1.0, 0.0);
    blend_outputs(&mut n, &o, 0.51, 0.5);
    macro_rules! new { ($($f:ident),*) => { expect!(n, 1.0; $($f),*) }; }
    snapped!(new);
    // The snap point is the caller's (m_fSnapPoint): 1.01 never reaches
    // the new value, -0.01 always has it.
    let (mut n, o) = pair(10.0, 20.0, 1.0, 0.0);
    blend_outputs(&mut n, &o, 0.99, 1.01);
    snapped!(old);
    let (mut n, o) = pair(10.0, 20.0, 1.0, 0.0);
    blend_outputs(&mut n, &o, 0.0, -0.01);
    snapped!(new);
}

#[test]
fn blend_leaves_motion_inputs_q_and_wave_mode_alone() {
    let (mut n, o) = pair(10.0, 20.0, 1.0, 0.0);
    let before = n.clone();
    blend_outputs(&mut n, &o, 0.3, 0.5);
    assert_eq!(n.zoom, 1.25);
    assert_eq!(n.sx, 0.5);
    assert_eq!(n.wave_mode, 6.0);
    assert_eq!(n.q, before.q);
    assert_eq!((n.monitor, n.bass), (4.0, 0.7));
    assert_eq!(n.warp_scale, before.warp_scale);
}

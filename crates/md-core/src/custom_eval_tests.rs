//! Tests for `eval_custom_waves` and `eval_custom_shapes` (Task 7.3).

use crate::{eval_custom_shapes, eval_custom_waves, run_per_frame, FrameInputs, PresetState};
use md_eval::GlobalMem;
use md_preset::Preset;
use md_render::{custom_wave_inputs, CustomWaveFrame, SoundData};
use rand_core::SeedableRng;
use rand_pcg::Pcg32;
use std::cell::RefCell;
use std::rc::Rc;

fn load(edit: impl FnOnce(&mut Preset)) -> PresetState {
    let mut p = Preset::defaults();
    edit(&mut p);
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

/// An enabled wave 0 with `samples` points, no smoothing.
fn wave(p: &mut Preset, samples: i32, init: &str, frame: &str, point: &str) {
    let w = &mut p.waves[0];
    w.enabled = 1;
    w.samples = samples;
    w.sep = 0;
    w.smoothing = 0.0;
    w.init = init.into();
    w.per_frame = frame.into();
    w.per_point = point.into();
}

fn shape(p: &mut Preset, instances: i32, init: &str, frame: &str) {
    let s = &mut p.shapes[0];
    s.enabled = 1;
    s.instances = instances;
    s.init = init.into();
    s.per_frame = frame.into();
}

#[test]
fn disabled_wave_produces_nothing() {
    let mut st = load(|p| {
        wave(p, 8, "", "", "");
        p.waves[0].enabled = 0;
    });
    let snd = SoundData::default();
    assert!(eval_custom_waves(&mut st, &inputs(&snd)).is_empty());
}

#[test]
fn per_point_sees_sample_in_unit_range() {
    let mut st = load(|p| wave(p, 5, "", "", "x = sample; y = value1; r = sample * 0.5;"));
    let snd = SoundData::default();
    let w = eval_custom_waves(&mut st, &inputs(&snd));
    assert_eq!(w.len(), 1);
    let pts = &w[0].points;
    assert_eq!(pts.len(), 5);
    for (j, pt) in pts.iter().enumerate() {
        let t = f64::from(j as f32 * (1.0 / 4.0));
        assert_eq!(pt.x, t);
        assert_eq!(pt.r, t * 0.5);
        assert_eq!(pt.y, 0.0);
    }
    assert_eq!(pts[4].x, 1.0);
}

#[test]
fn per_point_starts_from_default_position_and_per_frame_colour() {
    let mut st = load(|p| wave(p, 3, "", "r = 0.25; g = 0.5; b = 0.75; a = 0.125;", ""));
    let snd = SoundData::default();
    let w = eval_custom_waves(&mut st, &inputs(&snd));
    for pt in &w[0].points {
        assert_eq!((pt.x, pt.y), (0.5, 0.5));
        assert_eq!((pt.r, pt.g, pt.b, pt.a), (0.25, 0.5, 0.75, 0.125));
    }
}

#[test]
fn per_point_sees_inputs_and_wave_values() {
    let snd = SoundData {
        imm_rel: [1.5, 0.0, 0.0],
        ..SoundData::default()
    };
    let mut st = load(|p| {
        wave(
            p,
            2,
            "",
            "",
            "x = time; y = bass + frame; r = fps; g = progress;",
        );
    });
    let w = eval_custom_waves(&mut st, &inputs(&snd));
    let pt = w[0].points[0];
    assert_eq!((pt.x, pt.y, pt.r, pt.g), (2.5, 78.5, 30.0, 0.125));
}

#[test]
fn wave_values_use_its_own_unblended_wave_scale() {
    let mut snd = SoundData::default();
    for (j, x) in snd.wave[0].iter_mut().enumerate() {
        *x = j as f32 * 0.01;
    }
    let mut st = load(|p| wave(p, 6, "", "", "x = value1;"));
    let scale = st.preset.wave_scale.eval(-1.0);
    let f = CustomWaveFrame {
        samples: 6.0,
        sep: 0,
        scaling: st.preset.waves[0].scaling,
        smoothing: 0.0,
        wave_scale: scale,
        spectrum: false,
        use_dots: false,
    };
    let want = custom_wave_inputs(&f, &snd);
    let w = eval_custom_waves(&mut st, &inputs(&snd));
    let got: Vec<f64> = w[0].points.iter().map(|p| p.x).collect();
    let want: Vec<f64> = want.iter().map(|x| f64::from(x.0)).collect();
    assert_eq!(got, want);
    assert!(got.iter().any(|x| *x != 0.0));
}

#[test]
fn t_flows_from_per_frame_into_per_point_and_carries_point_to_point() {
    let mut st = load(|p| {
        wave(
            p,
            3,
            "t1 = 7;",
            "t2 = t1 + 1; t1 = t1 * 2;",
            "x = t1; y = t2; t3 = t3 + 1; r = t3;",
        );
    });
    let snd = SoundData::default();
    for _ in 0..2 {
        // t1 restarts from the init capture each frame (2457), so the
        // second frame repeats the first.
        let w = eval_custom_waves(&mut st, &inputs(&snd));
        let pts = &w[0].points;
        assert_eq!(pts.iter().map(|p| p.x).collect::<Vec<_>>(), [14.0; 3]);
        assert_eq!(pts.iter().map(|p| p.y).collect::<Vec<_>>(), [8.0; 3]);
        assert_eq!(pts.iter().map(|p| p.r).collect::<Vec<_>>(), [1.0, 2.0, 3.0]);
    }
}

#[test]
fn q_comes_from_the_presets_per_frame_result() {
    let mut st = load(|p| {
        p.per_frame = "q1 = 4; q2 = q1 + 1;".into();
        wave(p, 2, "", "q3 = q2 + 1;", "x = q1; y = q3;");
        shape(p, 1, "", "x = q2;");
    });
    let snd = SoundData::default();
    run_per_frame(&mut st, &inputs(&snd));
    let w = eval_custom_waves(&mut st, &inputs(&snd));
    assert_eq!((w[0].points[0].x, w[0].points[0].y), (4.0, 6.0));
    let s = eval_custom_shapes(&mut st, &inputs(&snd));
    assert_eq!(s[0].x, 5.0);
}

#[test]
fn samples_follow_the_per_frame_assignment_with_the_512_cap() {
    let snd = SoundData::default();
    for (code, n) in [
        ("samples = 600;", 512),
        ("samples = 3.9;", 3),
        ("samples = 512;", 512),
        ("samples = 1;", 0),
        ("samples = 0 - 5;", 0),
    ] {
        let mut st = load(|p| wave(p, 64, "", code, ""));
        let w = eval_custom_waves(&mut st, &inputs(&snd));
        let got = w.first().map_or(0, |w| w.points.len());
        assert_eq!(got, n, "{code}");
    }
    // One sample draws dots.
    let mut st = load(|p| {
        wave(p, 64, "", "samples = 1;", "");
        p.waves[0].use_dots = 1;
    });
    let w = eval_custom_waves(&mut st, &inputs(&snd));
    assert_eq!(w[0].points.len(), 1);
    assert!(w[0].use_dots);
}

#[test]
fn old_state_evaluation_fills_its_own_lists() {
    let snd = SoundData::default();
    let mut new = load(|p| {
        wave(p, 3, "", "", "x = 0.25;");
        shape(p, 2, "", "x = 0.75;");
    });
    let mut old = load(|p| {
        wave(p, 4, "", "", "x = 0.5;");
        shape(p, 3, "", "x = 0.125;");
    });
    let inp = inputs(&snd);
    let (nw, ow) = (
        eval_custom_waves(&mut new, &inp),
        eval_custom_waves(&mut old, &inp),
    );
    let (ns, os) = (
        eval_custom_shapes(&mut new, &inp),
        eval_custom_shapes(&mut old, &inp),
    );
    assert_eq!((nw[0].points.len(), nw[0].points[0].x), (3, 0.25));
    assert_eq!((ow[0].points.len(), ow[0].points[0].x), (4, 0.5));
    assert_eq!((ns.len(), ns[0].x), (2, 0.75));
    assert_eq!((os.len(), os[0].x), (3, 0.125));
}

#[test]
fn shape_instances_see_instance_index() {
    let mut st = load(|p| shape(p, 4, "", "x = instance * 0.1; y = num_inst;"));
    let snd = SoundData::default();
    let s = eval_custom_shapes(&mut st, &inputs(&snd));
    assert_eq!(s.len(), 4);
    for (i, v) in s.iter().enumerate() {
        assert_eq!(v.instance, i as u32);
        assert_eq!(v.num_inst, 4);
        assert_eq!(v.x, i as f64 * 0.1);
        assert_eq!(v.y, 4.0);
    }
}

#[test]
fn shape_per_frame_can_change_color() {
    let snd = SoundData {
        imm_rel: [0.5, 0.0, 0.0],
        ..SoundData::default()
    };
    let mut st = load(|p| {
        shape(
            p,
            1,
            "",
            "r = bass; g2 = 0.25; border_a = 1; sides = 5.9; additive = 1;",
        );
    });
    let s = eval_custom_shapes(&mut st, &inputs(&snd));
    let d = &Preset::defaults().shapes[0];
    assert_eq!(s[0].r, 0.5);
    assert_eq!(s[0].g2, 0.25);
    assert_eq!(s[0].border_a, 1.0);
    // Not truncated here: md-render applies `(int)` at the point of use.
    assert_eq!(s[0].sides, 5.9);
    assert_eq!(s[0].additive, 1.0);
    // Untouched values are the preset's, reloaded each instance.
    assert_eq!(s[0].b, f64::from(d.b));
}

#[test]
fn shape_t_resets_to_init_capture_for_every_instance_and_frame() {
    let mut st = load(|p| shape(p, 3, "t1 = 5;", "t1 = t1 + 1; x = t1 * 0.01;"));
    let snd = SoundData::default();
    for _ in 0..2 {
        let s = eval_custom_shapes(&mut st, &inputs(&snd));
        assert!(s.iter().all(|v| v.x == 0.06), "{s:?}");
    }
}

#[test]
fn shape_user_variables_persist_across_instances() {
    let mut st = load(|p| shape(p, 3, "", "n = n + 1; x = n * 0.1;"));
    let snd = SoundData::default();
    let s = eval_custom_shapes(&mut st, &inputs(&snd));
    assert_eq!(
        s.iter().map(|v| v.x).collect::<Vec<_>>(),
        [0.1, 0.2, 0.30000000000000004]
    );
}

#[test]
fn disabled_or_zero_instance_shapes_produce_nothing() {
    let mut st = load(|p| {
        shape(p, 3, "", "");
        p.shapes[0].enabled = 0;
        shape(p, 3, "", "");
        p.shapes[0].enabled = 0;
        p.shapes[1].enabled = 1;
        p.shapes[1].instances = 0;
    });
    let snd = SoundData::default();
    assert!(eval_custom_shapes(&mut st, &inputs(&snd)).is_empty());
}

#[test]
fn failed_programs_behave_as_no_ops() {
    let mut st = load(|p| {
        wave(p, 3, "", "this is not valid(", "x = ;");
        shape(p, 2, "", "x = (;");
    });
    assert!(!st.errors.is_empty());
    let snd = SoundData::default();
    let w = eval_custom_waves(&mut st, &inputs(&snd));
    assert_eq!(w[0].points.len(), 3);
    assert_eq!(w[0].points[0].x, 0.5);
    assert_eq!(eval_custom_shapes(&mut st, &inputs(&snd)).len(), 2);
}

#[test]
fn evaluation_is_deterministic() {
    let mut snd = SoundData::default();
    for (j, x) in snd.wave[1].iter_mut().enumerate() {
        *x = (j as f32 * 0.3).sin();
    }
    let build = || {
        load(|p| {
            wave(
                p,
                32,
                "t1 = 1;",
                "t2 = t1 + time;",
                "x = x + value2 + t2; t4 = t4 + sample; y = t4;",
            );
            shape(p, 3, "", "n = n + 1; x = n * bass + instance; r = sin(n);");
        })
    };
    let run = |st: &mut PresetState| {
        let mut all = Vec::new();
        for f in 0..3 {
            let mut inp = inputs(&snd);
            inp.frame = f;
            all.push((eval_custom_waves(st, &inp), eval_custom_shapes(st, &inp)));
        }
        all
    };
    assert_eq!(run(&mut build()), run(&mut build()));
}

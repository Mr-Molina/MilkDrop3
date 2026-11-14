//! Tests for [`Switcher`] and the timers (Task 7.5). A fake [`ShaderLoader`]
//! stands in for the renderer; the clock is fixed; no GPU.

use crate::timing::{SwitchConfig, SwitchEvent};
use crate::{InitInputs, ShaderLoader, Switcher};
use md_eval::GlobalMem;
use md_preset::Preset;
use md_render::SoundData;
use rand_core::SeedableRng;
use rand_pcg::Pcg32;
use std::cell::RefCell;
use std::rc::Rc;

/// Records every call; the "Gpu" is the preset's tag (its warp shader text).
#[derive(Default)]
struct Fake {
    compiled: Vec<(String, String)>,
    loaded: u32,
}

impl ShaderLoader for Fake {
    type Gpu = String;
    fn compile(&mut self, p: &Preset) -> Result<String, String> {
        self.compiled
            .push((p.warp_shader.clone(), p.comp_shader.clone()));
        Ok(p.warp_shader.clone())
    }
    fn preset_loaded(&mut self) {
        self.loaded += 1;
    }
    fn blend_geometry(&self) -> ((u32, u32), (f32, f32)) {
        ((8, 8), (0.75, 1.0))
    }
}

fn tagged(tag: &str) -> Preset {
    let mut p = Preset::defaults();
    p.warp_shader = tag.into();
    p.comp_shader = format!("{tag}-comp");
    p
}

fn sw() -> Switcher<String> {
    Switcher::new(Rc::new(RefCell::new(GlobalMem::new())))
}

fn rng() -> Pcg32 {
    Pcg32::seed_from_u64(7)
}

/// blend_auto 2, 10 s between presets, no random part: next = start + 12.
fn cfg() -> SwitchConfig {
    SwitchConfig {
        blend_time_auto: 2.0,
        time_between_presets: 10.0,
        time_between_presets_rand: 0.0,
        hard_cuts_disabled: false,
        hard_cut_loudness_thresh: 2.5,
        hard_cut_halflife: 60.0,
    }
}

fn quiet() -> SoundData {
    SoundData::default()
}

fn loud() -> SoundData {
    SoundData {
        imm_rel: [6.0; 3],
        ..SoundData::default()
    }
}

/// Load `p` and return the switcher with `a` already current (instant).
fn with_current(tag: &str, f: &mut Fake, now: f32) -> Switcher<String> {
    let mut s = sw();
    s.load_preset(tagged(tag), 0.0, now, &InitInputs::default(), f, &mut rng())
        .unwrap();
    s
}

fn start_blend(s: &mut Switcher<String>, tag: &str, f: &mut Fake, now: f32) {
    s.load_preset(tagged(tag), 4.0, now, &InitInputs::default(), f, &mut rng())
        .unwrap();
}

fn run_ticks(s: &mut Switcher<String>, n: u32, f: &mut Fake, now: f32) {
    for _ in 0..n {
        s.tick(now, f, &mut rng());
    }
}

#[test]
fn first_load_swaps_immediately_and_reports_loaded() {
    let mut f = Fake::default();
    let mut s = sw();
    // Blend time 5, but nothing valid to blend from: instant
    // (plugin.cpp:7241-7243).
    s.load_preset(
        tagged("a"),
        5.0,
        1.0,
        &InitInputs::default(),
        &mut f,
        &mut rng(),
    )
    .unwrap();
    assert!(!s.is_pending());
    assert!(!s.is_blending());
    assert_eq!(s.gpu().map(String::as_str), Some("a"));
    assert_eq!(f.compiled.len(), 1);
    assert_eq!(f.loaded, 1);
}

#[test]
fn instant_load_uses_instant_init_with_outgoing_progress() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut r = rng();
    // Outgoing preset's timers: start 0, next 12.
    s.update_timers(0.0, 30.0, 0, &quiet(), &cfg(), &mut r);
    let mut b = tagged("b");
    b.per_frame_init = "q1 = bass; q2 = progress;".into();
    let inputs = InitInputs {
        imm_rel: [3.0, 0.0, 0.0],
        ..InitInputs::default()
    };
    s.load_preset(b, 0.0, 6.0, &inputs, &mut f, &mut r).unwrap();
    // Swapped at once, no ticks.
    assert!(!s.is_pending());
    let cur = s.current().unwrap();
    assert_eq!(cur.preset.warp_shader, "b");
    assert_eq!(cur.q_init[0], 3.0, "Instant: init sees the live bass");
    assert_eq!(
        cur.q_init[1], 0.5,
        "progress from the OUTGOING timers (6/12)"
    );
    // An instant load starts no blend and resets the timers afterwards.
    assert!(!s.is_blending());
    assert_eq!(s.preset_start_time(), 6.0);
    assert!(s.next_preset_time() < 0.0);
}

#[test]
fn blended_load_uses_blended_init() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut b = tagged("b");
    b.per_frame_init = "q1 = bass; q2 = progress;".into();
    let inputs = InitInputs {
        imm_rel: [3.0, 0.0, 0.0],
        progress: 0.5,
        ..InitInputs::default()
    };
    s.load_preset(b, 4.0, 1.0, &inputs, &mut f, &mut rng())
        .unwrap();
    assert!(s.is_pending());
    run_ticks(&mut s, 8, &mut f, 1.0);
    let cur = s.current().unwrap();
    assert_eq!(cur.preset.warp_shader, "b");
    assert_eq!(cur.q_init[0], 0.0, "Blended: init sees everything as 0");
    assert_eq!(cur.q_init[1], 0.0);
}

#[test]
fn fake_loader_sees_both_shaders_at_tick_2_and_nothing_at_tick_5() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let base = f.compiled.len();
    start_blend(&mut s, "b", &mut f, 1.0);
    // Ticks 1..8; the counter is 1 on the first call (plugin.cpp:7295, 7355-7356).
    let mut seen = Vec::new();
    for _ in 0..8 {
        s.tick(1.0, &mut f, &mut rng());
        seen.push(f.compiled.len() - base);
    }
    // After call 1 (tick 1): none. Call 2 (tick 2): one compile of BOTH
    // shaders. Calls 3-7 (ticks 3-7, including the no-op tick 5): nothing.
    assert_eq!(seen, vec![0, 1, 1, 1, 1, 1, 1, 1]);
    assert_eq!(
        f.compiled.last().unwrap(),
        &("b".to_string(), "b-comp".to_string())
    );
    assert!(!s.is_pending());
}

#[test]
fn tick_8_swaps_and_starts_blend() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    s.update_timers(0.0, 30.0, 0, &quiet(), &cfg(), &mut rng());
    start_blend(&mut s, "b", &mut f, 3.0);
    run_ticks(&mut s, 7, &mut f, 3.0);
    assert!(s.is_pending(), "still pending after 7 ticks");
    assert_eq!(s.current().unwrap().preset.warp_shader, "a");
    run_ticks(&mut s, 1, &mut f, 3.5);
    assert!(!s.is_pending());
    assert_eq!(s.current().unwrap().preset.warp_shader, "b");
    assert_eq!(s.old().unwrap().preset.warp_shader, "a");
    assert_eq!(s.gpu().map(String::as_str), Some("b"));
    assert_eq!(s.old_gpu().map(String::as_str), Some("a"));
    assert!(s.is_blending());
    let t = s.current().unwrap().preset.transition;
    assert!(t.blending);
    assert_eq!((t.start_time, t.duration), (3.5, 4.0));
    // Blend pattern randomized (a wipe/plasma/radial pattern is never the
    // all-(1, 0) default), sized for the 8x8 grid.
    let pat = s.blend_pattern();
    assert_eq!(pat.len(), 81);
    assert!(pat.iter().any(|v| v.a != 1.0 || v.c != 0.0));
    // Preset timers reset (plugin.cpp:7339-7340) and preset_loaded called
    // once per completed load (first + this one).
    assert_eq!(s.preset_start_time(), 3.5);
    assert!(s.next_preset_time() < 0.0);
    assert_eq!(f.loaded, 2);
}

#[test]
fn preset_loaded_is_called_per_load() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    assert_eq!(f.loaded, 1);
    start_blend(&mut s, "b", &mut f, 1.0);
    run_ticks(&mut s, 7, &mut f, 1.0);
    assert_eq!(f.loaded, 1, "not before the swap");
    run_ticks(&mut s, 1, &mut f, 1.0);
    assert_eq!(f.loaded, 2);
    s.load_preset(
        tagged("c"),
        0.0,
        9.0,
        &InitInputs::default(),
        &mut f,
        &mut rng(),
    )
    .unwrap();
    assert_eq!(f.loaded, 3);
}

#[test]
fn blended_load_during_pending_replaces_it_and_restarts_ticks() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    start_blend(&mut s, "b", &mut f, 1.0);
    run_ticks(&mut s, 3, &mut f, 1.0);
    let compiled = f.compiled.len();
    start_blend(&mut s, "c", &mut f, 1.0);
    assert!(s.is_pending());
    // Ticks restart at 1: seven calls leave it pending, the eighth lands.
    run_ticks(&mut s, 7, &mut f, 1.0);
    assert!(s.is_pending());
    assert_eq!(s.current().unwrap().preset.warp_shader, "a");
    run_ticks(&mut s, 1, &mut f, 1.0);
    assert!(!s.is_pending());
    assert_eq!(s.current().unwrap().preset.warp_shader, "c");
    assert_eq!(s.old().unwrap().preset.warp_shader, "a");
    // Recompiled: only "c" compiled after the replacement.
    assert_eq!(f.compiled.len(), compiled + 1);
    assert_eq!(f.compiled.last().unwrap().0, "c");
}

#[test]
fn instant_load_during_pending_applies_at_once_and_pending_still_lands() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    start_blend(&mut s, "b", &mut f, 1.0);
    run_ticks(&mut s, 3, &mut f, 1.0);
    s.load_preset(
        tagged("c"),
        0.0,
        2.0,
        &InitInputs::default(),
        &mut f,
        &mut rng(),
    )
    .unwrap();
    assert_eq!(s.current().unwrap().preset.warp_shader, "c");
    assert!(s.is_pending(), "the pending load stays");
    assert!(!s.is_blending());
    assert!(s.old().is_none() && s.old_gpu().is_none());
    run_ticks(&mut s, 5, &mut f, 3.0);
    assert!(!s.is_pending());
    assert_eq!(s.current().unwrap().preset.warp_shader, "b");
    // Blends away from the instant preset.
    assert_eq!(s.old().unwrap().preset.warp_shader, "c");
    assert_eq!(s.old_gpu().map(String::as_str), Some("c"));
}

#[test]
fn motion_vectors_freeze_on_the_old_preset_at_blend_start() {
    let mut f = Fake::default();
    let mut a = tagged("a");
    a.mv_a.set(0.5); // visible
    a.mv_x.set(12.0);
    let mut s = sw();
    s.load_preset(a, 0.0, 0.0, &InitInputs::default(), &mut f, &mut rng())
        .unwrap();
    let mut b = tagged("b");
    b.mv_a.set(0.0); // transparent
    b.mv_x.set(40.0);
    s.load_preset(b, 4.0, 1.0, &InitInputs::default(), &mut f, &mut rng())
        .unwrap();
    run_ticks(&mut s, 8, &mut f, 1.0);
    // New preset keeps the old grid (X = old value), and so does the OLD
    // preset itself (state.cpp:826-835): both are 12 at any time.
    let new = &s.current().unwrap().preset;
    let old = &s.old().unwrap().preset;
    assert_eq!(new.mv_x.eval(2.0), 12.0);
    assert_eq!(old.mv_x.value(), 12.0);
    assert!(!old.mv_x.is_blending());
    // mv_a still fades.
    assert_eq!(new.mv_a.value(), 0.0);
    assert!(new.mv_a.is_blending());
}

#[test]
fn motion_vectors_not_frozen_when_new_is_visible() {
    let mut f = Fake::default();
    let mut a = tagged("a");
    a.mv_a.set(0.5);
    a.mv_x.set(12.0);
    let mut s = sw();
    s.load_preset(a, 0.0, 0.0, &InitInputs::default(), &mut f, &mut rng())
        .unwrap();
    let mut b = tagged("b");
    b.mv_a.set(0.5);
    b.mv_x.set(40.0);
    s.load_preset(b, 4.0, 1.0, &InitInputs::default(), &mut f, &mut rng())
        .unwrap();
    run_ticks(&mut s, 8, &mut f, 1.0);
    assert_eq!(s.old().unwrap().preset.mv_x.value(), 12.0);
    assert_eq!(s.current().unwrap().preset.mv_x.value(), 40.0);
}

#[test]
fn auto_advance_after_interval() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut r = rng();
    // Frame 0: next = 0 + 2 + 10 + 0 = 12 (milkdropfs.cpp:771-775).
    let ev = s.update_timers(0.0, 30.0, 0, &quiet(), &cfg(), &mut r);
    assert!(ev.is_empty());
    assert_eq!(s.next_preset_time(), 12.0);
    assert!(s
        .update_timers(11.9, 30.0, 1, &quiet(), &cfg(), &mut r)
        .is_empty());
    // `next < now` is strict.
    assert!(s
        .update_timers(12.0, 30.0, 2, &quiet(), &cfg(), &mut r)
        .is_empty());
    let ev = s.update_timers(12.1, 30.0, 3, &quiet(), &cfg(), &mut r);
    assert_eq!(ev, vec![SwitchEvent::LoadRandom { blend_time: 2.0 }]);
}

#[test]
fn auto_advance_random_part_stays_in_range() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut c = cfg();
    c.time_between_presets_rand = 10.0;
    s.update_timers(0.0, 30.0, 0, &quiet(), &c, &mut rng());
    let n = s.next_preset_time();
    assert!((12.0..22.0).contains(&n), "{n}");
}

#[test]
fn no_auto_advance_while_a_load_is_pending() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut r = rng();
    s.update_timers(0.0, 30.0, 0, &quiet(), &cfg(), &mut r);
    start_blend(&mut s, "b", &mut f, 1.0);
    assert!(s
        .update_timers(20.0, 30.0, 1, &quiet(), &cfg(), &mut r)
        .is_empty());
}

#[test]
fn hard_cut_triggers_and_doubles_threshold() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut r = rng();
    // Frame 0: thresh = 2 * loudness_thresh = 5; 6+6+6 = 18 > 15.
    let ev = s.update_timers(0.0, 30.0, 0, &loud(), &cfg(), &mut r);
    assert_eq!(ev, vec![SwitchEvent::LoadRandom { blend_time: 0.0 }]);
    assert_eq!(s.hard_cut_thresh(), 10.0);
    // 18 < 30: no second cut; the threshold decays (no doubling).
    let ev = s.update_timers(0.03, 30.0, 1, &loud(), &cfg(), &mut r);
    assert!(ev.is_empty());
    assert!(s.hard_cut_thresh() < 10.0);
}

#[test]
fn hard_cut_doubles_even_while_a_load_is_pending_but_starts_none() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut r = rng();
    s.update_timers(0.0, 30.0, 0, &quiet(), &cfg(), &mut r);
    start_blend(&mut s, "b", &mut f, 0.1);
    let before = s.hard_cut_thresh();
    let ev = s.update_timers(0.2, 30.0, 1, &loud(), &cfg(), &mut r);
    assert!(ev.is_empty(), "milkdropfs.cpp:832");
    assert_eq!(s.hard_cut_thresh(), before * 2.0, "milkdropfs.cpp:834");
}

#[test]
fn threshold_decays_toward_loudness_thresh() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut r = rng();
    s.update_timers(0.0, 30.0, 0, &quiet(), &cfg(), &mut r);
    assert_eq!(s.hard_cut_thresh(), 5.0 - decay_step(5.0));
    // One more frame follows the same formula from the new value.
    let t1 = s.hard_cut_thresh();
    s.update_timers(0.03, 30.0, 1, &quiet(), &cfg(), &mut r);
    let expect = (t1 - 2.5) * (-1.3863f32 / (60.0 * 30.0)).exp() + 2.5;
    assert_eq!(s.hard_cut_thresh(), expect);
    assert!(s.hard_cut_thresh() < t1 && s.hard_cut_thresh() > 2.5);
    // Long run: converges to the loudness threshold.
    for i in 2..200_000u32 {
        s.update_timers(i as f32 * 0.03, 30.0, i, &quiet(), &cfg(), &mut r);
    }
    assert!((s.hard_cut_thresh() - 2.5).abs() < 1e-3);
}

/// What one decay step does to `t` at fps 30, halflife 60, loudness 2.5.
fn decay_step(t: f32) -> f32 {
    t - ((t - 2.5) * (-1.3863f32 / (60.0 * 30.0)).exp() + 2.5)
}

#[test]
fn hard_cuts_disabled_or_low_fps_never_cut() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut r = rng();
    let mut c = cfg();
    c.hard_cuts_disabled = true;
    assert!(s
        .update_timers(0.0, 30.0, 0, &loud(), &c, &mut r)
        .is_empty());
    assert_eq!(s.hard_cut_thresh(), 5.0, "no decay when disabled either");
    assert!(s
        .update_timers(0.1, 1.0, 1, &loud(), &cfg(), &mut r)
        .is_empty());
}

#[test]
fn locked_preset_never_advances() {
    for (user, code) in [(true, false), (false, true)] {
        let mut f = Fake::default();
        let mut s = with_current("a", &mut f, 0.0);
        s.locked_by_user = user;
        s.locked_by_code = code;
        let mut r = rng();
        for (i, now) in [0.0f32, 13.0, 100.0, 5000.0].into_iter().enumerate() {
            let ev = s.update_timers(now, 30.0, i as u32, &loud(), &cfg(), &mut r);
            assert!(ev.is_empty(), "locked user={user} code={code} at {now}");
            // Timers frozen at zero progress (plugin.cpp:3926-3931).
            assert_eq!(s.preset_start_time(), now);
            assert_eq!(s.progress(now), 0.0);
        }
        s.locked_by_user = false;
        s.locked_by_code = false;
        let ev = s.update_timers(5000.1, 30.0, 9, &quiet(), &cfg(), &mut r);
        assert!(ev.is_empty(), "the unlock frame recomputes next, no cut");
        let ev = s.update_timers(5013.0, 30.0, 10, &quiet(), &cfg(), &mut r);
        assert_eq!(ev.len(), 1);
    }
}

#[test]
fn blend_t_and_progress_follow_the_cpp_formulas() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut r = rng();
    s.update_timers(0.0, 30.0, 0, &quiet(), &cfg(), &mut r);
    // progress = (now - start) / (next - start) = 3 / 12.
    assert_eq!(s.progress(3.0), 0.25);
    assert_eq!(s.blend_t(3.0), 1.0, "not blending");
    start_blend(&mut s, "b", &mut f, 10.0);
    run_ticks(&mut s, 8, &mut f, 10.0);
    // blend_t = (now - blend_start) / blend_duration = 1 / 4.
    assert_eq!(s.blend_t(11.0), 0.25);
    s.update_timers(11.0, 30.0, 5, &quiet(), &cfg(), &mut r);
    assert_eq!(s.current().unwrap().blend_progress, 0.25);
    assert!(s.is_blending());
    // Past the end the blend is switched off (milkdropfs.cpp:817-820).
    s.update_timers(14.5, 30.0, 6, &quiet(), &cfg(), &mut r);
    assert!(!s.is_blending());
}

#[test]
fn finish_pending_jumps_to_the_new_preset() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    start_blend(&mut s, "b", &mut f, 1.0);
    s.finish_pending(1.0, &mut f, &mut rng());
    assert!(!s.is_pending());
    assert_eq!(s.current().unwrap().preset.warp_shader, "b");
    assert_eq!(s.gpu().map(String::as_str), Some("b"));
}

#[test]
fn zero_blend_auto_can_emit_advance_and_hard_cut_in_one_frame() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    let mut r = rng();
    let mut c = cfg();
    c.blend_time_auto = 0.0;
    s.update_timers(0.0, 30.0, 0, &quiet(), &c, &mut r);
    let ev = s.update_timers(11.0, 30.0, 1, &loud(), &c, &mut r);
    assert_eq!(
        ev,
        vec![
            SwitchEvent::LoadRandom { blend_time: 0.0 },
            SwitchEvent::LoadRandom { blend_time: 0.0 }
        ]
    );
    // With a blended advance the hard cut is skipped (the load is pending).
    let mut s = with_current("a", &mut f, 0.0);
    s.update_timers(0.0, 30.0, 0, &quiet(), &cfg(), &mut r);
    let ev = s.update_timers(13.0, 30.0, 1, &loud(), &cfg(), &mut r);
    assert_eq!(ev, vec![SwitchEvent::LoadRandom { blend_time: 2.0 }]);
}

#[test]
fn old_is_hidden_unless_blending() {
    let mut f = Fake::default();
    let mut s = with_current("a", &mut f, 0.0);
    start_blend(&mut s, "b", &mut f, 1.0);
    run_ticks(&mut s, 8, &mut f, 1.0);
    assert!(s.old().is_some() && s.old_gpu().is_some());
    s.update_timers(20.0, 30.0, 1, &quiet(), &cfg(), &mut rng());
    assert!(!s.is_blending());
    assert!(s.old().is_none() && s.old_gpu().is_none() && s.old_mut().is_none());
}

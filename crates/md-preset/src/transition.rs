//! Preset blending: a port of `CState::StartBlendFrom`
//! (state.cpp:704-849@6f7cd5d).
//!
//! What C++ does, in order, with `this` = the new preset (`s_to`):
//! 1. Copies three "old" values from `s_from` for the renderer
//!    (`m_fVideoEchoAlphaOld`, `m_nVideoEchoOrientationOld`,
//!    `m_nOldWaveMode`); every other bool, int, string and code block of
//!    `s_to` simply stays as loaded -- it switches immediately.
//! 2. Sets the preset-level `m_bBlending = true`, `m_fBlendStartTime`,
//!    `m_fBlendDuration` -- unconditionally, even for a tiny duration.
//! 3. Calls `CBlendableFloat::StartBlendFrom` for each of the 53
//!    `CBlendableFloat` members ([`Preset::blendables`]); each is skipped
//!    when the duration is under 0.001.
//! 4. Motion vectors: if the old preset's `mv_a` was visible (>= 0.001) and
//!    the new one's is transparent, `mv_x`..`mv_b` (not `mv_a`) are not
//!    morphed: C++ assigns `s_from->X = s_to->X.eval(t)` and then
//!    `s_to->X = s_from->X.eval(t)`. On `s_to` that is exactly
//!    `X = X.eval(t)` -- the start-of-blend value (the old preset's value),
//!    with the blend cancelled -- so only `mv_a` fades. (The two C++ `if`s
//!    test the same condition; the reverse case the comment describes,
//!    transparent-before, is not handled in C++ and is not here.)
//!
//! Divergence: C++ also writes into `s_from` (step 4 freezes its `mv_x`..
//! `mv_b` at their current value, and `eval` clears finished blend flags);
//! `from` is borrowed immutably here. This differs only if the old preset's
//! motion-vector fields were themselves still mid-blend, or after a blend
//! shorter than 0.001 s.

use crate::preset::Preset;

/// `CState`'s preset-level blend members (state.h:267-276, 382-385
/// @6f7cd5d). `m_fBlendProgress` is per-frame renderer state and is not
/// stored; it is `(time - start_time) / duration` (milkdropfs.cpp:817).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    /// `m_bBlending`.
    pub blending: bool,
    /// `m_fBlendStartTime`.
    pub start_time: f32,
    /// `m_fBlendDuration`.
    pub duration: f32,
    /// `m_fVideoEchoAlphaOld`: the old preset's `fVideoEchoAlpha` target.
    pub video_echo_alpha_old: f32,
    /// `m_nVideoEchoOrientationOld`.
    pub video_echo_orientation_old: i32,
    /// `m_nOldWaveMode`.
    pub old_wave_mode: i32,
}

impl Default for Transition {
    /// `CState::Default`: `m_bBlending = false` (state.cpp:533@6f7cd5d),
    /// `m_nOldWaveMode = -1` (state.cpp:566@6f7cd5d). The other members are
    /// never initialised by C++ before `StartBlendFrom`; zero here.
    fn default() -> Transition {
        Transition {
            blending: false,
            start_time: 0.0,
            duration: 0.0,
            video_echo_alpha_old: 0.0,
            video_echo_orientation_old: 0,
            old_wave_mode: -1,
        }
    }
}

/// `mv_a` below this counts as transparent (state.cpp:824-825@6f7cd5d).
const MV_TRANSPARENT: f32 = 0.001;

impl Preset {
    /// `CState::StartBlendFrom(from, now, duration)`: start blending this
    /// (newly loaded) preset from `from`. See the module docs.
    pub fn start_blend(&mut self, from: &Preset, now: f32, duration: f32) {
        // Bools, ints and strings change instantly: s_to keeps its own; only
        // the three "old" values are copied over (state.cpp:710-712@6f7cd5d).
        let t = &mut self.transition;
        t.video_echo_alpha_old = from.video_echo_alpha.eval(-1.0);
        t.video_echo_orientation_old = from.video_echo_orientation;
        t.old_wave_mode = from.wave_mode;
        // state.cpp:738-740@6f7cd5d: unconditional.
        t.blending = true;
        t.start_time = now;
        t.duration = duration;

        // CBlendableFloats blend over time (state.cpp:767-820@6f7cd5d).
        let pairs = self.blendables_mut().into_iter().zip(from.blendables());
        for ((_, to), (_, f)) in pairs {
            to.start_blend_from(f, now, duration);
        }

        // Motion vectors fading out keep the old grid (state.cpp:824-847@6f7cd5d).
        let old_transparent = from.mv_a.eval(-1.0) < MV_TRANSPARENT;
        let new_transparent = self.mv_a.eval(-1.0) < MV_TRANSPARENT;
        if !old_transparent && new_transparent {
            for x in [
                &mut self.mv_x,
                &mut self.mv_y,
                &mut self.mv_dx,
                &mut self.mv_dy,
                &mut self.mv_l,
                &mut self.mv_r,
                &mut self.mv_g,
                &mut self.mv_b,
            ] {
                x.set(x.eval(now));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A preset whose i-th blendable is `base + i * step`.
    fn with_blendables(base: f32, step: f32) -> Preset {
        let mut p = Preset::defaults();
        for (i, (_, b)) in p.blendables_mut().into_iter().enumerate() {
            b.set(base + i as f32 * step);
        }
        p
    }

    #[test]
    fn blend_midpoint_is_average_for_blendable_fields() {
        // F62: every field blendables() lists must interpolate. Both mv_a
        // values are visible, so no motion-vector special case applies.
        let from = with_blendables(0.1, 0.01);
        let mut to = with_blendables(1.0, 0.03);
        to.start_blend(&from, 10.0, 4.0);
        let pairs = from.blendables().into_iter().zip(to.blendables());
        let mut n = 0;
        for ((name, a), (_, b)) in pairs {
            let (a, b_target) = (a.value(), b.value());
            assert_ne!(a, b_target, "{name}");
            assert!(b.is_blending(), "{name} not blending");
            assert_eq!(b.eval(10.0), a, "{name} at start");
            assert_eq!(b.eval(12.0), (a + b_target) / 2.0, "{name} at midpoint");
            assert_eq!(b.eval(14.0), b_target, "{name} at end");
            n += 1;
        }
        assert_eq!(n, to.blendables().len());
    }

    #[test]
    fn non_blendable_fields_switch_immediately() {
        let mut from = with_blendables(0.1, 0.01);
        from.wave_mode = 5;
        from.video_echo_orientation = 3;
        from.video_echo_alpha.set(0.75);
        from.rating = 1.0;
        from.warp_anim_speed = 9.0;
        from.tex_wrap = !from.tex_wrap;
        from.invert = true;
        from.warp_ps = 2;
        from.per_frame = "x=1;".into();
        from.warp_shader = "old".into();
        from.waves[1].enabled = 1;
        from.shapes[2].sides = 9;

        let mut to = with_blendables(1.0, 0.03);
        to.wave_mode = 2;
        to.video_echo_orientation = 1;
        let before = to.clone();
        to.start_blend(&from, 3.0, 2.0);

        // Everything but the blendables and the transition is untouched.
        let mut rest = to.clone();
        for ((_, r), (_, b)) in rest.blendables_mut().into_iter().zip(before.blendables()) {
            *r = *b;
        }
        rest.transition = before.transition;
        assert_eq!(rest, before);

        assert_eq!(
            to.transition,
            Transition {
                blending: true,
                start_time: 3.0,
                duration: 2.0,
                video_echo_alpha_old: 0.75,
                video_echo_orientation_old: 3,
                old_wave_mode: 5,
            }
        );
    }

    #[test]
    fn tiny_duration_flags_preset_but_blends_no_field() {
        // m_bBlending is set unconditionally; CBlendableFloat skips < 0.001.
        let from = with_blendables(0.1, 0.01);
        let mut to = with_blendables(1.0, 0.03);
        to.start_blend(&from, 1.0, 0.0005);
        assert!(to.transition.blending);
        assert!(to.blendables().iter().all(|(_, b)| !b.is_blending()));
    }

    #[test]
    fn blend_starts_from_old_presets_current_value() {
        // f_from->eval(fAnimTime): an old preset still mid-blend contributes
        // its blended value, not its target.
        let older = with_blendables(0.0, 0.0);
        let mut from = with_blendables(2.0, 0.0);
        from.start_blend(&older, 0.0, 4.0); // zoom is 1.0 at t=2
        let mut to = with_blendables(4.0, 0.0);
        to.start_blend(&from, 2.0, 2.0);
        assert_eq!(to.zoom.eval(2.0), 1.0);
        assert_eq!(to.zoom.eval(3.0), 2.5);
    }

    #[test]
    fn motion_vectors_fading_out_keep_old_grid() {
        // Old visible, new transparent: mv_x..mv_b jump to the old values
        // and stay; only mv_a fades.
        let mut from = with_blendables(0.1, 0.01);
        from.mv_a.set(0.8);
        let mut to = with_blendables(1.0, 0.03);
        to.mv_a.set(0.0);
        to.start_blend(&from, 0.0, 2.0);
        let fixed = [
            (&to.mv_x, &from.mv_x),
            (&to.mv_y, &from.mv_y),
            (&to.mv_dx, &from.mv_dx),
            (&to.mv_dy, &from.mv_dy),
            (&to.mv_l, &from.mv_l),
            (&to.mv_r, &from.mv_r),
            (&to.mv_g, &from.mv_g),
            (&to.mv_b, &from.mv_b),
        ];
        for (t, f) in fixed {
            assert!(!t.is_blending());
            assert_eq!(t.value(), f.value());
            assert_eq!(t.eval(1.0), f.value());
        }
        assert_eq!(to.mv_a.eval(1.0), 0.4);
        assert!(to.zoom.is_blending());

        // Old transparent, new visible: no special case, all morph.
        let mut from = with_blendables(0.1, 0.01);
        from.mv_a.set(0.0);
        let mut to = with_blendables(1.0, 0.03);
        to.start_blend(&from, 0.0, 2.0);
        assert!(to.mv_x.is_blending() && to.mv_b.is_blending());
    }
}

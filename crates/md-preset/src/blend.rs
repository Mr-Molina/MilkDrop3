//! Blendable floats, ported from `CBlendableFloat` (state.cpp:1900-1975,
//! state.h:54).
//!
//! The C++ value holds a target (`val`), the value to blend from, and a
//! start time and duration. `eval` interpolates `from -> val` only while the
//! time is inside `[start, start + duration]`; at any other time -- including
//! before the window starts and for negative times -- it returns the TARGET
//! value (`val`), never the start value.

/// Minimum blend duration accepted by `StartBlendFrom` (state.cpp:1941).
const MIN_BLEND_DURATION: f32 = 0.001;

/// A float that can linearly blend from an old value to a new target.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Blendable {
    /// `val`: the target (and resting) value.
    value: f32,
    /// `m_fBlendFrom`.
    from: f32,
    /// `m_bBlending`.
    blending: bool,
    /// `m_fBlendStartTime`.
    start_time: f32,
    /// `m_fBlendDuration`.
    duration: f32,
}

impl Blendable {
    /// A resting value with no blend in progress.
    pub fn new(v: f32) -> Blendable {
        Blendable {
            value: v,
            ..Blendable::default()
        }
    }

    /// `operator=`: set the value and cancel any blend.
    pub fn set(&mut self, v: f32) {
        self.value = v;
        self.blending = false;
    }

    /// The target value (`val`).
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Whether a blend has been started and not cancelled by `set`.
    /// (C++ also clears this lazily inside `eval`; here `eval` is pure.)
    pub fn is_blending(&self) -> bool {
        self.blending
    }

    /// `StartBlendFrom`: the target becomes `to`; the value blends from
    /// `from` over `[start, end]`. As in C++, a duration under 0.001 starts
    /// no blend and leaves any existing blend state untouched.
    pub fn blend_to(&mut self, from: f32, to: f32, start: f32, end: f32) {
        self.value = to;
        let duration = end - start;
        if duration < MIN_BLEND_DURATION {
            return;
        }
        self.from = from;
        self.blending = true;
        self.start_time = start;
        self.duration = duration;
    }

    /// `CBlendableFloat::StartBlendFrom(f_from, time, duration)`
    /// (state.cpp:1937-1946@6f7cd5d): keep the target, blend from `from`'s
    /// value AT `time` (its own blend, if any, included) over `duration`
    /// starting at `time`. Unlike [`Blendable::blend_to`] the duration is
    /// stored as given, not recomputed as `end - start`.
    pub fn start_blend_from(&mut self, from: &Blendable, time: f32, duration: f32) {
        if duration < MIN_BLEND_DURATION {
            return;
        }
        self.from = from.eval(time);
        self.blending = true;
        self.start_time = time;
        self.duration = duration;
    }

    /// `CBlendableFloat::eval`. Returns the target outside the blend window
    /// (before it starts, after it ends, or for `time < 0`), otherwise
    /// `from*(1-mix) + val*mix` with `mix = (time-start)/duration`.
    ///
    /// C++ also clears `m_bBlending` as a side effect when `time` leaves the
    /// window; this version is pure, which differs only if time later
    /// re-enters the window (animation time is monotonic in the player).
    pub fn eval(&self, time: f32) -> f32 {
        if time < 0.0 {
            return self.value;
        }
        let in_window =
            self.blending && time <= self.start_time + self.duration && time >= self.start_time;
        if !in_window {
            return self.value;
        }
        let mix = (time - self.start_time) / self.duration;
        self.from * (1.0 - mix) + self.value * mix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// from 10 -> 20 over t in [2, 6].
    fn blending() -> Blendable {
        let mut b = Blendable::new(0.0);
        b.blend_to(10.0, 20.0, 2.0, 6.0);
        b
    }

    #[test]
    fn eval_before_start_returns_target_not_start() {
        // state.cpp:1922 `fTime < m_fBlendStartTime` ends the blend -> val.
        assert_eq!(blending().eval(1.0), 20.0);
        assert_eq!(blending().eval(1.999), 20.0);
    }

    #[test]
    fn negative_time_returns_target() {
        // state.cpp:1914 `fTime < 0` -> val, even mid-blend setups.
        let mut b = Blendable::new(0.0);
        b.blend_to(10.0, 20.0, -4.0, 4.0);
        assert_eq!(b.eval(-1.0), 20.0);
    }

    #[test]
    fn eval_after_end_returns_end() {
        assert_eq!(blending().eval(6.001), 20.0);
        assert_eq!(blending().eval(1000.0), 20.0);
    }

    #[test]
    fn window_edges_are_inclusive() {
        // `>` / `<` comparisons: exactly at start -> from, exactly at end -> val.
        assert_eq!(blending().eval(2.0), 10.0);
        assert_eq!(blending().eval(6.0), 20.0);
    }

    #[test]
    fn eval_midway_is_linear() {
        assert_eq!(blending().eval(4.0), 15.0);
        assert_eq!(blending().eval(3.0), 12.5);
        assert_eq!(blending().eval(5.0), 17.5);
    }

    #[test]
    fn zero_duration_returns_end() {
        // duration < 0.001: no blend starts, value is simply the target.
        let mut b = Blendable::new(5.0);
        b.blend_to(10.0, 20.0, 3.0, 3.0);
        assert!(!b.is_blending());
        assert_eq!(b.eval(3.0), 20.0);
        assert_eq!(b.eval(2.0), 20.0);
        assert_eq!(b.eval(9.0), 20.0);
        // reversed window is also under the minimum.
        b.blend_to(10.0, 30.0, 5.0, 4.0);
        assert!(!b.is_blending());
        assert_eq!(b.eval(4.5), 30.0);
    }

    #[test]
    fn duration_below_minimum_starts_no_blend_but_minimum_does() {
        let mut b = Blendable::new(0.0);
        b.blend_to(10.0, 20.0, 1.0, 1.0005);
        assert!(!b.is_blending());
        b.blend_to(10.0, 20.0, 1.0, 1.002);
        assert!(b.is_blending());
        assert_eq!(b.eval(1.0), 10.0);
    }

    #[test]
    fn short_blend_leaves_existing_blend_running() {
        // C++ returns early from StartBlendFrom without clearing m_bBlending.
        let mut b = blending();
        b.blend_to(0.0, 40.0, 3.0, 3.0);
        assert!(b.is_blending());
        // old window (2..6) and old `from` (10) remain; val is the new target.
        assert_eq!(b.eval(2.0), 10.0);
        assert_eq!(b.eval(4.0), 25.0);
        assert_eq!(b.eval(7.0), 40.0);
    }

    #[test]
    fn start_blend_from_keeps_target_and_reads_from_at_time() {
        let mut to = Blendable::new(20.0);
        to.start_blend_from(&blending(), 4.0, 2.0); // blending() is 15 at t=4
        assert_eq!(to.value(), 20.0);
        assert_eq!(to.eval(4.0), 15.0);
        assert_eq!(to.eval(5.0), 17.5);
        assert_eq!(to.eval(6.5), 20.0);
        let mut short = Blendable::new(1.0);
        short.start_blend_from(&blending(), 4.0, 0.0009);
        assert!(!short.is_blending());
    }

    #[test]
    fn set_cancels_blend() {
        let mut b = blending();
        b.set(7.0);
        assert!(!b.is_blending());
        assert_eq!(b.eval(4.0), 7.0);
        assert_eq!(b.value(), 7.0);
    }

    #[test]
    fn never_started_returns_value() {
        let b = Blendable::new(3.5);
        assert_eq!(b.eval(0.0), 3.5);
        assert_eq!(b.eval(10.0), 3.5);
        assert_eq!(Blendable::default().eval(1.0), 0.0);
    }

    #[test]
    fn blend_at_time_zero_start() {
        let mut b = Blendable::new(0.0);
        b.blend_to(0.0, 1.0, 0.0, 2.0);
        assert_eq!(b.eval(0.0), 0.0);
        assert_eq!(b.eval(1.0), 0.5);
        assert_eq!(b.eval(2.0), 1.0);
    }
}

//! Preset timers, auto-advance and hard cuts (`RenderFrame`,
//! milkdropfs.cpp:765-850, and the lock handling in plugin.cpp:3920-3932).

use rand_core::Rng;

/// The settings the timers read. md-platform's config is Task 8.1; these are
/// the minimal fields, named after the `CPlugin` members with the defaults
/// of plugin.cpp:939-946. Callers apply the clamps of plugin.cpp:1233-1236
/// (`time_between_presets_rand >= 0`, `time_between_presets >= 0.1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwitchConfig {
    /// `m_fBlendTimeAuto` (2.7).
    pub blend_time_auto: f32,
    /// `m_fTimeBetweenPresets` (16).
    pub time_between_presets: f32,
    /// `m_fTimeBetweenPresetsRand` (10).
    pub time_between_presets_rand: f32,
    /// `m_bHardCutsDisabled` (true).
    pub hard_cuts_disabled: bool,
    /// `m_fHardCutLoudnessThresh` (2.5).
    pub hard_cut_loudness_thresh: f32,
    /// `m_fHardCutHalflife` (60).
    pub hard_cut_halflife: f32,
}

impl Default for SwitchConfig {
    fn default() -> SwitchConfig {
        SwitchConfig {
            blend_time_auto: 2.7,
            time_between_presets: 16.0,
            time_between_presets_rand: 10.0,
            hard_cuts_disabled: true,
            hard_cut_loudness_thresh: 2.5,
            hard_cut_halflife: 60.0,
        }
    }
}

/// What the caller must do after [`crate::Switcher::update_timers`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SwitchEvent {
    /// `LoadRandomPreset(blend_time)`: the auto-advance (`blend_time_auto`,
    /// milkdropfs.cpp:785) or a hard cut (0, milkdropfs.cpp:833). The caller
    /// picks from its library and calls [`crate::Switcher::load_preset`].
    LoadRandom { blend_time: f32 },
}

/// The per-frame inputs of [`Timers::update`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimerFrame {
    /// `GetTime()`.
    pub now: f32,
    /// `GetFps()`.
    pub fps: f32,
    /// `GetFrame()`.
    pub frame: u32,
    /// `imm_rel[0] + imm_rel[1] + imm_rel[2]` (milkdropfs.cpp:830).
    pub loudness: f32,
    /// `m_bPresetLockedByUser || m_bPresetLockedByCode`.
    pub locked: bool,
}

/// What one frame of timer logic decided.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimerOutcome {
    /// `m_fNextPresetTime < GetTime()` (milkdropfs.cpp:782).
    pub advance_due: bool,
    /// The hard-cut rule fired (milkdropfs.cpp:830).
    pub hard_cut: bool,
}

/// `m_fPresetStartTime`, `m_fNextPresetTime` and the function-static
/// `m_fHardCutThresh`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timers {
    /// `m_fPresetStartTime`.
    pub start: f32,
    /// `m_fNextPresetTime`; negative means "not set" (plugin.cpp:1026).
    pub next: f32,
    /// `m_fHardCutThresh` (milkdropfs.cpp:825).
    pub hard_cut_thresh: f32,
}

impl Default for Timers {
    fn default() -> Timers {
        Timers {
            start: 0.0,
            next: -1.0,
            hard_cut_thresh: 0.0,
        }
    }
}

impl Timers {
    /// A preset finished loading (plugin.cpp:7267-7268, 7339-7340): restart
    /// the clock; `next` is recomputed by the next [`Timers::update`].
    pub fn reset(&mut self, now: f32) {
        self.start = now;
        self.next = -1.0;
    }

    /// `progress` (milkdropfs.cpp:494): through the preset's slot.
    pub fn progress(&self, now: f32) -> f32 {
        crate::per_frame::preset_progress(now, self.start, self.next)
    }

    /// One frame, in the C++ order: the lock freeze (plugin.cpp:3920-3932,
    /// which runs before `RenderFrame`), frame-0 init (milkdropfs.cpp:
    /// 765-769, 826-827), `next` when unset (771-775), the auto-advance test
    /// (782), then the hard-cut rule (828-847).
    ///
    /// "Locked" holds the preset start at `now` and `next` unset every frame,
    /// so `next` is always `now + blend + interval` ahead: the advance never
    /// comes due. The hard-cut rule also needs both locks off (828).
    pub fn update(
        &mut self,
        f: &TimerFrame,
        cfg: &SwitchConfig,
        rng: &mut impl Rng,
    ) -> TimerOutcome {
        let TimerFrame {
            now,
            fps,
            frame,
            loudness,
            locked,
        } = *f;
        if locked {
            self.start = now;
            self.next = -1.0;
        }
        if frame == 0 {
            self.start = now;
            self.hard_cut_thresh = cfg.hard_cut_loudness_thresh * 2.0;
        }
        if self.next < 0.0 {
            // `m_fTimeBetweenPresetsRand * (rand()%1000)*0.001f`; the draw
            // comes from the caller's generator (A.4).
            let dt = cfg.time_between_presets_rand * (rng.next_u32() % 1000) as f32 * 0.001;
            self.next = now + cfg.blend_time_auto + cfg.time_between_presets + dt;
        }
        let mut out = TimerOutcome {
            advance_due: self.next < now,
            hard_cut: false,
        };
        if fps > 1.0 && !cfg.hard_cuts_disabled && !locked {
            if loudness > self.hard_cut_thresh * 3.0 {
                out.hard_cut = true;
                self.hard_cut_thresh *= 2.0;
            } else {
                let k = -1.3863 / (cfg.hard_cut_halflife * fps);
                self.hard_cut_thresh = (self.hard_cut_thresh - cfg.hard_cut_loudness_thresh)
                    * k.exp()
                    + cfg.hard_cut_loudness_thresh;
            }
        }
        out
    }
}

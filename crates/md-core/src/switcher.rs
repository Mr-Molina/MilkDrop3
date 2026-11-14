//! Preset switching: the instant and the progressive (8-tick) load, the
//! blend start, and the timers (`LoadPreset`, `LoadPresetTick`,
//! plugin.cpp:7203-7357; `StartBlendFrom`, state.cpp:704-849).
//!
//! The library pick stays with the caller ([`crate::PresetLibrary`],
//! [`crate::History`]): [`Switcher::update_timers`] returns
//! [`SwitchEvent::LoadRandom`] and the caller calls
//! [`Switcher::load_preset`] with what it picked.
//!
//! Departures from the brief (the C++ wins, or a ruling applies):
//! - F44: shaders compile through [`ShaderLoader`], BOTH at tick 2 in one
//!   call; tick 5 does nothing (the C++ compiles one shader per tick,
//!   plugin.cpp:7315-7318).
//! - `current` is an `Option`: before the first load the C++ holds
//!   `INVALID_PRESET_DESC` and forces an instant load (plugin.cpp:7241-7243).
//! - `load_preset` takes a parsed [`Preset`] ([`Switcher::load_path`] reads a
//!   file). Only the automatic callers skip while a load is pending
//!   (milkdropfs.cpp:784, 832, in [`Switcher::update_timers`]); a direct
//!   load never does. A blended one replaces the pending load and restarts
//!   its ticks (plugin.cpp:7285-7298); an instant one applies at once and
//!   leaves the pending load in place, which still lands at tick 8 and
//!   blends away from the instant preset.
//! - The blend pattern lives here (`blend_pattern`, sized from
//!   [`ShaderLoader::blend_geometry`]) and is passed to
//!   `WarpMesh::compute_uvs`; the renderer's own `WarpMesh` keeps only the
//!   unblended `a = 1, c = 0` it was built with.
//! - Not ported here: the warp/comp shader locks (`ApplyFlags`,
//!   plugin.cpp:7255-7257, 7290-7291), the history, the "preset not found"
//!   UI message. The `m_fHardCutThresh *= 2` after a user step
//!   (plugin.cpp:6191, 6471) is dead code: it hits the member, while
//!   `RenderFrame` reads the function-static of the same name
//!   (milkdropfs.cpp:825), so it is not ported.

use crate::error::CoreError;
use crate::state::{InitInputs, InitMode, PresetState};
use crate::timing::{SwitchConfig, SwitchEvent, TimerFrame, Timers};
use md_eval::GlobalMem;
use md_preset::{parse_milk, Preset};
use md_render::{randomize_blend_pattern, Renderer, SoundData, VertInfo};
use rand_core::Rng;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

/// The part of the renderer the switcher needs (F44), so tests need no GPU.
pub trait ShaderLoader {
    /// A preset's compiled shaders (`PresetGpu` for the renderer).
    type Gpu;
    /// Compile `preset`'s warp and composite shaders (`LoadShaders`).
    fn compile(&mut self, preset: &Preset) -> Result<Self::Gpu, String>;
    /// `m_nPresetsLoadedTotal++` (plugin.cpp:7307): a load completed
    /// (Task 5.4: protects the current and blend-from textures).
    fn preset_loaded(&mut self);
    /// `((m_nGridX, m_nGridY), (m_fAspectX, m_fAspectY))` for
    /// `RandomizeBlendPattern` (plugin.cpp:7043-7185).
    fn blend_geometry(&self) -> ((u32, u32), (f32, f32));
}

impl ShaderLoader for Renderer {
    type Gpu = md_render::PresetGpu;
    fn compile(&mut self, preset: &Preset) -> Result<Self::Gpu, String> {
        self.load_preset(preset).map_err(|e| e.to_string())
    }
    fn preset_loaded(&mut self) {
        self.textures_mut().preset_loaded();
    }
    fn blend_geometry(&self) -> ((u32, u32), (f32, f32)) {
        (self.mesh().grid(), self.mesh().aspect())
    }
}

/// `m_nLoadingPreset` and what it has gathered (`m_pNewState`,
/// `m_NewShaders`, `m_fLoadingPresetBlendTime`).
struct Pending<G> {
    state: PresetState,
    blend_time: f32,
    /// `m_nLoadingPreset`: 1 at the load, +1 after every tick.
    tick: u8,
    gpu: Option<G>,
}

/// The loaded presets, their shaders, the pending load and the timers.
pub struct Switcher<G> {
    current: Option<PresetState>,
    old: Option<PresetState>,
    pending: Option<Pending<G>>,
    gpu: Option<G>,
    old_gpu: Option<G>,
    blend_pattern: Vec<VertInfo>,
    timers: Timers,
    global: Rc<RefCell<GlobalMem>>,
    /// `m_bPresetLockedByUser` (scroll lock).
    pub locked_by_user: bool,
    /// `m_bPresetLockedByCode` (a menu is up, plugin.cpp:3922).
    pub locked_by_code: bool,
    /// The last shader compile error, for the UI.
    pub last_error: Option<String>,
}

impl<G> Switcher<G> {
    /// No preset yet; the first load is instant.
    pub fn new(global: Rc<RefCell<GlobalMem>>) -> Switcher<G> {
        Switcher {
            current: None,
            old: None,
            pending: None,
            gpu: None,
            old_gpu: None,
            blend_pattern: Vec::new(),
            timers: Timers::default(),
            global,
            locked_by_user: false,
            locked_by_code: false,
            last_error: None,
        }
    }

    pub fn current(&self) -> Option<&PresetState> {
        self.current.as_ref()
    }
    pub fn current_mut(&mut self) -> Option<&mut PresetState> {
        self.current.as_mut()
    }
    /// `m_pOldState`, only while [`Switcher::is_blending`]: the preset being
    /// blended from. (An instant load keeps the previous preset internally,
    /// as the C++ keeps `m_pOldState`, but it is not exposed.)
    pub fn old(&self) -> Option<&PresetState> {
        self.old.as_ref().filter(|_| self.is_blending())
    }
    pub fn old_mut(&mut self) -> Option<&mut PresetState> {
        if self.is_blending() {
            self.old.as_mut()
        } else {
            None
        }
    }
    pub fn gpu(&self) -> Option<&G> {
        self.gpu.as_ref()
    }
    /// `m_OldShaders`, only while [`Switcher::is_blending`] (hand it to the
    /// renderer as `old_gpu`).
    pub fn old_gpu(&self) -> Option<&G> {
        self.old_gpu.as_ref().filter(|_| self.is_blending())
    }
    /// The blend pattern for `WarpMesh::compute_uvs`.
    pub fn blend_pattern(&self) -> &[VertInfo] {
        &self.blend_pattern
    }
    /// `m_nLoadingPreset != 0`.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    /// `m_pState->m_bBlending`.
    pub fn is_blending(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|c| c.preset.transition.blending)
    }
    pub fn preset_start_time(&self) -> f32 {
        self.timers.start
    }
    pub fn next_preset_time(&self) -> f32 {
        self.timers.next
    }
    pub fn hard_cut_thresh(&self) -> f32 {
        self.timers.hard_cut_thresh
    }
    /// `progress` (milkdropfs.cpp:494): `(now - start) / (next - start)`.
    pub fn progress(&self, now: f32) -> f32 {
        self.timers.progress(now)
    }

    /// `m_fBlendProgress` (milkdropfs.cpp:817): `(now - blend_start) /
    /// blend_duration` while blending, else 1.
    pub fn blend_t(&self, now: f32) -> f32 {
        match &self.current {
            Some(c) if c.preset.transition.blending => {
                let t = &c.preset.transition;
                (now - t.start_time) / t.duration
            }
            _ => 1.0,
        }
    }

    /// Per frame, before the per-frame equations (`RenderFrame`,
    /// milkdropfs.cpp:765-850): timers, `m_fBlendProgress` and the blend
    /// end, auto-advance and hard cuts. Call it only on frames that are not
    /// redraws (milkdropfs.cpp:777, plugin.cpp:3917-3919). The caller loads
    /// what the events ask
    /// for. `frame` is `GetFrame()`; `sound.imm_rel` feeds the hard cut.
    pub fn update_timers(
        &mut self,
        now: f32,
        fps: f32,
        frame: u32,
        sound: &SoundData,
        cfg: &SwitchConfig,
        rng: &mut impl Rng,
    ) -> Vec<SwitchEvent> {
        let locked = self.locked_by_user || self.locked_by_code;
        let loudness: f32 = sound.imm_rel.iter().sum();
        let frame = TimerFrame {
            now,
            fps,
            frame,
            loudness,
            locked,
        };
        let out = self.timers.update(&frame, cfg, rng);
        // milkdropfs.cpp:814-821.
        let t = self.blend_t(now);
        if let Some(c) = self
            .current
            .as_mut()
            .filter(|c| c.preset.transition.blending)
        {
            c.blend_progress = t;
            if t > 1.0 {
                c.preset.transition.blending = false;
            }
        }
        let mut events = Vec::new();
        // "don't start a load if one is already underway!" (784, 832).
        if out.advance_due && !self.is_pending() {
            events.push(SwitchEvent::LoadRandom {
                blend_time: cfg.blend_time_auto,
            });
        }
        // The C++ guard reads m_nLoadingPreset right after the advance's
        // LoadRandomPreset: a blended advance has made it pending, so the
        // hard cut skips; with blend_time_auto == 0 both loads fire.
        let advance_pends = !events.is_empty() && cfg.blend_time_auto != 0.0;
        if out.hard_cut && !self.is_pending() && !advance_pends {
            events.push(SwitchEvent::LoadRandom { blend_time: 0.0 });
        }
        events
    }

    /// `LoadPreset` (plugin.cpp:7203-7299) for a parsed preset. `inputs` are
    /// the live values an instant load's init code sees (`progress` is
    /// replaced by the outgoing preset's, Task 7.1). A load while one is
    /// pending is accepted (see the module docs).
    ///
    /// Blend time 0, or no valid current preset, swaps at once
    /// (`InitMode::Instant`, plugin.cpp:7245-7281): no ticks and no blend.
    /// Otherwise the preset is parsed into the pending slot
    /// (`InitMode::Blended`) and [`Switcher::tick`] finishes it.
    pub fn load_preset(
        &mut self,
        preset: Preset,
        blend_time: f32,
        now: f32,
        inputs: &InitInputs,
        loader: &mut impl ShaderLoader<Gpu = G>,
        rng: &mut impl Rng,
    ) -> Result<(), CoreError> {
        let blend_time = if self.current.is_none() {
            0.0
        } else {
            blend_time
        };
        if blend_time == 0.0 {
            let mut i = inputs.clone();
            i.progress = f64::from(self.progress(now));
            let state = PresetState::from_preset_with(
                preset,
                self.global.clone(),
                rng,
                &InitMode::Instant(i),
            )?;
            self.old = self.current.replace(state);
            self.old_gpu = self.gpu.take();
            self.gpu = compile(
                loader,
                &self.current.as_ref().expect("set above").preset,
                &mut self.last_error,
            );
            self.timers.reset(now);
            loader.preset_loaded();
        } else {
            let state = PresetState::from_preset_with(
                preset,
                self.global.clone(),
                rng,
                &InitMode::Blended,
            )?;
            // A blended request replaces a pending one and restarts the tick
            // count (plugin.cpp:7285-7298); the C++ leaks the old pending
            // state's shaders there, which is not mirrored.
            self.pending = Some(Pending {
                state,
                blend_time,
                tick: 1,
                gpu: None,
            });
        }
        Ok(())
    }

    /// Read and parse `path`, then [`Switcher::load_preset`]. A missing or
    /// unparsable file is an error and changes nothing (plugin.cpp:
    /// 7211-7218).
    pub fn load_path(
        &mut self,
        path: &Path,
        blend_time: f32,
        now: f32,
        inputs: &InitInputs,
        loader: &mut impl ShaderLoader<Gpu = G>,
        rng: &mut impl Rng,
    ) -> Result<(), CoreError> {
        let bytes = std::fs::read(path)
            .map_err(|e| CoreError::Preset(format!("{}: {e}", path.display())))?;
        let preset = parse_milk(&String::from_utf8_lossy(&bytes))
            .map_err(|e| CoreError::Preset(format!("{}: {e}", path.display())))?;
        self.load_preset(preset, blend_time, now, inputs, loader, rng)
    }

    /// `LoadPresetTick` (plugin.cpp:7313-7357), once per frame after the
    /// frame is drawn (plugin.cpp:3992-3994). With a load pending: tick 2
    /// compiles both shaders (F44; the C++ compiles one per tick at 2 and
    /// 5), tick 5 does nothing, tick 8 applies the preset
    /// ([`Switcher::finish_pending`]); then the counter advances.
    pub fn tick(&mut self, now: f32, loader: &mut impl ShaderLoader<Gpu = G>, rng: &mut impl Rng) {
        let Some(p) = self.pending.as_mut() else {
            return;
        };
        match p.tick {
            2 => {
                p.gpu = compile(loader, &p.state.preset, &mut self.last_error);
            }
            8 => {
                self.finish_pending(now, loader, rng);
                return;
            }
            _ => {}
        }
        if let Some(p) = self.pending.as_mut() {
            p.tick += 1;
        }
    }

    /// Tick 8 (plugin.cpp:7320-7352): swap the states and shaders,
    /// randomize the blend pattern, start the blend, reset the preset
    /// timers. The C++ device-reset path (plugin.cpp:3768-3774) also forces
    /// `m_bBlending = false` right after; 7.9 must do that
    /// (`current_mut().preset.transition.blending = false`). Also what the
    /// C++ does to jump to the new preset when the
    /// device is reset (plugin.cpp:3768-3771); shaders not yet compiled
    /// are compiled here (the C++ would run with none). No-op without a
    /// pending load.
    pub fn finish_pending(
        &mut self,
        now: f32,
        loader: &mut impl ShaderLoader<Gpu = G>,
        rng: &mut impl Rng,
    ) {
        let Some(p) = self.pending.take() else {
            return;
        };
        let Pending {
            mut state,
            blend_time,
            gpu,
            ..
        } = p;
        let gpu = match gpu {
            Some(g) => Some(g),
            None => compile(loader, &state.preset, &mut self.last_error),
        };
        // RandomizeBlendPattern (plugin.cpp:7334).
        let ((gx, gy), aspect) = loader.blend_geometry();
        let n = ((gx.max(1) + 1) * (gy.max(1) + 1)) as usize;
        self.blend_pattern.resize(
            n,
            VertInfo {
                a: 1.0,
                c: 0.0,
                ..VertInfo::default()
            },
        );
        randomize_blend_pattern(rng, gx, gy, aspect, &mut self.blend_pattern);
        self.old = self.current.take();
        self.old_gpu = self.gpu.take();
        if let Some(old) = self.old.as_mut() {
            start_blend_from(&mut state, old, now, blend_time);
        }
        self.current = Some(state);
        self.gpu = gpu;
        self.timers.reset(now);
        loader.preset_loaded();
    }
}

/// `CState::StartBlendFrom` on the new preset `to` and, for the part
/// [`Preset::start_blend`] cannot do (Task 2.5 ruling), the OLD preset
/// `from`: when the old motion vectors were visible and the new are not,
/// `s_from->MvX.. = s_to->MvX.eval(now)` (state.cpp:826-835). Both then
/// hold the start-of-blend value, so the old preset's blend is cancelled and
/// its values frozen (`to` was already frozen by `start_blend`).
fn start_blend_from(to: &mut PresetState, from: &mut PresetState, now: f32, duration: f32) {
    let was_visible = from.preset.mv_a.eval(-1.0) >= 0.001;
    let new_transparent = to.preset.mv_a.eval(-1.0) < 0.001;
    to.preset.start_blend(&from.preset, now, duration);
    if was_visible && new_transparent {
        let (t, f) = (&to.preset, &mut from.preset);
        f.mv_x.set(t.mv_x.value());
        f.mv_y.set(t.mv_y.value());
        f.mv_dx.set(t.mv_dx.value());
        f.mv_dy.set(t.mv_dy.value());
        f.mv_l.set(t.mv_l.value());
        f.mv_r.set(t.mv_r.value());
        f.mv_g.set(t.mv_g.value());
        f.mv_b.set(t.mv_b.value());
    }
}

/// `LoadShaders`: a failed compile leaves no shaders and is kept for the UI.
fn compile<G>(
    loader: &mut impl ShaderLoader<Gpu = G>,
    preset: &Preset,
    last_error: &mut Option<String>,
) -> Option<G> {
    match loader.compile(preset) {
        Ok(g) => Some(g),
        Err(e) => {
            *last_error = Some(e);
            None
        }
    }
}

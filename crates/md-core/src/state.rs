//! [`PresetState`]: a loaded preset with its expression contexts
//! (`CState` after `ImportFromFile`: `Default` then `RecompileExpressions`,
//! state.cpp:1282 and 1452).

use crate::custom::{ShapeState, WaveState};
use crate::error::{CoreError, ProgramError};
use crate::vars::{PerFrameRefs, PerPixelRefs};
use md_eval::{GlobalMem, Program, Vm};
use md_preset::Preset;
use md_render::{randomize_preset_vars, PerFrameVars, RotParams};
use rand_core::Rng;
use std::cell::RefCell;
use std::rc::Rc;

/// The plugin-global values the C++ init code sees through
/// `LoadPerFrameEvallibVars` and its wave/shape twins (`GetTime()`,
/// `GetFps()`, `GetFrame()`, `mysound`, grid and window size, progress
/// through the preset, milkdropfs.cpp:463-553). All 0 by default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InitInputs {
    /// `GetTime()`: the clock the non-motion blendables are evaluated at.
    pub now: f32,
    /// `time`: `GetTime() - m_fStartTime`.
    pub time: f64,
    pub fps: f64,
    pub frame: f64,
    pub progress: f64,
    /// `mysound.imm_rel` (`bass mid treb`).
    pub imm_rel: [f64; 3],
    /// `mysound.avg_rel` (`bass_att mid_att treb_att`).
    pub avg_rel: [f64; 3],
    pub meshx: f64,
    pub meshy: f64,
    pub pixelsx: f64,
    pub pixelsy: f64,
    /// `m_fInvAspectX/Y`.
    pub aspectx: f64,
    pub aspecty: f64,
}

/// How the preset is being loaded; it decides what the init code sees
/// (`LoadPerFrameEvallibVars(g_plugin.m_pState)`, state.cpp:1684, and the
/// wave/shape twins at 1750 and 1821, write into `m_pState`'s variables).
#[derive(Clone, Debug, PartialEq)]
pub enum InitMode {
    /// Blend time 0 (first preset, hard cuts): the states are swapped
    /// before `Import` (plugin.cpp:7251-7259), so `m_pState` is the
    /// incoming preset and its init code sees its own values plus the live
    /// inputs. The caller supplies `progress` from the OUTGOING preset's
    /// timers, which are reset only after `Import` (plugin.cpp:7259, 7267).
    Instant(InitInputs),
    /// Blended load: `Import` runs into `m_pNewState` while `m_pState` is
    /// still the outgoing state (plugin.cpp:7293), so the loaders write to
    /// the outgoing state and the incoming init code runs over freshly reset
    /// variables: preset, wave and shape init see everything as 0 and the
    /// wave/shape init get `q` = 0.
    Blended,
}

/// Compile `src` (already comment-stripped) into `vm`. Empty source gives
/// `None` silently (`if (buf[0])`, state.cpp:1666 and 1699); a compile error gives
/// `None` and a [`ProgramError`] for `section`.
pub(crate) fn compile_section(
    vm: &mut Vm,
    src: &str,
    section: &str,
    errors: &mut Vec<ProgramError>,
) -> Option<Program> {
    if src.trim().is_empty() {
        return None;
    }
    match Program::compile(vm, src) {
        Ok(p) => Some(p),
        Err(error) => {
            errors.push(ProgramError {
                section: section.to_string(),
                error,
            });
            None
        }
    }
}

/// A preset and its evaluation contexts.
pub struct PresetState {
    pub preset: Preset,
    /// Per-frame `Vm` (`m_pf_eel`); the init code ran in it, so user
    /// variables and `megabuf` it set persist into the per-frame code.
    pub pf: Vm,
    /// Per-pixel `Vm` (`m_pv_eel`).
    pub pv: Vm,
    pub pf_refs: PerFrameRefs,
    pub pv_refs: PerPixelRefs,
    pub pf_program: Option<Program>,
    pub pv_program: Option<Program>,
    pub waves: [WaveState; 4],
    pub shapes: [ShapeState; 4],
    /// `q_values_after_init_code` (state.h:440).
    pub q_init: [f64; 32],
    /// `monitor_after_init_code` (state.h:441); the C++ also refreshes it
    /// after every per-frame run (milkdropfs.cpp:670).
    pub monitor_init: f64,
    /// `m_rand_preset`, four `FRAND` drawn at load.
    pub rand_preset: [f32; 4],
    /// `m_xlate`/`m_rot_base`/`m_rot_speed` for the 24 shader rotation
    /// slots (the C++ fills 20; see `md_render::randomize_preset_vars`).
    pub rot: [RotParams; 24],
    /// `m_fBlendProgress`.
    pub blend_progress: f32,
    /// Compile errors in load order, for the UI to show.
    pub errors: Vec<ProgramError>,
}

impl PresetState {
    /// [`PresetState::from_preset_with`] as an instant load with all
    /// plugin inputs 0.
    pub fn from_preset(
        preset: Preset,
        global: Rc<RefCell<GlobalMem>>,
        rng: &mut impl Rng,
    ) -> Result<PresetState, CoreError> {
        Self::from_preset_with(
            preset,
            global,
            rng,
            &InitMode::Instant(InitInputs::default()),
        )
    }

    /// Draw the per-preset random values (`Default` ->
    /// `RandomizePresetVars`, state.cpp:699), register every variable,
    /// compile all programs and run the init code in the order of
    /// `RecompileExpressions` (state.cpp:1552-1873): preset
    /// `per_frame_init`, per-frame, per-pixel; then each wave's and each
    /// shape's init, per-frame and (waves) per-point code.
    ///
    /// What the init code sees depends on `mode` (see [`InitMode`]).
    /// `q`/`monitor` with no init code are 0 on purpose: the C++ holds the
    /// stale `q_values_after_init_code` of whatever preset the reused
    /// `CState` slot held before (state.h:440-441, never reset).
    pub fn from_preset_with(
        preset: Preset,
        global: Rc<RefCell<GlobalMem>>,
        rng: &mut impl Rng,
        mode: &InitMode,
    ) -> Result<PresetState, CoreError> {
        let loaded = match mode {
            InitMode::Instant(i) => Some(i),
            InitMode::Blended => None,
        };
        let (rand_preset, rot) = randomize_preset_vars(rng);
        let mut errors = Vec::new();

        let mut pf = Vm::new(global.clone());
        let mut pv = Vm::new(global.clone());
        let pf_refs = PerFrameRefs::register(&mut pf);
        let pv_refs = PerPixelRefs::register(&mut pv);

        let mut q_init = [0.0; 32];
        let mut monitor_init = 0.0;
        let init = preset.per_frame_init_eel();
        if let Some(prog) = compile_section(&mut pf, &init, "per_frame_init", &mut errors) {
            if let Some(inputs) = loaded {
                let mut v = PerFrameVars::from_preset(&preset, inputs.now);
                v.time = inputs.time;
                v.fps = inputs.fps;
                v.frame = inputs.frame;
                v.progress = inputs.progress;
                v.bass = inputs.imm_rel[0];
                v.mid = inputs.imm_rel[1];
                v.treb = inputs.imm_rel[2];
                v.bass_att = inputs.avg_rel[0];
                v.mid_att = inputs.avg_rel[1];
                v.treb_att = inputs.avg_rel[2];
                v.meshx = inputs.meshx;
                v.meshy = inputs.meshy;
                v.pixelsx = inputs.pixelsx;
                v.pixelsy = inputs.pixelsy;
                v.aspectx = inputs.aspectx;
                v.aspecty = inputs.aspecty;
                pf_refs.load(&mut pf, &v);
            }
            prog.run(&mut pf);
            for (x, r) in q_init.iter_mut().zip(&pf_refs.q) {
                *x = pf.get(*r);
            }
            monitor_init = pf.get(pf_refs.monitor);
        }
        let pf_program =
            compile_section(&mut pf, &preset.per_frame_eel(), "per_frame", &mut errors);
        let pv_program =
            compile_section(&mut pv, &preset.per_pixel_eel(), "per_pixel", &mut errors);

        let waves = std::array::from_fn(|i| {
            WaveState::load(i, &preset.waves[i], &global, &q_init, loaded, &mut errors)
        });
        let shapes = std::array::from_fn(|i| {
            ShapeState::load(i, &preset.shapes[i], &global, &q_init, loaded, &mut errors)
        });

        Ok(PresetState {
            preset,
            pf,
            pv,
            pf_refs,
            pv_refs,
            pf_program,
            pv_program,
            waves,
            shapes,
            q_init,
            monitor_init,
            rand_preset,
            rot,
            blend_progress: 0.0,
            errors,
        })
    }
}

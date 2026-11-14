//! The minimal headless driver (F55). Replaced by `md_core::Visualizer` in
//! Task 7.9.
//!
//! Scope: per-frame values come only from the static preset
//! ([`PerFrameVars::from_preset`]) plus `progress`; no equations or custom
//! waves run. Each enabled custom shape is drawn once from its static
//! (preset-file) values (Task 6.9 ruling; `num_inst` is ignored without
//! equations). Everything is deterministic: two [`FixedClock`]s at 60 fps
//! (frame `n` renders at time `n / 60`, `n` counting from 1, for both the
//! absolute clock and the since-preset-start clock), one `Pcg32` seeded from
//! the `--seed` for `rand_preset`, the rotations, `rand_frame` and the
//! preset-time jitter, and a [`WavSource`] advanced exactly once per frame.

use crate::render_cmd::RenderArgs;
use md_analysis::{Analyzer, SoundData};
use md_audio::{AudioError, AudioSource, SilenceSource, WavSource};
use md_preset::{parse_milk, ParseError, Preset};
use md_render::noise::crand;
use md_render::{
    frand, randomize_preset_vars, read_texture_level, FrameInput, Gpu, PerFrameVars, PresetGpu,
    RenderError, RenderSettings, Renderer, RgbaImage, RotParams, ShapeFrameVars, Texture2d,
};
use md_testkit::FixedClock;
use rand_core::SeedableRng;
use rand_pcg::Pcg32;
use std::time::Duration;

/// Render rate of the headless clock and of the WAV pacing (F51).
pub const FPS: f32 = 60.0;
/// `m_fBlendTimeAuto` (plugin.cpp:940), `m_fTimeBetweenPresets` (:941) and
/// `m_fTimeBetweenPresetsRand` (:942): the C++ defaults.
const BLEND_TIME_AUTO: f32 = 2.7;
const TIME_BETWEEN_PRESETS: f32 = 16.0;
const TIME_BETWEEN_PRESETS_RAND: f32 = 10.0;

/// Driver errors.
#[derive(Debug, thiserror::Error)]
pub enum HeadlessError {
    #[error("cannot read preset {path}: {source}")]
    ReadPreset {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("cannot parse preset: {0}")]
    Preset(#[from] ParseError),
    #[error("audio: {0}")]
    Audio(#[from] AudioError),
    #[error("renderer: {0}")]
    Render(#[from] RenderError),
}

/// A preset rendered frame by frame into an offscreen `Rgba8Unorm` target.
pub struct Headless {
    renderer: Renderer,
    preset: Preset,
    pg: PresetGpu,
    rot: [RotParams; 24],
    /// One static instance per enabled shape (shape order).
    shapes: Vec<ShapeFrameVars>,
    rand_preset: [f32; 4],
    rng: Pcg32,
    audio: Box<dyn AudioSource>,
    analyzer: Analyzer,
    abs_clock: FixedClock,
    preset_clock: FixedClock,
    /// `m_fNextPresetTime - m_fPresetStartTime`, the progress denominator.
    preset_duration: f32,
    frame: u32,
    target: Texture2d,
}

impl Headless {
    /// Load the preset and audio named by `args` (`frames`/`every`/`out` are
    /// not read here).
    pub fn new(gpu: &Gpu, args: &RenderArgs) -> Result<Headless, HeadlessError> {
        let text = std::fs::read(&args.preset).map_err(|source| HeadlessError::ReadPreset {
            path: args.preset.clone(),
            source,
        })?;
        let preset = parse_milk(&String::from_utf8_lossy(&text))?;
        // F51: loop, 60 fps, advanced once per rendered frame.
        let audio: Box<dyn AudioSource> = match &args.wav {
            Some(p) => Box::new(WavSource::open(p, true, FPS)?),
            None => Box::new(SilenceSource),
        };
        let (w, h) = args.size;
        let mut renderer = Renderer::new(
            gpu.device(),
            gpu.queue(),
            RenderSettings::new(w, h),
            args.seed,
        )?;
        let (w, h) = renderer.window();
        if let Some(dir) = args.preset.parent() {
            renderer.textures_mut().set_preset_dir(Some(dir));
        }
        // `load_preset` does not count the load; `preset_loaded()` does
        // (`m_nPresetsLoadedTotal++`, plugin.cpp:7307).
        let pg = renderer.load_preset(&preset)?;
        renderer.textures_mut().preset_loaded();
        let mut rng = Pcg32::seed_from_u64(args.seed);
        let (rand_preset, rot) = randomize_preset_vars(&mut rng);
        // milkdropfs.cpp:771-775: the first UpdateTime sets
        // `m_fNextPresetTime = GetTime() + m_fBlendTimeAuto +
        // m_fTimeBetweenPresets + m_fTimeBetweenPresetsRand * (rand()%1000) * 0.001`
        // with the preset starting at the same time, so the duration is
        // the sum (plugin.cpp:940-942 defaults).
        let dt = TIME_BETWEEN_PRESETS_RAND * (crand(&mut rng) % 1000) as f32 * 0.001;
        let preset_duration = BLEND_TIME_AUTO + TIME_BETWEEN_PRESETS + dt;
        let shapes = static_shapes(&preset);
        let target = Texture2d::new_target(gpu, w, h, wgpu_rgba8(), false);
        Ok(Headless {
            renderer,
            preset,
            pg,
            rot,
            shapes,
            rand_preset,
            rng,
            audio,
            analyzer: Analyzer::new(),
            abs_clock: FixedClock::new(f64::from(FPS)),
            preset_clock: FixedClock::new(f64::from(FPS)),
            preset_duration,
            frame: 0,
            target,
        })
    }

    /// Render the next frame (numbered from 1) and read it back.
    pub fn render_frame(&mut self) -> RgbaImage {
        self.frame += 1;
        self.abs_clock.advance();
        self.preset_clock.advance();
        let now = self.abs_clock.now() as f32;
        let time = self.preset_clock.now() as f32;

        let (mut l, mut r) = ([0u8; 576], [0u8; 576]);
        self.audio
            .next_frame(&mut l, &mut r, Duration::from_millis(0));
        let sound: SoundData = self.analyzer.analyze(&l, &r, FPS).clone();

        // `LoadPerFrameEvallibVars` line 494 (milkdropfs.cpp):
        // progress = (GetTime() - m_fPresetStartTime) /
        //            (m_fNextPresetTime - m_fPresetStartTime).
        let progress = time / self.preset_duration;
        let mut vars = PerFrameVars::from_preset(&self.preset, now);
        vars.progress = f64::from(progress);

        let rand_frame = [0; 4].map(|_| frand(&mut self.rng));
        // fWarpTime is the absolute clock (milkdropfs.cpp:767).
        self.renderer.mesh_mut().compute_uvs(&vars, None, now, None);
        let input = FrameInput {
            time,
            frame: self.frame,
            fps: FPS,
            progress,
            rand_frame,
            rand_preset: self.rand_preset,
            shapes: &self.shapes,
            now,
            preset_start_time: now - time,
            ..FrameInput::new(&self.preset, &self.pg, &vars, &self.rot, &sound)
        };
        let ctx = self.renderer.begin_frame(&input);
        self.renderer.present(ctx, &self.target.view);
        let bytes = read_texture_level(self.renderer.gpu(), &self.target.tex, 0);
        RgbaImage::from_raw(self.target.w, self.target.h, bytes)
            .expect("readback is w*h RGBA8 bytes")
    }
}

/// Each enabled shape's preset-file values as one instance (no per-frame
/// program runs, so `instance` is 0 and `num_inst` 1).
fn static_shapes(preset: &Preset) -> Vec<ShapeFrameVars> {
    preset
        .shapes
        .iter()
        .filter(|s| s.enabled != 0)
        .map(|s| ShapeFrameVars {
            x: f64::from(s.x),
            y: f64::from(s.y),
            rad: f64::from(s.rad),
            ang: f64::from(s.ang),
            tex_ang: f64::from(s.tex_ang),
            tex_zoom: f64::from(s.tex_zoom),
            r: f64::from(s.r),
            g: f64::from(s.g),
            b: f64::from(s.b),
            a: f64::from(s.a),
            r2: f64::from(s.r2),
            g2: f64::from(s.g2),
            b2: f64::from(s.b2),
            a2: f64::from(s.a2),
            border_r: f64::from(s.border_r),
            border_g: f64::from(s.border_g),
            border_b: f64::from(s.border_b),
            border_a: f64::from(s.border_a),
            sides: f64::from(s.sides),
            textured: f64::from(s.textured),
            additive: f64::from(s.additive),
            thick: f64::from(s.thick_outline),
            instance: 0,
            num_inst: 1,
        })
        .collect()
}

fn wgpu_rgba8() -> md_render::wgpu::TextureFormat {
    md_render::wgpu::TextureFormat::Rgba8Unorm
}

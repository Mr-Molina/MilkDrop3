//! The [`Renderer`] (A.3; created here in Task 5.5 per F36 — Task 5.7 adds
//! `begin_frame`, `present`, `resize` and the composite pass): the
//! ping-pong main textures, the blur textures, the [`TextureStore`], the
//! fixed-function [`PipelineCache`], the [`ShaderCache`] and preset loading.
//!
//! Preset shader compilation (F44): [`Renderer::load_preset`] compiles both
//! the warp and the composite shader in one call. md-core's preset switcher
//! (Milestone 7) reaches it through its `ShaderLoader` trait, implemented
//! for `Renderer` there and faked in its tests, and calls it at tick 2 of
//! its 8-tick schedule.

use crate::gpu::Gpu;
use crate::pipelines::PipelineCache;
use crate::preset_bind::Resolver;
use crate::preset_gpu::{PresetGpu, PresetLayouts, ProgramSpec, ProgramTargets, ShaderProgram};
use crate::shader_cache::ShaderCache;
use crate::texture::{Texture2d, Texture3d};
use crate::textures::TextureStore;
use crate::warp::WarpBuffers;
use md_hlsl::{PsVersion, ShaderInput, ShaderKind, TranslateError};
use md_preset::Preset;
use rand_core::SeedableRng;
use std::collections::HashSet;
use std::path::PathBuf;

/// The preamble pasted before every preset shader.
pub const INCLUDE_FX: &str = include_str!("../../../assets/hlsl/include.fx");
/// The default warp shader body (`data/warp_ps.fx`, F20): the fallback.
pub const DEFAULT_WARP_PS: &str = include_str!("../../../assets/hlsl/warp_ps.fx");
/// The default composite shader body (`data/comp_ps.fx`, F20): the fallback.
pub const DEFAULT_COMP_PS: &str = include_str!("../../../assets/hlsl/comp_ps.fx");

/// Renderer configuration (A.3, plus `shader_cache_dir` per F35).
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSettings {
    /// Main (ping-pong) texture size, clamped to `1..=max_texture_dimension_2d`.
    pub tex_w: u32,
    pub tex_h: u32,
    /// Warp mesh size (C++ default 64 × 64).
    pub grid_x: u32,
    pub grid_y: u32,
    /// `m_nMaxPSVersion`: `None` disables preset shaders.
    pub max_ps: PsVersion,
    /// Anisotropic filtering for fixed-function draws (preset samplers
    /// never use it: the C++ hard-codes `bAniso = false`).
    pub anisotropic: bool,
    /// Bits per channel of the main textures: 16 → `Rgba16Float`,
    /// otherwise `Rgba8Unorm`.
    pub tex_bits: u8,
    /// Directory of the on-disk shader cache (supplied by md-core from
    /// md-platform's cache dir); `None` keeps the cache in memory only.
    pub shader_cache_dir: Option<PathBuf>,
}

impl RenderSettings {
    /// Settings for a `tex_w`×`tex_h` main texture and C++ defaults
    /// otherwise (grid 64×64, ps_3_0, 8 bits, no disk cache).
    pub fn new(tex_w: u32, tex_h: u32) -> RenderSettings {
        RenderSettings {
            tex_w,
            tex_h,
            grid_x: 64,
            grid_y: 64,
            max_ps: PsVersion::Ps3_0,
            anisotropic: false,
            tex_bits: 8,
            shader_cache_dir: None,
        }
    }

    /// The main texture format for `tex_bits`.
    pub fn format(&self) -> wgpu::TextureFormat {
        if self.tex_bits == 16 {
            wgpu::TextureFormat::Rgba16Float
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        }
    }
}

/// Renderer errors.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum RenderError {
    /// A preset shader failed to translate (or wgpu rejected it); the
    /// built-in default shader is used instead. Recorded in
    /// [`Renderer::last_errors`], not returned.
    #[error("{which:?} shader failed ({error}); using the default shader")]
    ShaderFallback {
        which: ShaderKind,
        error: TranslateError,
    },
    /// The built-in default shader itself failed (a translator bug).
    #[error("built-in default {which:?} shader failed: {error}")]
    DefaultShader {
        which: ShaderKind,
        error: TranslateError,
    },
}

/// md-preset's `PsVersion` as md-hlsl's (F23). A free function: the orphan
/// rule forbids `impl From<md_preset::PsVersion> for md_hlsl::PsVersion`
/// outside both crates.
pub fn hlsl_ps_version(v: md_preset::PsVersion) -> PsVersion {
    match v {
        md_preset::PsVersion::None => PsVersion::None,
        md_preset::PsVersion::Ps2_0 => PsVersion::Ps2_0,
        md_preset::PsVersion::Ps2_X => PsVersion::Ps2_X,
        md_preset::PsVersion::Ps3_0 => PsVersion::Ps3_0,
        md_preset::PsVersion::Ps4_0 => PsVersion::Ps4_0,
    }
}

/// Sizes of the six blur textures for a `w`×`h` main texture, exactly as
/// `AllocateMyDX9Stuff` (plugin.cpp:1987-2007@6f7cd5d): levels 0, 1 halve, then
/// every even level halves again (each at least 16), widths rounded to
/// `((w + 3) / 16) * 16`, heights to `((h + 3) / 4) * 4`. Textures 1, 3, 5
/// are what presets see as `blur1..3` (¼, ⅛, 1/16 of the main texture —
/// the Milestone 5 text's "½, ¼, ⅛" names textures 0, 2, 4).
pub fn blur_texture_sizes(w: u32, h: u32) -> [(u32, u32); 6] {
    let (mut w, mut h) = (w, h);
    let mut out = [(0, 0); 6];
    for (i, o) in out.iter_mut().enumerate() {
        if i % 2 == 0 || i < 2 {
            w = (w / 2).max(16);
            h = (h / 2).max(16);
        }
        *o = (((w + 3) / 16) * 16, h.div_ceil(4) * 4);
    }
    out
}

/// The MilkDrop renderer.
pub struct Renderer {
    pub(crate) gpu: Gpu,
    pub(crate) settings: RenderSettings,
    pub(crate) vs: [Texture2d; 2],
    /// Index of the ping-pong texture the warp pass draws into this frame
    /// (the C++'s `m_lpVS[1]`); the other one is its source (`m_lpVS[0]`).
    pub(crate) vs_target: usize,
    blur: Vec<Texture2d>,
    textures: TextureStore,
    pub(crate) pipelines: PipelineCache,
    shaders: ShaderCache,
    layouts: PresetLayouts,
    pub(crate) rng: rand_pcg::Pcg32,
    pub(crate) warp_buffers: Option<WarpBuffers>,
    last_errors: Vec<RenderError>,
    warned_textures: HashSet<String>,
    fallback_2d: Texture2d,
    fallback_3d: Texture3d,
}

impl Renderer {
    /// A renderer drawing with (clones of) `device` and `queue`. `seed`
    /// drives the noise textures and the random-texture picks.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut settings: RenderSettings,
        seed: u64,
    ) -> Result<Renderer, RenderError> {
        let gpu = Gpu::new(device, queue);
        let max = device.limits().max_texture_dimension_2d.max(1);
        settings.tex_w = settings.tex_w.clamp(1, max);
        settings.tex_h = settings.tex_h.clamp(1, max);
        let format = settings.format();
        let (w, h) = (settings.tex_w, settings.tex_h);
        let vs = [0, 1].map(|_| Texture2d::new_target(&gpu, w, h, format, true));
        let blur = blur_texture_sizes(w, h)
            .iter()
            .map(|&(bw, bh)| Texture2d::new_target(&gpu, bw.min(max), bh.min(max), format, false))
            .collect();
        // Unbound D3D9 stages sample opaque black (ledger Task 5.5 ruling).
        let black = [0, 0, 0, 255u8];
        Ok(Renderer {
            textures: TextureStore::new(&gpu, seed),
            pipelines: PipelineCache::new(),
            shaders: ShaderCache::new(settings.shader_cache_dir.clone()),
            layouts: PresetLayouts::new(device),
            rng: rand_pcg::Pcg32::seed_from_u64(seed ^ 0x6d64_7261_6e64_7478),
            last_errors: Vec::new(),
            warned_textures: HashSet::new(),
            fallback_2d: Texture2d::from_rgba8(&gpu, 1, 1, &black, false),
            fallback_3d: Texture3d::from_rgba8(&gpu, 1, 1, 1, &black, false),
            vs,
            vs_target: 0,
            warp_buffers: None,
            blur,
            settings,
            gpu,
        })
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// The settings in effect (texture size clamped).
    pub fn settings(&self) -> &RenderSettings {
        &self.settings
    }

    /// Ping-pong main texture `i` (0 or 1).
    pub fn vs(&self, i: usize) -> &Texture2d {
        &self.vs[i]
    }

    /// The ping-pong texture a pass drawing into `VS[target]` reads as
    /// `sampler_main`.
    pub fn other_vs(target: usize) -> usize {
        1 - (target & 1)
    }

    /// The ping-pong texture the warp pass draws into this frame (0 until
    /// [`Renderer::swap_vs`]); it reads [`Renderer::other_vs`] of it.
    /// The C++ draws `m_lpVS[0]` into `m_lpVS[1]` and swaps the pointers at
    /// the end of the frame (milkdropfs.cpp:983-990, 1134-1136); md-render
    /// keeps the textures in place (preset bind groups reference them by
    /// index) and swaps this index instead.
    pub fn warp_target(&self) -> usize {
        self.vs_target
    }

    /// End-of-frame ping-pong flip (milkdropfs.cpp:1134-1136): this frame's
    /// warp target becomes the next frame's warp source.
    pub fn swap_vs(&mut self) {
        self.vs_target = Renderer::other_vs(self.vs_target);
    }

    /// Blur texture `i` (0..6; presets read 1, 3, 5). Placeholders until
    /// the blur passes (Task 6.1) render them.
    pub fn blur_texture(&self, i: usize) -> &Texture2d {
        &self.blur[i]
    }

    pub fn textures(&self) -> &TextureStore {
        &self.textures
    }

    /// The texture store (set its directories here).
    pub fn textures_mut(&mut self) -> &mut TextureStore {
        &mut self.textures
    }

    pub fn pipelines_mut(&mut self) -> &mut PipelineCache {
        &mut self.pipelines
    }

    pub fn shader_cache(&self) -> &ShaderCache {
        &self.shaders
    }

    /// The shader fallbacks of the last [`Renderer::load_preset`].
    pub fn last_errors(&self) -> &[RenderError] {
        &self.last_errors
    }

    /// Compile `preset`'s warp and composite shaders (each where its
    /// `PSVERSION_*` is not 0 and `max_ps` is not `None`), resolve their
    /// textures and build their pipelines. A shader that fails falls back
    /// to the built-in default, recorded in [`Renderer::last_errors`]
    /// (cleared first). Errs only if a default shader fails.
    pub fn load_preset(&mut self, preset: &Preset) -> Result<PresetGpu, RenderError> {
        self.last_errors.clear();
        let shaders_on = self.settings.max_ps != PsVersion::None;
        let load = |r: &mut Renderer, kind, body: &str, v| {
            if shaders_on {
                r.load_shader(kind, body, hlsl_ps_version(v))
            } else {
                Ok(None)
            }
        };
        let warp = load(
            self,
            ShaderKind::Warp,
            &preset.warp_shader,
            preset.warp_ps_version(),
        );
        let comp = load(
            self,
            ShaderKind::Comp,
            &preset.comp_shader,
            preset.comp_ps_version(),
        );
        // m_nPresetsLoadedTotal++: protects this preset's textures.
        self.textures.preset_loaded();
        Ok(PresetGpu {
            warp: warp?,
            comp: comp?,
        })
    }

    fn load_shader(
        &mut self,
        kind: ShaderKind,
        body: &str,
        version: PsVersion,
    ) -> Result<Option<ShaderProgram>, RenderError> {
        if version == PsVersion::None {
            return Ok(None);
        }
        match self.build_program(kind, body, version, false) {
            Ok(p) => return Ok(Some(p)),
            Err(error) => {
                tracing::warn!(which = ?kind, %error, "preset shader failed; using the default");
                self.last_errors
                    .push(RenderError::ShaderFallback { which: kind, error });
            }
        }
        let default = match kind {
            ShaderKind::Warp => DEFAULT_WARP_PS,
            ShaderKind::Comp => DEFAULT_COMP_PS,
        };
        self.build_program(kind, default, PsVersion::Ps2_0, true)
            .map(Some)
            .map_err(|error| RenderError::DefaultShader { which: kind, error })
    }

    fn build_program(
        &mut self,
        kind: ShaderKind,
        body: &str,
        ps_version: PsVersion,
        fallback: bool,
    ) -> Result<ShaderProgram, TranslateError> {
        let input = ShaderInput {
            kind,
            body,
            preamble: INCLUDE_FX,
            ps_version,
        };
        let shader = self.shaders.get(&self.gpu, &input)?;
        let mut resolver = Resolver {
            gpu: &self.gpu,
            textures: &mut self.textures,
            rng: &mut self.rng,
            warned: &mut self.warned_textures,
            fallback_2d: &self.fallback_2d,
            fallback_3d: &self.fallback_3d,
            blur_sizes: blur_texture_sizes(self.settings.tex_w, self.settings.tex_h),
        };
        let (sources, texsize_values) = resolver.resolve(&shader.translated);
        let spec = ProgramSpec {
            kind,
            shader,
            sources,
            texsize_values,
            fallback,
        };
        let targets = ProgramTargets {
            vs: &self.vs,
            blur: &self.blur,
        };
        self.layouts
            .build(&self.gpu, &mut self.pipelines, spec, &targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_sizes_follow_allocate_my_dx9_stuff() {
        // The C++ comment's example: main 1024 -> 512, 256, 128, 128, 64, 64.
        let s = blur_texture_sizes(1024, 1024);
        assert_eq!(s.map(|(w, _)| w), [512, 256, 128, 128, 64, 64]);
        assert_eq!(s.map(|(_, h)| h), [512, 256, 128, 128, 64, 64]);
        // Rounding and the 16 minimum.
        let s = blur_texture_sizes(100, 30);
        assert_eq!(s[0], (48, 16)); // w 50 -> 48, h max(16, 15) = 16
        assert_eq!(s[5], (16, 16));
    }

    #[test]
    fn ps_versions_convert() {
        use md_preset::PsVersion as P;
        assert_eq!(hlsl_ps_version(P::None), PsVersion::None);
        assert_eq!(hlsl_ps_version(P::Ps2_0), PsVersion::Ps2_0);
        assert_eq!(hlsl_ps_version(P::Ps2_X), PsVersion::Ps2_X);
        assert_eq!(hlsl_ps_version(P::Ps3_0), PsVersion::Ps3_0);
        assert_eq!(hlsl_ps_version(P::Ps4_0), PsVersion::Ps4_0);
    }

    #[test]
    fn other_vs_is_the_other_ping_pong_texture() {
        assert_eq!(Renderer::other_vs(0), 1);
        assert_eq!(Renderer::other_vs(1), 0);
    }
}

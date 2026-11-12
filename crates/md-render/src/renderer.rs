//! The [`Renderer`] (A.3; created here in Task 5.5 per F36 — Task 5.7 adds
//! `begin_frame`, `present` ([`crate::frame`]), `resize` and the composite
//! pass ([`crate::comp`])): the ping-pong main textures, the blur textures,
//! the warp mesh, the [`TextureStore`], the fixed-function
//! [`PipelineCache`], the [`ShaderCache`] and preset loading.
//!
//! Sizes (F56, Review Focus #5): the window is clamped to
//! `1..=max_texture_dimension_2d` in [`Renderer::new`] and
//! [`Renderer::resize`]; the main texture size follows
//! [`main_texture_size`] (`AllocateMyDX9Stuff`, plugin.cpp:1844-1889@6f7cd5d)
//! and is clamped the same way, so no size ever panics.
//!
//! Preset shader compilation (F44): [`Renderer::load_preset`] compiles both
//! the warp and the composite shader in one call. md-core's preset switcher
//! (Milestone 7) reaches it through its `ShaderLoader` trait, implemented
//! for `Renderer` there and faked in its tests, and calls it at tick 2 of
//! its 8-tick schedule.

use crate::comp::{comp_grid, CompGrid, COMP_FORMAT};
use crate::dynbuf::DynamicVertexBuffer;
use crate::gpu::Gpu;
use crate::mesh::{mesh_aspect, WarpMesh};
use crate::noise::crand;
use crate::pipelines::PipelineCache;
use crate::preset_bind::Resolver;
use crate::preset_gpu::{PresetGpu, PresetLayouts, ProgramSpec, ProgramTargets, ShaderProgram};
use crate::shader_cache::ShaderCache;
use crate::tex_size::TexSize;
use crate::tex_size::{blur_texture_sizes, main_texture_size, main_textures};
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

/// Renderer configuration (A.3, plus `shader_cache_dir` per F35 and the
/// texture sizing options of Task 5.7).
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSettings {
    /// The initial window size; with [`TexSize::Fixed`] also the main
    /// (ping-pong) texture size. Clamped to `1..=max_texture_dimension_2d`.
    pub tex_w: u32,
    pub tex_h: u32,
    /// How the main texture size follows the window.
    pub tex_size: TexSize,
    /// `m_nCanvasStretch` in percent (0 = auto = 100: the C++'s slow-GPU
    /// grey list is not ported).
    pub canvas_stretch: u32,
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
    /// Settings for a `tex_w`×`tex_h` window and C++ defaults otherwise
    /// (auto texture size, no canvas stretch, grid 64×64, ps_3_0, 8 bits,
    /// no disk cache).
    pub fn new(tex_w: u32, tex_h: u32) -> RenderSettings {
        RenderSettings {
            tex_w,
            tex_h,
            tex_size: TexSize::Auto,
            canvas_stretch: 100,
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

/// The MilkDrop renderer.
pub struct Renderer {
    pub(crate) gpu: Gpu,
    pub(crate) settings: RenderSettings,
    pub(crate) vs: [Texture2d; 2],
    /// Index of the ping-pong texture the warp pass draws into this frame
    /// (the C++'s `m_lpVS[1]`); the other one is its source (`m_lpVS[0]`).
    pub(crate) vs_target: usize,
    pub(crate) blur: Vec<Texture2d>,
    textures: TextureStore,
    pub(crate) pipelines: PipelineCache,
    shaders: ShaderCache,
    layouts: PresetLayouts,
    pub(crate) rng: rand_pcg::Pcg32,
    pub(crate) warp_buffers: Option<WarpBuffers>,
    /// The window (composite target) size, `GetWidth()` × `GetHeight()`.
    pub(crate) window: (u32, u32),
    /// Bumped whenever the main/blur textures are replaced; preset
    /// programs bound to an older generation are rebound before drawing.
    pub(crate) generation: u64,
    pub(crate) mesh: WarpMesh,
    /// `m_nFramesSinceResize`: 0 clears the warp source on the next frame.
    pub(crate) frames_since_resize: u32,
    /// `m_fRandStart[4]` (plugin.cpp:6623-6626), for the hue shader.
    pub(crate) rand_start: [f32; 4],
    /// The composite's intermediate target (`Rgba8Unorm`, window size).
    pub(crate) comp_tex: Texture2d,
    pub(crate) comp_grid: CompGrid,
    pub(crate) comp_buffers: Option<WarpBuffers>,
    pub(crate) dynbuf: DynamicVertexBuffer,
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
        let window = (settings.tex_w.clamp(1, max), settings.tex_h.clamp(1, max));
        let (w, h) = main_texture_size(
            window,
            settings.tex_size,
            window,
            settings.canvas_stretch,
            max,
        );
        (settings.tex_w, settings.tex_h) = (w, h);
        let format = settings.format();
        let (vs, blur) = main_textures(&gpu, w, h, format);
        let mut rng = rand_pcg::Pcg32::seed_from_u64(seed ^ 0x6d64_7261_6e64_7478);
        // plugin.cpp:6623-6626: `(rand() % N) * 0.01`.
        let rand_start = [64841, 53751, 42661, 31571].map(|n| (crand(&mut rng) % n) as f32 * 0.01);
        let (ax, ay) = mesh_aspect(w, h);
        // Unbound D3D9 stages sample opaque black (ledger Task 5.5 ruling).
        let black = [0, 0, 0, 255u8];
        Ok(Renderer {
            window,
            generation: 0,
            mesh: WarpMesh::new(settings.grid_x, settings.grid_y, ax, ay),
            frames_since_resize: 0,
            rand_start,
            comp_tex: Texture2d::new_target(&gpu, window.0, window.1, COMP_FORMAT, false),
            comp_grid: comp_grid(window, (w, h)),
            comp_buffers: None,
            dynbuf: DynamicVertexBuffer::new(&gpu, 1024),
            textures: TextureStore::new(&gpu, seed),
            pipelines: PipelineCache::new(),
            shaders: ShaderCache::new(settings.shader_cache_dir.clone()),
            layouts: PresetLayouts::new(device),
            rng,
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

    /// The settings in effect (`tex_w`/`tex_h` hold the main texture size
    /// actually allocated).
    pub fn settings(&self) -> &RenderSettings {
        &self.settings
    }

    /// The window (composite target) size after clamping.
    pub fn window(&self) -> (u32, u32) {
        self.window
    }

    /// The warp mesh (grid from the settings, aspect from the main texture
    /// size). md-core computes its UVs every frame through
    /// [`Renderer::mesh_mut`] ([`WarpMesh::compute_uvs`]: the current
    /// preset, then the old one with the blend pattern while blending)
    /// before [`Renderer::begin_frame`]; the warp pass draws it and the
    /// composite reads its blend alphas. A resize that changes the texture
    /// size replaces it (identity UVs).
    pub fn mesh(&self) -> &WarpMesh {
        &self.mesh
    }

    pub fn mesh_mut(&mut self) -> &mut WarpMesh {
        &mut self.mesh
    }

    /// `m_fRandStart` (the hue shader's random phases).
    pub fn rand_start(&self) -> [f32; 4] {
        self.rand_start
    }

    /// The composite's intermediate texture (`Rgba8Unorm`, window size):
    /// after [`Renderer::present`] it holds the frame copied to the target.
    pub fn comp_texture(&self) -> &Texture2d {
        &self.comp_tex
    }

    /// Clear the warp source at the start of the next frame, as after a
    /// resize (`m_nFramesSinceResize = 0`).
    pub fn request_clear(&mut self) {
        self.frames_since_resize = 0;
    }

    /// Resize to a `w`×`h` window (`OnResizeGraphicsWindow` →
    /// `AllocateMyDX9Stuff`). Both are clamped to
    /// `1..=max_texture_dimension_2d` (F56). The main texture size follows
    /// [`main_texture_size`]; when it changes the ping-pong, blur textures
    /// and warp mesh are replaced (preset programs are rebound lazily, see
    /// [`crate::ShaderProgram`]). The composite grid and target follow the
    /// window, and the next frame clears the warp source as the C++ does
    /// after every resize (`m_nFramesSinceResize = 0`).
    pub fn resize(&mut self, w: u32, h: u32) {
        let max = self.gpu.device().limits().max_texture_dimension_2d.max(1);
        let window = (w.clamp(1, max), h.clamp(1, max));
        let s = &self.settings;
        let current = (s.tex_w, s.tex_h);
        let tex = main_texture_size(window, s.tex_size, current, s.canvas_stretch, max);
        let format = s.format();
        if tex != current {
            let (vs, blur) = main_textures(&self.gpu, tex.0, tex.1, format);
            (self.vs, self.blur) = (vs, blur);
            (self.settings.tex_w, self.settings.tex_h) = tex;
            self.generation += 1;
            let (ax, ay) = mesh_aspect(tex.0, tex.1);
            self.mesh = WarpMesh::new(self.settings.grid_x, self.settings.grid_y, ax, ay);
        }
        if window != self.window || tex != current {
            self.window = window;
            self.comp_tex =
                Texture2d::new_target(&self.gpu, window.0, window.1, COMP_FORMAT, false);
            self.comp_grid = comp_grid(window, tex);
            // Bind groups of the dropped textures.
            self.pipelines.clear_bind_groups();
        }
        self.frames_since_resize = 0;
    }

    /// Rebind `prog` if it was bound to replaced textures.
    pub(crate) fn rebind(&mut self, prog: &crate::ShaderProgram) {
        if prog.generation() == self.generation {
            return;
        }
        let targets = ProgramTargets {
            vs: &self.vs,
            blur: &self.blur,
            generation: self.generation,
        };
        prog.rebind(&self.gpu, &mut self.pipelines, &targets);
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
            generation: self.generation,
        };
        self.layouts
            .build(&self.gpu, &mut self.pipelines, spec, &targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

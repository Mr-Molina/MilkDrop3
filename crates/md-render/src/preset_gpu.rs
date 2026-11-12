//! Compiled preset shaders: [`ShaderProgram`] (one warp or comp pixel
//! shader with its pipelines, uniform buffers and bound textures) and
//! [`PresetGpu`] (both of a preset), built by [`crate::Renderer::load_preset`].
//!
//! Layout (Milestone 4 contract, `assets/shaders/README.md`): group 0 holds
//! the preset's `U: MdUniforms` at binding 0 and `T: TexSizes` at 1 for the
//! fragment stage and the built-in vertex shader's `VsParams` at 2; group 1
//! holds `tex_i` / `samp_i` at bindings `2i` / `2i+1` for every bound
//! sampler. The vertex stage is the built-in `warp_vs` / `comp_vs` with the
//! single [`Vertex`] layout; topology triangle list, culling off.
//!
//! Blending (milkdropfs.cpp:1008-1045@6f7cd5d warp, 1072-1115 comp, and the blend
//! states at 2124-2138 / 4397-4410, same file): a shader draws with blending off when
//! no blend is running and on pass 0 of a blend; pass 1 uses
//! `SrcAlpha/InvSrcAlpha` ([`BlendMode::Alpha`]); the composite's "funky
//! reversal" (old preset with a comp shader, new one without) draws the old
//! comp shader with `InvSrcAlpha/SrcAlpha` ([`BlendMode::InvAlpha`]). Every
//! program has all three pipelines. The colour target is the main
//! (ping-pong) texture format.
//!
//! `main` binds one of the two ping-pong textures, so group 1 exists twice:
//! [`ShaderProgram::bind`] takes the index of the texture to read — when
//! drawing into `VS[t]` that is [`crate::Renderer::other_vs`]`(t)`.
//!
//! Resize (Task 5.7, ledger Task 5.5 note): [`crate::Renderer::resize`]
//! replaces the main and blur textures, so group 1 must be rebuilt (the
//! slot sources are kept). [`PresetGpu`]s belong to the caller, so the
//! renderer cannot reach them at resize time; instead every program
//! remembers the renderer's texture generation it was bound for, and the
//! frame loop rebinds a stale program before drawing it (group 1 lives
//! behind a `Mutex` so this works through the `&PresetGpu` in
//! [`crate::FrameInput`]). Rebinding also rewrites the `texsize_blurN`
//! values and the warp program's `VsParams`.

use crate::blend::BlendMode;
use crate::gpu::{block_on, Gpu};
use crate::pipelines::PipelineCache;
use crate::preset_bind::{sampler_kind, SlotSource};
use crate::shader_cache::CachedShader;
use crate::texture::Texture2d;
use crate::uniforms::{texsizes_bytes, MdUniforms, VsParams};
use crate::vertex::Vertex;
use md_hlsl::layout::{BUILTIN_VS_PARAMS_BINDING, BUILTIN_VS_PARAMS_SIZE};
use md_hlsl::{SamplerBinding, ShaderKind, TranslateError};
use std::sync::{Arc, Mutex, PoisonError};
use wgpu::util::DeviceExt;

/// The compiled warp and composite shaders of one preset; `None` where the
/// preset's `PSVERSION_*` is 0 (fixed-function path) or preset shaders are
/// disabled (`RenderSettings::max_ps == None`).
#[derive(Debug)]
pub struct PresetGpu {
    pub warp: Option<ShaderProgram>,
    pub comp: Option<ShaderProgram>,
}

impl PresetGpu {
    /// The highest blur level (0..=3) either shader samples: the blur
    /// passes beyond it are skipped (F39).
    pub fn highest_blur(&self) -> u8 {
        [&self.warp, &self.comp]
            .into_iter()
            .flatten()
            .map(|p| p.highest_blur)
            .max()
            .unwrap_or(0)
    }
}

/// One compiled preset pixel shader, ready to draw.
#[derive(Debug)]
pub struct ShaderProgram {
    pub kind: ShaderKind,
    /// Blending off (no blend, or pass 0 of a blend).
    pub pipeline: wgpu::RenderPipeline,
    /// [`BlendMode::Alpha`] (blend pass 1).
    pub blend_pipeline: wgpu::RenderPipeline,
    /// [`BlendMode::InvAlpha`] (the composite's flipped-alpha pass).
    pub flip_pipeline: wgpu::RenderPipeline,
    /// `U`, `T` and `VsParams`.
    pub bind_group_0: wgpu::BindGroup,
    /// Textures and samplers, with `main` bound to ping-pong texture 0 or 1
    /// (see [`ShaderProgram::bind_group_1`]).
    group1: Mutex<Group1>,
    bgl1: wgpu::BindGroupLayout,
    /// The `MdUniforms` buffer (write with [`ShaderProgram::write_uniforms`]).
    pub uniforms: wgpu::Buffer,
    /// The `VsParams` buffer, initially for the main texture size.
    pub vs_params: wgpu::Buffer,
    /// The `TexSizes` buffer holding [`ShaderProgram::texsize_values`].
    pub texsizes: wgpu::Buffer,
    /// Full `texsize_*` names, in `TexSizes` order.
    pub texsize_names: Vec<String>,
    /// `(w, h, 1/w, 1/h)` per `texsize_names` entry (zero when unresolved),
    /// as resolved at load time (a resize rewrites the buffer's blur sizes).
    pub texsize_values: Vec<[f32; 4]>,
    /// Bound samplers in slot order.
    pub samplers: Vec<SamplerBinding>,
    /// What each slot binds (kept so bind groups can be rebuilt).
    pub sources: Vec<SlotSource>,
    /// Highest blur level the shader samples (0..=3).
    pub highest_blur: u8,
    fallback: bool,
    shader: Arc<CachedShader>,
}

/// Group 1 of a program and the renderer texture generation it binds.
#[derive(Debug)]
struct Group1 {
    generation: u64,
    groups: [wgpu::BindGroup; 2],
}

impl ShaderProgram {
    /// Whether this is the built-in default shader standing in for a
    /// preset shader that failed to compile.
    pub fn is_fallback(&self) -> bool {
        self.fallback
    }

    /// The translated WGSL and its module (shared with the shader cache).
    pub fn shader(&self) -> &CachedShader {
        &self.shader
    }

    /// The pipeline for `blend`: `Off`, `Alpha` or `InvAlpha`.
    pub fn pipeline_for(&self, blend: BlendMode) -> Option<&wgpu::RenderPipeline> {
        match blend {
            BlendMode::Off => Some(&self.pipeline),
            BlendMode::Alpha => Some(&self.blend_pipeline),
            BlendMode::InvAlpha => Some(&self.flip_pipeline),
            _ => None,
        }
    }

    /// Group 1 with `sampler_main` reading ping-pong texture `main` (0 or
    /// 1), `None` for another index.
    pub fn bind_group_1(&self, main: usize) -> Option<wgpu::BindGroup> {
        let g = self.group1.lock().unwrap_or_else(PoisonError::into_inner);
        g.groups.get(main).cloned()
    }

    /// The renderer texture generation group 1 binds.
    pub(crate) fn generation(&self) -> u64 {
        self.group1
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .generation
    }

    /// Set the pipeline for `blend` and both bind groups on `pass`, with
    /// `sampler_main` reading ping-pong texture `main` (0 or 1). `false`
    /// (nothing set) for another blend mode or index. The caller sets the
    /// vertex buffer and draws.
    pub fn bind(&self, pass: &mut wgpu::RenderPass<'_>, main: usize, blend: BlendMode) -> bool {
        let (Some(pipeline), Some(group1)) = (self.pipeline_for(blend), self.bind_group_1(main))
        else {
            return false;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind_group_0, &[]);
        pass.set_bind_group(1, &group1, &[]);
        true
    }

    /// Rebuild group 1 for the renderer's current textures
    /// (`targets.generation`) if it binds older ones, keeping the slot sources; rewrite the
    /// `texsize_blurN` values for the new blur sizes and, for a warp
    /// program, `VsParams` for the new main texture size.
    pub(crate) fn rebind(&self, gpu: &Gpu, cache: &mut PipelineCache, targets: &ProgramTargets) {
        let generation = targets.generation;
        let mut g = self.group1.lock().unwrap_or_else(PoisonError::into_inner);
        if g.generation == generation {
            return;
        }
        g.groups = [0, 1].map(|m| {
            group1(
                gpu,
                cache,
                &self.bgl1,
                &self.samplers,
                &self.sources,
                targets,
                m,
            )
        });
        g.generation = generation;
        let values: Vec<[f32; 4]> = self
            .texsize_names
            .iter()
            .zip(&self.texsize_values)
            .map(|(name, v)| {
                let root = md_hlsl::texsize_root(name).unwrap_or(name);
                match crate::preset_bind::blur_level(root) {
                    Some(l) => {
                        let t = &targets.blur[usize::from(l) * 2 - 1];
                        let (w, h) = (t.w as f32, t.h as f32);
                        [w, h, 1.0 / w, 1.0 / h]
                    }
                    None => *v,
                }
            })
            .collect();
        gpu.queue()
            .write_buffer(&self.texsizes, 0, &texsizes_bytes(&values));
        if self.kind == ShaderKind::Warp {
            let [w, h] = [targets.vs[0].w, targets.vs[0].h];
            self.write_vs_params(gpu, VsParams::for_target(w, h));
        }
    }

    /// Upload this frame's `MdUniforms`.
    pub fn write_uniforms(&self, gpu: &Gpu, u: &MdUniforms) {
        gpu.queue().write_buffer(&self.uniforms, 0, &u.to_bytes());
    }

    /// Upload `VsParams` (the target's texel size; the composite draws to
    /// the window, not the main texture).
    pub fn write_vs_params(&self, gpu: &Gpu, p: VsParams) {
        gpu.queue().write_buffer(&self.vs_params, 0, &p.to_bytes());
    }
}

/// Group-0 layout and built-in vertex shaders shared by every program.
#[derive(Debug)]
pub(crate) struct PresetLayouts {
    bgl0: wgpu::BindGroupLayout,
    warp_vs: wgpu::ShaderModule,
    comp_vs: wgpu::ShaderModule,
}

fn uniform_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    min: Option<u64>,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: min.and_then(wgpu::BufferSize::new),
        },
        count: None,
    }
}

impl PresetLayouts {
    pub fn new(device: &wgpu::Device) -> PresetLayouts {
        let fs = wgpu::ShaderStages::FRAGMENT;
        let bgl0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("md-render preset group 0"),
            entries: &[
                uniform_entry(0, fs, Some(MdUniforms::SIZE)),
                uniform_entry(1, fs, None),
                uniform_entry(
                    BUILTIN_VS_PARAMS_BINDING,
                    wgpu::ShaderStages::VERTEX,
                    Some(BUILTIN_VS_PARAMS_SIZE),
                ),
            ],
        });
        let module = |label, src: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        PresetLayouts {
            bgl0,
            warp_vs: module(
                "md-render warp_vs",
                include_str!("../../../assets/shaders/warp_vs.wgsl"),
            ),
            comp_vs: module(
                "md-render comp_vs",
                include_str!("../../../assets/shaders/comp_vs.wgsl"),
            ),
        }
    }
}

/// What a program draws into and reads from.
pub(crate) struct ProgramTargets<'a> {
    pub vs: &'a [Texture2d; 2],
    pub blur: &'a [Texture2d],
    /// The renderer's texture generation (bumped when they are replaced).
    pub generation: u64,
}

/// One compiled shader plus its resolved slots, before GPU objects exist.
pub(crate) struct ProgramSpec {
    pub kind: ShaderKind,
    pub shader: Arc<CachedShader>,
    pub sources: Vec<SlotSource>,
    pub texsize_values: Vec<[f32; 4]>,
    pub fallback: bool,
}

fn group1_layout(device: &wgpu::Device, samplers: &[SamplerBinding]) -> wgpu::BindGroupLayout {
    let fs = wgpu::ShaderStages::FRAGMENT;
    let entries: Vec<_> = samplers
        .iter()
        .flat_map(|b| {
            let view_dimension = if b.dims == 3 {
                wgpu::TextureViewDimension::D3
            } else {
                wgpu::TextureViewDimension::D2
            };
            [
                wgpu::BindGroupLayoutEntry {
                    binding: 2 * b.slot,
                    visibility: fs,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2 * b.slot + 1,
                    visibility: fs,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ]
        })
        .collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("md-render preset group 1"),
        entries: &entries,
    })
}

/// Build group 1 with `main` reading `targets.vs[main]`.
fn group1(
    gpu: &Gpu,
    cache: &mut PipelineCache,
    layout: &wgpu::BindGroupLayout,
    samplers: &[SamplerBinding],
    sources: &[SlotSource],
    targets: &ProgramTargets,
    main: usize,
) -> wgpu::BindGroup {
    let parts: Vec<(wgpu::TextureView, wgpu::Sampler)> = samplers
        .iter()
        .zip(sources)
        .map(|(b, src)| {
            let view = match src {
                SlotSource::Main => targets.vs[main].view.clone(),
                SlotSource::Blur(l) => targets.blur[usize::from(*l) * 2 - 1].view.clone(),
                SlotSource::Tex2d(t) => t.view.clone(),
                SlotSource::Tex3d(t) => t.view.clone(),
            };
            (view, cache.sampler(gpu, sampler_kind(b)).clone())
        })
        .collect();
    let entries: Vec<_> = samplers
        .iter()
        .zip(&parts)
        .flat_map(|(b, (view, sampler))| {
            [
                wgpu::BindGroupEntry {
                    binding: 2 * b.slot,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2 * b.slot + 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ]
        })
        .collect();
    gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("md-render preset group 1"),
        layout,
        entries: &entries,
    })
}

impl PresetLayouts {
    /// Create the buffers, bind groups and the three pipelines of `spec`.
    /// A wgpu validation error (a vertex/fragment interface mismatch would
    /// be a translator bug) is returned as a [`TranslateError`] at line 0
    /// so the caller can fall back.
    pub fn build(
        &self,
        gpu: &Gpu,
        cache: &mut PipelineCache,
        spec: ProgramSpec,
        targets: &ProgramTargets,
    ) -> Result<ShaderProgram, TranslateError> {
        let device = gpu.device();
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let t = &spec.shader.translated;
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("md-render MdUniforms"),
            size: MdUniforms::SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let [w, h] = [targets.vs[0].w, targets.vs[0].h];
        let vs_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("md-render preset VsParams"),
            contents: &VsParams::for_target(w, h).to_bytes(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let texsizes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("md-render TexSizes"),
            contents: &texsizes_bytes(&spec.texsize_values),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("md-render preset group 0"),
            layout: &self.bgl0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: texsizes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: BUILTIN_VS_PARAMS_BINDING,
                    resource: vs_params.as_entire_binding(),
                },
            ],
        });
        let bgl1 = group1_layout(device, &t.samplers);
        let groups =
            [0, 1].map(|m| group1(gpu, cache, &bgl1, &t.samplers, &spec.sources, targets, m));
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("md-render preset"),
            bind_group_layouts: &[Some(&self.bgl0), Some(&bgl1)],
            immediate_size: 0,
        });
        let vs = match spec.kind {
            ShaderKind::Warp => &self.warp_vs,
            ShaderKind::Comp => &self.comp_vs,
        };
        let pipeline = |blend: BlendMode| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("md-render preset"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: vs,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(Vertex::LAYOUT)],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &spec.shader.module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: targets.vs[0].format,
                        blend: blend.state(),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let (off, alpha, flip) = (
            pipeline(BlendMode::Off),
            pipeline(BlendMode::Alpha),
            pipeline(BlendMode::InvAlpha),
        );
        if let Some(e) = block_on(scope.pop()) {
            return Err(TranslateError::new(
                0,
                0,
                format!("wgpu rejected the preset shader pipeline: {e}"),
            ));
        }
        Ok(ShaderProgram {
            kind: spec.kind,
            pipeline: off,
            blend_pipeline: alpha,
            flip_pipeline: flip,
            bind_group_0,
            group1: Mutex::new(Group1 {
                generation: targets.generation,
                groups,
            }),
            bgl1,
            uniforms,
            vs_params,
            texsizes,
            texsize_names: t.texsize_names.clone(),
            texsize_values: spec.texsize_values.clone(),
            samplers: t.samplers.clone(),
            highest_blur: t.highest_blur,
            sources: spec.sources.clone(),
            fallback: spec.fallback,
            shader: spec.shader.clone(),
        })
    }
}

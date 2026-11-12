//! The fixed-function pipeline cache and the batch that records draws.
//!
//! Every non-shader draw of the C++ is one [`PipelineKey`]: a texture-stage
//! [`FixedMode`] (one built-in WGSL file each), a D3D9 [`BlendMode`], a
//! [`Topo`] and the target format. Pipelines are built lazily and cached;
//! fans and points share the triangle-list pipeline after their CPU
//! conversion ([`crate::fixed`]).
//!
//! Layout (assets/shaders/README.md, "Bindings"): group 0 holds `VsParams`
//! at binding 2 for the vertex stage in **every** pipeline (texel size of
//! the target, `D3D9_PIXEL_SHIFT`); textured modes add group 1 with the
//! texture at binding 0 and the sampler at binding 1. Culling off, no depth
//! (`D3DRS_CULLMODE D3DCULL_NONE`, support.cpp:75).

use crate::blend::BlendMode;
use crate::dynbuf::DynamicVertexBuffer;
use crate::fixed::{fan_to_list, points_to_quads, FixedMode, SamplerKind, Topo};
use crate::gpu::Gpu;
use crate::texture::Texture2d;
use crate::uniforms::VsParams;
use crate::vertex::Vertex;
use md_hlsl::layout::{BUILTIN_VS_PARAMS_BINDING, BUILTIN_VS_PARAMS_SIZE};
use std::borrow::Cow;
use std::collections::HashMap;

/// Identifies one fixed-function pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    pub mode: FixedMode,
    pub blend: BlendMode,
    pub topo: Topo,
    pub target_format: wgpu::TextureFormat,
}

impl PipelineKey {
    pub fn new(
        mode: FixedMode,
        blend: BlendMode,
        topo: Topo,
        target_format: wgpu::TextureFormat,
    ) -> PipelineKey {
        PipelineKey {
            mode,
            blend,
            topo,
            target_format,
        }
    }

    /// The key of the pipeline actually used (fans and points draw with the
    /// triangle-list pipeline).
    pub fn canonical(self) -> PipelineKey {
        PipelineKey {
            topo: self.topo.canonical(),
            ..self
        }
    }
}

/// Bind groups (texture × sampler) kept before the cache is flushed; stale
/// entries (dropped textures) are only released by a flush.
const MAX_TEXTURE_BIND_GROUPS: usize = 256;
/// `VsParams` bind groups (one per target size) kept before a flush.
const MAX_VS_BIND_GROUPS: usize = 16;

/// Built-in fixed-function pipelines, samplers and bind groups, created on
/// first use. Holds no device: every method takes the [`Gpu`]; use one
/// cache per device.
#[derive(Debug, Default)]
pub struct PipelineCache {
    layouts: Option<Layouts>,
    shaders: [Option<wgpu::ShaderModule>; 4],
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    samplers: HashMap<SamplerKind, wgpu::Sampler>,
    vs_groups: HashMap<(u32, u32), wgpu::BindGroup>,
    tex_groups: HashMap<(wgpu::TextureView, SamplerKind), wgpu::BindGroup>,
}

#[derive(Debug)]
struct Layouts {
    vs_bgl: wgpu::BindGroupLayout,
    tex_bgl: wgpu::BindGroupLayout,
    color: wgpu::PipelineLayout,
    textured: wgpu::PipelineLayout,
}

impl Layouts {
    fn new(device: &wgpu::Device) -> Layouts {
        let vs_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("md-render VsParams"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: BUILTIN_VS_PARAMS_BINDING,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(BUILTIN_VS_PARAMS_SIZE),
                },
                count: None,
            }],
        });
        let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("md-render fixed texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = |label, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let color = layout("md-render fixed color", &[Some(&vs_bgl)]);
        let textured = layout("md-render fixed textured", &[Some(&vs_bgl), Some(&tex_bgl)]);
        Layouts {
            vs_bgl,
            tex_bgl,
            color,
            textured,
        }
    }
}

impl PipelineCache {
    /// An empty cache.
    pub fn new() -> PipelineCache {
        PipelineCache::default()
    }

    fn layouts(&mut self, gpu: &Gpu) -> &Layouts {
        self.layouts
            .get_or_insert_with(|| Layouts::new(gpu.device()))
    }

    /// The pipeline for `key` (its [`canonical`](PipelineKey::canonical)
    /// form), built on first request.
    pub fn get(&mut self, gpu: &Gpu, key: PipelineKey) -> &wgpu::RenderPipeline {
        let key = key.canonical();
        if !self.pipelines.contains_key(&key) {
            let pipeline = self.build(gpu, key);
            self.pipelines.insert(key, pipeline);
        }
        &self.pipelines[&key]
    }

    fn build(&mut self, gpu: &Gpu, key: PipelineKey) -> wgpu::RenderPipeline {
        let device = gpu.device();
        let shader = self.shaders[key.mode.index()]
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("md-render fixed"),
                    source: wgpu::ShaderSource::Wgsl(key.mode.wgsl().into()),
                })
            })
            .clone();
        let layouts = self.layouts(gpu);
        let layout = if key.mode.textured() {
            &layouts.textured
        } else {
            &layouts.color
        };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("md-render fixed"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::LAYOUT)],
            },
            primitive: wgpu::PrimitiveState {
                topology: key.topo.primitive(),
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: key.target_format,
                    blend: key.blend.state(),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Number of pipelines built so far.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Whether no pipeline has been built yet.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    /// The sampler for `kind` (one per kind).
    pub fn sampler(&mut self, gpu: &Gpu, kind: SamplerKind) -> &wgpu::Sampler {
        self.samplers
            .entry(kind)
            .or_insert_with(|| gpu.device().create_sampler(&kind.descriptor()))
    }

    /// The group-0 bind group with `VsParams::for_target(w, h)`, cached per
    /// target size (its contents depend on nothing else).
    pub fn vs_bind_group(&mut self, gpu: &Gpu, w: u32, h: u32) -> wgpu::BindGroup {
        if let Some(g) = self.vs_groups.get(&(w, h)) {
            return g.clone();
        }
        if self.vs_groups.len() >= MAX_VS_BIND_GROUPS {
            self.vs_groups.clear();
        }
        let device = gpu.device();
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("md-render VsParams"),
            size: BUILTIN_VS_PARAMS_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue()
            .write_buffer(&buf, 0, &VsParams::for_target(w, h).to_bytes());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("md-render VsParams"),
            layout: &self.layouts(gpu).vs_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: BUILTIN_VS_PARAMS_BINDING,
                resource: buf.as_entire_binding(),
            }],
        });
        self.vs_groups.insert((w, h), group.clone());
        group
    }

    /// The group-1 bind group sampling `view` with `kind`, cached per
    /// (view, kind). The cache is flushed when it reaches
    /// `MAX_TEXTURE_BIND_GROUPS` entries or by [`clear_bind_groups`]
    /// (call it after dropping many textures, e.g. on resize).
    ///
    /// [`clear_bind_groups`]: Self::clear_bind_groups
    pub fn texture_bind_group(
        &mut self,
        gpu: &Gpu,
        view: &wgpu::TextureView,
        kind: SamplerKind,
    ) -> wgpu::BindGroup {
        let key = (view.clone(), kind);
        if let Some(g) = self.tex_groups.get(&key) {
            return g.clone();
        }
        if self.tex_groups.len() >= MAX_TEXTURE_BIND_GROUPS {
            self.tex_groups.clear();
        }
        let sampler = self.sampler(gpu, kind).clone();
        let group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("md-render fixed texture"),
            layout: &self.layouts(gpu).tex_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        self.tex_groups.insert(key, group.clone());
        group
    }

    /// Drop all cached bind groups (they keep their textures alive).
    pub fn clear_bind_groups(&mut self) {
        self.tex_groups.clear();
        self.vs_groups.clear();
    }
}

/// Records fixed-function draws into one render pass, uploading vertices
/// through a [`DynamicVertexBuffer`] (call its `begin_frame` once per frame
/// as usual; a batch never resets it).
pub struct DrawBatch<'a> {
    gpu: &'a Gpu,
    cache: &'a mut PipelineCache,
    vbuf: &'a mut DynamicVertexBuffer,
    pass: wgpu::RenderPass<'a>,
    target: (u32, u32),
    point_size: f32,
}

impl<'a> DrawBatch<'a> {
    /// A batch drawing into `pass`, whose colour attachment is a
    /// `target_w`×`target_h` texture (for `VsParams` and point sizes).
    pub fn new(
        gpu: &'a Gpu,
        cache: &'a mut PipelineCache,
        vbuf: &'a mut DynamicVertexBuffer,
        mut pass: wgpu::RenderPass<'a>,
        target_w: u32,
        target_h: u32,
    ) -> DrawBatch<'a> {
        let (w, h) = (target_w.max(1), target_h.max(1));
        let group = cache.vs_bind_group(gpu, w, h);
        pass.set_bind_group(0, &group, &[]);
        DrawBatch {
            gpu,
            cache,
            vbuf,
            pass,
            target: (w, h),
            point_size: 1.0,
        }
    }

    /// Begin a render pass on mip 0 of `target` with `load` and wrap it.
    pub fn begin(
        gpu: &'a Gpu,
        cache: &'a mut PipelineCache,
        vbuf: &'a mut DynamicVertexBuffer,
        encoder: &'a mut wgpu::CommandEncoder,
        target: &Texture2d,
        load: wgpu::LoadOp<wgpu::Color>,
    ) -> DrawBatch<'a> {
        let view = if target.mips > 1 {
            target.level_view(0)
        } else {
            target.view.clone()
        };
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("md-render fixed batch"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        DrawBatch::new(gpu, cache, vbuf, pass, target.w, target.h)
    }

    /// `D3DRS_POINTSIZE` in target pixels for later [`Topo::Points`] draws
    /// (default 1.0, the D3D9 default; values below 1 act as 1).
    pub fn set_point_size(&mut self, px: f32) {
        self.point_size = px;
    }

    /// The wrapped render pass, for callers mixing in their own draws (they
    /// must restore bind group 0 if they change it).
    pub fn pass(&mut self) -> &mut wgpu::RenderPass<'a> {
        &mut self.pass
    }

    /// Draw `verts` as `key.topo` with `key`'s pipeline. Fans are converted
    /// to lists and points to quads of the current point size first.
    /// `texture` is required by textured modes and ignored by
    /// [`FixedMode::Color`]; `sampler` applies to it.
    ///
    /// # Panics
    /// If `key.mode` is textured and `texture` is `None`.
    pub fn draw(
        &mut self,
        key: PipelineKey,
        verts: &[Vertex],
        texture: Option<&Texture2d>,
        sampler: SamplerKind,
    ) {
        let verts: Cow<[Vertex]> = match key.topo {
            Topo::TriFan => Cow::Owned(fan_to_list(verts)),
            Topo::Points => Cow::Owned(points_to_quads(verts, self.point_size, self.target)),
            _ => Cow::Borrowed(verts),
        };
        if verts.is_empty() {
            return;
        }
        let tex_group = key.mode.textured().then(|| {
            let t = texture.unwrap_or_else(|| panic!("{:?} needs a texture", key.mode));
            self.cache.texture_bind_group(self.gpu, &t.view, sampler)
        });
        let pipeline = self.cache.get(self.gpu, key).clone();
        self.pass.set_pipeline(&pipeline);
        if let Some(g) = &tex_group {
            self.pass.set_bind_group(1, g, &[]);
        }
        let slice = self.vbuf.write(&verts);
        self.pass.set_vertex_buffer(0, slice);
        self.pass.draw(0..verts.len() as u32, 0..1);
    }

    /// End the render pass.
    pub fn finish(self) {}
}

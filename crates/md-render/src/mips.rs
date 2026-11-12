//! GPU mip-chain generation with a built-in blit pipeline.
//!
//! The C++ relies on `D3DUSAGE_AUTOGENMIPMAP`; here each level `i` is drawn
//! from level `i - 1` with one fullscreen triangle sampling bilinearly at
//! the destination texel centre — an exact 2×2 box filter for even sizes.
//! The WGSL is tiny and private to this pass, so it is inline
//! ([`MIP_BLIT_WGSL`]) rather than in `assets/shaders/`; a unit test
//! validates it with naga, and wgpu validates it again at pipeline
//! creation. One pipeline per target format is cached in the [`Gpu`].

use crate::gpu::Gpu;
use crate::texture::Texture2d;
use std::collections::HashMap;
use std::sync::Mutex;

pub(crate) const MIP_BLIT_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    // Fullscreen triangle: uv (0,0), (2,0), (0,2); v grows downwards.
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src, samp, in.uv, 0.0);
}
"#;

/// The shared blit resources and one pipeline per target format.
#[derive(Debug, Default)]
pub(crate) struct MipCache {
    inner: Mutex<Option<MipBlit>>,
}

#[derive(Debug)]
struct MipBlit {
    shader: wgpu::ShaderModule,
    bgl: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

impl MipBlit {
    fn new(device: &wgpu::Device) -> MipBlit {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("md-render mip blit"),
            source: wgpu::ShaderSource::Wgsl(MIP_BLIT_WGSL.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("md-render mip blit"),
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("md-render mip blit"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("md-render mip blit"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        MipBlit {
            shader,
            bgl,
            layout,
            sampler,
            pipelines: HashMap::new(),
        }
    }

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let (shader, layout) = (&self.shader, &self.layout);
        self.pipelines
            .entry(format)
            .or_insert_with(|| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("md-render mip blit"),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module: shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: shader,
                        entry_point: Some("fs_main"),
                        compilation_options: Default::default(),
                        targets: &[Some(format.into())],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            })
            .clone()
    }
}

/// Fill levels `1..mips` of `tex` from its level 0 and submit the work.
/// A no-op for a texture without mips. `tex` must be renderable and
/// sampleable (every [`Texture2d`] is) in a filterable format.
pub fn generate_mips(gpu: &Gpu, tex: &Texture2d) {
    if tex.mips <= 1 {
        return;
    }
    let device = gpu.device();
    let mut guard = gpu.mips.inner.lock().unwrap_or_else(|e| e.into_inner());
    let blit = guard.get_or_insert_with(|| MipBlit::new(device));
    let pipeline = blit.pipeline(device, tex.format);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("md-render generate_mips"),
    });
    for level in 1..tex.mips {
        let src = tex.level_view(level - 1);
        let dst = tex.level_view(level);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("md-render mip blit"),
            layout: &blit.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&blit.sampler),
                },
            ],
        });
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("md-render mip level"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..3, 0..1);
    }
    gpu.queue().submit([enc.finish()]);
}

#[cfg(test)]
mod tests {
    #[test]
    fn mip_blit_wgsl_validates() {
        let module = naga::front::wgsl::parse_str(super::MIP_BLIT_WGSL).expect("parse");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("validate");
    }
}

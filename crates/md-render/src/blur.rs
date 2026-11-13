//! The blur chain (Task 6.1): `CPlugin::BlurPasses`
//! (code/vis_milk2/milkdropfs.cpp:1501-1655@6f7cd5d), run after the warp
//! pass and before the waves and the composite.
//!
//! `2 * highest_used` full-screen passes (at most [`NUM_BLUR_TEX`]) draw
//! blur texture `i` from the previous one: pass 0 reads the warp *source*
//! (`m_lpVS[0]`, the previous frame's main texture; the C++'s own comment
//! says "current frame" but the swap happens at the end of the frame) with
//! its mip chain, pass `i > 0` reads blur texture `i - 1`. Even passes are
//! the long horizontal `blur1_ps`, odd passes the short vertical
//! `blur2_ps`; presets read textures 1, 3, 5 as `blur1..3`.
//!
//! Quirks kept from the C++: pass 0's `_c0` (source texel size) is the
//! *window* size (`GetWidth()`, 1514-1516) although it samples the main
//! texture; the edge darkening only applies to pass 1.
//!
//! Zero-width ranges: [`safe_blur_min_max`](crate::safe_blur_min_max)
//! collapses a range narrower than 0.1 to `min == max`, and the C++ then
//! divides by zero (`fscale = inf`, `fbias = ±inf` or NaN reach the pixel
//! shader). Here [`blur_scale_bias`] keeps every divisor at least
//! [`MIN_RANGE`], which is the limit of the C++ formula as the width goes
//! to zero: the stage becomes a hard step at `min`. Nothing non-finite is
//! ever written to a uniform.
//!
//! D3D9's half-pixel rule is the built-in `blur_vs` shader's
//! (`VsParams::for_target` of the destination texture); the quad's UVs are
//! the C++'s untouched `0..1` ones.

use crate::fixed::SamplerKind;
use crate::frame_vars::PerFrameVars;
use crate::gpu::Gpu;
use crate::renderer::Renderer;
use crate::shader_params::safe_blur_min_max;
use crate::uniforms::{BlurUniforms, VsParams};
use crate::vertex::Vertex;
use md_hlsl::layout::{BLUR_UNIFORMS_SIZE, BUILTIN_VS_PARAMS_BINDING, BUILTIN_VS_PARAMS_SIZE};
use wgpu::util::DeviceExt;

/// `NUM_BLUR_TEX`.
pub const NUM_BLUR_TEX: usize = 6;

/// The tap weights `w[8]` ("user can specify these", never done).
pub const BLUR_WEIGHTS: [f32; 8] = [4.0, 3.8, 3.5, 2.9, 1.9, 1.2, 0.7, 0.3];

/// Smallest divisor [`blur_scale_bias`] uses for a range width.
pub const MIN_RANGE: f32 = 1e-6;

/// `fscale[3]` and `fbias[3]` of `BlurPasses` (1559-1575): the progressive
/// scale and bias taking each level's `[min..max]` range to the next, from
/// the output of `GetSafeBlurMinMax`.
pub fn blur_scale_bias(mn: [f32; 3], mx: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let recip = |width: f32| 1.0 / width.max(MIN_RANGE);
    let mut scale = [0.0; 3];
    let mut bias = [0.0; 3];
    scale[0] = recip(mx[0] - mn[0]);
    bias[0] = -mn[0] * scale[0];
    let d0 = recip(mx[0] - mn[0]);
    let t_min = (mn[1] - mn[0]) * d0;
    let t_max = (mx[1] - mn[0]) * d0;
    scale[1] = recip(t_max - t_min);
    bias[1] = -t_min * scale[1];
    let d1 = recip(mx[1] - mn[1]);
    let t_min = (mn[2] - mn[1]) * d1;
    let t_max = (mx[2] - mn[1]) * d1;
    scale[2] = recip(t_max - t_min);
    bias[2] = -t_min * scale[2];
    (scale, bias)
}

/// The constants of blur pass `i` (`BlurUniforms` `_c0`..`_c6`): `src` is
/// the source texture size the C++ reports, `scale`/`bias` the level's
/// [`blur_scale_bias`] entry, `edge_darken` `blur1_edge_darken`.
pub fn pass_uniforms(
    i: usize,
    src: (u32, u32),
    scale: f32,
    bias: f32,
    edge_darken: f32,
) -> BlurUniforms {
    let w = BLUR_WEIGHTS;
    let (sw, sh) = (src.0 as f32, src.1 as f32);
    let mut u = BlurUniforms::default();
    u.c[0] = [sw, sh, 1.0 / sw, 1.0 / sh];
    if i.is_multiple_of(2) {
        // Long horizontal pass (1612-1634).
        let w1 = w[0] + w[1];
        let w2 = w[2] + w[3];
        let w3 = w[4] + w[5];
        let w4 = w[6] + w[7];
        let d1 = 2.0 * w[1] / w1;
        let d2 = 2.0 + 2.0 * w[3] / w2;
        let d3 = 4.0 + 2.0 * w[5] / w3;
        let d4 = 6.0 + 2.0 * w[7] / w4;
        let w_div = 0.5 / (w1 + w2 + w3 + w4);
        u.c[1] = [w1, w2, w3, w4];
        u.c[2] = [d1, d2, d3, d4];
        u.c[3] = [scale, bias, w_div, 0.0];
    } else {
        // Short vertical pass (1636-1666).
        let w1 = w[0] + w[1] + w[2] + w[3];
        let w2 = w[4] + w[5] + w[6] + w[7];
        let d1 = 2.0 * ((w[2] + w[3]) / w1);
        let d2 = 2.0 + 2.0 * ((w[6] + w[7]) / w2);
        let w_div = 1.0 / ((w1 + w2) * 2.0);
        u.c[5] = [w1, w2, d1, d2];
        // Only the first vertical pass darkens the edges.
        u.c[6] = if i == 1 {
            [w_div, 1.0 - edge_darken, edge_darken, 5.0]
        } else {
            [w_div, 1.0, 0.0, 5.0]
        };
    }
    u
}

/// Pipelines, samplers and per-pass buffers of the blur chain, created on
/// the first pass. Each pass has its own uniform buffers because the whole
/// frame is one submit (`Queue::write_buffer` lands before it).
#[derive(Debug)]
pub(crate) struct BlurGpu {
    format: wgpu::TextureFormat,
    pipelines: [wgpu::RenderPipeline; 2],
    bgl1: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    quad: wgpu::Buffer,
    passes: Vec<PassBuffers>,
}

#[derive(Debug)]
struct PassBuffers {
    uniforms: wgpu::Buffer,
    vs_params: wgpu::Buffer,
    group0: wgpu::BindGroup,
}

impl BlurGpu {
    fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> BlurGpu {
        let device = gpu.device();
        let entry = |binding, visibility, size| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(size),
            },
            count: None,
        };
        let bgl0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("md-render blur uniforms"),
            entries: &[
                entry(0, wgpu::ShaderStages::FRAGMENT, BLUR_UNIFORMS_SIZE),
                entry(
                    BUILTIN_VS_PARAMS_BINDING,
                    wgpu::ShaderStages::VERTEX,
                    BUILTIN_VS_PARAMS_SIZE,
                ),
            ],
        });
        let bgl1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("md-render blur source"),
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
            label: Some("md-render blur"),
            bind_group_layouts: &[Some(&bgl0), Some(&bgl1)],
            immediate_size: 0,
        });
        let module = |label, src: &'static str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let vs = module(
            "blur_vs",
            include_str!("../../../assets/shaders/blur_vs.wgsl"),
        );
        let fs = [
            module(
                "blur1_ps",
                include_str!("../../../assets/shaders/blur1_ps.wgsl"),
            ),
            module(
                "blur2_ps",
                include_str!("../../../assets/shaders/blur2_ps.wgsl"),
            ),
        ];
        let pipelines = fs.each_ref().map(|fs| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("md-render blur"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &vs,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(Vertex::LAYOUT)],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: fs,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        // The C++'s quad (1530-1550): v = 0 at the bottom.
        let quad: Vec<u8> = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
            .iter()
            .flat_map(|&(x, y): &(f32, f32)| {
                Vertex {
                    pos: [x, y, 0.0],
                    uv: [(x + 1.0) * 0.5, (y + 1.0) * 0.5, 0.0, 0.0],
                    ..Vertex::default()
                }
                .to_bytes()
            })
            .collect();
        let quad = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("md-render blur quad"),
            contents: &quad,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let buf = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let passes = (0..NUM_BLUR_TEX)
            .map(|_| {
                let uniforms = buf("md-render blur uniforms", BLUR_UNIFORMS_SIZE);
                let vs_params = buf("md-render blur VsParams", BUILTIN_VS_PARAMS_SIZE);
                let group0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("md-render blur group 0"),
                    layout: &bgl0,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniforms.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: BUILTIN_VS_PARAMS_BINDING,
                            resource: vs_params.as_entire_binding(),
                        },
                    ],
                });
                PassBuffers {
                    uniforms,
                    vs_params,
                    group0,
                }
            })
            .collect();
        BlurGpu {
            format,
            pipelines,
            bgl1,
            sampler: device.create_sampler(&SamplerKind::LinearClamp.descriptor()),
            quad,
            passes,
        }
    }
}

impl Renderer {
    /// `BlurPasses`: record `min(6, 2 * highest_used)` passes into `enc`,
    /// each drawing blur texture `i`; nothing for 0. `vars` supplies the
    /// post-equation `blur1..3_min/_max` and `blur1_edge_darken` (F38);
    /// `highest_used` is [`crate::PresetGpu::highest_blur`] (F39).
    ///
    /// The caller must have submitted nothing since the warp pass wrote the
    /// main texture's mips; the passes read the warp *source* (see the
    /// module docs) and write only the blur textures.
    pub fn blur_passes(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        vars: &PerFrameVars,
        highest_used: u8,
    ) {
        let passes = (usize::from(highest_used) * 2).min(NUM_BLUR_TEX);
        if passes == 0 {
            return;
        }
        let format = self.blur[0].format;
        let state = match self.blur_gpu.take() {
            Some(s) if s.format == format => s,
            _ => BlurGpu::new(&self.gpu, format),
        };
        let (mn, mx) = safe_blur_min_max(vars);
        let (scale, bias) = blur_scale_bias(mn, mx);
        let edge_darken = vars.blur1_edge_darken as f32;
        let main = &self.vs[Renderer::other_vs(self.vs_target)];
        for i in 0..passes {
            let dst = &self.blur[i];
            let (src_view, src_size) = if i == 0 {
                (&main.view, self.window)
            } else {
                (
                    &self.blur[i - 1].view,
                    (self.blur[i - 1].w, self.blur[i - 1].h),
                )
            };
            let bufs = &state.passes[i];
            let u = pass_uniforms(i, src_size, scale[i / 2], bias[i / 2], edge_darken);
            self.gpu
                .queue()
                .write_buffer(&bufs.uniforms, 0, &u.to_bytes());
            self.gpu.queue().write_buffer(
                &bufs.vs_params,
                0,
                &VsParams::for_target(dst.w, dst.h).to_bytes(),
            );
            let group1 = self
                .gpu
                .device()
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("md-render blur group 1"),
                    layout: &state.bgl1,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(src_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&state.sampler),
                        },
                    ],
                });
            let mut pass = crate::comp::clear_pass(enc, "md-render blur", &dst.view);
            pass.set_pipeline(&state.pipelines[i % 2]);
            pass.set_bind_group(0, &bufs.group0, &[]);
            pass.set_bind_group(1, &group1, &[]);
            pass.set_vertex_buffer(0, state.quad.slice(..));
            pass.draw(0..4, 0..1);
        }
        self.blur_gpu = Some(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_constants_follow_blur_passes() {
        let u = pass_uniforms(0, (96, 64), 2.0, -0.5, 0.25);
        assert_eq!(u.c[0], [96.0, 64.0, 1.0 / 96.0, 1.0 / 64.0]);
        let w = BLUR_WEIGHTS;
        let (w1, w2, w3, w4) = (w[0] + w[1], w[2] + w[3], w[4] + w[5], w[6] + w[7]);
        assert_eq!(u.c[1], [w1, w2, w3, w4]);
        assert_eq!(
            u.c[2],
            [
                2.0 * w[1] / w1,
                2.0 + 2.0 * w[3] / w2,
                4.0 + 2.0 * w[5] / w3,
                6.0 + 2.0 * w[7] / w4
            ]
        );
        assert_eq!(u.c[3], [2.0, -0.5, 0.5 / (w1 + w2 + w3 + w4), 0.0]);
        assert_eq!(u.c[4], [0.0; 4]);

        let v = pass_uniforms(1, (48, 32), 1.0, 0.0, 0.25);
        let (a, b) = (w[0] + w[1] + w[2] + w[3], w[4] + w[5] + w[6] + w[7]);
        let w_div = 1.0 / ((a + b) * 2.0);
        assert_eq!(
            v.c[5],
            [
                a,
                b,
                2.0 * ((w[2] + w[3]) / a),
                2.0 + 2.0 * ((w[6] + w[7]) / b)
            ]
        );
        // Only pass 1 darkens the edges.
        assert_eq!(v.c[6], [w_div, 0.75, 0.25, 5.0]);
        assert_eq!(
            pass_uniforms(3, (48, 32), 1.0, 0.0, 0.25).c[6],
            [w_div, 1.0, 0.0, 5.0]
        );
        assert_eq!(
            pass_uniforms(5, (48, 32), 1.0, 0.0, 0.25).c[6],
            [w_div, 1.0, 0.0, 5.0]
        );
    }

    #[test]
    fn scale_and_bias_chain_the_levels() {
        // Level 1 [0.1, 0.9]; level 2 [0.3, 0.7] inside it; level 3 equal.
        let (s, b) = blur_scale_bias([0.1, 0.3, 0.3], [0.9, 0.7, 0.7]);
        assert!((s[0] - 1.25).abs() < 1e-5 && (b[0] + 0.125).abs() < 1e-5);
        // Level 2 relative to level 1: [0.25, 0.75] -> scale 2, bias -0.5.
        assert!((s[1] - 2.0).abs() < 1e-5 && (b[1] + 0.5).abs() < 1e-5);
        // Level 3 equals level 2: the identity.
        assert!((s[2] - 1.0).abs() < 1e-5 && b[2].abs() < 1e-5);
    }

    #[test]
    fn zero_width_ranges_stay_finite() {
        // GetSafeBlurMinMax collapses narrow ranges to min == max.
        let mut v = PerFrameVars::default();
        for (mn, mx) in [(0.5, 0.5), (0.0, 0.0), (-1.0, 3.0), (2.0, -2.0)] {
            (v.blur1_min, v.blur1_max) = (mn, mx);
            (v.blur2_min, v.blur2_max) = (mn, mx);
            (v.blur3_min, v.blur3_max) = (mn, mx);
            let (lo, hi) = safe_blur_min_max(&v);
            let (s, b) = blur_scale_bias(lo, hi);
            assert!(
                s.iter().chain(&b).all(|x| x.is_finite()),
                "{mn}..{mx}: {s:?} {b:?}"
            );
            for i in 0..6 {
                let u = pass_uniforms(i, (16, 16), s[i / 2], b[i / 2], 0.25);
                assert!(u.c.iter().flatten().all(|x| x.is_finite()));
            }
        }
        // Collapsed: a hard step at `min` (scale 1/MIN_RANGE).
        let (s, b) = blur_scale_bias([0.2; 3], [0.2; 3]);
        assert_eq!(s[0], 1.0 / MIN_RANGE);
        assert!((b[0] + 0.2 / MIN_RANGE).abs() < 1.0);
    }
}

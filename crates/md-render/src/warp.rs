//! The warp pass (Task 5.6): the previous frame, sampled through the warp
//! mesh, drawn into the warp target (single-level, as in the C++).
//!
//! Mirrors code/vis_milk2/milkdropfs.cpp:
//! - which passes run (`RenderFrame`, 1006-1047): no blend → one pass with
//!   the current preset's warp shader, or the fixed-function path for a
//!   MilkDrop 1 (`PSVERSION_WARP 0`) preset; while blending, pass 0 draws
//!   the OLD preset (blending off, only tiles not fully opaque — "flip
//!   culling") and pass 1 the NEW one (`SRCALPHA/INVSRCALPHA`, only tiles
//!   with some alpha), except when neither preset has a warp shader: then
//!   one fixed-function pass without blending, the blend living entirely
//!   in the mixed UVs;
//! - `WarpedBlit_Shaders` (2090-2182): the preset's warp pixel shader with
//!   `warp_vs`, vertex colour white with alpha `mix2`, the mesh drawn in two
//!   halves (`m_indices_list` quadrants 0-1, then 2-3) with the `ang` of the
//!   left half of the middle row forced to `-π` for the first half and `+π`
//!   for the second (the angle-wrap seam), tiles culled by alpha while
//!   blending;
//! - `WarpedBlit_NoShaders` (1866-2088): fixed function, texture × vertex
//!   colour `D3DCOLOR_RGBA_01(decay, decay, decay, 1)` with the vertex alpha
//!   kept, alpha `SELECTARG1 DIFFUSE` ([`FixedMode::TexModulate`]),
//!   bilinear + linear mip, wrap when `var_pf_wrap > m_fSnapPoint` else
//!   clamp. The decay and wrap come from the CURRENT preset even when this
//!   path draws the old one (1910-1918). The C++ flips each vertex's `y`
//!   and `PrepareFor2DDrawing`'s ortho projection (support.cpp:178-184)
//!   flips it back, so positions are the shader path's. The 16-bit
//!   back-buffer decay clamp (1923-1928) never applies: md-render targets
//!   are 8 or 16 bits per channel.
//!
//! Vertex data (ledger Task 4.8 ruling): exactly the C++'s `MYVERTEX` —
//! `uv.xy` = warped UV `+ 0.5/texsize` (1835-1837), `uv.zw` = `tu_orig,
//! tv_orig` with the same offset (plugin.cpp:2299-2300), `rad_ang`, diffuse.
//! The D3D9 half-pixel rasterization shift is `warp_vs`'s job.
//!
//! The C++ un-indexes the triangles in batches for old drivers; here one
//! indexed draw per half/pass draws the same triangles in the same order.
//! Not ported: the `INVALID_PRESET_DESC` clear (md-core's concern).

use crate::blend::BlendMode;
use crate::fixed::{FixedMode, SamplerKind, Topo};
use crate::gpu::Gpu;
use crate::mesh::WarpMesh;
use crate::pipelines::PipelineKey;
use crate::preset_gpu::PresetGpu;
use crate::renderer::Renderer;
use crate::shader_params::{shader_uniforms, FrameShaderInputs, PresetShaderInputs};
use crate::vertex::{vertices_to_bytes, Vertex};
use std::ops::Range;

/// One preset as the warp pass sees it: its compiled shaders and the
/// `ApplyShaderParams` inputs of its state.
#[derive(Clone, Copy, Debug)]
pub struct WarpPreset<'a> {
    pub gpu: &'a PresetGpu,
    pub shader: PresetShaderInputs<'a>,
}

/// `m_fSnapPoint` (milkdropfs.cpp:587-619): the blend progress at which
/// boolean values switch from the old preset to the new one — `-0.01`
/// when only the old preset has a composite shader, `1.01` when only the
/// new one does, else (and when not blending) `0.5`.
pub fn snap_point(blending: bool, old_comp: bool, new_comp: bool) -> f32 {
    match (blending, old_comp, new_comp) {
        (true, true, false) => -0.01,
        (true, false, true) => 1.01,
        _ => 0.5,
    }
}

/// Which triangles a pass draws (`bCullTiles` / `bFlipCulling`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cull {
    /// Every triangle.
    None,
    /// Triangles with any vertex alpha above 0 (`(d1|d2|d3) > 0`).
    Visible,
    /// Triangles with any vertex alpha below 255 (`(d1&d2&d3) < 255`).
    NotOpaque,
}

impl Cull {
    pub(crate) fn keeps(self, a: [u8; 3]) -> bool {
        match self {
            Cull::None => true,
            Cull::Visible => (a[0] | a[1] | a[2]) > 0,
            Cull::NotOpaque => (a[0] & a[1] & a[2]) < 255,
        }
    }
}

/// One `WarpedBlit_*` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WarpPassPlan {
    /// `Some(old)`: `WarpedBlit_Shaders` with the old (`true`) or new
    /// preset's warp shader; `None`: `WarpedBlit_NoShaders`.
    pub shader: Option<bool>,
    pub blend: BlendMode,
    pub cull: Cull,
}

/// The passes `RenderFrame` runs (milkdropfs.cpp:1006-1047).
pub fn plan_warp_passes(blending: bool, old_warp: bool, new_warp: bool) -> Vec<WarpPassPlan> {
    let new_pass = |blend, cull| WarpPassPlan {
        shader: new_warp.then_some(false),
        blend,
        cull,
    };
    if !blending || (!old_warp && !new_warp) {
        return vec![new_pass(BlendMode::Off, Cull::None)];
    }
    vec![
        WarpPassPlan {
            shader: old_warp.then_some(true),
            blend: BlendMode::Off,
            cull: Cull::NotOpaque,
        },
        new_pass(BlendMode::Alpha, Cull::Visible),
    ]
}

/// The vertices and per-pass index ranges of one warp pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WarpGeometry {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// Index range of each plan, in plan order.
    pub ranges: Vec<Range<u32>>,
}

/// Build the vertex blocks and index lists for `plans` over `mesh`, for a
/// `tex`-sized target, with `decay_rgb` the fixed-function vertex colour.
pub fn warp_geometry(
    mesh: &WarpMesh,
    tex: (u32, u32),
    plans: &[WarpPassPlan],
    decay_rgb: u8,
) -> WarpGeometry {
    let (gx, gy) = mesh.grid();
    let n = mesh.positions().len() as u32;
    let tox = 0.5 / tex.0.max(1) as f32;
    let toy = 0.5 / tex.1.max(1) as f32;
    let alphas = mesh.alphas();
    let vertex = |i: usize, rgb: u8, ang: f32| {
        let [x, y] = mesh.positions()[i];
        let [u, v] = mesh.uvs()[i];
        Vertex {
            pos: [x, y, 0.0],
            color: [rgb, rgb, rgb, alphas[i]],
            uv: [u + tox, v + toy, x * 0.5 + 0.5 + tox, -y * 0.5 + 0.5 + toy],
            rad_ang: [mesh.vert_info()[i].rad, ang],
        }
    };
    let mut g = WarpGeometry::default();
    let block = |g: &mut WarpGeometry, rgb: u8, seam: Option<f32>| -> u32 {
        let base = g.vertices.len() as u32;
        // milkdropfs.cpp:2149-2152: the seam is the left half of row gy/2.
        let seam_start = (gy / 2) * (gx + 1);
        let seam_end = seam_start + gx / 2;
        g.vertices.extend((0..n).map(|i| {
            let ang = match seam {
                Some(a) if (seam_start..seam_end).contains(&i) => a,
                _ => mesh.vert_info()[i as usize].ang,
            };
            vertex(i as usize, rgb, ang)
        }));
        base
    };
    let mut shader_bases = None;
    let mut fixed_base = None;
    let list = mesh.indices();
    let half = list.len() / 2;
    for plan in plans {
        let start = g.indices.len() as u32;
        let parts: Vec<(&[u32], u32)> = match plan.shader {
            Some(_) => {
                let (a, b) = *shader_bases.get_or_insert_with(|| {
                    let pi = std::f32::consts::PI;
                    (block(&mut g, 255, Some(-pi)), block(&mut g, 255, Some(pi)))
                });
                vec![(&list[..half], a), (&list[half..], b)]
            }
            None => {
                let base = *fixed_base.get_or_insert_with(|| block(&mut g, decay_rgb, None));
                vec![(list, base)]
            }
        };
        for (tris, base) in parts {
            for t in tris.chunks_exact(3) {
                let a = [0, 1, 2].map(|k| alphas[t[k] as usize]);
                if plan.cull.keeps(a) {
                    g.indices.extend(t.iter().map(|&i| i + base));
                }
            }
        }
        g.ranges.push(start..g.indices.len() as u32);
    }
    g
}

/// `D3DCOLOR_RGBA_01(d, d, d, 1)`'s colour byte (milkdropfs.cpp:40):
/// `(int)(d * 255)` masked to 8 bits by `D3DCOLOR_RGBA`.
pub fn decay_byte(decay: f32) -> u8 {
    ((decay * 255.0) as i32 & 0xff) as u8
}

/// The warp pass's (and the composite grid's) vertex and index buffers,
/// grown on demand.
#[derive(Debug)]
pub(crate) struct WarpBuffers {
    pub(crate) vbuf: wgpu::Buffer,
    pub(crate) ibuf: wgpu::Buffer,
}

fn buffer(gpu: &Gpu, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    gpu.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("md-render warp mesh"),
        size: size.max(4),
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl WarpBuffers {
    /// Upload `g`, replacing a buffer that is too small.
    fn upload(slot: &mut Option<WarpBuffers>, gpu: &Gpu, g: &WarpGeometry) {
        WarpBuffers::upload_raw(slot, gpu, &g.vertices, &g.indices);
    }

    /// Upload `vertices` and `indices`, replacing a buffer that is too small.
    pub(crate) fn upload_raw(
        slot: &mut Option<WarpBuffers>,
        gpu: &Gpu,
        vertices: &[Vertex],
        indices: &[u32],
    ) {
        let v = vertices_to_bytes(vertices);
        let i: Vec<u8> = indices.iter().flat_map(|x| x.to_le_bytes()).collect();
        let b = slot.get_or_insert_with(|| WarpBuffers {
            vbuf: buffer(gpu, v.len() as u64, wgpu::BufferUsages::VERTEX),
            ibuf: buffer(gpu, i.len() as u64, wgpu::BufferUsages::INDEX),
        });
        if b.vbuf.size() < v.len() as u64 {
            b.vbuf = buffer(gpu, v.len() as u64, wgpu::BufferUsages::VERTEX);
        }
        if b.ibuf.size() < i.len() as u64 {
            b.ibuf = buffer(gpu, i.len() as u64, wgpu::BufferUsages::INDEX);
        }
        if !v.is_empty() {
            gpu.queue().write_buffer(&b.vbuf, 0, &v);
        }
        if !i.is_empty() {
            gpu.queue().write_buffer(&b.ibuf, 0, &i);
        }
    }
}

impl Renderer {
    /// Record the warp pass into `enc`: draw [`Renderer::warp_target`]
    /// from the other ping-pong texture through `mesh` (whose UVs and
    /// alphas [`WarpMesh::compute_uvs`] computed this frame). `old` is the blend-from
    /// preset while a blend is running (pass 0 draws it, pass 1 draws `new` with the mesh alphas),
    /// `None` otherwise.
    ///
    /// Shader uniforms are uploaded with `Queue::write_buffer` and so take
    /// effect at the next submit: record at most one warp pass per preset
    /// program per submission (`old` and `new` must be different
    /// [`PresetGpu`]s).
    pub fn warp_pass(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        frame: &FrameShaderInputs,
        new: &WarpPreset,
        old: Option<&WarpPreset>,
        mesh: &WarpMesh,
    ) {
        let blending = old.is_some();
        let old_warp = old.is_some_and(|o| o.gpu.warp.is_some());
        let plans = plan_warp_passes(blending, old_warp, new.gpu.warp.is_some());
        let tex = (self.settings.tex_w, self.settings.tex_h);
        let new_vars = new.shader.vars;
        let geometry = warp_geometry(mesh, tex, &plans, decay_byte(new_vars.decay as f32));
        WarpBuffers::upload(&mut self.warp_buffers, &self.gpu, &geometry);

        let snap = snap_point(
            blending,
            old.is_some_and(|o| o.gpu.comp.is_some()),
            new.gpu.comp.is_some(),
        );
        let fixed_sampler = if new_vars.wrap > f64::from(snap) {
            SamplerKind::LinearWrap
        } else {
            SamplerKind::LinearClamp
        };
        let target = self.vs_target;
        let source = Renderer::other_vs(target);
        let format = self.vs[target].format;
        let (gpu, vs) = (&self.gpu, &self.vs);

        // Per-pass pipelines and bind groups, resolved before the pass.
        enum Draw<'p> {
            Shader(&'p crate::ShaderProgram, BlendMode),
            Fixed(wgpu::RenderPipeline, [wgpu::BindGroup; 2]),
        }
        let mut draws = Vec::with_capacity(plans.len());
        for plan in &plans {
            match plan.shader {
                Some(is_old) => {
                    let p = if is_old { old.unwrap_or(new) } else { new };
                    let prog = p.gpu.warp.as_ref().expect("planned from warp.is_some()");
                    let u = shader_uniforms(frame, &p.shader, tex, &mut self.rng);
                    prog.write_uniforms(gpu, &u);
                    draws.push(Draw::Shader(prog, plan.blend));
                }
                None => {
                    let key =
                        PipelineKey::new(FixedMode::TexModulate, plan.blend, Topo::TriList, format);
                    let pipeline = self.pipelines.get(gpu, key).clone();
                    let g0 = self.pipelines.vs_bind_group(gpu, tex.0, tex.1);
                    let g1 =
                        self.pipelines
                            .texture_bind_group(gpu, &vs[source].view, fixed_sampler);
                    draws.push(Draw::Fixed(pipeline, [g0, g1]));
                }
            }
        }

        let view = vs[target].level_view(0);
        let buffers = self.warp_buffers.as_ref().expect("uploaded above");
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("md-render warp"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_vertex_buffer(0, buffers.vbuf.slice(..));
            pass.set_index_buffer(buffers.ibuf.slice(..), wgpu::IndexFormat::Uint32);
            for (draw, range) in draws.iter().zip(&geometry.ranges) {
                if range.is_empty() {
                    continue;
                }
                match draw {
                    Draw::Shader(prog, blend) => {
                        prog.bind(&mut pass, source, *blend);
                    }
                    Draw::Fixed(pipeline, [g0, g1]) => {
                        pass.set_pipeline(pipeline);
                        pass.set_bind_group(0, g0, &[]);
                        pass.set_bind_group(1, g1, &[]);
                    }
                }
                pass.draw_indexed(range.clone(), 0, 0..1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::VertInfo;

    fn shader(old: bool, blend: BlendMode, cull: Cull) -> WarpPassPlan {
        WarpPassPlan {
            shader: Some(old),
            blend,
            cull,
        }
    }

    fn fixed(blend: BlendMode, cull: Cull) -> WarpPassPlan {
        WarpPassPlan {
            shader: None,
            blend,
            cull,
        }
    }

    #[test]
    fn passes_follow_render_frame() {
        use BlendMode::{Alpha, Off};
        assert_eq!(
            plan_warp_passes(false, true, true),
            [shader(false, Off, Cull::None)]
        );
        assert_eq!(
            plan_warp_passes(false, true, false),
            [fixed(Off, Cull::None)]
        );
        assert_eq!(
            plan_warp_passes(true, true, true),
            [
                shader(true, Off, Cull::NotOpaque),
                shader(false, Alpha, Cull::Visible)
            ]
        );
        assert_eq!(
            plan_warp_passes(true, false, true),
            [
                fixed(Off, Cull::NotOpaque),
                shader(false, Alpha, Cull::Visible)
            ]
        );
        assert_eq!(
            plan_warp_passes(true, true, false),
            [
                shader(true, Off, Cull::NotOpaque),
                fixed(Alpha, Cull::Visible)
            ]
        );
        assert_eq!(
            plan_warp_passes(true, false, false),
            [fixed(Off, Cull::None)]
        );
    }

    #[test]
    fn snap_point_follows_the_comp_shader_code() {
        assert_eq!(snap_point(false, true, false), 0.5);
        assert_eq!(snap_point(true, true, false), -0.01);
        assert_eq!(snap_point(true, false, true), 1.01);
        assert_eq!(snap_point(true, true, true), 0.5);
        assert_eq!(snap_point(true, false, false), 0.5);
    }

    #[test]
    fn decay_byte_truncates_and_wraps_like_d3dcolor() {
        assert_eq!(decay_byte(1.0), 255);
        assert_eq!(decay_byte(0.98), 249); // (int)249.9
        assert_eq!(decay_byte(1.01), 1); // 257 & 0xff
    }

    #[test]
    fn shader_geometry_has_two_seam_blocks_and_offsets() {
        let mut mesh = WarpMesh::new(4, 4, 1.0, 1.0);
        mesh.compute_uvs(&Default::default(), None, 0.0, None);
        let plans = [shader(false, BlendMode::Off, Cull::None)];
        let g = warp_geometry(&mesh, (100, 50), &plans, 0);
        assert_eq!(g.vertices.len(), 2 * 25);
        assert_eq!(g.ranges, vec![0..(4 * 4 * 6)]);
        // Seam: row 2, x = 0, 1 (left half), ang -π then +π; x = 2 (centre) 0.
        let pi = std::f32::consts::PI;
        assert_eq!(g.vertices[10].rad_ang[1], -pi);
        assert_eq!(g.vertices[25 + 11].rad_ang[1], pi);
        assert_eq!(g.vertices[12].rad_ang[1], 0.0);
        // Second half indices point into the +π block.
        assert!(g.indices[..48].iter().all(|&i| i < 25));
        assert!(g.indices[48..].iter().all(|&i| i >= 25));
        // uv_orig with the C++ half-texel offset; white, opaque.
        let v = g.vertices[0];
        assert!((v.uv[2] - 0.005).abs() < 1e-7 && (v.uv[3] - 1.01).abs() < 1e-6);
        assert_eq!(v.color, [255; 4]);
    }

    #[test]
    fn blend_culling_drops_hidden_tiles() {
        let mut mesh = WarpMesh::new(2, 2, 1.0, 1.0);
        let vars = crate::PerFrameVars::default();
        mesh.compute_uvs(&vars, None, 0.0, None);
        // Left column fully new (mix2 = 1), the rest mid-blend or old.
        let mut info = vec![VertInfo::default(); 9];
        for (i, v) in info.iter_mut().enumerate() {
            v.c = if i % 3 == 0 { 1.0 } else { 0.0 };
        }
        mesh.compute_uvs(&vars, None, 0.0, Some((&info, 0.0)));
        let plans = plan_warp_passes(true, false, true);
        let g = warp_geometry(&mesh, (8, 8), &plans, decay_byte(0.5));
        // Pass 0 (fixed, flip culling): every triangle has a vertex below
        // 255. Pass 1 (Visible): every triangle touches column 0 or not.
        let tri_count = |r: &Range<u32>| (r.end - r.start) / 3;
        assert_eq!(tri_count(&g.ranges[0]), 8);
        // Cells in column 1 (x = 1..2) have alphas 0 only.
        assert_eq!(tri_count(&g.ranges[1]), 4);
        // Fixed vertices carry the decay colour and the blend alpha.
        let f = g.indices[0] as usize;
        assert_eq!(g.vertices[f].color[..3], [127, 127, 127]);
    }
}

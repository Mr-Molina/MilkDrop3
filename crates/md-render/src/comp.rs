//! The composite pass ("show to user"): the warped main texture drawn to
//! the screen through the preset's composite shader
//! (`ShowToUser_Shaders`, code/vis_milk2/milkdropfs.cpp:4289-4475@6f7cd5d)
//! or the fixed-function path ([`crate::comp_fixed`]).
//!
//! - The grid ([`comp_grid`], plugin.cpp:2038-2152@6f7cd5d): `32 × 24`
//!   vertices (`FCGSX`, `FCGSY`); the middle row and column are doubled
//!   (`i2 = i - i/(FCGSX/2)`), `u`, `v` squished towards the centre
//!   (`SquishToCenter(·, 3)`), positions `x = (u - 0.5/W)*2 - 1`,
//!   `y = -((v - 0.5/H)*2 - 1)` (the C++'s own -0.5 px offset, kept per the
//!   ledger Task 4.8 ruling; `comp_vs` adds the D3D9 raster shift),
//!   `rad`/`ang` from `UvToMathSpace` with the `ang` fix-ups along the
//!   centre lines, and an index list that skips the doubled cells and
//!   orients each quadrant's triangles for `ang` interpolation.
//! - Colours (4316-4392): four corner hues ([`hue_shades`]) bilinearly
//!   interpolated over the vertex positions (`hue_shader` in the preset
//!   shader is the vertex colour); alpha 1, or while blending the warp
//!   mesh's blend alpha bilinearly sampled at the vertex
//!   ([`comp_vertices`]).
//! - Passes ([`plan_comp_passes`], `RenderFrame` 1072-1119): see there.
//!   Tiles are culled by alpha exactly as the C++ does while blending.
//!
//! Target (ledger Task 5.5 note): the C++ composites into an 8-bit
//! `X8R8G8B8` back buffer that saturates after every draw (gamma passes,
//! the brighten/solarize/invert filters and blend pass 1 all see the
//! clamped result). md-render composites into an intermediate texture of
//! [`COMP_FORMAT`] (`Rgba8Unorm`, whatever the main texture format) and
//! the window size ([`crate::Renderer::comp_texture`]); every composite
//! pipeline (preset composite programs and the fixed passes) targets that
//! format. The result is then copied into the caller's target view with a
//! cached fixed-function pipeline for the target's format (any non-sRGB
//! colour format; the copy writes alpha 1). One extra full-screen pass, in
//! exchange for one pipeline set per program regardless of how many
//! surface formats are used, and an inspectable composite. sRGB targets
//! are rejected: the C++ back buffer is not gamma-encoded.

use crate::blend::BlendMode;
use crate::comp_fixed::FixedCompDraw;
use crate::fixed::{FixedMode, SamplerKind, Topo};
use crate::mesh::{mesh_aspect, WarpMesh};
use crate::pipelines::{DrawBatch, PipelineKey};
use crate::preset_gpu::ShaderProgram;
use crate::renderer::Renderer;
use crate::vertex::Vertex;
use crate::warp::{Cull, WarpBuffers};
use std::f32::consts::{PI, TAU};
use std::ops::Range;

/// The composite's format: the C++'s 8-bit back buffer (see the module docs).
pub const COMP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// `FCGSX`: composite grid vertices across (plugin.h:511).
pub const COMP_GRID_X: u32 = 32;
/// `FCGSY`: composite grid vertices down (plugin.h:512).
pub const COMP_GRID_Y: u32 = 24;

/// The composite grid: `COMP_GRID_X * COMP_GRID_Y` vertices (white,
/// `uv.xy = (u, v)`, `rad_ang`) and the triangle list over them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompGrid {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// `SquishToCenter` (plugin.cpp:1501-1507).
fn squish_to_center(x: f32, exp: f32) -> f32 {
    if x > 0.5 {
        return (x * 2.0 - 1.0).powf(exp) * 0.5 + 0.5;
    }
    (1.0 - (1.0 - x * 2.0).powf(exp)) * 0.5
}

/// `UvToMathSpace` (milkdropfs.cpp:3779-3795) with `m_fAspectX/Y`.
fn uv_to_math_space(u: f32, v: f32, ax: f32, ay: f32) -> (f32, f32) {
    let px = (u * 2.0 - 1.0) * ax;
    let py = (v * 2.0 - 1.0) * ay;
    let rad = (px * px + py * py).sqrt() / (ax * ax + ay * ay).sqrt();
    let mut ang = py.atan2(px);
    if ang < 0.0 {
        ang += TAU;
    }
    (rad, ang)
}

/// The `ang` fix-ups along the centre lines (plugin.cpp:2069-2109).
fn fix_ang(i: u32, j: u32, ang: f32) -> f32 {
    let (hx, hy) = (COMP_GRID_X / 2, COMP_GRID_Y / 2);
    let k = if i == hx - 1 {
        match j {
            j if j < hy - 1 => 1.5,
            j if j == hy - 1 => 1.25,
            j if j == hy => 0.75,
            _ => 0.5,
        }
    } else if i == hx {
        match j {
            j if j < hy - 1 => 1.5,
            j if j == hy - 1 => 1.75,
            j if j == hy => 0.25,
            _ => 0.5,
        }
    } else if j == hy - 1 {
        match i {
            i if i < hx - 1 => 1.0,
            i if i == hx - 1 => 1.25,
            i if i == hx => 1.75,
            _ => 2.0,
        }
    } else if j == hy {
        match i {
            i if i < hx - 1 => 1.0,
            i if i == hx - 1 => 0.75,
            i if i == hx => 0.25,
            _ => 0.0,
        }
    } else {
        return ang;
    };
    PI * k
}

/// The composite grid for a `window`-sized target and `tex`-sized main
/// textures (`m_fAspectX/Y` come from the texture size), exactly as
/// `AllocateMyDX9Stuff` builds `m_comp_verts` / `m_comp_indices`
/// (plugin.cpp:2038-2152).
pub fn comp_grid(window: (u32, u32), tex: (u32, u32)) -> CompGrid {
    let (ax, ay) = mesh_aspect(tex.0, tex.1);
    let half_w = 0.5 / window.0.max(1) as f32;
    let half_h = 0.5 / window.1.max(1) as f32;
    let div_x = 1.0 / (COMP_GRID_X - 2) as f32;
    let div_y = 1.0 / (COMP_GRID_Y - 2) as f32;
    let mut vertices = Vec::with_capacity((COMP_GRID_X * COMP_GRID_Y) as usize);
    for j in 0..COMP_GRID_Y {
        let j2 = j - j / (COMP_GRID_Y / 2);
        let v = squish_to_center(j2 as f32 * div_y, 3.0);
        let sy = -((v - half_h) * 2.0 - 1.0);
        for i in 0..COMP_GRID_X {
            let i2 = i - i / (COMP_GRID_X / 2);
            let u = squish_to_center(i2 as f32 * div_x, 3.0);
            let sx = (u - half_w) * 2.0 - 1.0;
            let (rad, ang) = uv_to_math_space(u, v, ax, ay);
            vertices.push(Vertex {
                pos: [sx, sy, 0.0],
                color: [255; 4],
                uv: [u, v, 0.0, 0.0],
                rad_ang: [rad, fix_ang(i, j, ang)],
            });
        }
    }
    let (gx, gy) = (COMP_GRID_X, COMP_GRID_Y);
    let mut indices = Vec::with_capacity(((gx - 2) * (gy - 2) * 6) as usize);
    for y in 0..gy - 1 {
        if y == gy / 2 - 1 {
            continue;
        }
        for x in 0..gx - 1 {
            if x == gx / 2 - 1 {
                continue;
            }
            let left_half = x < gx / 2;
            let top_half = y < gy / 2;
            let center_4 = (x == gx / 2 || x == gx / 2 - 1) && (y == gy / 2 || y == gy / 2 - 1);
            let at = |yy: u32, xx: u32| yy * gx + xx;
            if (u32::from(left_half) + u32::from(top_half) + u32::from(center_4)) % 2 == 1 {
                indices.extend([
                    at(y, x),
                    at(y, x + 1),
                    at(y + 1, x + 1),
                    at(y + 1, x + 1),
                    at(y + 1, x),
                    at(y, x),
                ]);
            } else {
                indices.extend([
                    at(y + 1, x),
                    at(y, x),
                    at(y, x + 1),
                    at(y, x + 1),
                    at(y + 1, x + 1),
                    at(y + 1, x),
                ]);
            }
        }
    }
    CompGrid { vertices, indices }
}

/// `CosineInterp` (utility.h:42).
pub fn cosine_interp(x: f32) -> f32 {
    0.5 - 0.5 * (x * PI).cos()
}

/// The four corner hues of the hue shader (milkdropfs.cpp:4323-4337, the
/// same in `ShowToUser_NoShaders` 4044-4056) at `GetTime()` = `now`:
/// `0.6 + 0.3 sin(now*30*k + phase + i*step + m_fRandStart[n])` per
/// channel, divided by the channel maximum and remapped to `0.5 + 0.5 x`.
pub fn hue_shades(now: f32, rand_start: [f32; 4]) -> [[f32; 3]; 4] {
    let mut shade = [[1.0f32; 3]; 4];
    for (i, s) in shade.iter_mut().enumerate() {
        let i = i as f32;
        s[0] = 0.6 + 0.3 * (now * 30.0 * 0.0143 + 3.0 + i * 21.0 + rand_start[3]).sin();
        s[1] = 0.6 + 0.3 * (now * 30.0 * 0.0107 + 1.0 + i * 13.0 + rand_start[1]).sin();
        s[2] = 0.6 + 0.3 * (now * 30.0 * 0.0129 + 6.0 + i * 9.0 + rand_start[2]).sin();
        let max = s[0].max(s[1]).max(s[2]);
        for c in s.iter_mut() {
            *c = 0.5 + 0.5 * (*c / max);
        }
    }
    shade
}

/// The grid with this frame's vertex colours (milkdropfs.cpp:4340-4390):
/// the hues bilinearly interpolated over `x = pos.x*0.5 + 0.5`,
/// `y = pos.y*0.5 + 0.5`, alpha 1 — or, with `blend`, the warp mesh's
/// blend alpha bilinearly sampled at `(x*(gx+1), y*(gy+1))` clamped to
/// `0..=g-1` (the C++'s index arithmetic, kept as is).
pub fn comp_vertices(
    grid: &CompGrid,
    shade: &[[f32; 3]; 4],
    blend: Option<&WarpMesh>,
) -> Vec<Vertex> {
    let mut out = grid.vertices.clone();
    for p in &mut out {
        let mut x = p.pos[0] * 0.5 + 0.5;
        let mut y = p.pos[1] * 0.5 + 0.5;
        let col = [0, 1, 2].map(|c| {
            shade[0][c] * x * y
                + shade[1][c] * (1.0 - x) * y
                + shade[2][c] * x * (1.0 - y)
                + shade[3][c] * (1.0 - x) * (1.0 - y)
        });
        let mut alpha = 1.0f64;
        if let Some(mesh) = blend {
            let (gx, gy) = mesh.grid();
            let a = mesh.alphas();
            x *= (gx + 1) as f32;
            y *= (gy + 1) as f32;
            x = x.min(gx as f32 - 1.0).max(0.0);
            y = y.min(gy as f32 - 1.0).max(0.0);
            let (nx, ny) = (x as usize, y as usize);
            let dx = f64::from(x) - nx as f64;
            let dy = f64::from(y) - ny as f64;
            let row = (gx + 1) as usize;
            let at = |yy: usize, xx: usize| f64::from(a[yy * row + xx]);
            alpha = at(ny, nx) * (1.0 - dx) * (1.0 - dy)
                + at(ny, nx + 1) * dx * (1.0 - dy)
                + at(ny + 1, nx) * (1.0 - dx) * dy
                + at(ny + 1, nx + 1) * dx * dy;
            alpha /= 255.0;
        }
        let ch = |v: f32| ((v * 255.0) as i32 & 0xff) as u8;
        p.color = [
            ch(col[0]),
            ch(col[1]),
            ch(col[2]),
            ((alpha * 255.0) as i32 & 0xff) as u8,
        ];
    }
    out
}

/// One `ShowToUser_*` call of `RenderFrame`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompPass {
    /// `ShowToUser_NoShaders` (always the whole screen, always first).
    Fixed,
    /// `ShowToUser_Shaders` with the old (`old`) or new preset's composite
    /// shader, blend mode and tile culling.
    Shader {
        old: bool,
        blend: BlendMode,
        cull: Cull,
    },
}

/// The composite passes of `RenderFrame` (milkdropfs.cpp:1072-1119): no
/// blend → the new preset's shader, or the fixed path; blending with both
/// shaders → old (blending off, tiles not fully opaque) then new
/// (`SrcAlpha/InvSrcAlpha`, tiles with some alpha); only the new one has a
/// shader → fixed then new; only the old one ("THA FUNKY REVERSAL") →
/// fixed then the OLD shader with `InvSrcAlpha/SrcAlpha` on the tiles not
/// fully opaque; neither → fixed (the blend lives in the blended values).
pub fn plan_comp_passes(blending: bool, old_comp: bool, new_comp: bool) -> Vec<CompPass> {
    let shader = |old, blend, cull| CompPass::Shader { old, blend, cull };
    match (blending, old_comp, new_comp) {
        (false, _, true) => vec![shader(false, BlendMode::Off, Cull::None)],
        (false, _, false) | (true, false, false) => vec![CompPass::Fixed],
        (true, true, true) => vec![
            shader(true, BlendMode::Off, Cull::NotOpaque),
            shader(false, BlendMode::Alpha, Cull::Visible),
        ],
        (true, false, true) => vec![
            CompPass::Fixed,
            shader(false, BlendMode::Alpha, Cull::Visible),
        ],
        (true, true, false) => vec![
            CompPass::Fixed,
            shader(true, BlendMode::InvAlpha, Cull::NotOpaque),
        ],
    }
}

/// The triangles of `indices` a pass with `cull` draws, by vertex alpha.
pub fn cull_indices(vertices: &[Vertex], indices: &[u32], cull: Cull) -> Vec<u32> {
    indices
        .chunks_exact(3)
        .filter(|t| cull.keeps([0, 1, 2].map(|k| vertices[t[k] as usize].color[3])))
        .flatten()
        .copied()
        .collect()
}

/// Everything the composite of one frame draws, prepared by
/// [`Renderer::begin_frame`](crate::Renderer::begin_frame).
#[derive(Debug)]
pub(crate) struct CompFrame<'a> {
    /// Passes with their program (`None` for [`CompPass::Fixed`]).
    pub passes: Vec<(CompPass, Option<&'a ShaderProgram>)>,
    /// The fixed path's quads.
    pub fixed: Vec<FixedCompDraw>,
    /// Stage-0 address mode the fixed path inherits.
    pub fixed_sampler: SamplerKind,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// Index range per pass (empty for the fixed pass).
    pub ranges: Vec<Range<u32>>,
}

/// The full-screen copy quad (triangle strip) for a `w`×`h` target: UVs
/// offset by half a target pixel so every target pixel samples the source
/// at its own centre after the raster shift. Positions are pre-projection
/// like every fixed-function draw's (`y = -1` is the top after the fixed
/// vertex shader's ortho flip), so `v = 0` is at `y = -1`.
fn blit_quad(w: u32, h: u32) -> [Vertex; 4] {
    let (ou, ov) = (0.5 / w.max(1) as f32, 0.5 / h.max(1) as f32);
    [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].map(|(x, y): (f32, f32)| Vertex {
        pos: [x, y, 0.0],
        color: [255; 4],
        uv: [(x + 1.0) * 0.5 + ou, (y + 1.0) * 0.5 + ov, 0.0, 0.0],
        ..Vertex::default()
    })
}

impl Renderer {
    /// Record the composite of `frame` into the intermediate composite
    /// texture, reading [`Renderer::warp_target`], then copy it to `target`.
    pub(crate) fn record_comp(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        frame: &CompFrame,
        title: &[crate::title::TitleDraw],
        sprites: &[crate::sprites_draw::SpriteDraw],
        text: (&[crate::text_draw::TextDraw], &[crate::text_draw::TextDraw]),
        target: &wgpu::TextureView,
    ) {
        WarpBuffers::upload_raw(
            &mut self.comp_buffers,
            &self.gpu,
            &frame.vertices,
            &frame.indices,
        );
        let main = self.vs_target;
        let source = &self.vs[main];
        let comp = &self.comp_tex;
        let format = comp.format;
        let buffers = self.comp_buffers.as_ref().expect("uploaded above");
        let pass = clear_pass(enc, "md-render composite", &comp.view);
        let mut batch = DrawBatch::new(
            &self.gpu,
            &mut self.pipelines,
            &mut self.dynbuf,
            pass,
            comp.w,
            comp.h,
        );
        for ((pass, prog), range) in frame.passes.iter().zip(&frame.ranges) {
            match (pass, prog) {
                (CompPass::Fixed, _) => {
                    let sampler = frame.fixed_sampler;
                    for d in &frame.fixed {
                        let mode = if d.textured {
                            FixedMode::TexModulate
                        } else {
                            FixedMode::Color
                        };
                        let key = PipelineKey::new(mode, d.blend, Topo::TriStrip, format);
                        batch.draw(key, &d.verts, Some(source), sampler);
                    }
                }
                (CompPass::Shader { blend, .. }, Some(prog)) => {
                    if range.is_empty() {
                        continue;
                    }
                    let p = batch.pass();
                    let bound = prog.bind(p, main, *blend);
                    debug_assert!(bound, "comp programs carry all three pipelines");
                    p.set_vertex_buffer(0, buffers.vbuf.slice(..));
                    p.set_index_buffer(buffers.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    p.draw_indexed(range.clone(), 0, 0..1);
                }
                (CompPass::Shader { .. }, None) => {}
            }
        }
        // The title animation over the composite (6.7, mfs:1123-1126), the
        // user sprites (6.8, :1131), then text and boxes over them (6.6;
        // DrawNow runs after MyRenderFn, pluginshell.cpp:1393-1405), before
        // the copy. The sprites inherit stage 0's sampler: linear/wrap after
        // a shader composite (RestoreShaderParams), else the fixed path's.
        let sprite_sampler = match frame.passes.last() {
            Some((CompPass::Shader { .. }, _)) => SamplerKind::LinearWrap,
            _ => frame.fixed_sampler,
        };
        Renderer::record_title(&mut batch, title, format);
        Renderer::record_sprites(&mut batch, sprites, format, sprite_sampler);
        Renderer::record_text(&mut batch, text.0, text.1, format);
        batch.finish();
        // Burnt sprites go into the warp target after the composite read it.
        Renderer::record_sprite_burns(
            (&self.gpu, &mut self.pipelines, &mut self.dynbuf),
            &self.vs[main],
            enc,
            sprites,
            sprite_sampler,
        );

        // Copy to the caller's target (its texture's format and size).
        let t = target.texture();
        assert!(
            !t.format().is_srgb(),
            "present target {:?} is sRGB; the composite is not gamma-encoded",
            t.format()
        );
        let (tw, th) = (t.width(), t.height());
        let sampler = if (tw, th) == (comp.w, comp.h) {
            SamplerKind::PointClamp
        } else {
            SamplerKind::LinearClamp
        };
        let pass = clear_pass(enc, "md-render present", target);
        let mut batch = DrawBatch::new(
            &self.gpu,
            &mut self.pipelines,
            &mut self.dynbuf,
            pass,
            tw,
            th,
        );
        let key = PipelineKey::new(
            FixedMode::TexModulate,
            BlendMode::Off,
            Topo::TriStrip,
            t.format(),
        );
        batch.draw(key, &blit_quad(tw, th), Some(comp), sampler);
        batch.finish();
    }
}

/// A render pass on `view` cleared to opaque black.
pub(crate) fn clear_pass<'e>(
    enc: &'e mut wgpu::CommandEncoder,
    label: &'static str,
    view: &wgpu::TextureView,
) -> wgpu::RenderPass<'e> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_follow_render_frame() {
        use BlendMode::{Alpha, InvAlpha, Off};
        let s = |old, blend, cull| CompPass::Shader { old, blend, cull };
        assert_eq!(
            plan_comp_passes(false, true, true),
            [s(false, Off, Cull::None)]
        );
        assert_eq!(plan_comp_passes(false, true, false), [CompPass::Fixed]);
        assert_eq!(
            plan_comp_passes(true, true, true),
            [
                s(true, Off, Cull::NotOpaque),
                s(false, Alpha, Cull::Visible)
            ]
        );
        assert_eq!(
            plan_comp_passes(true, false, true),
            [CompPass::Fixed, s(false, Alpha, Cull::Visible)]
        );
        assert_eq!(
            plan_comp_passes(true, true, false),
            [CompPass::Fixed, s(true, InvAlpha, Cull::NotOpaque)]
        );
        assert_eq!(plan_comp_passes(true, false, false), [CompPass::Fixed]);
    }

    #[test]
    fn grid_doubles_the_centre_lines_and_fixes_ang() {
        let g = comp_grid((64, 48), (64, 48));
        let at = |i: u32, j: u32| g.vertices[(j * COMP_GRID_X + i) as usize];
        // Columns 15 and 16 share u = 0.5; rows 11 and 12 share v = 0.5.
        assert_eq!(at(15, 0).uv[0], 0.5);
        assert_eq!(at(16, 0).uv[0], 0.5);
        assert_eq!(at(0, 11).uv[1], 0.5);
        assert_eq!(at(0, 12).uv[1], 0.5);
        // ang fix-ups at the centre.
        assert_eq!(at(15, 11).rad_ang[1], PI * 1.25);
        assert_eq!(at(16, 12).rad_ang[1], PI * 0.25);
        assert_eq!(at(20, 11).rad_ang[1], PI * 2.0);
        assert_eq!(at(15, 2).rad_ang[1], PI * 1.5);
        // Corner rad = 1; ang of the top-left corner (u = v = 0) with
        // m_fAspectY = 0.75.
        let c = at(0, 0);
        assert!((c.rad_ang[0] - 1.0).abs() < 1e-6);
        assert!((c.rad_ang[1] - ((-0.75f32).atan2(-1.0) + TAU)).abs() < 1e-5);
        // Squished towards the centre: u(1) = (1 - (1 - 2/30)^3) / 2.
        let u1 = (1.0 - (1.0 - 2.0 / 30.0f32).powf(3.0)) * 0.5;
        assert!((at(1, 0).uv[0] - u1).abs() < 1e-6);
        // No index touches the doubled cells (x = 15 or y = 11 as the
        // cell's top-left).
        for t in g.indices.chunks_exact(6) {
            let xs: Vec<u32> = t.iter().map(|i| i % COMP_GRID_X).collect();
            let ys: Vec<u32> = t.iter().map(|i| i / COMP_GRID_X).collect();
            let (x0, y0) = (*xs.iter().min().unwrap(), *ys.iter().min().unwrap());
            assert!(x0 != 15 && y0 != 11, "cell ({x0}, {y0})");
        }
    }

    #[test]
    fn hues_are_normalized_to_the_brightest_channel() {
        for s in hue_shades(12.5, [1.0, 2.0, 3.0, 4.0]) {
            let max = s[0].max(s[1]).max(s[2]);
            assert!((max - 1.0).abs() < 1e-6, "{s:?}");
            assert!(s.iter().all(|&c| (0.5..=1.0).contains(&c)));
        }
    }

    #[test]
    fn vertex_alpha_samples_the_warp_mesh_while_blending() {
        let g = comp_grid((64, 48), (64, 48));
        let white = [[1.0f32; 3]; 4];
        let v = comp_vertices(&g, &white, None);
        // Weights sum to 1 up to f32 rounding (truncated to 254 at worst).
        assert!(v
            .iter()
            .all(|v| v.color[3] == 255 && v.color[..3].iter().all(|&c| c >= 254)));
        // A mesh whose blend alpha is 0 everywhere: every comp vertex 0.
        let mut mesh = WarpMesh::new(8, 8, 1.0, 0.75);
        let vars = crate::PerFrameVars::default();
        mesh.compute_uvs(&vars, None, 0.0, None);
        let pattern = vec![crate::VertInfo::default(); 81];
        mesh.compute_uvs(&vars, None, 0.0, Some((&pattern, 0.0)));
        assert!(mesh.alphas().iter().all(|&a| a == 0));
        let v = comp_vertices(&g, &white, Some(&mesh));
        assert!(v.iter().all(|v| v.color[3] == 0));
        assert_eq!(cull_indices(&v, &g.indices, Cull::Visible).len(), 0);
        assert_eq!(
            cull_indices(&v, &g.indices, Cull::NotOpaque).len(),
            g.indices.len()
        );
    }

    #[test]
    fn colours_interpolate_the_corner_hues() {
        let g = comp_grid((64, 48), (64, 48));
        let mut shade = [[0.0f32; 3]; 4];
        shade[0] = [1.0, 0.0, 0.0]; // weight x*y: the top-right corner
        let v = comp_vertices(&g, &shade, None);
        let tr = v[(COMP_GRID_X - 1) as usize]; // u = 1, v = 0
        assert!(tr.color[0] >= 254, "{:?}", tr.color);
        // Top-left: x = -1/128 (the -0.5 px offset) gives a weight of
        // -0.0079: (int)(-2.01) & 0xff = 254, the C++'s D3DCOLOR wrap.
        assert_eq!(v[0].color[0], 254);
    }
}

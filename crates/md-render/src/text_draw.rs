//! The GPU half of the text pass ([`crate::text_pass`] queues the
//! messages): [`Renderer::flush_text`] lays the queue out, keeps each
//! font's atlas texture current and records the draws that
//! [`Renderer::present`](crate::Renderer::present) issues over the
//! composite.
//!
//! Where it draws: `CTextManager::DrawNow` runs inside `BeginScene` after
//! `MyRenderFn` (the whole frame, composite included) and draws into the
//! render target that is current then, the back buffer
//! (pluginshell.cpp:1393-1405; textmgr.cpp:383 saves it, :245 `bRTT` is
//! false without a text surface). So the text goes into the composite
//! intermediate (window-sized, `Rgba8Unorm`) after the
//! composite and before the copy to the caller's view — never into the main
//! textures, so it does not feed back into the next frame.
//!
//! States, per message in queue order (textmgr.cpp:592-622):
//!
//! * Boxes (`pfont == NULL`): `D3DRS_ALPHABLENDENABLE` FALSE (:596), the
//!   colour written as is (alpha too, which the final copy forces to 1),
//!   untextured: [`BlendMode::Off`], `fixed_color`. The box's vertices are at
//!   `-1 + 2 * rect.left / width` etc. (:609-612), i.e. exactly on the
//!   rect's pixel edges, so it covers pixels `[left, right) x [top,
//!   bottom)`. md-render's built-in vertex shaders shift positions by
//!   `D3D9_PIXEL_SHIFT` to reproduce D3D9's pixel-centre rule, so the box
//!   vertices are used unshifted and cover the same pixels.
//! * Glyphs: drawn by `ID3DXFont::DrawTextW` (:605), which is inside D3DX9
//!   and not in this tree. It is an alpha-blended sprite draw
//!   (`SRCALPHA`/`INVSRCALPHA`, the `text` shader's contract) of glyph
//!   coverage modulated by `color`: [`BlendMode::Alpha`], [`FixedMode::Text`].
//! * Sampler: the glyph texels are copied 1:1 to pixels, so
//!   [`SamplerKind::PointClamp`] is exact. textmgr.cpp:664-666 sets
//!   POINT/POINT/POINT, but only for the text-surface blit (:635-668), which
//!   runs only with `fix_slow_text` (a text surface); it does not touch the
//!   glyph draws and is not ported.
//!
//! Half-pixel convention: D3DX's font sprite places glyph vertices at the
//! pixel position minus half a pixel (the usual D3D9 texel-to-pixel
//! correction), so glyph texels land exactly on pixels. The shared vertex
//! shaders add `+0.5` px (F29), so glyph quad corners are built at
//! `pixel - 0.5` and end up on integer pixel edges: pixel centre `x + 0.5`
//! then samples the texel centre `(x - x0 + 0.5) / w` of the atlas.

use crate::blend::BlendMode;
use crate::fixed::{FixedMode, SamplerKind, Topo};
use crate::frame::FrameCtx;
use crate::gpu::Gpu;
use crate::pipelines::{DrawBatch, PipelineKey};
use crate::renderer::Renderer;
use crate::text_pass::{Item, TextQueue, FONT_SLOTS};
use crate::texture::Texture2d;
use crate::vertex::Vertex;
use md_text::{layout, Atlas, GlyphQuad, Layout, Rect};

/// One recorded draw of the text pass, issued by `present`.
#[derive(Clone, Debug)]
pub(crate) enum TextDraw {
    /// A box: a triangle strip of four vertices.
    Box { verts: [Vertex; 4] },
    /// One text message: two triangles per glyph, sampling `tex`.
    Glyphs { tex: Texture2d, verts: Vec<Vertex> },
}

/// Pixel `(x, y)` in a `w`×`h` target as clip space.
fn ndc(x: f32, y: f32, (w, h): (u32, u32)) -> [f32; 3] {
    [
        2.0 * x / w.max(1) as f32 - 1.0,
        1.0 - 2.0 * y / h.max(1) as f32,
        0.0,
    ]
}

/// The box rect as a strip (top-left, top-right, bottom-left,
/// bottom-right) on the pixel edges.
pub(crate) fn box_vertices(rect: Rect, color: [u8; 4], target: (u32, u32)) -> [Vertex; 4] {
    let (l, t, r, b) = (
        rect.left as f32,
        rect.top as f32,
        rect.right as f32,
        rect.bottom as f32,
    );
    [(l, t), (r, t), (l, b), (r, b)].map(|(x, y)| Vertex {
        pos: ndc(x, y, target),
        color,
        ..Vertex::default()
    })
}

/// The glyph quads as a triangle list, corners half a pixel up and left
/// (see the module docs).
pub(crate) fn glyph_vertices(
    quads: &[GlyphQuad],
    color: [u8; 4],
    target: (u32, u32),
) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(quads.len() * 6);
    for q in quads {
        let at = |x: f32, y: f32, u: f32, v: f32| Vertex {
            pos: ndc(x - 0.5, y - 0.5, target),
            color,
            uv: [u, v, 0.0, 0.0],
            ..Vertex::default()
        };
        let tl = at(q.x0, q.y0, q.u0, q.v0);
        let tr = at(q.x1, q.y0, q.u1, q.v0);
        let bl = at(q.x0, q.y1, q.u0, q.v1);
        let br = at(q.x1, q.y1, q.u1, q.v1);
        out.extend_from_slice(&[tl, tr, bl, bl, tr, br]);
    }
    out
}

/// Bring the GPU copy of `atlas` up to date (recreate it when the atlas
/// grew, otherwise upload the changed region) and return it.
fn sync_atlas(gpu: &Gpu, atlas: &mut Atlas, slot: &mut Option<Texture2d>) -> Texture2d {
    let dirty = atlas.take_dirty();
    let (pixels, w, h) = atlas.texture_rgba();
    let stale = slot.as_ref().is_none_or(|t| (t.w, t.h) != (w, h));
    if stale {
        *slot = Some(Texture2d::from_rgba8(gpu, w, h, pixels, false));
    } else if let (Some(d), Some(t)) = (dirty, slot.as_ref()) {
        gpu.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &t.tex,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: d.x,
                    y: d.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: (u64::from(d.y) * u64::from(w) + u64::from(d.x)) * 4,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(d.h),
            },
            wgpu::Extent3d {
                width: d.w,
                height: d.h,
                depth_or_array_layers: 1,
            },
        );
    }
    slot.clone().expect("created above")
}

/// Lay out every text item; UVs are only valid for the atlas generation
/// they were computed in, so if a later layout reset the same font's atlas
/// (it was full), everything is laid out once more against the settled
/// atlases.
fn layout_all(queue: &mut TextQueue, items: &[Item]) -> Vec<Option<Layout>> {
    let mut laid = Vec::new();
    for _ in 0..2 {
        let mut gens = [0u64; FONT_SLOTS];
        laid = items
            .iter()
            .map(|it| match it {
                Item::Text {
                    font,
                    text,
                    rect,
                    flags,
                    ..
                } => {
                    let atlas = queue.atlas_mut(*font);
                    let l = layout(atlas, text, *rect, *flags);
                    gens[*font as usize] = atlas.generation();
                    Some(l)
                }
                Item::Box { .. } => None,
            })
            .collect();
        let settled = items.iter().all(|it| match it {
            Item::Text { font, .. } => queue.atlas_mut(*font).generation() == gens[*font as usize],
            Item::Box { .. } => true,
        });
        if settled {
            break;
        }
    }
    laid
}

impl Renderer {
    /// Lay out and queue `queue`'s messages to be drawn over this frame's
    /// composite by [`Renderer::present`], and empty the queue. May be
    /// called several times per frame; messages draw in call order.
    pub fn flush_text(&mut self, ctx: &mut FrameCtx, queue: &mut TextQueue) {
        let items = queue.take_items();
        if items.is_empty() {
            return;
        }
        let target = self.window;
        let laid = layout_all(queue, &items);
        // Textures first: every used font's atlas is final after layout.
        let mut tex: [Option<Texture2d>; FONT_SLOTS] = Default::default();
        for it in &items {
            if let Item::Text { font, .. } = it {
                let i = *font as usize;
                if tex[i].is_none() {
                    let mut slot = queue.textures[i].take();
                    tex[i] = Some(sync_atlas(&self.gpu, queue.atlas_mut(*font), &mut slot));
                    queue.textures[i] = slot;
                }
            }
        }
        for (it, layout) in items.iter().zip(laid) {
            match (it, layout) {
                (Item::Box { rect, color }, _) => ctx.text.push(TextDraw::Box {
                    verts: box_vertices(*rect, *color, target),
                }),
                (Item::Text { font, color, .. }, Some(l)) => {
                    let verts = glyph_vertices(&l.quads, *color, target);
                    if !verts.is_empty() {
                        let t = tex[*font as usize].clone().expect("synced above");
                        ctx.text.push(TextDraw::Glyphs { tex: t, verts });
                    }
                }
                (Item::Text { .. }, None) => {}
            }
        }
    }

    /// Issue the recorded text draws into the open composite pass.
    pub(crate) fn record_text(
        batch: &mut DrawBatch,
        draws: &[TextDraw],
        format: wgpu::TextureFormat,
    ) {
        let boxes = PipelineKey::new(FixedMode::Color, BlendMode::Off, Topo::TriStrip, format);
        let glyphs = PipelineKey::new(FixedMode::Text, BlendMode::Alpha, Topo::TriList, format);
        for d in draws {
            match d {
                TextDraw::Box { verts } => {
                    batch.draw(boxes, verts, None, SamplerKind::PointClamp);
                }
                TextDraw::Glyphs { tex, verts } => {
                    batch.draw(glyphs, verts, Some(tex), SamplerKind::PointClamp);
                }
            }
        }
    }
}

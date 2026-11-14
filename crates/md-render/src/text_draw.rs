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
//! Dark translucent boxes ([`TextQueue::draw_box`],
//! `CPluginShell::DrawDarkTranslucentBox`, pluginshell.cpp:1815-1855) are
//! drawn immediately in the C++, before `DrawNow` (pluginshell.cpp:1405), so
//! they sit under all of the frame's queued text: `SRCALPHA`/`INVSRCALPHA`
//! (:1828-1830), diffuse `0xD0000000` (:1847), pixel-edge vertices like the
//! textmgr boxes.
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

/// Pixel `(x, y)` in a `w`×`h` target as clip space (glyphs: D3DX's font
/// sprite, not a fixed-function ortho draw; `text.wgsl` does not flip).
fn ndc(x: f32, y: f32, (w, h): (u32, u32)) -> [f32; 3] {
    [
        2.0 * x / w.max(1) as f32 - 1.0,
        1.0 - 2.0 * y / h.max(1) as f32,
        0.0,
    ]
}

/// Pixel `(x, y)` in a `w`×`h` target as the C++'s pre-projection
/// coordinates of a fixed-function draw (`-1 + 2*y/h`, textmgr.cpp:609-612):
/// the fixed vertex shaders' ortho projection (textmgr.cpp:238) flips y to
/// [`ndc`]'s. `DrawDarkTranslucentBox` uses the pixel ortho `(w, h)`
/// projection instead (pluginshell.cpp:1806, vertices at :1840-1848), which
/// puts the same rect on the same pixels.
fn pre_ortho(x: f32, y: f32, (w, h): (u32, u32)) -> [f32; 3] {
    [
        2.0 * x / w.max(1) as f32 - 1.0,
        2.0 * y / h.max(1) as f32 - 1.0,
        0.0,
    ]
}

/// The box rect as a strip (top-left, top-right, bottom-left,
/// bottom-right) on the pixel edges, drawn with `fixed_color` (so in
/// pre-projection coordinates).
pub(crate) fn box_vertices(rect: Rect, color: [u8; 4], target: (u32, u32)) -> [Vertex; 4] {
    let (l, t, r, b) = (
        rect.left as f32,
        rect.top as f32,
        rect.right as f32,
        rect.bottom as f32,
    );
    [(l, t), (r, t), (l, b), (r, b)].map(|(x, y)| Vertex {
        pos: pre_ortho(x, y, target),
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

/// Bring the GPU copy of `atlas` up to date and return it. A new atlas
/// generation (it grew or was reset: every UV changed) gets a fresh
/// texture, so draws recorded earlier this frame keep sampling the texels
/// their UVs were computed for; within one generation glyphs are only
/// added, and the changed region is uploaded in place.
pub(crate) fn sync_atlas(
    gpu: &Gpu,
    atlas: &mut Atlas,
    slot: &mut Option<(Texture2d, u64)>,
) -> Texture2d {
    let dirty = atlas.take_dirty();
    let gen = atlas.generation();
    let (pixels, w, h) = atlas.texture_rgba();
    let stale = slot
        .as_ref()
        .is_none_or(|(t, g)| *g != gen || (t.w, t.h) != (w, h));
    if stale {
        *slot = Some((Texture2d::from_rgba8(gpu, w, h, pixels, false), gen));
    } else if let (Some(d), Some((t, _))) = (dirty, slot.as_ref()) {
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
    slot.as_ref().expect("created above").0.clone()
}

/// Lay out every text item. UVs are only valid for the atlas generation
/// they were computed in: each item records the generation after its own
/// layout, and if any differs from its font's final generation (a later
/// item of the same font grew or reset the atlas), everything is laid out
/// again against the settled atlases (once: the second pass finds every
/// glyph already placed, unless the atlas is too small for the whole set).
fn layout_all(queue: &mut TextQueue, items: &[Item]) -> Vec<Option<Layout>> {
    let mut laid = Vec::new();
    for _ in 0..2 {
        let (l, gens): (Vec<_>, Vec<_>) = items
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
                    (Some(l), Some((*font, atlas.generation())))
                }
                _ => (None, None),
            })
            .unzip();
        laid = l;
        let settled = gens
            .iter()
            .flatten()
            .all(|(font, g)| queue.atlas_mut(*font).generation() == *g);
        if settled {
            break;
        }
    }
    laid
}

impl Renderer {
    /// Lay out and queue `queue`'s messages to be drawn over this frame's
    /// composite by [`Renderer::present`], and empty the queue. May be
    /// called several times per frame; messages draw in call order, except
    /// that [`TextQueue::draw_box`] boxes of every call draw under all the
    /// text. A flush that grows or resets an atlas gives it a new texture,
    /// so earlier flushes' draws stay valid.
    ///
    /// The draws are sized for the window as it is now: do not call
    /// [`Renderer::resize`] between `flush_text` and `present` (the
    /// composite recorded by `begin_frame` has the same constraint).
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
                (Item::DarkBox { rect, color }, _) => ctx.text_under.push(TextDraw::Box {
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
        under: &[TextDraw],
        draws: &[TextDraw],
        format: wgpu::TextureFormat,
    ) {
        let translucent =
            PipelineKey::new(FixedMode::Color, BlendMode::Alpha, Topo::TriStrip, format);
        for d in under {
            if let TextDraw::Box { verts } = d {
                batch.draw(translucent, verts, None, SamplerKind::PointClamp);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use md_text::{FontSpec, TextFlags};

    fn text(font: crate::text_pass::FontId, t: &str, top: i32) -> Item {
        Item::Text {
            font,
            text: t.to_owned(),
            rect: Rect::new(0, top, 4000, top + 100),
            flags: TextFlags::default(),
            color: [255; 4],
        }
    }

    #[test]
    fn layouts_are_redone_when_a_later_item_grows_the_atlas() {
        use crate::text_pass::FontId::Simple;
        let spec = FontSpec {
            face: "Arial".into(),
            size_px: 40,
            bold: false,
            italic: false,
            antialiased: true,
        };
        let mut q = TextQueue::with_bundled_fonts(std::array::from_fn(|_| spec.clone()));
        // Items built without the draw-time layout: the first item is laid out
        // against the 256 px atlas, the second grows it.
        let many: String = (0x21u32..0x7f)
            .chain(0xc0..0x100)
            .filter_map(char::from_u32)
            .collect();
        let items = vec![text(Simple, "Rag", 0), text(Simple, &many, 200)];
        let laid = layout_all(&mut q, &items);
        let Item::Text {
            text: t,
            rect,
            flags,
            ..
        } = &items[0]
        else {
            unreachable!()
        };
        let fresh = layout(q.atlas_mut(Simple), t, *rect, *flags);
        assert!(
            q.atlas_mut(Simple).generation() > 0,
            "the atlas must have grown"
        );
        assert_eq!(laid[0].as_ref().expect("text"), &fresh);
    }
}

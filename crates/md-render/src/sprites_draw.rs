//! Drawing user sprites (`DrawUserSprites`, code/vis_milk2/milkdropfs.cpp:
//! 3432-3777; geometry and state in [`crate::sprites`]).
//!
//! [`Renderer::draw_sprites`] only records what the frame will draw; the
//! frame draws it in [`Renderer::present`], inside the composite pass after
//! the song title animation and before the text (`RenderFrame` calls
//! `ShowSongTitleAnim`, then `DrawUserSprites`, mfs:1123-1131; the text is
//! drawn after `MyRenderFn` returns, pluginshell.cpp:1393-1405). Burnt
//! sprites are drawn into the warp target, the main texture this frame
//! wrote (`m_lpVS[1]`, 3508-3521), after the composite has read it, so the
//! burn shows from the next frame on, as in the C++, where the composite
//! ran before `DrawUserSprites`.
//!
//! The sampler is stage 0's, which the sprites inherit: after a shader
//! composite `RestoreShaderParams` left linear/wrap (mfs:4474, 3797-3806),
//! after the fixed composite it is what that path left (the last warp
//! pass's address mode; the blur passes' clamp), as for the composite's own
//! fixed draws. Neither `DrawUserSprites` (its wrap lines are commented out,
//! 3443-3444) nor `ShowSongTitleAnim` sets it.

use crate::blend::BlendMode;
use crate::dynbuf::DynamicVertexBuffer;
use crate::fixed::{FixedMode, SamplerKind, Topo};
use crate::frame::FrameCtx;
use crate::gpu::Gpu;
use crate::pipelines::{DrawBatch, PipelineCache, PipelineKey};
use crate::renderer::Renderer;
use crate::sprites::{sprite_quads, sprite_state, SpriteFrame};
use crate::texture::Texture2d;
use crate::vertex::Vertex;

/// One sprite recorded in a frame: its texture and quads.
#[derive(Clone, Debug)]
pub(crate) struct SpriteDraw {
    tex: Texture2d,
    mode: FixedMode,
    blend: BlendMode,
    screen: [Vertex; 4],
    burn: Option<[Vertex; 4]>,
}

impl SpriteDraw {
    fn draw(
        &self,
        batch: &mut DrawBatch,
        quad: &[Vertex; 4],
        fmt: wgpu::TextureFormat,
        s: SamplerKind,
    ) {
        let key = PipelineKey::new(self.mode, self.blend, Topo::TriStrip, fmt);
        batch.draw(key, quad, Some(&self.tex), s);
    }
}

impl Renderer {
    /// Record `sprites`, in order, for this frame (see the module docs);
    /// returns how many were drawn. A sprite whose texture no longer exists
    /// ([`crate::TextureStore::kill_sprite`]) is skipped, as the C++ skips
    /// slots without a surface (mfs:3474).
    pub fn draw_sprites(&mut self, ctx: &mut FrameCtx, sprites: &[SpriteFrame]) -> usize {
        let before = ctx.sprites.len();
        for s in sprites {
            let Some(tex) = self.textures().texture(s.texture) else {
                continue;
            };
            let quads = sprite_quads(s, (tex.w, tex.h), self.window);
            let (mode, blend) = sprite_state(s.blendmode);
            ctx.sprites.push(SpriteDraw {
                tex: tex.clone(),
                mode,
                blend,
                screen: quads.screen,
                burn: quads.burn,
            });
        }
        ctx.sprites.len() - before
    }

    /// Issue the frame's sprites into the open composite pass.
    pub(crate) fn record_sprites(
        batch: &mut DrawBatch,
        draws: &[SpriteDraw],
        fmt: wgpu::TextureFormat,
        sampler: SamplerKind,
    ) {
        for d in draws {
            d.draw(batch, &d.screen, fmt, sampler);
        }
    }

    /// The burn draws, into the warp target after the composite.
    pub(crate) fn record_sprite_burns(
        res: (&Gpu, &mut PipelineCache, &mut DynamicVertexBuffer),
        target: &Texture2d,
        enc: &mut wgpu::CommandEncoder,
        draws: &[SpriteDraw],
        sampler: SamplerKind,
    ) {
        if draws.iter().all(|d| d.burn.is_none()) {
            return;
        }
        let mut batch = DrawBatch::begin(res.0, res.1, res.2, enc, target, wgpu::LoadOp::Load);
        for d in draws {
            if let Some(q) = &d.burn {
                d.draw(&mut batch, q, target.format, sampler);
            }
        }
        batch.finish();
    }
}

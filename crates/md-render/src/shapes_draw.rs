//! `DrawCustomShapes`' GPU side: every shape instance of `FrameInput`
//! drawn into the warp target ([`crate::shapes`] builds the vertices).

use crate::blend::BlendMode;
use crate::fixed::{FixedMode, SamplerKind, Topo};
use crate::frame::FrameInput;
use crate::mesh::mesh_aspect;
use crate::pipelines::{DrawBatch, PipelineKey};
use crate::renderer::Renderer;
use crate::shapes::{build_shape, ShapeFrameVars, ShapeView};

impl Renderer {
    /// `DrawCustomShapes` (milkdropfs.cpp:2215-2395): the new preset's
    /// shapes, then, while `input.blending()`, the old preset's (`rep` 0 and
    /// 1, 2217-2223), with `alpha_mult` = `blend_t` and `1 - blend_t`
    /// (2224-2226). They draw into the warp target (`m_lpVS[1]`) and a
    /// textured shape samples the warp *source* (`m_lpVS[0]`, 2330), which
    /// is a different texture, so there is no feedback hazard. `sampler` is
    /// the stage-0 state the draw inherits (nothing in the function sets
    /// it; see the frame loop). There is no alpha threshold: every instance
    /// is drawn.
    pub(crate) fn draw_shapes(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        input: &FrameInput,
        sampler: SamplerKind,
    ) {
        let blending = input.blending();
        let mut reps: Vec<(&[ShapeFrameVars], f32)> = vec![(input.shapes, 1.0)];
        if blending {
            reps[0].1 = input.blend_t;
            reps.push((input.old_shapes, 1.0 - input.blend_t));
        }
        if reps.iter().all(|(s, _)| s.is_empty()) {
            return;
        }
        let (tex_w, tex_h) = (self.settings.tex_w, self.settings.tex_h);
        let (_, aspect_y) = mesh_aspect(tex_w, tex_h);
        let src = &self.vs[Renderer::other_vs(self.vs_target)];
        let target = &self.vs[self.vs_target];
        let format = target.format;
        let mut batch = DrawBatch::begin(
            &self.gpu,
            &mut self.pipelines,
            &mut self.dynbuf,
            enc,
            target,
            wgpu::LoadOp::Load,
        );
        let (mut fill, mut border) = (Vec::new(), Vec::new());
        for (shapes, alpha_mult) in reps {
            let view = ShapeView {
                aspect_y,
                tex: (tex_w, tex_h),
                alpha_mult,
            };
            for v in shapes {
                fill.clear();
                border.clear();
                let geo = build_shape(v, &view, &mut fill, &mut border);
                let blend = if geo.additive {
                    BlendMode::AlphaAdditive
                } else {
                    BlendMode::Alpha
                };
                let (mode, tex) = if geo.textured {
                    (FixedMode::TexModulate, Some(src))
                } else {
                    (FixedMode::Color, None)
                };
                let key = PipelineKey::new(mode, blend, Topo::TriFan, format);
                batch.draw(key, &fill, tex, sampler);
                // A LINESTRIP of `sides` segments per copy (2360).
                let key = PipelineKey::new(FixedMode::Color, blend, Topo::LineStrip, format);
                for copy in border.chunks(geo.sides + 1) {
                    batch.draw(key, copy, None, SamplerKind::LinearClamp);
                }
            }
        }
        batch.finish();
    }
}

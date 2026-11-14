//! `DrawCustomWaves`' GPU side: every custom wave of `FrameInput` drawn
//! into the warp target ([`crate::custom_waves`] builds the vertices).

use crate::blend::BlendMode;
use crate::custom_waves::{build_custom_wave, CustomWaveDraw, CustomWaveView};
use crate::fixed::{FixedMode, SamplerKind};
use crate::frame::FrameInput;
use crate::mesh::mesh_aspect;
use crate::pipelines::{DrawBatch, PipelineKey};
use crate::renderer::Renderer;

impl Renderer {
    /// `DrawCustomWaves` (milkdropfs.cpp:2496-2680): the new preset's waves,
    /// then, while `input.blending()`, the old preset's (`rep` 0 and 1,
    /// 2507-2510), with `alpha_mult` = `blend_t` and `1 - blend_t`
    /// (2512-2513). Called after the shapes and before the built-in wave
    /// (1054-1056). Each wave is a line strip (or points) with
    /// `SRCALPHA` / `ONE` when additive, else `INVSRCALPHA` (2646-2648),
    /// untextured.
    pub(crate) fn draw_custom_waves(&mut self, enc: &mut wgpu::CommandEncoder, input: &FrameInput) {
        let mut reps: Vec<(&[CustomWaveDraw], f32)> = vec![(input.custom_waves, 1.0)];
        if input.blending() {
            reps[0].1 = input.blend_t;
            reps.push((input.old_custom_waves, 1.0 - input.blend_t));
        }
        if reps.iter().all(|(w, _)| w.is_empty()) {
            return;
        }
        let tex = (self.settings.tex_w, self.settings.tex_h);
        let aspect = mesh_aspect(tex.0, tex.1);
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
        let mut verts = Vec::new();
        for (waves, alpha_mult) in reps {
            let view = CustomWaveView {
                aspect,
                tex,
                alpha_mult,
            };
            for w in waves {
                verts.clear();
                let geo = build_custom_wave(w, &view, &mut verts);
                if geo.copies == 0 {
                    continue;
                }
                let blend = if w.additive {
                    BlendMode::AlphaAdditive
                } else {
                    BlendMode::Alpha
                };
                let key = PipelineKey::new(FixedMode::Color, blend, geo.topo, format);
                batch.set_point_size(geo.point_size);
                for copy in verts.chunks(geo.verts_per_copy) {
                    batch.draw(key, copy, None, SamplerKind::LinearClamp);
                }
                // `ptsize = 1.0f` again after the wave (2669-2671).
                batch.set_point_size(1.0);
            }
        }
        batch.finish();
    }
}

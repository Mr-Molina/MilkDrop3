//! `DrawWave`'s GPU side: the built-in wave drawn into the warp target
//! ([`crate::wave`] builds the vertices).

use crate::blend::BlendMode;
use crate::fixed::{FixedMode, SamplerKind};
use crate::frame::FrameInput;
use crate::mesh::mesh_aspect;
use crate::pipelines::{DrawBatch, PipelineKey};
use crate::renderer::Renderer;
use crate::wave::{build_wave, WaveBlend, WaveParams, WaveView, NUM_WAVES};

impl Renderer {
    /// `DrawWave` into the warp target (the C++'s `m_lpVS[1]`, a single
    /// level, so nothing to regenerate): the wave of `input.vars` and
    /// `input.sound`, blended with the old preset's mode while
    /// `input.blending()`. Records nothing when the wave is not drawn
    /// (alpha below 0.004, milkdropfs.cpp:3178).
    pub(crate) fn draw_wave(&mut self, enc: &mut wgpu::CommandEncoder, input: &FrameInput) {
        let (tex_w, tex_h) = (self.settings.tex_w, self.settings.tex_h);
        let blend = input.blending().then(|| WaveBlend {
            old_mode: input.old_preset.unwrap_or(input.preset).wave_mode,
            progress: input.blend_t,
        });
        let (ax, ay) = mesh_aspect(tex_w, tex_h);
        let view = WaveView {
            aspect: (ax, ay),
            time: input.now,
            tex: (tex_w, tex_h),
        };
        let params = WaveParams::from_vars(input.vars);
        // (int)wave_mode % NUM_WAVES (2770); `as` saturates like a sane cast.
        let mode = (input.vars.wave_mode as i32) % NUM_WAVES;
        let mut verts = Vec::new();
        let geo = build_wave(mode, blend, input.sound, &params, &view, &mut verts);
        if geo.draws.is_empty() {
            return;
        }
        let target = &self.vs[self.vs_target];
        let blend_mode = if geo.additive {
            BlendMode::AlphaAdditive
        } else {
            BlendMode::Alpha
        };
        let key = PipelineKey::new(FixedMode::Color, blend_mode, geo.topo, target.format);
        let mut batch = DrawBatch::begin(
            &self.gpu,
            &mut self.pipelines,
            &mut self.dynbuf,
            enc,
            target,
            wgpu::LoadOp::Load,
        );
        // D3DRS_POINTSIZE is left at the default 1.0 here (fat dots are
        // offset copies, 3213-3262).
        batch.set_point_size(1.0);
        for r in geo.draws {
            batch.draw(key, &verts[r], None, SamplerKind::LinearClamp);
        }
        batch.finish();
    }
}

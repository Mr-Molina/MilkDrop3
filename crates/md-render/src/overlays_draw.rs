//! The GPU side of the overlays ([`crate::overlays`] builds the vertices):
//! the motion vectors drawn into the warp *source* before the warp pass,
//! and `DrawSprites`' darken-centre fan and borders drawn into the warp
//! target after the waves.

use crate::blend::BlendMode;
use crate::fixed::{FixedMode, SamplerKind, Topo};
use crate::frame_vars::PerFrameVars;
use crate::mesh::mesh_aspect;
use crate::overlays::{
    build_borders, build_darken_center, build_motion_vectors, Border, MotionMesh, MotionVectorVars,
};
use crate::pipelines::{DrawBatch, PipelineKey};
use crate::renderer::Renderer;

impl Renderer {
    /// `DrawMotionVectors` (milkdropfs.cpp:1156-1305), called at 965 with
    /// `m_lpVS[0]` (the warp source, [`Renderer::other_vs`] of the target)
    /// as the render target, before the first-frame clear (971-981) and the
    /// warp pass: the lines are then warped with the rest of the image.
    /// Uses the previous frame's mesh UVs (see [`MotionMesh`]), one draw of
    /// the blended `vars` (`SRCALPHA`, `INVSRCALPHA`, 1226-1227).
    pub(crate) fn draw_motion_vectors(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        vars: &PerFrameVars,
    ) {
        let mv = MotionVectorVars::from_vars(vars);
        let tex = (self.settings.tex_w, self.settings.tex_h);
        let mesh = MotionMesh {
            grid: self.mesh.grid(),
            uvs: &self.prev_uvs,
            tex,
        };
        let mut verts = Vec::new();
        if build_motion_vectors(&mv, &mesh, &mut verts) == 0 {
            return;
        }
        let target = &self.vs[Renderer::other_vs(self.vs_target)];
        let key = PipelineKey::new(
            FixedMode::Color,
            BlendMode::Alpha,
            Topo::LineList,
            target.format,
        );
        let mut batch = DrawBatch::begin(
            &self.gpu,
            &mut self.pipelines,
            &mut self.dynbuf,
            enc,
            target,
            wgpu::LoadOp::Load,
        );
        batch.draw(key, &verts, None, SamplerKind::LinearClamp);
        batch.finish();
    }

    /// `DrawSprites`' fixed part (milkdropfs.cpp:3300-3405), after the
    /// built-in wave (1056-1057), into the warp target: the `darken_center`
    /// fan when `var_pf_darken_center` is non-zero (3310; a NaN counts as
    /// non-zero), then the outer and inner borders. All `SRCALPHA`,
    /// `INVSRCALPHA` (3313-3314, 3355-3356), untextured.
    pub(crate) fn draw_sprites_fixed(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        vars: &PerFrameVars,
    ) {
        let v = vars;
        let (_, aspect_y) = mesh_aspect(self.settings.tex_w, self.settings.tex_h);
        let mut fan = Vec::new();
        if v.darken_center != 0.0 {
            build_darken_center(aspect_y, &mut fan);
        }
        let mut quads = Vec::new();
        build_borders(Border::outer(v), Border::inner(v), &mut quads);
        if fan.is_empty() && quads.is_empty() {
            return;
        }
        let target = &self.vs[self.vs_target];
        let key = PipelineKey::new(
            FixedMode::Color,
            BlendMode::Alpha,
            Topo::TriFan,
            target.format,
        );
        let mut batch = DrawBatch::begin(
            &self.gpu,
            &mut self.pipelines,
            &mut self.dynbuf,
            enc,
            target,
            wgpu::LoadOp::Load,
        );
        batch.draw(key, &fan, None, SamplerKind::LinearClamp);
        // One fan of 4 vertices per quad (3389).
        for quad in quads.chunks(4) {
            batch.draw(key, quad, None, SamplerKind::LinearClamp);
        }
        batch.finish();
    }

    /// Remember this frame's mesh UVs for the next frame's motion vectors
    /// (the C++'s `m_verts` outlives the frame).
    pub(crate) fn snapshot_mesh_uvs(&mut self) {
        self.prev_uvs.clear();
        self.prev_uvs.extend_from_slice(self.mesh.uvs());
    }
}

//! The frame loop (A.3 `begin_frame` / `present`, Task 5.7), in
//! `RenderFrame`'s order (code/vis_milk2/milkdropfs.cpp:751-1137@6f7cd5d):
//!
//! 1. motion vectors into the warp source (Milestone 6 hook, 6.5);
//! 2. on the first frame after a resize (`m_nFramesSinceResize == 0`,
//!    971-981) clear the warp source (`m_lpVS[0]`, here
//!    [`Renderer::other_vs`]`(`[`Renderer::warp_target`]`)`) to black;
//! 3. the warp pass ([`Renderer::warp_pass`]) into the warp target, with
//!    (single-level, as in the C++: no mip generation anywhere in the frame);
//! 4. blur passes ([`Renderer::blur_passes`]), custom shapes
//!    ([`Renderer::draw_shapes`], 6.3), custom waves
//!    ([`Renderer::draw_custom_waves`], 6.4), the built-in
//!    wave ([`Renderer::draw_wave`], 6.2), sprites incl. `darken_center` and borders (Milestone 6), the
//!    song title burn-in (6.7) — no-op hooks here, drawn into the warp
//!    target (no mips to regenerate: the main textures have one level);
//! 5. ([`Renderer::present`]) the composite into the caller's target
//!    ([`crate::comp`]), then title/user sprites on top (Milestone 6
//!    hooks), one submit, and the ping-pong swap ([`Renderer::swap_vs`]).
//!
//! Uniforms go through `Queue::write_buffer`, so each preset program is
//! written once per submission: the old and new presets of a blend must be
//! distinct [`PresetGpu`]s (each has its own buffers), and one frame is one
//! submit (ledger Task 5.6 note). Warp and composite programs are distinct
//! objects, so both run in the same submit.
//!
//! [`FrameInput`] is A.3's struct plus F38's `vars`/`old_vars`/`rot` and
//! what the ledger Task 5.6 note adds (the old preset's rotations and
//! `rand_preset`, per-preset start times), plus the compiled shaders. The
//! window size the note asks for is the renderer's own
//! ([`Renderer::resize`]): both blend passes read it from there, so it
//! cannot disagree with the composite target. A.3's `q` and `vert_blend`
//! are kept but not read: the shaders read the post-equation `vars.q`
//! (F38), and the blend pattern is applied by md-core when it computes the
//! renderer's [`WarpMesh`](crate::WarpMesh) UVs ([`Renderer::mesh_mut`]).

use crate::comp::{comp_vertices, cull_indices, hue_shades, plan_comp_passes, CompFrame, CompPass};
use crate::comp_fixed::{fixed_comp_draws, EchoBlend, FixedCompParams};
use crate::custom_waves::CustomWaveDraw;
use crate::fixed::SamplerKind;
use crate::frame_vars::PerFrameVars;
use crate::mesh::VertInfo;
use crate::preset_gpu::PresetGpu;
use crate::renderer::Renderer;
use crate::shader_params::{shader_uniforms, FrameShaderInputs, PresetShaderInputs, RotParams};
use crate::shapes::ShapeFrameVars;
use crate::uniforms::VsParams;
use crate::warp::{plan_warp_passes, snap_point, WarpPreset};
use md_analysis::SoundData;
use md_hlsl::PsVersion;
use md_preset::Preset;

/// Everything one frame draws (A.3 + F38 + the ledger Task 5.6 note).
///
/// Blending is on when `old_gpu` is `Some`; `old_preset`, `old_vars` and
/// `old_rot` then describe the blend-from preset (each falls back to the
/// current one's when `None`).
#[derive(Clone, Copy, Debug)]
pub struct FrameInput<'a> {
    /// `GetTime() - m_fStartTime` (shader `time`, rotations, `_c8.._c11`).
    pub time: f32,
    pub frame: u32,
    pub fps: f32,
    /// Progress through the current preset (`_c2.w`).
    pub progress: f32,
    /// This frame's analysis (`_c3` = `imm_rel`, `_c4` = `avg_rel`).
    pub sound: &'a SoundData,
    pub preset: &'a Preset,
    pub old_preset: Option<&'a Preset>,
    /// `m_fBlendProgress` (0..1 while blending).
    pub blend_t: f32,
    /// A.3; not read (the shaders read `vars.q`, F38).
    pub q: [f32; 32],
    /// `m_rand_frame`.
    pub rand_frame: [f32; 4],
    /// The current preset's `m_rand_preset`.
    pub rand_preset: [f32; 4],
    /// A.3; not read (md-core applies it in [`crate::WarpMesh::compute_uvs`]).
    pub vert_blend: &'a [VertInfo],
    /// F38: the current preset's post-equation values (blended while
    /// blending, as `RunPerFrameEquations` leaves them).
    pub vars: &'a PerFrameVars,
    /// F38: the old preset's (its q/blur values for blend pass 0).
    pub old_vars: Option<&'a PerFrameVars>,
    /// F38: the current preset's rotations.
    pub rot: &'a [RotParams; 24],
    /// The old preset's rotations and `m_rand_preset`.
    pub old_rot: Option<&'a [RotParams; 24]>,
    pub old_rand_preset: [f32; 4],
    /// `GetTime()` (hue shader, echo blend), and both presets'
    /// `GetPresetStartTime()` (for `_c2.x`).
    pub now: f32,
    pub preset_start_time: f32,
    pub old_preset_start_time: f32,
    /// The compiled shaders of the current and the old preset.
    pub gpu: &'a PresetGpu,
    pub old_gpu: Option<&'a PresetGpu>,
    /// Task 6.3: every enabled custom shape instance of the current preset
    /// after its per-frame program ran (shape 0..4, instances in order),
    /// as `DrawCustomShapes` leaves them; filled by md-core.
    pub shapes: &'a [ShapeFrameVars],
    /// The old preset's, drawn only while blending.
    pub old_shapes: &'a [ShapeFrameVars],
    /// Task 6.4: every enabled custom wave of the current preset with at
    /// least the samples `DrawCustomWaves` needs, after its per-frame and
    /// per-point programs ran (wave 0..4); filled by md-core (F38).
    pub custom_waves: &'a [CustomWaveDraw],
    /// The old preset's, drawn only while blending.
    pub old_custom_waves: &'a [CustomWaveDraw],
}

impl<'a> FrameInput<'a> {
    /// A non-blending frame at time 0 of `preset`; set the other fields
    /// with struct update syntax.
    pub fn new(
        preset: &'a Preset,
        gpu: &'a PresetGpu,
        vars: &'a PerFrameVars,
        rot: &'a [RotParams; 24],
        sound: &'a SoundData,
    ) -> FrameInput<'a> {
        FrameInput {
            time: 0.0,
            frame: 0,
            fps: 60.0,
            progress: 0.0,
            sound,
            preset,
            old_preset: None,
            blend_t: 0.0,
            q: [0.0; 32],
            rand_frame: [0.0; 4],
            rand_preset: [0.0; 4],
            vert_blend: &[],
            vars,
            old_vars: None,
            rot,
            old_rot: None,
            old_rand_preset: [0.0; 4],
            now: 0.0,
            preset_start_time: 0.0,
            old_preset_start_time: 0.0,
            gpu,
            old_gpu: None,
            shapes: &[],
            old_shapes: &[],
            custom_waves: &[],
            old_custom_waves: &[],
        }
    }

    /// Whether a preset blend is running.
    pub fn blending(&self) -> bool {
        self.old_gpu.is_some()
    }

    fn new_shader(&self) -> PresetShaderInputs<'a> {
        PresetShaderInputs {
            vars: self.vars,
            rot: self.rot,
            rand_preset: self.rand_preset,
            time_since_preset_start: self.now - self.preset_start_time,
        }
    }

    fn old_shader(&self) -> PresetShaderInputs<'a> {
        PresetShaderInputs {
            vars: self.old_vars.unwrap_or(self.vars),
            rot: self.old_rot.unwrap_or(self.rot),
            rand_preset: self.old_rand_preset,
            time_since_preset_start: self.now - self.old_preset_start_time,
        }
    }
}

/// A frame recorded by [`Renderer::begin_frame`], finished by
/// [`Renderer::present`]. Dropping it discards the frame (nothing is
/// submitted and the ping-pong textures are not swapped).
pub struct FrameCtx<'a> {
    encoder: wgpu::CommandEncoder,
    comp: CompFrame<'a>,
}

impl std::fmt::Debug for FrameCtx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameCtx")
            .field("comp", &self.comp)
            .finish()
    }
}

impl Renderer {
    /// Record steps 1-4 of the frame (see the module docs) and prepare the
    /// composite. [`Renderer::mesh_mut`] must hold this frame's UVs.
    pub fn begin_frame<'a>(&mut self, input: &FrameInput<'a>) -> FrameCtx<'a> {
        let blending = input.blending();
        let progs = [&input.gpu.warp, &input.gpu.comp]
            .into_iter()
            .chain(input.old_gpu.map(|g| &g.warp))
            .chain(input.old_gpu.map(|g| &g.comp))
            .flatten();
        for p in progs {
            self.rebind(p);
        }
        // One dynamic vertex buffer frame: every fixed-function draw of this
        // frame (Milestone 6 hooks, the composite) writes after this.
        self.dynbuf.begin_frame();
        let mut encoder =
            self.gpu
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("md-render frame"),
                });

        // 1. Motion vectors (Milestone 6).
        // 2. First frame after a resize: clear the old VS.
        if self.frames_since_resize == 0 {
            let src = &self.vs[Renderer::other_vs(self.vs_target)];
            for level in 0..src.mips {
                let view = src.level_view(level);
                drop(crate::comp::clear_pass(
                    &mut encoder,
                    "md-render clear VS",
                    &view,
                ));
            }
        }

        // 3. The warp pass.
        let frame = FrameShaderInputs {
            time: input.time,
            fps: input.fps,
            frame: input.frame,
            progress: input.progress,
            imm_rel: input.sound.imm_rel,
            avg_rel: input.sound.avg_rel,
            rand_frame: input.rand_frame,
            window: self.window,
        };
        let new = WarpPreset {
            gpu: input.gpu,
            shader: input.new_shader(),
        };
        let old = input.old_gpu.map(|gpu| WarpPreset {
            gpu,
            shader: input.old_shader(),
        });
        // warp_pass borrows the renderer mutably and the mesh shared: move
        // the mesh out for the call (the 1×1 stand-in is never drawn).
        let mesh = std::mem::replace(&mut self.mesh, crate::WarpMesh::new(1, 1, 1.0, 1.0));
        self.warp_pass(&mut encoder, &frame, &new, old.as_ref(), &mesh);
        self.mesh = mesh;

        // The stage-0 address mode the fixed composite inherits: the last
        // warp pass's (WarpedBlit_NoShaders sets it from `wrap`,
        // milkdropfs.cpp:1907; the shader path's RestoreShaderParams resets
        // it to wrap, 3797-3806). The blur passes (6.1) set clamp.
        let old_comp = input.old_gpu.is_some_and(|g| g.comp.is_some());
        let new_comp = input.gpu.comp.is_some();
        let snap = snap_point(blending, old_comp, new_comp);
        let old_warp = input.old_gpu.is_some_and(|g| g.warp.is_some());
        let last_warp = plan_warp_passes(blending, old_warp, input.gpu.warp.is_some());
        let mut fixed_sampler = match last_warp.last().and_then(|p| p.shader) {
            None if input.vars.wrap <= f64::from(snap) => SamplerKind::LinearClamp,
            _ => SamplerKind::LinearWrap,
        };

        // 4. Blur passes (milkdropfs.cpp:1051: `m_nMaxPSVersion > 0`); the
        //    highest level any drawn shader samples (the old preset's too
        //    while blending). They leave stage 0 clamped, which the
        //    no-shader composite inherits. They write only the blur
        //    textures, and main has a single level, so hooks
        //    that draw into main (6.2-6.5, 6.7) need no mip regeneration.
        let highest = input
            .gpu
            .highest_blur()
            .max(input.old_gpu.map_or(0, PresetGpu::highest_blur));
        if self.settings.max_ps != PsVersion::None && highest > 0 {
            self.blur_passes(&mut encoder, input.vars, highest);
            fixed_sampler = SamplerKind::LinearClamp;
        }
        // Custom shapes (6.3, DrawCustomShapes, 1054; they inherit the
        // stage-0 sampler state, `fixed_sampler`), custom waves (6.4,
        // DrawCustomWaves, 1055). Then the built-in wave (DrawWave, after
        // them so it draws over them, 1054-1056), into the warp target.
        self.draw_shapes(&mut encoder, input, fixed_sampler);
        self.draw_custom_waves(&mut encoder, input);
        self.draw_wave(&mut encoder, input);
        // Sprites/darken_center/borders (6.5), title burn-in (6.7):
        // Milestone 6.

        let comp = self.prepare_comp(input, &frame, snap, fixed_sampler);
        FrameCtx { encoder, comp }
    }

    /// The composite's vertex data, plans and uniforms
    /// (`ShowToUser_Shaders` / `ShowToUser_NoShaders`).
    fn prepare_comp<'a>(
        &mut self,
        input: &FrameInput<'a>,
        frame: &FrameShaderInputs,
        snap: f32,
        fixed_sampler: SamplerKind,
    ) -> CompFrame<'a> {
        let blending = input.blending();
        let tex = (self.settings.tex_w, self.settings.tex_h);
        let old_comp = input.old_gpu.and_then(|g| g.comp.as_ref());
        let plans = plan_comp_passes(blending, old_comp.is_some(), input.gpu.comp.is_some());
        let mut out = CompFrame {
            passes: Vec::new(),
            fixed: Vec::new(),
            fixed_sampler,
            vertices: Vec::new(),
            indices: Vec::new(),
            ranges: Vec::new(),
        };
        if plans.iter().any(|p| matches!(p, CompPass::Shader { .. })) {
            let shade = hue_shades(input.now, self.rand_start);
            out.vertices = comp_vertices(&self.comp_grid, &shade, blending.then_some(&self.mesh));
        }
        for plan in &plans {
            let start = out.indices.len() as u32;
            let prog = match *plan {
                CompPass::Fixed => {
                    out.fixed = self.fixed_comp(input, snap);
                    None
                }
                CompPass::Shader { old, cull, .. } => {
                    let (prog, shader) = if old {
                        (old_comp, input.old_shader())
                    } else {
                        (input.gpu.comp.as_ref(), input.new_shader())
                    };
                    let prog = prog.expect("planned from comp.is_some()");
                    let u = shader_uniforms(frame, &shader, tex, &mut self.rng);
                    prog.write_uniforms(&self.gpu, &u);
                    prog.write_vs_params(
                        &self.gpu,
                        VsParams::for_target(self.window.0, self.window.1),
                    );
                    let idx = cull_indices(&out.vertices, &self.comp_grid.indices, cull);
                    out.indices.extend(idx);
                    Some(prog)
                }
            };
            out.passes.push((*plan, prog));
            out.ranges.push(start..out.indices.len() as u32);
        }
        out
    }

    /// The no-shader composite's quads for this frame.
    fn fixed_comp(&self, input: &FrameInput, snap: f32) -> Vec<crate::comp_fixed::FixedCompDraw> {
        let v = input.vars;
        let p = input.preset;
        let blend = input.blending().then(|| {
            let t = &p.transition;
            EchoBlend {
                progress: input.blend_t,
                snap,
                alpha_now: p.video_echo_alpha.eval(input.now),
                alpha_old: t.video_echo_alpha_old,
                orient: p.video_echo_orientation,
                orient_old: t.video_echo_orientation_old,
            }
        });
        fixed_comp_draws(&FixedCompParams {
            window: self.window,
            tex: (self.settings.tex_w, self.settings.tex_h),
            now: input.now,
            rand_start: self.rand_start,
            shader_amount: v.shader,
            echo_zoom: v.echo_zoom,
            echo_alpha: v.echo_alpha,
            echo_orient: v.echo_orient,
            gamma: v.gamma,
            brighten: v.brighten,
            darken: v.darken,
            solarize: v.solarize,
            invert: v.invert,
            blend,
        })
    }

    /// Step 5: composite into `target` (any non-sRGB colour format; its
    /// texture's size), submit the frame and swap the ping-pong textures.
    ///
    /// # Panics
    /// If `target`'s texture format is sRGB (the C++ back buffer is not
    /// gamma-encoded).
    pub fn present(&mut self, ctx: FrameCtx, target: &wgpu::TextureView) {
        let FrameCtx { mut encoder, comp } = ctx;
        self.record_comp(&mut encoder, &comp, target);
        // Title animation and user sprites (Milestone 6) go here.
        self.gpu.queue().submit([encoder.finish()]);
        self.swap_vs();
        self.frames_since_resize = self.frames_since_resize.saturating_add(1);
    }
}

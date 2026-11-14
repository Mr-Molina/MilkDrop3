//! The song title texture and its animation on the GPU.
//!
//! * [`Renderer::render_title_texture`] is `RenderStringToTitleTexture`
//!   (milkdropfs.cpp:230-458, song-title branch :399-448): the text is drawn
//!   white on black into a texture `max(tex_w, tex_h)` wide and a quarter
//!   as high (plugin.cpp:2177-2178; clamped to the device limit here, where
//!   the C++ retries smaller sizes when allocation fails, :2192-2208), with
//!   the `songtitle` font (F43) at `nSize * title_w / 256` px, at least 6
//!   (plugin.cpp:2228-2229, the "double size" font), centred on the
//!   texture's middle row (:439-447). A title wider than the texture is cut
//!   to `len * 0.91 * title_w / width` characters plus `...` when it has
//!   more than 8 (:417-436). The glyphs and layout are 6.6's
//!   ([`md_text::layout`], the glyph vertices of [`crate::text_draw`]) with
//!   their own atlas for the double-size font. The font is always
//!   anti-aliased: the C++ creates the D3DX font with `ANTIALIASED_QUALITY`
//!   whatever the setting (plugin.cpp:2247).
//! * The animation ([`crate::title_mesh`]) is drawn twice per frame in
//!   `RenderFrame`'s order (mfs:1059-1067, 1123-1129): once into the main
//!   texture (so the next frame's warp carries it) on the frame its
//!   progress reaches 1 ("burn it into the VS"), after `DrawSprites`' fixed
//!   draws and before the composite, and on every frame while it runs, over
//!   the composite and under the text pass, as [`Renderer::draw_title_anim`]
//!   (called by [`Renderer::begin_frame`]).
//! * The song title's launch (`LaunchSongTitleAnim`, plugin.cpp:8546-8565)
//!   is [`Renderer::start_song_title`]; md-core calls it with the clock
//!   it passes as [`FrameInput::now`](crate::FrameInput) (`GetTime()`).
//!   Custom messages (7.8) set [`Renderer::supertext_mut`] themselves.
//!
//! Draw states: `D3DRS_ALPHABLENDENABLE` TRUE; the shadow pass is `ZERO`,
//! `INVSRCCOLOR`, the text pass `ONE`, `ONE` (:4490-4494, 4698-4706); no
//! texture stage state is set, so the standard `MODULATE(DIFFUSE, TEXTURE)`
//! / `SELECTARG1 DIFFUSE` applies ([`FixedMode::TexModulate`]). The sampler
//! is linear/clamp (the C++ inherits whatever the previous draw left; the
//! mesh stays inside the texture and its edges are black).

use crate::blend::BlendMode;
use crate::cast::c_int;
use crate::comp::COMP_FORMAT;
use crate::fixed::{FixedMode, SamplerKind, Topo};
use crate::frame::FrameCtx;
use crate::mesh::mesh_aspect;
use crate::pipelines::{DrawBatch, PipelineKey};
use crate::renderer::Renderer;
use crate::text_draw::{glyph_vertices, sync_atlas};
use crate::texture::Texture2d;
use crate::title_mesh::{
    title_font_px, title_passes, MeshInput, SuperText, TitlePlan, SONG_TITLE_DURATION,
};
use crate::vertex::Vertex;
use md_text::{fonts, layout, Align, Atlas, FontSpec, Rect, TextFlags};

/// The `songtitle` font and the fonts it may use.
#[derive(Clone)]
struct TitleFont {
    spec: FontSpec,
    bundled_only: bool,
}

/// The double-size font's atlas (rebuilt when the title texture size
/// changes) and its GPU copy.
struct TitleAtlas {
    atlas: Atlas,
    px: u32,
    tex: Option<(Texture2d, u64)>,
}

/// The renderer's title state (`m_lpDDSTitle`, `m_supertext`,
/// `m_d3dx_title_font_doublesize`).
pub(crate) struct TitleState {
    st: SuperText,
    /// `m_supertext.szTextW`.
    text: String,
    /// `m_supertext.bRedrawSuperText`.
    redraw: bool,
    tex: Option<Texture2d>,
    font: Option<TitleFont>,
    atlas: Option<TitleAtlas>,
    /// `m_fSongTitleAnimDuration`.
    song_duration: f32,
}

impl TitleState {
    pub(crate) fn new() -> TitleState {
        TitleState {
            st: SuperText::default(),
            text: String::new(),
            redraw: false,
            tex: None,
            font: None,
            atlas: None,
            song_duration: SONG_TITLE_DURATION,
        }
    }

    /// The main texture changed size: the title texture and font are
    /// reallocated and redrawn (plugin.cpp:2175-2221).
    pub(crate) fn invalidate(&mut self) {
        self.tex = None;
        self.atlas = None;
        self.redraw = true;
    }
}

/// One screen-mode draw recorded in a frame: the title texture and the
/// two passes' triangle lists.
#[derive(Clone, Debug)]
pub(crate) struct TitleDraw {
    tex: Texture2d,
    passes: [Vec<Vertex>; 2],
}

/// The two draws of `ShowSongTitleAnim` into the current pass.
fn record_passes(
    batch: &mut DrawBatch,
    tex: &Texture2d,
    passes: &[Vec<Vertex>; 2],
    fmt: wgpu::TextureFormat,
) {
    let blends = [BlendMode::ZeroInvSrcColor, BlendMode::Additive];
    for (verts, blend) in passes.iter().zip(blends) {
        let key = PipelineKey::new(FixedMode::TexModulate, blend, Topo::TriList, fmt);
        batch.draw(key, verts, Some(tex), SamplerKind::LinearClamp);
    }
}

impl Renderer {
    /// The `songtitle` slot's font (F43; md-core converts its
    /// `FontConfig`), matched against the system fonts with the bundled
    /// font as the fallback. Call it before the first title and whenever
    /// the font setting changes.
    pub fn set_title_font(&mut self, spec: FontSpec) {
        self.set_font(spec, false);
    }

    /// As [`set_title_font`](Self::set_title_font) with only the bundled
    /// font (deterministic; for tests and headless runs).
    pub fn set_title_font_bundled(&mut self, spec: FontSpec) {
        self.set_font(spec, true);
    }

    fn set_font(&mut self, spec: FontSpec, bundled_only: bool) {
        self.title.font = Some(TitleFont { spec, bundled_only });
        self.title.atlas = None;
        self.title.redraw = !self.title.text.is_empty();
    }

    /// `m_fSongTitleAnimDuration` for the next [`start_song_title`]
    /// (default [`SONG_TITLE_DURATION`]).
    ///
    /// [`start_song_title`]: Self::start_song_title
    pub fn set_song_title_duration(&mut self, seconds: f32) {
        self.title.song_duration = seconds;
    }

    /// `m_supertext`.
    pub fn supertext(&self) -> &SuperText {
        &self.title.st
    }

    /// `m_supertext`, for custom messages (7.8) to fill in. Their text goes
    /// through [`render_title_texture`](Self::render_title_texture)
    /// only for song titles: the custom-message font fitting
    /// (`g_title_font_sizes`, mfs:45, 315-397) is 7.8's.
    pub fn supertext_mut(&mut self) -> &mut SuperText {
        &mut self.title.st
    }

    /// The title texture (`m_lpDDSTitle`), once allocated.
    pub fn title_texture(&self) -> Option<&Texture2d> {
        self.title.tex.as_ref()
    }

    /// `LaunchSongTitleAnim` (plugin.cpp:8546-8565): show `title` for
    /// the song-title duration starting at clock `now` (the frame's
    /// [`FrameInput::now`](crate::FrameInput)). The texture is drawn by the
    /// next [`begin_frame`](Self::begin_frame) (`bRedrawSuperText`,
    /// mfs:942-947), which turns the animation off again if that fails.
    /// Returns `false`, doing nothing, when no title font was set.
    pub fn start_song_title(&mut self, title: &str, now: f32) -> bool {
        let Some(font) = &self.title.font else {
            return false;
        };
        let st = &mut self.title.st;
        st.is_song_title = true;
        st.font_size = font.spec.size_px as f32;
        (st.x, st.y, st.growth) = (0.5, 0.5, 1.0);
        st.duration = self.title.song_duration;
        st.color = [255; 3];
        st.start_time = now;
        self.title.text = title.to_owned();
        self.title.redraw = true;
        true
    }

    /// The title texture's size: `max(tex_w, tex_h)` by a quarter of it
    /// (plugin.cpp:2177-2178), limited to the device's maximum.
    fn title_size(&self) -> (u32, u32) {
        let max = self.gpu.device().limits().max_texture_dimension_2d.max(1);
        let x = self.settings.tex_w.max(self.settings.tex_h).clamp(1, max);
        (x, (x / 4).max(1))
    }

    /// `RenderStringToTitleTexture` for a song title: draw `text` into the
    /// title texture (allocating it) and set
    /// [`SuperText::font_size_used`]. Returns `false` for empty text or
    /// before [`set_title_font`](Self::set_title_font) (mfs:235-236).
    /// Submits its own command buffer.
    pub fn render_title_texture(&mut self, text: &str) -> bool {
        self.title.text = text.to_owned();
        self.title.redraw = false;
        let Some(font) = self.title.font.clone() else {
            return false;
        };
        if text.is_empty() {
            return false;
        }
        let (tx, ty) = self.title_size();
        let tex = match &self.title.tex {
            Some(t) if (t.w, t.h) == (tx, ty) => t.clone(),
            _ => {
                let t = Texture2d::new_target(&self.gpu, tx, ty, COMP_FORMAT, false);
                self.title.tex = Some(t.clone());
                t
            }
        };
        let px = title_font_px(font.spec.size_px, tx);
        let mut ta = self
            .title
            .atlas
            .take()
            .filter(|a| a.px == px)
            .unwrap_or_else(|| {
                let spec = FontSpec {
                    size_px: px,
                    antialiased: true,
                    ..font.spec.clone()
                };
                let db = if font.bundled_only {
                    fonts::bundled_database()
                } else {
                    fonts::system_database()
                };
                TitleAtlas {
                    atlas: Atlas::new(&spec, &db),
                    px,
                    tex: None,
                }
            });

        let (xi, yi) = (tx as i32, ty as i32);
        let single = TextFlags {
            single_line: true,
            ..TextFlags::default()
        };
        // 1. DT_CALCRECT in the 1/21..17/21 band (mfs:307-313, 416).
        let band = Rect::new(0, yi / 21, xi, yi * 17 / 21);
        let calc = layout(&mut ta.atlas, text, band, single).bounds;
        let h = calc.height();
        // 2. Too wide: cut to 91% and add "..." when longer than 8 (:417-436).
        let mut shown = text.to_owned();
        if calc.width() > xi {
            let len = text.chars().count();
            let keep_frac = 0.91f32 * tx as f32 / calc.width() as f32;
            if len > 8 {
                let keep = c_int(f64::from(len as f32 * keep_frac)).clamp(0, len as i32) as usize;
                shown = text.chars().take(keep).collect::<String>() + "...";
            }
        }
        // 3. Draw centred on the middle row (:439-447).
        let rect = Rect::new(0, yi / 2 - h / 2, xi, yi / 2 + h / 2);
        let centred = TextFlags {
            align: Align::Center,
            ..single
        };
        // UVs are valid for one atlas generation: lay out again if it moved.
        let mut laid = layout(&mut ta.atlas, &shown, rect, centred);
        for _ in 0..2 {
            let before = ta.atlas.generation();
            laid = layout(&mut ta.atlas, &shown, rect, centred);
            if ta.atlas.generation() == before {
                break;
            }
        }
        self.title.st.font_size_used = laid.bounds.height();
        let atlas_tex = sync_atlas(&self.gpu, &mut ta.atlas, &mut ta.tex);
        let verts = glyph_vertices(&laid.quads, [255; 4], (tx, ty));
        self.title.atlas = Some(ta);

        let mut enc = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("md-render title texture"),
            });
        // Cleared to opaque black (0xFF000000, mfs:268-286), then the glyphs
        // blended over it as the text pass does.
        let mut batch = DrawBatch::begin(
            &self.gpu,
            &mut self.pipelines,
            &mut self.dynbuf,
            &mut enc,
            &tex,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        if !verts.is_empty() {
            let key =
                PipelineKey::new(FixedMode::Text, BlendMode::Alpha, Topo::TriList, tex.format);
            batch.draw(key, &verts, Some(&atlas_tex), SamplerKind::PointClamp);
        }
        batch.finish();
        self.gpu.queue().submit([enc.finish()]);
        true
    }

    /// The part of the frame before the passes (mfs:942-947): redraw the
    /// title texture if needed, then the frame's plan for the animation.
    pub(crate) fn title_begin_frame(&mut self, now: f32) -> TitlePlan {
        if self.title.redraw {
            let text = self.title.text.clone();
            if !self.render_title_texture(&text) {
                self.title.st.start_time = -1.0;
            }
        }
        self.title.st.plan(now)
    }

    /// `if (fProgress >= 1) fStartTime = -1` (mfs:1127-1128), after the
    /// frame's draws.
    pub(crate) fn title_end_frame(&mut self, plan: &TitlePlan) {
        if plan.finished {
            self.title.st.start_time = -1.0;
        }
    }

    fn title_mesh(&self, progress: f32, now: f32, target: (u32, u32)) -> Option<TitleDraw> {
        let tex = self.title.tex.clone()?;
        let (_, ay) = mesh_aspect(self.settings.tex_w, self.settings.tex_h);
        let passes = title_passes(&MeshInput {
            st: &self.title.st,
            progress,
            now,
            title: (tex.w, tex.h),
            tex: (self.settings.tex_w, self.settings.tex_h),
            target,
            inv_aspect_y: 1.0 / ay,
        });
        Some(TitleDraw { tex, passes })
    }

    /// `ShowSongTitleAnim(GetWidth(), GetHeight(), progress)` over the
    /// composite (mfs:1123-1126): recorded into `ctx`, drawn by
    /// [`present`](Self::present) after the composite and before the text
    /// pass. `now` is `GetTime()` (the warp wobble's clock). Does nothing
    /// before the title texture exists. [`begin_frame`](Self::begin_frame)
    /// already calls it while the animation runs; call it directly only to
    /// draw a pose of your own.
    pub fn draw_title_anim(&mut self, ctx: &mut FrameCtx, progress: f32, now: f32) {
        if let Some(d) = self.title_mesh(progress, now, self.window) {
            ctx.title.push(d);
        }
    }

    /// The burn-in (mfs:1059-1067): `ShowSongTitleAnim(m_nTexSizeX,
    /// m_nTexSizeY, 1.0)` into the warp target, after `DrawSprites`.
    pub(crate) fn burn_title(&mut self, enc: &mut wgpu::CommandEncoder, now: f32) {
        let tex = (self.settings.tex_w, self.settings.tex_h);
        let Some(d) = self.title_mesh(1.0, now, tex) else {
            return;
        };
        let target = &self.vs[self.vs_target];
        let mut batch = DrawBatch::begin(
            &self.gpu,
            &mut self.pipelines,
            &mut self.dynbuf,
            enc,
            target,
            wgpu::LoadOp::Load,
        );
        record_passes(&mut batch, &d.tex, &d.passes, target.format);
        batch.finish();
    }

    /// Issue the frame's recorded title draws into the open composite pass.
    pub(crate) fn record_title(
        batch: &mut DrawBatch,
        draws: &[TitleDraw],
        fmt: wgpu::TextureFormat,
    ) {
        for d in draws {
            record_passes(batch, &d.tex, &d.passes, fmt);
        }
    }
}

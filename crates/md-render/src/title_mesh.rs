//! The song-title / custom-message animation's state and geometry
//! (`td_supertext`, plugin.h:134-155; `ShowSongTitleAnim`,
//! milkdropfs.cpp:4477-4713; `mfs` below, `plugin` = plugin.cpp).
//!
//! The C++ has no per-animation random values: the only "noise" is the
//! warp wobble, a sum of sines of `GetTime()` (mfs:4555-4560), so
//! [`MeshInput::now`] is the clock the caller passes and the headless
//! render is deterministic with no RNG involved (the brief's `seed` has no
//! C++ counterpart).
//!
//! Positions below are the C++'s; the emitted vertices carry `-y` because
//! `ShowSongTitleAnim` is a fixed-function draw and `PrepareFor2DDrawing`'s
//! ortho projection (`D3DXMatrixOrthoLH(2, -2)`, support.cpp:179) flips y
//! (the same convention as `comp_fixed`).
//!
//! Geometry (all in `f32`, as the C++): a 16 × 8 vertex grid (mfs:4496-4497,
//! 4533-4545; `v3[128]`, indices for 15 × 7 quads, :4636-4649), `u = x / 15`,
//! `v = (y / 7 - 0.5) * VERT_CLIP + 0.5`, `VERT_CLIP = 0.75` (mfs:43),
//! `pos = (2u - 1, 2v - 1)` scaled by the size, then, for a song title,
//! the warp, a scale down over time and the y aspect; for a custom message
//! the growth and the user position instead.

use crate::cast::{c_int, rgba_01_f32};
use crate::vertex::Vertex;

/// `m_fSongTitleAnimDuration` default, seconds (plugin.cpp:966; read from
/// the ini at :1217).
pub const SONG_TITLE_DURATION: f32 = 1.7;
/// Grid width and height in vertices (mfs:4496-4497, 4533-4545; `v3[128]`).
pub const GRID_X: usize = 16;
pub const GRID_Y: usize = 8;
/// `VERT_CLIP` (mfs:43).
pub const VERT_CLIP: f32 = 0.75;
/// The progress the on-screen draw is clamped to (mfs:1126).
pub const SCREEN_MAX_PROGRESS: f32 = 0.9999;

/// `td_supertext`: what the title texture was drawn for and how it
/// animates. Only the fields `ShowSongTitleAnim` and the launch code read
/// are kept (the font face, bold and italic live in the font spec).
#[derive(Clone, Debug, PartialEq)]
pub struct SuperText {
    /// `bIsSongTitle`: a song title (otherwise a custom message).
    pub is_song_title: bool,
    /// `fFontSize`: the song title font's `nSize` (plugin.cpp:8553), or the
    /// custom message's 0..100 size.
    pub font_size: f32,
    /// `nFontSizeUsed`: the height in pixels of the text drawn into the
    /// title texture (set by `render_title_texture`).
    pub font_size_used: i32,
    /// `fX`, `fY` in 0..1 (custom messages; 0.5 for a song title).
    pub x: f32,
    pub y: f32,
    /// `fGrowth` (custom messages; 1 for a song title).
    pub growth: f32,
    /// `fFadeTime` (custom messages).
    pub fade_time: f32,
    /// `nColorR/G/B` (255 for a song title).
    pub color: [u8; 3],
    /// `fStartTime`, `GetTime()` at launch; -1 is the off state
    /// (plugin.cpp:1117).
    pub start_time: f32,
    /// `fDuration` (`m_fSongTitleAnimDuration` for a song title).
    pub duration: f32,
}

impl Default for SuperText {
    /// The off state of a song title.
    fn default() -> Self {
        SuperText {
            is_song_title: true,
            font_size: 0.0,
            font_size_used: 0,
            x: 0.5,
            y: 0.5,
            growth: 1.0,
            fade_time: 0.0,
            color: [255; 3],
            start_time: -1.0,
            duration: SONG_TITLE_DURATION,
        }
    }
}

/// What one frame does for the animation (`RenderFrame`, mfs:1059-1067 and
/// :1123-1129).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TitlePlan {
    /// "If song title animation just ended, burn it into the VS" (mfs:1062):
    /// draw it into the main texture with progress 1.
    pub burn_in: bool,
    /// Draw it over the composite with this progress,
    /// `min(progress, 0.9999)` (mfs:1126).
    pub screen: Option<f32>,
    /// `progress >= 1`: the state goes off after this frame (mfs:1127-1128).
    pub finished: bool,
}

impl SuperText {
    /// `fProgress = (GetTime() - fStartTime) / fDuration` (mfs:1059). A zero
    /// or negative duration counts as finished.
    pub fn progress(&self, now: f32) -> f32 {
        if self.duration > 0.0 {
            (now - self.start_time) / self.duration
        } else {
            1.0
        }
    }

    /// Whether the animation is on (`fStartTime >= 0`).
    pub fn is_active(&self) -> bool {
        self.start_time >= 0.0
    }

    /// The frame's drawing at clock `now`: nothing while off.
    pub fn plan(&self, now: f32) -> TitlePlan {
        if !self.is_active() {
            return TitlePlan {
                burn_in: false,
                screen: None,
                finished: false,
            };
        }
        let progress = self.progress(now);
        TitlePlan {
            burn_in: progress >= 1.0,
            screen: Some(progress.min(SCREEN_MAX_PROGRESS)),
            finished: progress >= 1.0,
        }
    }
}

/// Everything `ShowSongTitleAnim(w, h, fProgress)` reads.
#[derive(Clone, Copy, Debug)]
pub struct MeshInput<'a> {
    pub st: &'a SuperText,
    /// `fProgress`; a value >= 1 is the burn-in call (the "hacky" 1-pixel
    /// y nudge, mfs:4541-4542 and 4606-4607).
    pub progress: f32,
    /// `GetTime()` (the warp wobble's clock).
    pub now: f32,
    /// `m_nTitleTexSizeX/Y`.
    pub title: (u32, u32),
    /// `m_nTexSizeX/Y`, the main texture.
    pub tex: (u32, u32),
    /// `w`, `h`: the render target (`GetWidth/Height`, or the main texture
    /// for the burn-in). Only custom messages read them (mfs:4618-4626).
    pub target: (u32, u32),
    /// `m_fInvAspectY` (plugin.cpp:2038).
    pub inv_aspect_y: f32,
}

/// The triangle-list indices of the 15 × 7 quads (mfs:4636-4649).
fn indices() -> Vec<usize> {
    let mut out = Vec::with_capacity(7 * 15 * 6);
    for y in 0..7 {
        for x in 0..15 {
            let i = y * 16 + x;
            out.extend_from_slice(&[i, i + 1, i + 16, i + 1, i + 16, i + 17]);
        }
    }
    out
}

/// The grid before positioning: `(x, y, u, v)` per vertex (mfs:4532-4545
/// song title, :4597-4610 custom).
fn grid(size_x: f32, size_y: f32, nudge: Option<f32>) -> Vec<[f32; 4]> {
    let mut g = Vec::with_capacity(GRID_X * GRID_Y);
    for y in 0..GRID_Y {
        for x in 0..GRID_X {
            let tu = x as f32 / 15.0;
            let tv = (y as f32 / 7.0 - 0.5) * VERT_CLIP + 0.5;
            let mut py = (tv * 2.0 - 1.0) * size_y;
            if let Some(n) = nudge {
                py += n;
            }
            g.push([(tu * 2.0 - 1.0) * size_x, py, tu, tv]);
        }
    }
    g
}

/// The two draws of `ShowSongTitleAnim` (mfs:4658-4710), each as the 630
/// vertices of its triangle list: pass 0 is the shadow (blend `ZERO`,
/// `INVSRCCOLOR`, colour `(t, t, t, t)`), pass 1 the text (blend `ONE`,
/// `ONE`, colour `t * rgb / 255`, `t`). The nudges accumulate on the same
/// vertices: pass 0 is moved by `(+2, +2)` title-texture pixels, pass 1 by
/// a further `(-4, -4)`, so the text ends 2 px up-left and the shadow 2 px
/// down-right of the unnudged mesh (mfs:4678-4695).
pub fn title_passes(m: &MeshInput) -> [Vec<Vertex>; 2] {
    let st = m.st;
    let (tw, th) = (m.title.0 as f32, m.title.1 as f32);
    let (tex_w, tex_h) = (m.tex.0 as f32, m.tex.1 as f32);
    let p = m.progress;
    let used = st.font_size_used as f32;
    // Burn-in only: "a pretty hacky guess @ getting it to align" (:4541-4542).
    let nudge = (p >= 1.0).then(|| 1.0 / tex_h);

    let mut g;
    if st.is_song_title {
        // positioning (mfs:4499-4511)
        let mut size_x = 50.0 / used * 1.5f32.powf(st.font_size - 2.0);
        let mut size_y = size_x * th / tw;
        if size_x > 0.88 {
            size_y *= 0.88 / size_x;
            size_x = 0.88;
        }
        g = grid(size_x, size_y, nudge);
        // warping (mfs:4547-4562)
        let ramped = (1.0 - p * 1.5).max(0.0);
        let t2 = ramped.powf(1.8) * 1.3;
        let now = m.now;
        for v in &mut g {
            let (mut x, mut y) = (v[0], v[1]);
            x += t2 * 0.070 * (now * 0.31 + x * 0.39 - y * 1.94).sin();
            x += t2 * 0.044 * (now * 0.81 - x * 1.91 + y * 0.27).sin();
            x += t2 * 0.061 * (now * 1.31 + x * 0.61 + y * 0.74).sin();
            y += t2 * 0.061 * (now * 0.37 + x * 1.83 + y * 0.69).sin();
            y += t2 * 0.070 * (now * 0.67 + x * 0.42 - y * 1.39).sin();
            y += t2 * 0.087 * (now * 1.07 + x * 3.55 + y * 0.89).sin();
            (v[0], v[1]) = (x, y);
        }
        // scale down over time (mfs:4564-4569)
        let scale = 1.01 / (p.powf(0.21) + 0.01);
        for v in &mut g {
            v[0] *= scale;
            v[1] *= scale;
        }
    } else {
        // positioning (mfs:4574-4576)
        let size_x = tex_w / 1024.0 * 100.0 / used * 1.033f32.powf(st.font_size - 50.0);
        let size_y = size_x * th / tw;
        g = grid(size_x, size_y, nudge);
        // growth and the user position (mfs:4612-4634)
        let t = 1.0 * (1.0 - p) + p * st.growth;
        let mut dx = st.x * 2.0 - 1.0;
        let mut dy = st.y * 2.0 - 1.0;
        let (w, h) = (m.target.0 as f32, m.target.1 as f32);
        if m.target.0 != m.target.1 {
            let aspect = w / (h * 4.0 / 3.0);
            if aspect < 1.0 {
                dx /= aspect;
            } else {
                dy *= aspect;
            }
        }
        for v in &mut g {
            v[0] = v[0] * t + dx;
            v[1] = v[1] * t + dy;
        }
    }
    // `v3[i].y *= m_fInvAspectY` (mfs:4656).
    for v in &mut g {
        v[1] *= m.inv_aspect_y;
    }

    let idx = indices();
    let mut out = [Vec::new(), Vec::new()];
    for (it, pass) in out.iter_mut().enumerate() {
        // colours (mfs:4660-4676)
        let t = if st.is_song_title {
            p.powf(0.3) * 1.0
        } else {
            let x = (p / st.fade_time).min(1.0);
            0.5 - 0.5 * (x * std::f32::consts::PI).cos()
        };
        let color = if it == 0 {
            rgba_01_f32(t, t, t, t)
        } else {
            let [r, gc, b] = st.color.map(|c| t * c as f32 / 255.0);
            rgba_01_f32(r, gc, b, t)
        };
        // nudge down & right for the shadow, up & left for the text
        // (mfs:4678-4695; it accumulates)
        let (ox, oy) = if it == 0 {
            (2.0 / tw, 2.0 / th)
        } else {
            (-4.0 / tw, -4.0 / th)
        };
        for v in &mut g {
            v[0] += ox;
            v[1] += oy;
        }
        pass.extend(idx.iter().map(|&i| Vertex {
            // `PrepareFor2DDrawing`'s ortho projection (2, -2) flips y for
            // every fixed-function draw (support.cpp:179), as in comp_fixed.
            pos: [g[i][0], -g[i][1], 0.0],
            color,
            uv: [g[i][2], g[i][3], 0.0, 0.0],
            ..Vertex::default()
        }));
    }
    out
}

/// The `(int)` of a title-size computation (`nSize * m_nTitleTexSizeX /
/// 256`, plugin.cpp:2228): integer arithmetic.
pub(crate) fn title_font_px(size: u32, title_x: u32) -> u32 {
    let px = c_int(f64::from(size) * f64::from(title_x) / 256.0).max(0) as u32;
    // `if (songtitle_font_size<6) songtitle_font_size=6;` (plugin.cpp:2229)
    px.max(6)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(used: i32, size: f32) -> SuperText {
        SuperText {
            font_size: size,
            font_size_used: used,
            start_time: 0.0,
            ..SuperText::default()
        }
    }

    fn input<'a>(st: &'a SuperText, progress: f32) -> MeshInput<'a> {
        MeshInput {
            st,
            progress,
            now: 3.0,
            title: (1024, 256),
            tex: (1024, 512),
            target: (1024, 512),
            inv_aspect_y: 2.0,
        }
    }

    #[test]
    fn duration_is_the_cpp_default() {
        // plugin.cpp:966
        assert_eq!(SONG_TITLE_DURATION, 1.7);
        assert_eq!(SuperText::default().duration, 1.7);
        assert!(!SuperText::default().is_active());
    }

    #[test]
    fn plan_draws_only_while_active() {
        let mut st = song(64, 16.0);
        // off: nothing at any time (mfs:1062-1064, 1123-1125)
        st.start_time = -1.0;
        for now in [-5.0, 0.0, 1.0, 100.0] {
            let p = st.plan(now);
            assert!(!p.burn_in && p.screen.is_none() && !p.finished);
        }
        st.start_time = 0.0;
        let half = st.plan(0.85);
        assert_eq!(half.screen.map(|p| (p * 100.0).round()), Some(50.0));
        assert!(!half.burn_in && !half.finished);
        // just before the end: screen only
        let p = st.plan(1.69);
        assert!(p.screen.is_some() && !p.burn_in && !p.finished);
        // the first frame at progress >= 1: burn in, draw clamped, finish
        let end = st.plan(1.7);
        assert!(end.burn_in && end.finished);
        assert_eq!(end.screen, Some(SCREEN_MAX_PROGRESS));
        let late = st.plan(9.0);
        assert!(late.burn_in && late.finished);
        assert_eq!(late.screen, Some(SCREEN_MAX_PROGRESS));
    }

    #[test]
    fn zero_duration_finishes_at_once() {
        let st = SuperText {
            duration: 0.0,
            start_time: 1.0,
            ..SuperText::default()
        };
        assert!(st.plan(1.0).finished);
    }

    #[test]
    fn grid_is_16_by_8_with_630_indices_per_pass() {
        let st = song(64, 16.0);
        let [a, b] = title_passes(&input(&st, 0.5));
        assert_eq!((a.len(), b.len()), (630, 630));
        assert_eq!(indices().len(), 7 * 15 * 6);
        assert!(indices().iter().all(|&i| i < GRID_X * GRID_Y));
        assert_eq!(GRID_X * GRID_Y, 128);
    }

    #[test]
    fn final_pose_is_the_unwarped_mesh_with_the_nudges() {
        // progress 1: no warp (ramped = 0), scale 1.01 / 1.01 = 1, colour 1.
        let st = song(64, 16.0);
        let m = input(&st, 1.0);
        let [shadow, text] = title_passes(&m);
        // The first vertex is grid (0, 0): u = 0, v = (0 - .5) * .75 + .5.
        let (tw, th) = (1024.0f32, 256.0f32);
        let size_x = 0.88f32; // 50/64 * 1.5^14 is far above the clamp
        let size_y = 0.88 * th / tw;
        let tv = (0.0 - 0.5) * 0.75 + 0.5;
        let y0 = ((tv * 2.0 - 1.0) * size_y + 1.0 / 512.0) * 2.0;
        let near = |a: f32, b: f32| (a - b).abs() < 2e-5;
        assert!(
            near(shadow[0].pos[0], -size_x + 2.0 / tw),
            "{:?}",
            shadow[0]
        );
        assert!(near(shadow[0].pos[1], -(y0 + 2.0 / th)), "{:?}", shadow[0]);
        // the text pass: +2 - 4 = -2 px in total
        assert!(near(text[0].pos[0], -size_x - 2.0 / tw), "{:?}", text[0]);
        assert!(near(text[0].pos[1], -(y0 - 2.0 / th)), "{:?}", text[0]);
        assert_eq!(shadow[0].uv[..2], [0.0, tv]);
        assert_eq!(shadow[0].color, [255; 4]);
        assert_eq!(text[0].color, [255; 4]);
    }

    #[test]
    fn colour_ramps_with_progress_to_the_power_point_three() {
        let st = song(64, 16.0);
        let [shadow, text] = title_passes(&input(&st, 0.5));
        let t = 0.5f32.powf(0.3); // mfs:4665
        let byte = ((t * 255.0) as i32 & 0xFF) as u8;
        assert_eq!(shadow[0].color, [byte; 4]);
        assert_eq!(text[0].color, [byte; 4]);
        // a custom message tints the text pass only (mfs:4672)
        let c = SuperText {
            is_song_title: false,
            color: [255, 128, 0],
            fade_time: 0.5,
            font_size: 50.0,
            font_size_used: 64,
            start_time: 0.0,
            ..SuperText::default()
        };
        let [s, t] = title_passes(&input(&c, 0.5));
        assert_eq!(s[0].color, [255; 4]); // CosineInterp(min(1, 1)) = 1
        assert_eq!(t[0].color, [255, 128, 0, 255]);
    }

    #[test]
    fn warp_vanishes_late_and_scale_shrinks_the_mesh() {
        let st = song(64, 16.0);
        // p = 0.7: ramped = 0 -> no wobble, so `now` cannot matter ...
        let a = title_passes(&MeshInput {
            now: 1.0,
            ..input(&st, 0.7)
        });
        let b = title_passes(&MeshInput {
            now: 9.0,
            ..input(&st, 0.7)
        });
        assert_eq!(a, b);
        // ... while at p = 0.2 it does (mfs:4548: ramped = 1 - 1.5 p).
        let c = title_passes(&MeshInput {
            now: 1.0,
            ..input(&st, 0.2)
        });
        let d = title_passes(&MeshInput {
            now: 9.0,
            ..input(&st, 0.2)
        });
        assert_ne!(c, d);
        // scale = 1.01 / (p^0.21 + 0.01): bigger early (mfs:4564-4569)
        let early = title_passes(&input(&st, 0.7));
        let late = title_passes(&input(&st, 0.95));
        assert!(early[1][0].pos[0] < late[1][0].pos[0]);
    }

    #[test]
    fn custom_message_is_positioned_and_grown() {
        let c = SuperText {
            is_song_title: false,
            font_size: 50.0,
            font_size_used: 100,
            x: 1.0,
            y: 0.5,
            growth: 2.0,
            fade_time: 1.0,
            start_time: 0.0,
            ..SuperText::default()
        };
        // square target: no aspect correction (mfs:4618); progress 1 -> t = growth
        let m = MeshInput {
            target: (512, 512),
            tex: (1024, 1024),
            inv_aspect_y: 1.0,
            ..input(&c, 1.0)
        };
        let [shadow, _] = title_passes(&m);
        let size_x = 1024.0 / 1024.0 * 100.0 / 100.0 * 1.0;
        let expect = -size_x * 2.0 + 1.0 + 2.0 / 1024.0;
        assert!((shadow[0].pos[0] - expect).abs() < 1e-5, "{:?}", shadow[0]);
    }

    #[test]
    fn title_font_px_follows_the_int_formula() {
        assert_eq!(title_font_px(16, 1024), 64);
        assert_eq!(title_font_px(1, 64), 6); // floor of 6 (plugin.cpp:2229)
        assert_eq!(title_font_px(18, 160), 11); // 2880 / 256 truncated
    }
}

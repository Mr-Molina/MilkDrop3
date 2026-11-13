//! The no-shader composite, `ShowToUser_NoShaders`
//! (code/vis_milk2/milkdropfs.cpp:3967-4287@6f7cd5d): the composite used
//! for MilkDrop 1 presets (`PSVERSION_COMP 0`) and with shaders disabled.
//!
//! It draws the (just warped) main texture as one full-screen quad, a
//! triangle strip of four `SPRITEVERTEX`es, with the fixed-function
//! texture stage `MODULATE(DIFFUSE, TEXTURE)` / alpha `SELECTARG1 DIFFUSE`
//! ([`FixedMode::TexModulate`], milkdropfs.cpp:3990-3997) and bilinear
//! filtering (3980-3985; main has one level, so the mip filter is inert), then the post-processing
//! filters as untextured quads ([`FixedMode::Color`], `SetTexture(0,
//! NULL)`) of white, opaque vertices (4189-4199).
//!
//! Geometry (4000-4033): the quad is one pixel larger than the screen on
//! every side (`±(1 + 1/W)`, `±(1 + 1/H)`), multiplied by the aspect
//! correction `aspect = W / (H * m_fInvAspectY)` (`y *= aspect` when it is
//! above 1, else `x *= 1/aspect`). The vertices are the C++'s
//! pre-projection coordinates; the fixed-function ortho projection
//! (support.cpp:179, applied by the fixed vertex shaders) puts C++ `(x, y)`
//! at clip `(x, -y)`, so C++ Y = -1 is the top of the screen.
//! UVs: `v3[0]` (C++ `y = +1`, so the bottom-left on screen) has
//! `(lo, hi)`, i.e. `v = 1` at the bottom of the screen;
//! `lo/hi = 0.5 ∓ 0.5/zoom`.
//!
//! Vertex colours are `D3DCOLOR_RGBA_01` of the hue `shade` (all white
//! unless the preset's hue-shader amount `m_fShader` exceeds 0.001,
//! 4036-4061) times the echo `mix` and the gamma pass factor.
//!
//! # Blend table
//!
//! Every draw, in order, with the C++ `SRCBLEND/DESTBLEND` pair:
//!
//! | Draw | C++ lines | `SRCBLEND` / `DESTBLEND` | [`BlendMode`] |
//! |---|---|---|---|
//! | echo layer 0 (`i = 0`, zoom 1, `mix = 1 - echo_alpha`) | 4087-4089, 4116 | `ONE` / `ZERO` (blending on) | `Replace` |
//! | its gamma redraws, echo layer 1 (`zoom = echo_zoom`, flipped by `echo_orient`) and its gamma redraws | 4118-4122, 4124-4140 | `ONE` / `ONE` | `Additive` |
//! | no echo: first pass | 4148-4150 | blending off (`ONE` / `ZERO`) | `Off` |
//! | no echo: further gamma passes | 4166-4171 | `ONE` / `ONE` | `Additive` |
//! | brighten (square root): invert, square, invert | 4201-4225 | `INVDESTCOLOR` / `ZERO`; `ZERO` / `DESTCOLOR`; `INVDESTCOLOR` / `ZERO` | `InvDestColor`, `DestColor`, `InvDestColor` |
//! | darken (square) | 4227-4246 | `ZERO` / `DESTCOLOR` | `DestColor` |
//! | solarize | 4248-4265 | `ZERO` / `INVDESTCOLOR`; `DESTCOLOR` / `ONE` | `ZeroInvDestColor`, `DestColorAdd` |
//! | invert | 4267-4281 | `INVDESTCOLOR` / `ZERO` | `InvDestColor` |
//! | darken_center (not here, see below) | 3310-3345 | `SRCALPHA` / `INVSRCALPHA` | `Alpha` |
//!
//! The device caps checks (`D3DPBLENDCAPS_*`) always pass on wgpu.
//!
//! `darken_center` is not part of `ShowToUser_NoShaders`: the C++ draws
//! its fan in `DrawSprites` (milkdropfs.cpp:3310-3345) into the main
//! texture, before the composite and for every preset; that is the
//! sprites hook of the frame loop, whose geometry is Task 6.5's
//! `build_darken_center`. Its blend is listed above for completeness.
//!
//! Gamma (4110-4141 echo, 4152-4172 no echo): `gamma` is clamped to
//! `0..=8` by the per-frame code. Without echo the image is drawn
//! `int(gamma - 0.001) + 1` times, all at full strength except the last,
//! at `gamma - pass` (so gamma 2 draws twice at 1, gamma 2.5 three times
//! at 1, 1, 0.5). With echo each of the two layers is drawn once and then
//! redrawn `int(gamma - 0.0001)` more times, the last redraw at
//! `gamma - int(gamma - 0.0001)` (the C++ text; gamma 2 redraws once at
//! 1). Additive passes saturate in the 8-bit target as in D3D9.
//!
//! Echo during a blend (4069-4083): when both presets echo
//! (`echo_alpha.eval(now) > 0.01` and the old preset's `> 0.01`) with
//! different orientations, the old orientation is used before the snap
//! point with alpha `* (1 - 2 CosineInterp(t))`, the new one after with
//! `* (2 CosineInterp(t) - 1)`.

use crate::blend::BlendMode;
use crate::comp::{cosine_interp, hue_shades};
use crate::vertex::Vertex;

/// One quad of the no-shader composite (a 4-vertex triangle strip).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedCompDraw {
    pub blend: BlendMode,
    /// Textured with the main texture ([`crate::FixedMode::TexModulate`])
    /// or a plain colour quad ([`crate::FixedMode::Color`]).
    pub textured: bool,
    pub verts: [Vertex; 4],
}

/// The video-echo state of a running blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EchoBlend {
    /// `m_fBlendProgress`.
    pub progress: f32,
    /// `m_fSnapPoint` ([`crate::snap_point`]).
    pub snap: f32,
    /// `m_pState->m_fVideoEchoAlpha.eval(GetTime())`.
    pub alpha_now: f32,
    /// `m_fVideoEchoAlphaOld` ([`md_preset::Transition`]).
    pub alpha_old: f32,
    /// `m_nVideoEchoOrientation` of the new preset and the old one.
    pub orient: i32,
    pub orient_old: i32,
}

/// Inputs of [`fixed_comp_draws`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedCompParams {
    /// `GetWidth()`, `GetHeight()` (the composite target).
    pub window: (u32, u32),
    /// `m_nTexSizeX/Y` (for `m_fInvAspectY`).
    pub tex: (u32, u32),
    /// `GetTime()` and `m_fRandStart` (hue shader).
    pub now: f32,
    pub rand_start: [f32; 4],
    /// `m_fShader.eval(GetTime())`.
    pub shader_amount: f32,
    /// `var_pf_echo_zoom`, `var_pf_echo_alpha`, `var_pf_echo_orient`,
    /// `var_pf_gamma` (post-equation, blended).
    pub echo_zoom: f64,
    pub echo_alpha: f64,
    pub echo_orient: f64,
    pub gamma: f64,
    /// `var_pf_brighten`, `_darken`, `_solarize`, `_invert` (non-zero = on).
    pub brighten: f64,
    pub darken: f64,
    pub solarize: f64,
    pub invert: f64,
    /// `Some` while blending.
    pub blend: Option<EchoBlend>,
}

/// `D3DCOLOR_RGBA_01(r, g, b, a)` (milkdropfs.cpp:40): each channel
/// `(int)(x * 255)` masked to 8 bits by `D3DCOLOR_RGBA`, as R, G, B, A
/// bytes.
pub fn rgba_01(r: f32, g: f32, b: f32, a: f32) -> [u8; 4] {
    [r, g, b, a].map(|x| ((x * 255.0) as i32 & 0xff) as u8)
}

/// The quad's corners in C++ order (`v3[0..4]`, 4008-4015), at the C++'s
/// pre-projection positions.
fn quad(window: (u32, u32), mult: (f32, f32)) -> [Vertex; 4] {
    let ox = 1.0 + 1.0 / window.0.max(1) as f32;
    let oy = 1.0 + 1.0 / window.1.max(1) as f32;
    let cpp = [(-ox, oy), (ox, oy), (-ox, -oy), (ox, -oy)];
    cpp.map(|(x, y)| Vertex {
        pos: [x * mult.0, y * mult.1, 0.0],
        color: [255; 4],
        ..Vertex::default()
    })
}

/// `x_aspect_mult`, `y_aspect_mult` (milkdropfs.cpp:4019-4028).
fn aspect_mult(window: (u32, u32), tex: (u32, u32)) -> (f32, f32) {
    let (_, aspect_y) = crate::mesh::mesh_aspect(tex.0, tex.1);
    let inv_aspect_y = 1.0 / aspect_y;
    let aspect = window.0.max(1) as f32 / (window.1.max(1) as f32 * inv_aspect_y);
    if aspect > 1.0 {
        (1.0, aspect)
    } else {
        (1.0 / aspect, 1.0)
    }
}

fn set_uv(v: &mut [Vertex; 4], uv: [(f32, f32); 4]) {
    for (v, (u, t)) in v.iter_mut().zip(uv) {
        v.uv = [u, t, 0.0, 0.0];
    }
}

fn set_color(v: &mut [Vertex; 4], shade: &[[f32; 3]; 4], f: f32) {
    for (v, s) in v.iter_mut().zip(shade) {
        v.color = rgba_01(f * s[0], f * s[1], f * s[2], 1.0);
    }
}

/// Every quad `ShowToUser_NoShaders` draws, in order (see the module docs
/// and the blend table).
pub fn fixed_comp_draws(p: &FixedCompParams) -> Vec<FixedCompDraw> {
    let mut out = Vec::new();
    let mut v3 = quad(p.window, aspect_mult(p.window, p.tex));
    let mut shade = [[1.0f32; 3]; 4];
    if p.shader_amount > 0.001 {
        let hue = hue_shades(p.now, p.rand_start);
        let a = p.shader_amount;
        for (s, h) in shade.iter_mut().zip(hue) {
            *s = h.map(|c| c * a + (1.0 - a));
        }
    }
    let echo_zoom = p.echo_zoom as f32;
    let mut echo_alpha = p.echo_alpha as f32;
    let mut orient = (p.echo_orient as i32) % 4;
    let gamma = p.gamma as f32;
    if let Some(b) = p.blend {
        if b.alpha_now > 0.01 && b.alpha_old > 0.01 && b.orient != b.orient_old {
            if b.progress < b.snap {
                orient = b.orient_old;
                echo_alpha *= 1.0 - 2.0 * cosine_interp(b.progress);
            } else {
                echo_alpha *= 2.0 * cosine_interp(b.progress) - 1.0;
            }
        }
    }
    let draw = |out: &mut Vec<FixedCompDraw>, blend, verts| {
        out.push(FixedCompDraw {
            blend,
            textured: true,
            verts,
        })
    };
    if echo_alpha > 0.001 {
        let mut blend = BlendMode::Replace;
        for i in 0..2 {
            let zoom = if i == 0 { 1.0 } else { echo_zoom };
            let lo = 0.5 - 0.5 / zoom;
            let hi = 0.5 + 0.5 / zoom;
            let mut uv = [(lo, hi), (hi, hi), (lo, lo), (hi, lo)];
            if i == 1 {
                for (u, v) in &mut uv {
                    if orient % 2 != 0 {
                        *u = 1.0 - *u;
                    }
                    if orient >= 2 {
                        *v = 1.0 - *v;
                    }
                }
            }
            set_uv(&mut v3, uv);
            let mix = if i == 1 { echo_alpha } else { 1.0 - echo_alpha };
            set_color(&mut v3, &shade, mix);
            draw(&mut out, blend, v3);
            blend = BlendMode::Additive;
            if gamma > 0.001 {
                let redraws = (gamma - 0.0001) as i32;
                for r in 0..redraws {
                    let g = if r == redraws - 1 {
                        gamma - (gamma - 0.0001) as i32 as f32
                    } else {
                        1.0
                    };
                    set_color(&mut v3, &shade, g * mix);
                    draw(&mut out, blend, v3);
                }
            }
        }
    } else {
        set_uv(&mut v3, [(0.0, 1.0), (1.0, 1.0), (0.0, 0.0), (1.0, 0.0)]);
        let passes = (gamma - 0.001) as i32 + 1;
        for pass in 0..passes {
            let g = if pass == passes - 1 {
                gamma - pass as f32
            } else {
                1.0
            };
            set_color(&mut v3, &shade, g);
            let blend = if pass == 0 {
                BlendMode::Off
            } else {
                BlendMode::Additive
            };
            draw(&mut out, blend, v3);
        }
    }

    // The filters: untextured white quads without the aspect stretch
    // (a fresh v3, milkdropfs.cpp:4185-4197).
    let white = quad(p.window, (1.0, 1.0));
    let mut filter = |blends: &[BlendMode]| {
        for &blend in blends {
            out.push(FixedCompDraw {
                blend,
                textured: false,
                verts: white,
            });
        }
    };
    if p.brighten != 0.0 {
        filter(&[
            BlendMode::InvDestColor,
            BlendMode::DestColor,
            BlendMode::InvDestColor,
        ]);
    }
    if p.darken != 0.0 {
        filter(&[BlendMode::DestColor]);
    }
    if p.solarize != 0.0 {
        filter(&[BlendMode::ZeroInvDestColor, BlendMode::DestColorAdd]);
    }
    if p.invert != 0.0 {
        filter(&[BlendMode::InvDestColor]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> FixedCompParams {
        FixedCompParams {
            window: (100, 50),
            tex: (100, 50),
            now: 0.0,
            rand_start: [0.0; 4],
            shader_amount: 0.0,
            echo_zoom: 2.0,
            echo_alpha: 0.0,
            echo_orient: 0.0,
            gamma: 1.0,
            brighten: 0.0,
            darken: 0.0,
            solarize: 0.0,
            invert: 0.0,
            blend: None,
        }
    }

    fn blends(d: &[FixedCompDraw]) -> Vec<BlendMode> {
        d.iter().map(|d| d.blend).collect()
    }

    #[test]
    fn plain_quad_is_one_pixel_larger_and_upright() {
        let d = fixed_comp_draws(&params());
        assert_eq!(blends(&d), [BlendMode::Off]);
        let v = d[0].verts;
        // C++ v3[0] is at y = +1.02 before the projection (4008-4015),
        // which puts it at the bottom-left of the screen, with v = 1.
        assert_eq!(v[0].pos[..2], [-1.01, 1.02]);
        assert_eq!(v[0].uv[..2], [0.0, 1.0]);
        assert_eq!(v[3].pos[..2], [1.01, -1.02]);
        assert_eq!(v[3].uv[..2], [1.0, 0.0]);
        assert_eq!(v[0].color, [255; 4]);
    }

    #[test]
    fn gamma_passes_follow_the_cpp_loop() {
        let mut p = params();
        p.gamma = 2.0;
        let d = fixed_comp_draws(&p);
        assert_eq!(blends(&d), [BlendMode::Off, BlendMode::Additive]);
        assert!(d.iter().all(|d| d.verts[0].color == [255; 4]));
        p.gamma = 2.5;
        let d = fixed_comp_draws(&p);
        assert_eq!(d.len(), 3);
        assert_eq!(d[2].verts[0].color, [127, 127, 127, 255]);
        p.gamma = 0.5;
        let d = fixed_comp_draws(&p);
        assert_eq!(blends(&d), [BlendMode::Off]);
        assert_eq!(d[0].verts[0].color[0], 127);
    }

    #[test]
    fn echo_draws_two_layers_with_flips_and_mix() {
        let mut p = params();
        (p.echo_alpha, p.echo_zoom, p.echo_orient) = (0.25, 2.0, 3.0);
        let d = fixed_comp_draws(&p);
        assert_eq!(blends(&d), [BlendMode::Replace, BlendMode::Additive]);
        // Layer 0: zoom 1, mix 0.75; layer 1: zoom 2 (0.25..0.75), flipped
        // in u and v, mix 0.25.
        assert_eq!(d[0].verts[0].uv[..2], [0.0, 1.0]);
        assert_eq!(d[0].verts[0].color[0], 191);
        assert_eq!(d[1].verts[0].uv[..2], [0.75, 0.25]);
        assert_eq!(d[1].verts[0].color[0], 63);
        // Gamma 2 with echo: one additive redraw per layer.
        p.gamma = 2.0;
        assert_eq!(fixed_comp_draws(&p).len(), 4);
    }

    #[test]
    fn echo_blend_switches_orientation_at_the_snap_point() {
        let mut p = params();
        (p.echo_alpha, p.echo_orient) = (0.5, 1.0);
        let blend = EchoBlend {
            progress: 0.25,
            snap: 0.5,
            alpha_now: 0.5,
            alpha_old: 0.5,
            orient: 1,
            orient_old: 2,
        };
        p.blend = Some(blend);
        let d = fixed_comp_draws(&p);
        // Before the snap: the old orientation (v flipped, u not), alpha
        // 0.5 * (1 - 2 * cosine_interp(0.25)).
        let a = 0.5 * (1.0 - 2.0 * cosine_interp(0.25));
        assert_eq!(d[1].verts[0].uv[..2], [0.25, 0.25]);
        assert_eq!(d[1].verts[0].color[0], rgba_01(a, 0.0, 0.0, 0.0)[0]);
        p.blend = Some(EchoBlend {
            progress: 0.75,
            ..blend
        });
        let d = fixed_comp_draws(&p);
        assert_eq!(d[1].verts[0].uv[..2], [0.75, 0.75]);
    }

    #[test]
    fn filters_follow_the_blend_table() {
        let mut p = params();
        (p.brighten, p.darken, p.solarize, p.invert) = (1.0, 1.0, 1.0, 1.0);
        let d = fixed_comp_draws(&p);
        use BlendMode::*;
        assert_eq!(
            blends(&d),
            [
                Off,
                InvDestColor,
                DestColor,
                InvDestColor,
                DestColor,
                ZeroInvDestColor,
                DestColorAdd,
                InvDestColor
            ]
        );
        assert!(d[1..].iter().all(|d| !d.textured));
    }

    #[test]
    fn hue_shader_tints_the_corners() {
        let mut p = params();
        p.shader_amount = 1.0;
        let d = fixed_comp_draws(&p);
        let hue = hue_shades(0.0, [0.0; 4]);
        for (v, h) in d[0].verts.iter().zip(hue) {
            assert_eq!(v.color, rgba_01(h[0], h[1], h[2], 1.0));
        }
    }

    #[test]
    fn aspect_stretches_when_tex_and_window_differ() {
        let mut p = params();
        p.tex = (100, 100); // m_fInvAspectY = 1, aspect = 2: y *= 2
        let d = fixed_comp_draws(&p);
        assert!((d[0].verts[0].pos[1] - 2.04).abs() < 1e-6);
        // The filters keep the unstretched quad.
        p.invert = 1.0;
        let d = fixed_comp_draws(&p);
        assert!((d[1].verts[0].pos[1] - 1.02).abs() < 1e-6);
    }
}

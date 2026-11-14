//! User sprites: the per-frame values `DrawUserSprites` reads from a
//! sprite's variables, clamped as it clamps them, and the quad, colour and
//! blend state it draws (code/vis_milk2/milkdropfs.cpp:3432-3777; `mfs`
//! below). md-core runs the sprite's `init_`/`code_` lines (texmgr.cpp) and
//! hands the resulting variables over as [`SpriteVars`].
//!
//! * Clamps (3542-3556): `x`, `y` are `(float)var * 2 - 1` limited to
//!   `[-1000, 1000]`, `sx`, `sy` to `[-1000, 1000]`, `repeatx/y` to
//!   `[0.01, 100]`, `blendmode` is `(int)` truncated and limited to
//!   `[0, 4]`, `r g b a` to `[0, 1]`; `rot` is not clamped; `flipx/y` and
//!   `burn`/`done` are "`!= 0.0`". The C++ `min`/`max` are the `windows.h`
//!   macros `a < b ? a : b`, which let NaN through; so do these.
//! * Quad (3558-3640): corners `(+-sx, +-sy)` in strip order (flips swap
//!   them, not the UVs), scaled for the image aspect `img_h / img_w`
//!   (3568-3576), rotated, translated by `(x, y)`, scaled for the window
//!   aspect `w / h` (3598-3606), and for a burnt-in sprite that is killed
//!   this frame fitted to the main texture's 4:3 (3608-3616). UVs are
//!   `(+-0.5) * repeat + 0.5`. There is no y flip of the vertices (3618-3619
//!   is commented out): the ortho(2, -2) projection of the fixed shaders
//!   makes `var_y = 0` the top of the screen.
//! * Blend modes (3642-3729): 0 alpha blend, 1 decal (no blending), 2
//!   additive, 3 `SRCCOLOR`/`INVSRCCOLOR`, 4 colour key (ALPHAOP
//!   `MODULATE`); see [`sprite_state`] and [`sprite_color`].
//! * Burn (3508-3521, 3733-3752): a burnt sprite is drawn into the warp
//!   target (`m_lpVS[1]`, the main texture this frame wrote, next frame's
//!   warp source) with the burn quad, then to the screen with the same
//!   vertices run through the undo of the fit: the C++ runs that undo
//!   whether or not the fit was applied, so a burnt sprite that is not
//!   killed draws on screen stretched by the fit's inverse
//!   ([`SpriteQuads`]).

use crate::blend::BlendMode;
use crate::cast::{c_int, rgba_01_f32};
use crate::fixed::FixedMode;
use crate::texcache::TextureId;
use crate::vertex::Vertex;

/// A sprite's output variables after its per-frame code ran (`var_x` ...
/// `var_burn`, `var_done`), with the defaults `texmgr::LoadTex` sets
/// before its init code (texmgr.cpp:216-231).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteVars {
    pub x: f64,
    pub y: f64,
    pub sx: f64,
    pub sy: f64,
    pub rot: f64,
    pub repeatx: f64,
    pub repeaty: f64,
    pub flipx: f64,
    pub flipy: f64,
    pub blendmode: f64,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
    pub burn: f64,
    pub done: f64,
}

impl Default for SpriteVars {
    fn default() -> Self {
        SpriteVars {
            x: 0.5,
            y: 0.5,
            sx: 1.0,
            sy: 1.0,
            rot: 0.0,
            repeatx: 1.0,
            repeaty: 1.0,
            flipx: 0.0,
            flipy: 0.0,
            blendmode: 0.0,
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
            burn: 1.0,
            done: 0.0,
        }
    }
}

/// One sprite's values for a frame, clamped (see the module docs); build it
/// with [`SpriteFrame::from_vars`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteFrame {
    /// `var_x * 2 - 1`, clamped.
    pub x: f32,
    pub y: f32,
    pub sx: f32,
    pub sy: f32,
    pub repeatx: f32,
    pub repeaty: f32,
    pub rot: f32,
    pub flipx: bool,
    pub flipy: bool,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    /// 0..=4.
    pub blendmode: u8,
    /// Also draw into the warp target (`var_burn != 0`).
    pub burn: bool,
    /// The sprite is killed after this frame (`var_done != 0`); it still
    /// draws, and it changes the burn quad ([`sprite_quads`]).
    pub done: bool,
    /// From [`crate::TextureStore::load_sprite`].
    pub texture: TextureId,
}

/// C `min`/`max` macros: `a < b ? a : b`, `a > b ? a : b` (NaN falls to `b`).
fn cmin(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

fn cmax(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

/// `min(hi, max(lo, v))`.
fn clamp_c(lo: f32, hi: f32, v: f32) -> f32 {
    cmin(hi, cmax(lo, v))
}

impl SpriteFrame {
    /// The clamped values of `v` as `DrawUserSprites` computes them
    /// (mfs:3542-3556).
    pub fn from_vars(v: &SpriteVars, texture: TextureId) -> SpriteFrame {
        let f = |d: f64| d as f32;
        let unit = |d: f64| clamp_c(0.0, 1.0, f(d));
        SpriteFrame {
            x: clamp_c(-1000.0, 1000.0, f(v.x) * 2.0 - 1.0),
            y: clamp_c(-1000.0, 1000.0, f(v.y) * 2.0 - 1.0),
            sx: clamp_c(-1000.0, 1000.0, f(v.sx)),
            sy: clamp_c(-1000.0, 1000.0, f(v.sy)),
            repeatx: clamp_c(0.01, 100.0, f(v.repeatx)),
            repeaty: clamp_c(0.01, 100.0, f(v.repeaty)),
            rot: f(v.rot),
            flipx: v.flipx != 0.0,
            flipy: v.flipy != 0.0,
            r: unit(v.r),
            g: unit(v.g),
            b: unit(v.b),
            a: unit(v.a),
            blendmode: c_int(v.blendmode).clamp(0, 4) as u8,
            burn: v.burn != 0.0,
            done: v.done != 0.0,
            texture,
        }
    }
}

/// The stage-0 shader mode and blend state of `blendmode` (mfs:3642-3729):
/// 0 `SRCALPHA/INVSRCALPHA` (3650-3684), 1 blending off (3685-3691), 2
/// `ONE/ONE` (3692-3698), 3 `SRCCOLOR/INVSRCCOLOR` (3699-3705), 4 colour
/// key: `MODULATE` on both colour and alpha, `SRCALPHA/INVSRCALPHA`
/// (3706-3728). Values outside 0..=4 take `default:` = mode 0 (3651).
pub fn sprite_state(blendmode: u8) -> (FixedMode, BlendMode) {
    match blendmode {
        1 => (FixedMode::TexModulate, BlendMode::Off),
        2 => (FixedMode::TexModulate, BlendMode::Additive),
        3 => (FixedMode::TexModulate, BlendMode::SrcColor),
        4 => (FixedMode::TexModulateAlpha, BlendMode::Alpha),
        _ => (FixedMode::TexModulate, BlendMode::Alpha),
    }
}

/// The vertex diffuse colour (R, G, B, A bytes): modes 0 and 4 `(r, g, b,
/// a)`, modes 1 and 2 `(r*a, g*a, b*a, 1)`, mode 3 white
/// (`D3DCOLOR_RGBA_01`, mfs:3683, 3690, 3697, 3704, 3727).
pub fn sprite_color(f: &SpriteFrame) -> [u8; 4] {
    match f.blendmode {
        1 | 2 => rgba_01_f32(f.r * f.a, f.g * f.a, f.b * f.a, 1.0),
        3 => rgba_01_f32(1.0, 1.0, 1.0, 1.0),
        _ => rgba_01_f32(f.r, f.g, f.b, f.a),
    }
}

/// The four strip vertices of a sprite, in the C++'s pre-projection
/// coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteQuads {
    /// The on-screen draw.
    pub screen: [Vertex; 4],
    /// The draw into the warp target, for a burnt sprite.
    pub burn: Option<[Vertex; 4]>,
}

/// The quads of `f` for an image of `img` pixels (`img_w`, `img_h`) and a
/// window of `window` pixels (mfs:3558-3640, 3731-3752); see the module
/// docs for the burn quad.
pub fn sprite_quads(f: &SpriteFrame, img: (u32, u32), window: (u32, u32)) -> SpriteQuads {
    let (flipx, flipy) = (usize::from(f.flipx), usize::from(f.flipy));
    let mut p = [[0.0_f32; 2]; 4];
    // Corners (3559-3566): the flips move which strip slot gets which sign.
    p[flipx][0] = -f.sx;
    p[1 - flipx][0] = f.sx;
    p[2 + flipx][0] = -f.sx;
    p[3 - flipx][0] = f.sx;
    p[flipy * 2][1] = -f.sy;
    p[1 + flipy * 2][1] = -f.sy;
    p[2 - flipy * 2][1] = f.sy;
    p[3 - flipy * 2][1] = f.sy;
    // First aspect ratio: non-1:1 images (3568-3576).
    let aspect = img.1 as f32 / img.0 as f32;
    for v in &mut p {
        if aspect < 1.0 {
            v[1] *= aspect;
        } else {
            v[0] /= aspect;
        }
    }
    // 2D rotation (3578-3589), then translation (3591-3596).
    let (sin, cos) = (f.rot.sin(), f.rot.cos());
    for v in &mut p {
        let (x2, y2) = (v[0] * cos - v[1] * sin, v[0] * sin + v[1] * cos);
        *v = [x2 + f.x, y2 + f.y];
    }
    // Second aspect ratio: the window (3598-3606).
    let aspect = window.0 as f32 / window.1 as f32;
    for v in &mut p {
        if aspect > 1.0 {
            v[1] *= aspect;
        } else {
            v[0] /= aspect;
        }
    }
    // Third aspect ratio: fit to the main texture, only for a burnt sprite
    // that is killed this frame (3608-3616); the screen draw then undoes it
    // whenever the sprite is burnt (3741-3748).
    let fit = window.0 as f32 / (window.1 as f32 * 4.0 / 3.0);
    let apply = |p: &mut [[f32; 2]; 4]| {
        for v in p.iter_mut() {
            if fit < 1.0 {
                v[0] *= fit;
            } else {
                v[1] /= fit;
            }
        }
    };
    let undo = |p: &mut [[f32; 2]; 4]| {
        for v in p.iter_mut() {
            if fit < 1.0 {
                v[0] /= fit;
            } else {
                v[1] *= fit;
            }
        }
    };
    let mut burn = None;
    if f.burn {
        if f.done {
            apply(&mut p);
        }
        burn = Some(p);
        undo(&mut p);
    }
    // UVs (3621-3640): corners (+-0.5), repeated about the centre.
    let color = sprite_color(f);
    let make = |p: [[f32; 2]; 4]| {
        let uv = [[-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]];
        std::array::from_fn(|k| Vertex {
            pos: [p[k][0], p[k][1], 0.0],
            color,
            uv: [
                uv[k][0] * f.repeatx + 0.5,
                uv[k][1] * f.repeaty + 0.5,
                0.0,
                0.0,
            ],
            rad_ang: [0.0; 2],
        })
    };
    SpriteQuads {
        screen: make(p),
        burn: burn.map(make),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> TextureId {
        TextureId(0)
    }

    fn frame(f: impl FnOnce(&mut SpriteVars)) -> SpriteFrame {
        let mut v = SpriteVars::default();
        f(&mut v);
        SpriteFrame::from_vars(&v, id())
    }

    /// `frame` with burn off, so the screen quad is the plain geometry.
    fn flat(f: impl FnOnce(&mut SpriteVars)) -> SpriteFrame {
        frame(|v| {
            v.burn = 0.0;
            f(v)
        })
    }

    #[test]
    fn clamps_match_cpp() {
        // x, y: (float)var * 2 - 1, then [-1000, 1000] (mfs:3542-3543).
        let f = frame(|v| (v.x, v.y) = (0.75, 0.25));
        assert_eq!((f.x, f.y), (0.5, -0.5));
        let f = frame(|v| (v.x, v.y) = (1e6, -1e6));
        assert_eq!((f.x, f.y), (1000.0, -1000.0));
        // sx, sy: [-1000, 1000], no *2-1 (3544-3545).
        let f = frame(|v| (v.sx, v.sy) = (5000.0, -5000.0));
        assert_eq!((f.sx, f.sy), (1000.0, -1000.0));
        let f = frame(|v| (v.sx, v.sy) = (0.3, -0.3));
        assert_eq!((f.sx, f.sy), (0.3_f32, -0.3_f32));
        // repeat: [0.01, 100] (3549-3550).
        let f = frame(|v| (v.repeatx, v.repeaty) = (0.0, 1e3));
        assert_eq!((f.repeatx, f.repeaty), (0.01, 100.0));
        let f = frame(|v| (v.repeatx, v.repeaty) = (-4.0, 2.5));
        assert_eq!((f.repeatx, f.repeaty), (0.01, 2.5));
        // blendmode: (int) truncation, then [0, 4] (3552).
        for (raw, want) in [
            (0.0, 0),
            (2.9, 2),
            (4.0, 4),
            (7.9, 4),
            (-3.0, 0),
            (-0.9, 0),
            (f64::NAN, 0),
            (3e9, 0),
        ] {
            assert_eq!(frame(|v| v.blendmode = raw).blendmode, want, "{raw}");
        }
        // r, g, b, a: [0, 1] (3553-3556).
        let f = frame(|v| (v.r, v.g, v.b, v.a) = (2.0, -1.0, 0.5, f64::INFINITY));
        assert_eq!((f.r, f.g, f.b, f.a), (1.0, 0.0, 0.5, 1.0));
        // flip: `== 0.0 ? 0 : 1`, so NaN flips (3547-3548).
        let f = frame(|v| (v.flipx, v.flipy) = (0.001, 0.0));
        assert!(f.flipx && !f.flipy);
        assert!(frame(|v| v.flipx = f64::NAN).flipx);
        assert!(!frame(|v| v.flipx = -0.0).flipx);
        // burn and done are `!= 0.0` (3498-3499); rot is unclamped.
        let f = frame(|v| (v.burn, v.done, v.rot) = (0.0, 2.0, 99.0));
        assert!(!f.burn && f.done && f.rot == 99.0);
        // The C++ `min`/`max` macros let NaN through.
        assert!(frame(|v| v.x = f64::NAN).x.is_nan());
        // The defaults of texmgr.cpp:216-231.
        let f = SpriteFrame::from_vars(&SpriteVars::default(), id());
        assert_eq!((f.x, f.y, f.sx, f.sy), (0.0, 0.0, 1.0, 1.0));
        assert!(f.burn && !f.done && f.blendmode == 0);
    }

    #[test]
    fn blendmode_table() {
        use wgpu::BlendFactor as F;
        let f = frame(|v| (v.r, v.g, v.b, v.a) = (0.5, 0.25, 1.0, 0.5));
        let st = |m: u8| {
            let (mode, blend) = sprite_state(m);
            (mode, blend.factors())
        };
        // 0: alpha blend, diffuse (r, g, b, a) (mfs:3650-3684).
        assert_eq!(
            st(0),
            (
                FixedMode::TexModulate,
                Some((F::SrcAlpha, F::OneMinusSrcAlpha))
            )
        );
        assert_eq!(sprite_color(&f), [127, 63, 255, 127]);
        // 1: decal, blending off, diffuse (r*a, g*a, b*a, 1) (3685-3691).
        assert_eq!(st(1), (FixedMode::TexModulate, None));
        let f1 = SpriteFrame { blendmode: 1, ..f };
        assert_eq!(sprite_color(&f1), [63, 31, 127, 255]);
        // 2: additive ONE, ONE, same diffuse (3692-3698).
        assert_eq!(st(2), (FixedMode::TexModulate, Some((F::One, F::One))));
        let f2 = SpriteFrame { blendmode: 2, ..f };
        assert_eq!(sprite_color(&f2), [63, 31, 127, 255]);
        // 3: SRCCOLOR, INVSRCCOLOR, diffuse white (3699-3705).
        assert_eq!(
            st(3),
            (FixedMode::TexModulate, Some((F::Src, F::OneMinusSrc)))
        );
        let f3 = SpriteFrame { blendmode: 3, ..f };
        assert_eq!(sprite_color(&f3), [255; 4]);
        // 4: colour key, ALPHAOP MODULATE, alpha blend, diffuse (r, g, b, a)
        // (3706-3728).
        assert_eq!(
            st(4),
            (
                FixedMode::TexModulateAlpha,
                Some((F::SrcAlpha, F::OneMinusSrcAlpha))
            )
        );
        let f4 = SpriteFrame { blendmode: 4, ..f };
        assert_eq!(sprite_color(&f4), [127, 63, 255, 127]);
        // Anything else takes `default:` = case 0 (3650-3651).
        assert_eq!(st(9), st(0));
        assert_eq!(
            sprite_color(&SpriteFrame { blendmode: 9, ..f }),
            sprite_color(&f)
        );
    }

    fn near(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
    }

    fn xy(q: &[Vertex; 4]) -> [[f32; 2]; 4] {
        q.map(|v| [v.pos[0], v.pos[1]])
    }

    fn uv(q: &[Vertex; 4]) -> [[f32; 2]; 4] {
        q.map(|v| [v.uv[0], v.uv[1]])
    }

    fn assert_quad(got: [[f32; 2]; 4], want: [[f32; 2]; 4]) {
        for (g, w) in got.iter().zip(&want) {
            assert!(near(*g, *w), "{got:?} != {want:?}");
        }
    }

    #[test]
    fn quad_corners_flip_and_uvs() {
        let sq = (64, 64);
        let win = (100, 100);
        let q = sprite_quads(&flat(|_| {}), sq, win).screen;
        assert_quad(xy(&q), [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]]);
        assert_quad(uv(&q), [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        // flipx swaps the x of each strip pair, flipy the rows; UVs stay.
        let q = sprite_quads(&flat(|v| v.flipx = 1.0), sq, win).screen;
        assert_quad(xy(&q), [[1.0, -1.0], [-1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]);
        assert_quad(uv(&q), [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        let q = sprite_quads(&flat(|v| v.flipy = 1.0), sq, win).screen;
        assert_quad(xy(&q), [[-1.0, 1.0], [1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]]);
        // repeat: (+-0.5) * repeat + 0.5.
        let q = sprite_quads(&flat(|v| (v.repeatx, v.repeaty) = (2.0, 0.5)), sq, win).screen;
        assert_quad(
            uv(&q),
            [[-0.5, 0.25], [1.5, 0.25], [-0.5, 0.75], [1.5, 0.75]],
        );
        // Translation: x = var*2-1 after the scale (0.1 of the half-width).
        let q = sprite_quads(
            &flat(|v| (v.sx, v.sy, v.x, v.y) = (0.1, 0.1, 1.0, 0.75)),
            sq,
            win,
        )
        .screen;
        assert_quad(xy(&q), [[0.9, 0.4], [1.1, 0.4], [0.9, 0.6], [1.1, 0.6]]);
        // Rotation by 90 degrees: (x, y) -> (-y, x).
        let q = sprite_quads(&flat(|v| v.rot = std::f64::consts::FRAC_PI_2), sq, win).screen;
        assert_quad(xy(&q), [[1.0, -1.0], [1.0, 1.0], [-1.0, -1.0], [-1.0, 1.0]]);
    }

    #[test]
    fn aspect_ratios_follow_the_cpp() {
        let f = flat(|_| {});
        // First: a wide image (h/w < 1) shrinks y, a tall one shrinks x.
        let q = sprite_quads(&f, (200, 100), (100, 100)).screen;
        assert_quad(xy(&q), [[-1.0, -0.5], [1.0, -0.5], [-1.0, 0.5], [1.0, 0.5]]);
        let q = sprite_quads(&f, (100, 200), (100, 100)).screen;
        assert_quad(xy(&q), [[-0.5, -1.0], [0.5, -1.0], [-0.5, 1.0], [0.5, 1.0]]);
        // Second: a wide window (w/h > 1) stretches y, a tall one shrinks x.
        let q = sprite_quads(&f, (64, 64), (200, 100)).screen;
        assert_quad(xy(&q), [[-1.0, -2.0], [1.0, -2.0], [-1.0, 2.0], [1.0, 2.0]]);
        let q = sprite_quads(&f, (64, 64), (100, 200)).screen;
        assert_quad(xy(&q), [[-2.0, -1.0], [2.0, -1.0], [-2.0, 1.0], [2.0, 1.0]]);
    }

    #[test]
    fn burn_geometry_mirrors_the_cpp_including_its_undo() {
        // 200x100 window: third aspect = 200 / (100 * 4/3) = 1.5 >= 1, so
        // the burn quad's y is divided by 1.5 (mfs:3608-3616) and the
        // screen draw undoes it by multiplying (3741-3748).
        let win = (200, 100);
        let base = sprite_quads(&frame(|v| v.burn = 0.0), (64, 64), win);
        assert!(base.burn.is_none());
        assert_quad(
            xy(&base.screen),
            [[-1.0, -2.0], [1.0, -2.0], [-1.0, 2.0], [1.0, 2.0]],
        );
        // Killed and burnt: the VS quad is fitted, the screen quad is not.
        let q = sprite_quads(&frame(|v| v.done = 1.0), (64, 64), win);
        let b = q.burn.expect("burn quad");
        let y = 2.0 / 1.5;
        assert_quad(xy(&b), [[-1.0, -y], [1.0, -y], [-1.0, y], [1.0, y]]);
        assert_quad(xy(&q.screen), xy(&base.screen));
        // Burnt but not killed: the VS quad is unfitted and the C++ still
        // runs the undo before the screen draw, so the screen quad is
        // stretched by 1.5 in y.
        let q = sprite_quads(&frame(|_| {}), (64, 64), win);
        assert_quad(xy(&q.burn.expect("burn quad")), xy(&base.screen));
        let y = 2.0 * 1.5;
        assert_quad(xy(&q.screen), [[-1.0, -y], [1.0, -y], [-1.0, y], [1.0, y]]);
        // A tall window (aspect < 1) scales x by the third aspect instead.
        let q = sprite_quads(&frame(|v| v.done = 1.0), (64, 64), (100, 200));
        let a = 100.0 / (200.0 * 4.0 / 3.0);
        let x = 2.0 * a;
        assert_quad(
            xy(&q.burn.unwrap()),
            [[-x, -1.0], [x, -1.0], [-x, 1.0], [x, 1.0]],
        );
    }

    #[test]
    fn vertices_carry_colour_and_depth_zero() {
        let f = frame(|v| (v.r, v.g, v.b, v.a) = (1.0, 0.5, 0.0, 1.0));
        let q = sprite_quads(&f, (8, 8), (64, 64)).screen;
        assert!(q
            .iter()
            .all(|v| v.color == [255, 127, 0, 255] && v.pos[2] == 0.0));
    }
}

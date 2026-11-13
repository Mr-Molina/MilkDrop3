//! Custom shapes (`CPlugin::DrawCustomShapes`, code/vis_milk2/milkdropfs.cpp
//! 2215-2400@6f7cd5d): the vertices of one shape instance. The GPU side
//! is [`crate::shapes_draw`].
//!
//! md-core runs each shape's per-frame program once per instance
//! (`LoadCustomShapePerFrameEvallibVars`, 2402-2450, then
//! `NSEEL_code_execute`, 2245-2248) and hands the resulting
//! [`ShapeFrameVars`] over in `FrameInput::shapes` (F38); the order of the
//! list is the C++'s: shape 0..4, instances 0.. within a shape.
//!
//! The C++'s values are `double`s read with `(int)` / `(float)` casts at the
//! point of use; the fields keep that, so a count like `sides` is truncated
//! here, not by the caller.

use crate::vertex::Vertex;
use std::f32::consts::PI;

/// A shape instance's variables after its per-frame program ran
/// (`var_pf_*` of `m_shape[i]`, 2402-2450). `instance`/`num_inst` (`instance`
/// and `instances`) only feed the program; drawing does not read them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShapeFrameVars {
    pub x: f64,
    pub y: f64,
    pub rad: f64,
    pub ang: f64,
    pub tex_ang: f64,
    pub tex_zoom: f64,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
    pub r2: f64,
    pub g2: f64,
    pub b2: f64,
    pub a2: f64,
    pub border_r: f64,
    pub border_g: f64,
    pub border_b: f64,
    pub border_a: f64,
    /// `(int)` truncated, then clamped to 3..=100 (2262-2264).
    pub sides: f64,
    /// `(int)textured != 0` samples the warp source (2321-2327).
    pub textured: f64,
    /// `(int)additive != 0`: `SRCALPHA, ONE` instead of `SRCALPHA,
    /// INVSRCALPHA` (2268), for the border too.
    pub additive: f64,
    /// `(int)thick != 0`: four offset copies of the border (2349).
    pub thick: f64,
    pub instance: u32,
    pub num_inst: u32,
}

/// What the geometry depends on besides the shape itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeView {
    /// `m_fAspectY` ([`crate::mesh_aspect`]'s y).
    pub aspect_y: f32,
    /// `m_nTexSizeX/Y`, for the thick border's offsets (2352-2353).
    pub tex: (u32, u32),
    /// `alpha_mult`: 1 outside a blend; while blending the new preset's
    /// shapes use `m_fBlendProgress` and the old one's `1 - progress`
    /// (2224-2226).
    pub alpha_mult: f32,
}

/// What [`build_shape`] produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeGeometry {
    /// The clamped side count: the fill is a fan of `sides + 2` vertices
    /// (`sides` triangles).
    pub sides: usize,
    pub textured: bool,
    pub additive: bool,
    /// How many `sides + 1`-vertex line strips `out_border` holds: 0 (no
    /// border), 1, or 4 when thick.
    pub border_copies: usize,
}

/// `((int)(c * 255 * mult)) & 0xFF` (2272-2283); `a` is multiplied in
/// `double`, as the C++'s `double * int * float` is.
fn byte(c: f64, mult: f32) -> u8 {
    ((c * 255.0 * f64::from(mult)) as i32 & 0xFF) as u8
}

fn color(r: f64, g: f64, b: f64, a: f64, alpha_mult: f32) -> [u8; 4] {
    [
        byte(r, 1.0),
        byte(g, 1.0),
        byte(b, 1.0),
        byte(a, alpha_mult),
    ]
}

/// Append one shape instance's vertices: the fill fan (`sides + 2`
/// vertices: centre, ring `1..=sides`, ring 1 again, 2268-2297) to
/// `out_fill` and, when `border_a > 0`, the border line strips (`sides + 1`
/// vertices each, ring 1 back to ring 1, 2330-2360) to `out_border`. Both
/// carry clip-space positions with the C++'s flipped y; the textured fill's
/// UVs are the texture-space ring (`tu`, `tv`, zero for the rest).
pub fn build_shape(
    v: &ShapeFrameVars,
    view: &ShapeView,
    out_fill: &mut Vec<Vertex>,
    out_border: &mut Vec<Vertex>,
) -> ShapeGeometry {
    let sides = (v.sides as i32).clamp(3, 100) as usize;
    let centre = [(v.x * 2.0 - 1.0) as f32, (v.y * -2.0 + 1.0) as f32];
    let inner = color(v.r, v.g, v.b, v.a, view.alpha_mult);
    let outer = color(v.r2, v.g2, v.b2, v.a2, view.alpha_mult);
    let (rad, ang) = (v.rad as f32, v.ang as f32);
    let (tex_ang, zoom) = (v.tex_ang as f32, v.tex_zoom as f32);

    let first = out_fill.len();
    out_fill.push(Vertex {
        pos: [centre[0], centre[1], 0.0],
        color: inner,
        uv: [0.5, 0.5, 0.0, 0.0],
        rad_ang: [0.0; 2],
    });
    for j in 1..=sides {
        let t = (j - 1) as f32 / sides as f32;
        // "DON'T TOUCH!" (2288-2291): the C++ evaluates in float, left to
        // right.
        let a = t * PI * 2.0 + ang + PI * 0.25;
        let ta = t * PI * 2.0 + tex_ang + PI * 0.25;
        out_fill.push(Vertex {
            pos: [
                centre[0] + rad * a.cos() * view.aspect_y,
                centre[1] + rad * a.sin(),
                0.0,
            ],
            color: outer,
            uv: [
                0.5 + 0.5 * ta.cos() / zoom * view.aspect_y,
                0.5 + 0.5 * ta.sin() / zoom,
                0.0,
                0.0,
            ],
            rad_ang: [0.0; 2],
        });
    }
    out_fill.push(out_fill[first + 1]);

    let mut border_copies = 0;
    if v.border_a > 0.0 {
        let c = color(
            v.border_r,
            v.border_g,
            v.border_b,
            v.border_a,
            view.alpha_mult,
        );
        let ring: Vec<Vertex> = out_fill[first + 1..first + sides + 2]
            .iter()
            .map(|p| Vertex {
                pos: p.pos,
                color: c,
                ..Vertex::default()
            })
            .collect();
        border_copies = if v.thick as i32 != 0 { 4 } else { 1 };
        let x_inc = 2.0 / view.tex.0 as f32;
        let y_inc = 2.0 / view.tex.1 as f32;
        // The offsets accumulate over the copies (2354-2359): +x, +y, -x.
        let mut off = [0.0f32; 2];
        for it in 0..border_copies {
            match it {
                1 => off[0] += x_inc,
                2 => off[1] += y_inc,
                3 => off[0] -= x_inc,
                _ => {}
            }
            out_border.extend(ring.iter().map(|p| Vertex {
                pos: [p.pos[0] + off[0], p.pos[1] + off[1], p.pos[2]],
                ..*p
            }));
        }
    }
    ShapeGeometry {
        sides,
        textured: v.textured as i32 != 0,
        additive: v.additive as i32 != 0,
        border_copies,
    }
}

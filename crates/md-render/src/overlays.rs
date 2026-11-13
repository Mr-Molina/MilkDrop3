//! The fixed-function overlays of the main-texture draw sequence: the
//! flexible motion-vector field (`CPlugin::DrawMotionVectors`,
//! code/vis_milk2/milkdropfs.cpp:1156-1305@6f7cd5d, with
//! `ReversePropagatePoint`, 1431-1466), the outer and inner borders and
//! the `darken_center` fan (both in `CPlugin::DrawSprites`, 3300-3405). The
//! GPU side is [`crate::overlays_draw`].
//!
//! All values are the *post-equation per-frame values* of `FrameInput::vars`
//! (F38). While a preset blend runs `RunPerFrameEquations` has already
//! blended them into the current state (mv_* and ob_*/ib_* by `mix`,
//! `darken_center` by the snap point, 706-733), so each overlay is drawn
//! once, from the blended values, never once per preset.
//!
//! The vertices are the C++'s, in clip space with no flip (`WFVERTEX`
//! positions are used as is by these three functions), colours from
//! `D3DCOLOR_RGBA_01` ([`crate::cast::rgba_01_f32`]).

use crate::cast::{c_int, rgba_01_f32};
use crate::frame_vars::PerFrameVars;
use crate::vertex::Vertex;

/// The motion-vector variables (`var_pf_mv_*`), as `double`s the C++ casts
/// at the point of use.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MotionVectorVars {
    /// `mv_x`, `mv_y`: the grid counts; their fractional part shifts the
    /// grid (1173-1176).
    pub x: f64,
    pub y: f64,
    /// `mv_dx`, `mv_dy`: the grid offset (1206-1207).
    pub dx: f64,
    pub dy: f64,
    /// `mv_l`: the trail length multiplier.
    pub l: f64,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl MotionVectorVars {
    /// The `mv_*` values of a frame's variables.
    pub fn from_vars(v: &PerFrameVars) -> MotionVectorVars {
        MotionVectorVars {
            x: v.mv_x,
            y: v.mv_y,
            dx: v.mv_dx,
            dy: v.mv_dy,
            l: v.mv_l,
            r: v.mv_r,
            g: v.mv_g,
            b: v.mv_b,
            a: v.mv_a,
        }
    }
}

/// One border's variables (`ob_*` or `ib_*`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Border {
    /// The fraction of the half-screen the border covers (`ob_size`).
    pub size: f64,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Border {
    /// `ob_size`, `ob_r`.. of a frame's variables.
    pub fn outer(v: &PerFrameVars) -> Border {
        Border {
            size: v.ob_size,
            r: v.ob_r,
            g: v.ob_g,
            b: v.ob_b,
            a: v.ob_a,
        }
    }

    /// `ib_size`, `ib_r`.. of a frame's variables.
    pub fn inner(v: &PerFrameVars) -> Border {
        Border {
            size: v.ib_size,
            r: v.ib_r,
            g: v.ib_g,
            b: v.ib_b,
            a: v.ib_a,
        }
    }
}

/// What `m_verts` holds for the motion vectors: the warp mesh's grid and
/// its warped UVs (`tu`, `tv`).
///
/// The C++ draws the vectors at the top of `RenderFrame` (965), *before*
/// `ComputeGridAlphaValues` (1006) rewrites `m_verts`, so the UVs are the
/// **previous** frame's ([`crate::Renderer`] keeps that snapshot). The
/// UVs here are [`crate::WarpMesh::uvs`] (no half-texel offset); the
/// offset `0.5 / tex` the C++ adds (1836-1837) is applied when read.
#[derive(Clone, Copy, Debug)]
pub struct MotionMesh<'a> {
    /// `(m_nGridX, m_nGridY)`.
    pub grid: (u32, u32),
    /// `(grid_x + 1) * (grid_y + 1)` `(tu, tv)` pairs, row-major.
    pub uvs: &'a [[f32; 2]],
    /// `(m_nTexSizeX, m_nTexSizeY)`.
    pub tex: (u32, u32),
}

/// `ReversePropagatePoint` (1431-1466): the bilinear lookup of the mesh UVs
/// at `(fx, fy)` (both 0..1, y up), returning `(tu, 1 - tv)`; `None` where
/// the C++ returns false (a cell outside the grid, 1444-1451).
pub fn reverse_propagate_point(mesh: &MotionMesh, fx: f32, fy: f32) -> Option<(f32, f32)> {
    let (gx, gy) = (mesh.grid.0 as i32, mesh.grid.1 as i32);
    let y0 = c_int(f64::from(fy * gy as f32));
    let dy = fy * gy as f32 - y0 as f32;
    let x0 = c_int(f64::from(fx * gx as f32));
    let dx = fx * gx as f32 - x0 as f32;
    let (x1, y1) = (x0.wrapping_add(1), y0.wrapping_add(1));
    if x0 < 0 || y0 < 0 || x1 > gx || y1 > gy {
        return None;
    }
    let stride = (gx + 1) as usize;
    let ox = 0.5 / mesh.tex.0.max(1) as f32;
    let oy = 0.5 / mesh.tex.1.max(1) as f32;
    let at = |x: i32, y: i32| -> Option<[f32; 2]> {
        let uv = mesh.uvs.get(y as usize * stride + x as usize)?;
        Some([uv[0] + ox, uv[1] + oy])
    };
    let (a, b, c, d) = (at(x0, y0)?, at(x1, y0)?, at(x0, y1)?, at(x1, y1)?);
    // 1454-1461: accumulated left to right in `float`.
    let mut tu = a[0] * (1.0 - dx) * (1.0 - dy);
    let mut tv = a[1] * (1.0 - dx) * (1.0 - dy);
    tu += b[0] * dx * (1.0 - dy);
    tv += b[1] * dx * (1.0 - dy);
    tu += c[0] * (1.0 - dx) * dy;
    tv += c[1] * (1.0 - dx) * dy;
    tu += d[0] * dx * dy;
    tv += d[1] * dx * dy;
    Some((tu, 1.0 - tv))
}

/// Append the motion-vector field as a `LineList` (two vertices per line,
/// `DrawPrimitiveUP(D3DPT_LINELIST)` per row in the C++, 1301; one list is
/// the same triangles' worth of lines in the same order) and return the
/// number of lines.
///
/// `nX = (int)mv_x` and `nY = (int)mv_y` (1173-1174) are capped at 64 and 48
/// (1177-1178, which also zero the fractional part `dx`/`dy` of an axis
/// that was capped); nothing is drawn unless `mv_a >= 0.001` (1159) and
/// both counts are positive (1180). The fractional parts, clamped to 0..1
/// (1210-1213), widen the grid: `f = (i + 0.25) / (n + frac + 0.25 - 1)`,
/// then the grid is moved by `mv_dx`/`mv_dy` (1231-1245) and points outside
/// `(0.0001, 0.9999)` are skipped (1236, 1247). Each vector runs from the
/// grid point `(fx, fy)` to the point [`reverse_propagate_point`] finds,
/// scaled by `mv_l`, never shorter than one texel along the diagonal
/// (1256-1282: a zero-length vector, including `mv_l = 0`, becomes
/// `(min_len, min_len)`, so it is drawn, not dropped).
///
/// Where `reverse_propagate_point` returns `None` the C++ ignores the
/// result and reads uninitialized floats; the cell test cannot fail for a
/// point inside `(0.0001, 0.9999)`, so this is unreachable and the point is
/// taken as its own image (a zero vector).
pub fn build_motion_vectors(
    mv: &MotionVectorVars,
    mesh: &MotionMesh,
    out: &mut Vec<Vertex>,
) -> usize {
    if (mv.a as f32) < 0.001 || mv.a.is_nan() {
        return 0;
    }
    let mut n_x = c_int(mv.x);
    let mut n_y = c_int(mv.y);
    let mut dx = mv.x as f32 - n_x as f32;
    let mut dy = mv.y as f32 - n_y as f32;
    if n_x > 64 {
        n_x = 64;
        dx = 0.0;
    }
    if n_y > 48 {
        n_y = 48;
        dy = 0.0;
    }
    if n_x <= 0 || n_y <= 0 {
        return 0;
    }
    let dx2 = mv.dx as f32;
    let dy2 = mv.dy as f32;
    let len_mult = mv.l as f32;
    // `if (dx < 0) dx = 0; ... if (dx > 1) dx = 1;` (NaN passes through).
    if dx < 0.0 {
        dx = 0.0;
    }
    if dy < 0.0 {
        dy = 0.0;
    }
    if dx > 1.0 {
        dx = 1.0;
    }
    if dy > 1.0 {
        dy = 1.0;
    }
    let min_len = 1.0 * (1.0 / mesh.tex.0.max(1) as f32);
    let color = rgba_01_f32(mv.r as f32, mv.g as f32, mv.b as f32, mv.a as f32);
    let start = out.len();
    for y in 0..n_y {
        let mut fy = (y as f32 + 0.25) / (n_y as f32 + dy + 0.25 - 1.0);
        fy -= dy2;
        if !(fy > 0.0001 && fy < 0.9999) {
            continue;
        }
        for x in 0..n_x {
            let mut fx = (x as f32 + 0.25) / (n_x as f32 + dx + 0.25 - 1.0);
            fx += dx2;
            if !(fx > 0.0001 && fx < 0.9999) {
                continue;
            }
            let (mut fx2, mut fy2) = reverse_propagate_point(mesh, fx, fy).unwrap_or((fx, fy));
            // "Enforce minimum trail lengths" (1256-1282).
            let mut ddx = (fx2 - fx) * len_mult;
            let mut ddy = (fy2 - fy) * len_mult;
            let mut len = (ddx * ddx + ddy * ddy).sqrt();
            if len > min_len {
            } else if len > 0.000_000_01 {
                len = min_len / len;
                ddx *= len;
                ddy *= len;
            } else {
                ddx = min_len;
                ddy = min_len;
            }
            fx2 = fx + ddx;
            fy2 = fy + ddy;
            for (px, py) in [(fx, fy), (fx2, fy2)] {
                out.push(Vertex {
                    pos: [px * 2.0 - 1.0, py * 2.0 - 1.0, 0.0],
                    color,
                    ..Vertex::default()
                });
            }
        }
    }
    (out.len() - start) / 2
}

/// Append the four-vertex triangle-fan quads of both borders, outer first
/// (`it` 0 and 1, 3358-3402): each border with `a > 0.001` gives four
/// quads (one per 90-degree rotation, `cosf`/`sinf` of `1.570796327f` in
/// `float`, so the rotated corners carry the C++'s rounding), 16 vertices,
/// each quad `[(inner, inner), (outer, outer), (outer, -outer), (inner,
/// -inner)]` with `outer = 1` / `inner = 1 - ob_size` for the outer border
/// and `outer = 1 - ob_size` / `inner = 1 - ob_size - ib_size` for the
/// inner one (3376-3377). Returns the number of quads.
pub fn build_borders(ob: Border, ib: Border, out: &mut Vec<Vertex>) -> usize {
    let outer_size = ob.size as f32;
    let inner_size = ib.size as f32;
    let start = out.len();
    for (it, b) in [ob, ib].into_iter().enumerate() {
        let (r, g, bl, a) = (b.r as f32, b.g as f32, b.b as f32, b.a as f32);
        if a > 0.001 {
            let color = rgba_01_f32(r, g, bl, a);
            let (inner_rad, outer_rad) = if it == 0 {
                (1.0 - outer_size, 1.0)
            } else {
                (1.0 - outer_size - inner_size, 1.0 - outer_size)
            };
            let mut v = [
                [inner_rad, inner_rad],
                [outer_rad, outer_rad],
                [outer_rad, -outer_rad],
                [inner_rad, -inner_rad],
            ];
            for _ in 0..4 {
                out.extend(v.iter().map(|p| Vertex {
                    pos: [p[0], p[1], 0.0],
                    color,
                    ..Vertex::default()
                }));
                // "rotate by 90 degrees" (3391-3399).
                let t = std::f32::consts::FRAC_PI_2; // 1.570796327f rounds to this
                for p in &mut v {
                    let (x, y) = (p[0], p[1]);
                    *p = [x * t.cos() - y * t.sin(), x * t.sin() + y * t.cos()];
                }
            }
        }
    }
    (out.len() - start) / 4
}

/// Append the `darken_center` fan (3310-3347): six vertices, drawn as a
/// `D3DPT_TRIANGLEFAN` of 4 triangles. Vertex 0 is the centre, black with
/// alpha `3/32` (byte `(int)(3.0f/32.0f*255)` = 23); vertices 1-5 are black
/// with alpha 0 and form a diamond of half-size `0.05` (`x` scaled by
/// `aspect_y`, `m_fAspectY`): left, bottom, right, top, left again.
pub fn build_darken_center(aspect_y: f32, out: &mut Vec<Vertex>) {
    let center = rgba_01_f32(0.0, 0.0, 0.0, 3.0 / 32.0);
    let edge = rgba_01_f32(0.0, 0.0, 0.0, 0.0 / 32.0);
    let half = 0.05f32;
    let left = 0.0 - half * aspect_y;
    let pts = [
        ([0.0, 0.0], center),
        ([left, 0.0], edge),
        ([0.0, 0.0 - half], edge),
        ([0.0 + half * aspect_y, 0.0], edge),
        ([0.0, 0.0 + half], edge),
        ([left, 0.0], edge),
    ];
    out.extend(pts.map(|(p, color)| Vertex {
        pos: [p[0], p[1], 0.0],
        color,
        ..Vertex::default()
    }));
}

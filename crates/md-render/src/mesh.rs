//! The warp mesh (Task 5.6) and the per-frame / per-pixel variable blocks
//! md-core fills (F37: defined here so md-render never depends on md-eval
//! or md-core).
//!
//! [`WarpMesh::new`] mirrors the mesh set-up of `AllocateMyDX9Stuff`
//! (code/vis_milk2/plugin.cpp:2263-2361): vertex positions, `rad`/`ang`
//! and `m_indices_list`. [`WarpMesh::compute_uvs`] mirrors one `rep` of
//! `ComputeGridAlphaValues` (milkdropfs.cpp:1681-1864).
//!
//! Divergence (documented): the mesh stores the warped UVs **without** the
//! final half-texel offset `+0.5/texsize` (milkdropfs.cpp:1835-1837),
//! because it does not know the texture size; the warp pass adds it when
//! it builds the vertices (`crate::warp`). Mixing is affine, so the only
//! difference is float rounding in the last bit.

use crate::frame_vars::PerFrameVars;

/// Largest mesh the C++ accepts (`MAX_GRID_X`, `MAX_GRID_Y`, plugin.h).
pub const MAX_GRID_X: u32 = 192;
pub const MAX_GRID_Y: u32 = 144;

/// Per-vertex constants (`td_vertinfo`, A.3): `rad`/`ang` set at mesh
/// creation; `a`/`c` are the blend pattern (`mix2 = clamp(a*t + c, 0, 1)`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertInfo {
    pub rad: f32,
    pub ang: f32,
    pub a: f32,
    pub c: f32,
}

/// The per-vertex variables of a preset's per-pixel program
/// (`var_pv_*` that change per vertex, milkdropfs.cpp:1749-1762): inputs
/// `x y rad ang` and the motion values, which the program may overwrite.
/// Every other per-pixel variable (`time`, `bass`, `q1..q32`, …) is set
/// once per frame by md-core, as the C++ does (milkdropfs.cpp:640-660).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PerPixelVars {
    pub x: f64,
    pub y: f64,
    pub rad: f64,
    pub ang: f64,
    pub zoom: f64,
    pub zoomexp: f64,
    pub rot: f64,
    pub warp: f64,
    pub cx: f64,
    pub cy: f64,
    pub dx: f64,
    pub dy: f64,
    pub sx: f64,
    pub sy: f64,
}

/// A preset's per-pixel program, supplied by md-core (F37): it loads the
/// [`PerPixelVars`] into the evaluator, runs the code and writes the
/// motion values back.
pub type PerPixelRunner<'a> = dyn FnMut(&mut PerPixelVars) + 'a;

/// The aspect factors of a `w`×`h` warp texture (`m_fAspectX/Y`,
/// plugin.cpp:2035-2036): the shorter side gets `short/long`, the other 1.
pub fn mesh_aspect(w: u32, h: u32) -> (f32, f32) {
    let (w, h) = (w.max(1) as f32, h.max(1) as f32);
    let ax = if h > w { w / h } else { 1.0 };
    let ay = if w > h { h / w } else { 1.0 };
    (ax, ay)
}

/// The warp-animation values of `ComputeGridAlphaValues`
/// (milkdropfs.cpp:1700-1706), computed from the *current* preset once per
/// frame and shared by both `rep`s.
#[derive(Clone, Copy, Debug)]
struct WarpAnim {
    time: f32,
    scale_inv: f32,
    f: [f32; 4],
}

impl WarpAnim {
    fn new(v: &PerFrameVars, time: f32) -> WarpAnim {
        let wt = time * v.warp_anim_speed;
        WarpAnim {
            time: wt,
            scale_inv: 1.0 / v.warp_scale,
            f: [
                11.68 + 4.0 * (wt * 1.413 + 10.0).cos(),
                8.77 + 3.0 * (wt * 1.113 + 7.0).cos(),
                10.54 + 3.0 * (wt * 1.233 + 3.0).cos(),
                11.49 + 4.0 * (wt * 0.933 + 5.0).cos(),
            ],
        }
    }
}

/// The warp mesh: `(grid_x + 1) * (grid_y + 1)` vertices with their
/// per-frame UVs and blend alphas.
#[derive(Clone, Debug)]
pub struct WarpMesh {
    grid_x: u32,
    grid_y: u32,
    aspect: (f32, f32),
    pos: Vec<[f32; 2]>,
    info: Vec<VertInfo>,
    uv: Vec<[f32; 2]>,
    alpha: Vec<u8>,
    indices: Vec<u32>,
    anim: Option<WarpAnim>,
}

impl WarpMesh {
    /// A `grid_x`×`grid_y` mesh (each clamped to `1..=MAX_GRID_*`; the C++
    /// clamps the maximum only, plugin.cpp:1229-1232) for a texture with
    /// aspect factors [`mesh_aspect`]. UVs start as the identity.
    pub fn new(grid_x: u32, grid_y: u32, aspect_x: f32, aspect_y: f32) -> WarpMesh {
        let (gx, gy) = (grid_x.clamp(1, MAX_GRID_X), grid_y.clamp(1, MAX_GRID_Y));
        let n = ((gx + 1) * (gy + 1)) as usize;
        let mut pos = Vec::with_capacity(n);
        let mut info = Vec::with_capacity(n);
        // plugin.cpp:2279-2303.
        for y in 0..=gy {
            for x in 0..=gx {
                let px = x as f32 / gx as f32 * 2.0 - 1.0;
                let py = y as f32 / gy as f32 * 2.0 - 1.0;
                let rad = (px * px * aspect_x * aspect_x + py * py * aspect_y * aspect_y).sqrt();
                let ang = if y == gy / 2 && x == gx / 2 {
                    0.0
                } else {
                    (py * aspect_y).atan2(px * aspect_x)
                };
                pos.push([px, py]);
                info.push(VertInfo {
                    rad,
                    ang,
                    a: 1.0,
                    c: 0.0,
                });
            }
        }
        let uv = pos
            .iter()
            .map(|p| [p[0] * 0.5 + 0.5, -p[1] * 0.5 + 0.5])
            .collect();
        WarpMesh {
            grid_x: gx,
            grid_y: gy,
            aspect: (aspect_x, aspect_y),
            pos,
            info,
            uv,
            alpha: vec![255; n],
            indices: index_list(gx, gy),
            anim: None,
        }
    }

    /// `(grid_x, grid_y)` after clamping.
    pub fn grid(&self) -> (u32, u32) {
        (self.grid_x, self.grid_y)
    }

    /// `(aspect_x, aspect_y)`.
    pub fn aspect(&self) -> (f32, f32) {
        self.aspect
    }

    /// `m_vertinfo`: `rad`, `ang` (and `a = 1`, `c = 0`) per vertex.
    pub fn vert_info(&self) -> &[VertInfo] {
        &self.info
    }

    /// Clip-space `(x, y)` per vertex (`m_verts[].x/.y`, `z = 0`).
    pub fn positions(&self) -> &[[f32; 2]] {
        &self.pos
    }

    /// The warped UVs (`m_verts[].tu/.tv` without the half-texel offset).
    pub fn uvs(&self) -> &[[f32; 2]] {
        &self.uv
    }

    /// The blend alpha per vertex (`m_verts[].Diffuse >> 24`): 255 unless
    /// the last [`compute_uvs`](Self::compute_uvs) blended.
    pub fn alphas(&self) -> &[u8] {
        &self.alpha
    }

    /// `m_indices_list` (plugin.cpp:2335-2361): triangle-list indices over
    /// four quadrants (bottom-left, bottom-right, top-left, top-right),
    /// each walked from the screen centre outwards so the first half of
    /// the list (quadrants 0, 1) covers rows `0..grid_y/2` and the second
    /// half the rest. Odd grids leave the middle row/column out, as the
    /// C++ list does (the C++ then reads uninitialized indices for them).
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Compute the UVs of one preset (one `rep` of `ComputeGridAlphaValues`).
    ///
    /// Call it once per frame with the current preset and `blend = None`
    /// (`rep 0`: UVs replaced, alpha 255), then — while blending — once
    /// more with the old preset and `blend = Some((pattern, blend_progress))`
    /// (`rep 1`): the old UVs are mixed in with
    /// `mix2 = clamp(a * t + c, 0, 1)` as `tu = tu * mix2 + u_old * (1 - mix2)`
    /// and the vertex alpha becomes `(DWORD)(mix2 * 255)`. `pattern` holds the
    /// blend pattern's `a`, `c` per vertex (from
    /// [`crate::randomize_blend_pattern`]).
    ///
    /// The warp animation (`fWarpTime`, `fWarpScaleInv`, `f[4]`) comes from
    /// the *current* preset in both reps, as in the C++ (milkdropfs.cpp:1700):
    /// the `rep 0` call caches it and `rep 1` reuses it.
    ///
    /// `per_pixel`, when the preset has per-pixel code, runs once per vertex
    /// with `x = px*0.5*aspect_x + 0.5`, `y = py*-0.5*aspect_y + 0.5`, `rad`,
    /// `ang` and the per-frame motion values, and may overwrite the latter.
    ///
    /// # Panics
    /// If a blend `pattern` is shorter than the mesh.
    pub fn compute_uvs(
        &mut self,
        per_frame: &PerFrameVars,
        mut per_pixel: Option<&mut PerPixelRunner<'_>>,
        time: f32,
        blend: Option<(&[VertInfo], f32)>,
    ) {
        if blend.is_none() || self.anim.is_none() {
            self.anim = Some(WarpAnim::new(per_frame, time));
        }
        let anim = self.anim.expect("set above");
        if let Some((pattern, _)) = blend {
            assert!(pattern.len() >= self.pos.len(), "blend pattern too short");
        }
        let (ax, ay) = self.aspect;
        let (inv_ax, inv_ay) = (1.0 / ax, 1.0 / ay);
        let pf = per_frame;
        let base = PerPixelVars {
            zoom: pf.zoom,
            zoomexp: pf.zoomexp,
            rot: pf.rot,
            warp: pf.warp,
            cx: pf.cx,
            cy: pf.cy,
            dx: pf.dx,
            dy: pf.dy,
            sx: pf.sx,
            sy: pf.sy,
            ..PerPixelVars::default()
        };
        for n in 0..self.pos.len() {
            let [px, py] = self.pos[n];
            let VertInfo { rad, ang, .. } = self.info[n];
            let mut v = base;
            if let Some(run) = per_pixel.as_deref_mut() {
                // milkdropfs.cpp:1749-1762.
                v.x = f64::from(px * 0.5 * ax + 0.5);
                v.y = f64::from(py * -0.5 * ay + 0.5);
                v.rad = f64::from(rad);
                v.ang = f64::from(ang);
                run(&mut v);
            }
            let (u, vv) = warp_uv(&v, px, py, rad, ax, ay, &anim);
            // milkdropfs.cpp:1832-1834: undo the aspect fix.
            let u = (u - 0.5) * inv_ax + 0.5;
            let vv = (vv - 0.5) * inv_ay + 0.5;
            match blend {
                None => {
                    self.uv[n] = [u, vv];
                    self.alpha[n] = 255;
                }
                Some((pattern, t)) => {
                    // milkdropfs.cpp:1848-1858.
                    let VertInfo { a, c, .. } = pattern[n];
                    let mix2 = (a * t + c).clamp(0.0, 1.0);
                    let [tu, tv] = self.uv[n];
                    self.uv[n] = [tu * mix2 + u * (1.0 - mix2), tv * mix2 + vv * (1.0 - mix2)];
                    self.alpha[n] = (mix2 * 255.0) as u8;
                }
            }
        }
    }
}

/// The warped `(u, v)` of one vertex before the aspect undo
/// (milkdropfs.cpp:1795-1830), in `f32` as the C++ computes it.
fn warp_uv(
    v: &PerPixelVars,
    px: f32,
    py: f32,
    rad: f32,
    ax: f32,
    ay: f32,
    anim: &WarpAnim,
) -> (f32, f32) {
    let (zoom, zoomexp, rot, warp) = (v.zoom as f32, v.zoomexp as f32, v.rot as f32, v.warp as f32);
    let (cx, cy, dx, dy, sx, sy) = (
        v.cx as f32,
        v.cy as f32,
        v.dx as f32,
        v.dy as f32,
        v.sx as f32,
        v.sy as f32,
    );
    let zoom2 = zoom.powf(zoomexp.powf(rad * 2.0 - 1.0));
    let zoom2_inv = 1.0 / zoom2;
    let mut u = px * ax * 0.5 * zoom2_inv + 0.5;
    let mut vv = -py * ay * 0.5 * zoom2_inv + 0.5;
    // stretch
    u = (u - cx) / sx + cx;
    vv = (vv - cy) / sy + cy;
    // warp
    let (wt, ws, f) = (anim.time, anim.scale_inv, anim.f);
    let w = warp * 0.0035;
    u += w * (wt * 0.333 + ws * (px * f[0] - py * f[3])).sin();
    vv += w * (wt * 0.375 - ws * (px * f[2] + py * f[1])).cos();
    u += w * (wt * 0.753 - ws * (px * f[1] - py * f[2])).cos();
    vv += w * (wt * 0.825 + ws * (px * f[0] + py * f[3])).sin();
    // rotation about (cx, cy)
    let (u2, v2) = (u - cx, vv - cy);
    let (sin_rot, cos_rot) = rot.sin_cos();
    u = u2 * cos_rot - v2 * sin_rot + cx;
    vv = u2 * sin_rot + v2 * cos_rot + cy;
    // translation
    (u - dx, vv - dy)
}

/// `m_indices_list` (plugin.cpp:2335-2361).
fn index_list(gx: u32, gy: u32) -> Vec<u32> {
    let mut out = Vec::with_capacity((gx * gy * 6) as usize);
    for quadrant in 0..4 {
        for slice in 0..gy / 2 {
            for i in 0..gx / 2 {
                let xref = if quadrant & 1 != 0 { gx - 1 - i } else { i };
                let yref = if quadrant & 2 != 0 {
                    gy - 1 - slice
                } else {
                    slice
                };
                let v = xref + yref * (gx + 1);
                let row = gx + 1;
                out.extend_from_slice(&[v, v + 1, v + row, v + 1, v + row, v + row + 1]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_follows_tex_size() {
        assert_eq!(mesh_aspect(640, 480), (1.0, 0.75));
        assert_eq!(mesh_aspect(480, 640), (0.75, 1.0));
        assert_eq!(mesh_aspect(64, 64), (1.0, 1.0));
    }

    #[test]
    fn index_list_covers_every_cell_once_for_even_grids() {
        let (gx, gy) = (6, 4);
        let idx = index_list(gx, gy);
        assert_eq!(idx.len(), (gx * gy * 6) as usize);
        let mut cells: Vec<u32> = idx.chunks(6).map(|c| c[0]).collect();
        cells.sort_unstable();
        let want: Vec<u32> = (0..gy)
            .flat_map(|y| (0..gx).map(move |x| y * (gx + 1) + x))
            .collect();
        assert_eq!(cells, want);
        // The first half (quadrants 0, 1) is the bottom half of the mesh.
        assert!(idx[..idx.len() / 2]
            .chunks(6)
            .all(|c| c[0] / (gx + 1) < gy / 2));
    }

    #[test]
    fn grid_is_clamped() {
        let m = WarpMesh::new(1000, 0, 1.0, 1.0);
        assert_eq!(m.grid(), (MAX_GRID_X, 1));
    }
}

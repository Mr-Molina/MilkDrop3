//! Blend patterns for preset transitions: `RandomizeBlendPattern` and
//! `GenPlasma` (code/vis_milk2/plugin.cpp:7043-7199).
//!
//! Each pattern writes `a`, `c` per mesh vertex so that the warp mesh's
//! blend weight is `mix2 = clamp(a * t + c, 0, 1)` ([`crate::WarpMesh::compute_uvs`]):
//! with `band` the transition width and `t_v` in `[0, 1]` the vertex's
//! position in the sweep, `a = (1 + band) / band`, `c = (t_v - 1) / band`.
//!
//! Randomness (A.4): the preset RNG ([`rand_pcg::Pcg32`], any [`Rng`])
//! replaces `rand()`; [`frand`] keeps `FRAND`'s `(rand() % 7381) / 7380`
//! grid on the 15-bit [`crate::noise::crand`].
//!
//! Divergence: the API takes the mesh aspect (`m_fAspectX/Y`), which the
//! C++ reads from the plugin and the plan's signature omitted.

use crate::mesh::VertInfo;
use crate::noise::crand;
use rand_core::Rng;

/// The C++'s literal `6.28f` (not `TAU`): angles in `[0, 6.28]`.
#[allow(clippy::approx_constant)]
pub(crate) const CPP_6_28: f32 = 6.28;

/// The C++'s literal `1.41421f` (radial pattern, plugin.cpp:7150).
#[allow(clippy::approx_constant)]
const CPP_SQRT_2: f32 = 1.41421;

/// `FRAND` (plugin.cpp:508): uniform on `{0, 1/7380, …, 1}`.
pub fn frand<R: Rng + ?Sized>(rng: &mut R) -> f32 {
    (crand(rng) % 7381) as f32 / 7380.0
}

/// The blend pattern types (`mixtype`, plugin.cpp:7049-7163).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendPattern {
    /// `0`: constant uniform blend (`a = 1`, `c = 0`). The C++ no longer
    /// picks it ("half-speed for shader blending").
    Uniform,
    /// `1`: directional wipe.
    Wipe,
    /// `2`: plasma.
    Plasma,
    /// `3`: radial, outside-in or inside-out.
    Radial,
}

/// `RandomizeBlendPattern`: `mixtype = 1 + rand() % 3` (wipe, plasma or
/// radial), written into `out` (`(grid_x + 1) * (grid_y + 1)` entries;
/// only `a` and `c` change).
///
/// # Panics
/// If `out` is shorter than the mesh.
pub fn randomize_blend_pattern<R: Rng + ?Sized>(
    rng: &mut R,
    grid_x: u32,
    grid_y: u32,
    aspect: (f32, f32),
    out: &mut [VertInfo],
) {
    let pattern = match 1 + crand(rng) % 3 {
        1 => BlendPattern::Wipe,
        2 => BlendPattern::Plasma,
        _ => BlendPattern::Radial,
    };
    blend_pattern(rng, pattern, grid_x, grid_y, aspect, out);
}

/// Write blend pattern `pattern` into `out`, as one branch of
/// `RandomizeBlendPattern`.
///
/// # Panics
/// If `out` is shorter than `(grid_x + 1) * (grid_y + 1)`.
pub fn blend_pattern<R: Rng + ?Sized>(
    rng: &mut R,
    pattern: BlendPattern,
    grid_x: u32,
    grid_y: u32,
    aspect: (f32, f32),
    out: &mut [VertInfo],
) {
    let (gx, gy) = (grid_x.max(1), grid_y.max(1));
    let n = ((gx + 1) * (gy + 1)) as usize;
    assert!(out.len() >= n, "blend pattern buffer too short");
    let out = &mut out[..n];
    let (ax, ay) = aspect;
    // Every branch but Uniform: a = inv_band*(1+band), c = -inv_band + inv_band*t.
    let set = |v: &mut VertInfo, band: f32, t: f32| {
        let inv_band = 1.0 / band;
        v.a = inv_band * (1.0 + band);
        v.c = -inv_band + inv_band * t;
    };
    let at = |x: u32, y: u32| (y * (gx + 1) + x) as usize;
    match pattern {
        BlendPattern::Uniform => {
            for v in out.iter_mut() {
                v.a = 1.0;
                v.c = 0.0;
            }
        }
        BlendPattern::Wipe => {
            // plugin.cpp:7064-7093.
            let ang = frand(rng) * CPP_6_28;
            let (vx, vy) = (ang.cos(), ang.sin());
            let band = 0.1 + 0.2 * frand(rng);
            for y in 0..=gy {
                let fy = (y as f32 / gy as f32) * ay;
                for x in 0..=gx {
                    let fx = (x as f32 / gx as f32) * ax;
                    let t = (fx - 0.5) * vx + (fy - 0.5) * vy + 0.5;
                    let t = (t - 0.5) / 2.0f32.sqrt() + 0.5;
                    set(&mut out[at(x, y)], band, t);
                }
            }
        }
        BlendPattern::Plasma => {
            // plugin.cpp:7094-7137.
            let band = 0.12 + 0.13 * frand(rng);
            out[at(0, 0)].c = frand(rng);
            out[at(gx, 0)].c = frand(rng);
            out[at(0, gy)].c = frand(rng);
            out[at(gx, gy)].c = frand(rng);
            gen_plasma(rng, out, gx, aspect, (0, gx, 0, gy), 0.25);
            let minc = out.iter().map(|v| v.c).fold(out[0].c, f32::min);
            let maxc = out.iter().map(|v| v.c).fold(out[0].c, f32::max);
            let mult = 1.0 / (maxc - minc);
            for v in out.iter_mut() {
                let t = (v.c - minc) * mult;
                set(v, band, t);
            }
        }
        BlendPattern::Radial => {
            // plugin.cpp:7138-7162.
            let band = 0.02 + 0.14 * frand(rng) + 0.34 * frand(rng);
            let outside_in = crand(rng) % 2 == 1; // dir = (rand()%2)*2 - 1
            for y in 0..=gy {
                let dy = (y as f32 / gy as f32 - 0.5) * ay;
                for x in 0..=gx {
                    let dx = (x as f32 / gx as f32 - 0.5) * ax;
                    let t = (dx * dx + dy * dy).sqrt() * CPP_SQRT_2;
                    let t = if outside_in { t } else { 1.0 - t };
                    set(&mut out[at(x, y)], band, t);
                }
            }
        }
    }
}

/// `GenPlasma(x0, x1, y0, y1, dt)` (plugin.cpp:7165-7199): midpoint
/// displacement of the `c` heights over the cell `(x0..x1, y0..y1)` of a
/// mesh `grid_x` wide, recursing into the four quarters with `dt / 2`.
/// The corners must be set; edge midpoints on the left/top borders are
/// written only by the cells that touch them, as in the C++.
pub fn gen_plasma<R: Rng + ?Sized>(
    rng: &mut R,
    out: &mut [VertInfo],
    grid_x: u32,
    aspect: (f32, f32),
    cell: (u32, u32, u32, u32),
    dt: f32,
) {
    let (x0, x1, y0, y1) = cell;
    let (ax, ay) = aspect;
    let w = grid_x + 1;
    let at = |x: u32, y: u32| (y * w + x) as usize;
    let (midx, midy) = ((x0 + x1) / 2, (y0 + y1) / 2);
    let t00 = out[at(x0, y0)].c;
    let t01 = out[at(x1, y0)].c;
    let t10 = out[at(x0, y1)].c;
    let t11 = out[at(x1, y1)].c;
    let mut jitter = |scale: f32| (frand(rng) * 2.0 - 1.0) * dt * scale;
    if y1 - y0 >= 2 {
        if x0 == 0 {
            out[at(x0, midy)].c = 0.5 * (t00 + t10) + jitter(ay);
        }
        out[at(x1, midy)].c = 0.5 * (t01 + t11) + jitter(ay);
    }
    if x1 - x0 >= 2 {
        if y0 == 0 {
            out[at(midx, y0)].c = 0.5 * (t00 + t01) + jitter(ax);
        }
        out[at(midx, y1)].c = 0.5 * (t10 + t11) + jitter(ax);
    }
    if y1 - y0 >= 2 && x1 - x0 >= 2 {
        let t00 = out[at(x0, midy)].c;
        let t01 = out[at(x1, midy)].c;
        let t10 = out[at(midx, y0)].c;
        let t11 = out[at(midx, y1)].c;
        out[at(midx, midy)].c = 0.25 * (t10 + t11 + t00 + t01) + jitter(1.0);
        let half = dt * 0.5;
        gen_plasma(rng, out, grid_x, aspect, (x0, midx, y0, midy), half);
        gen_plasma(rng, out, grid_x, aspect, (midx, x1, y0, midy), half);
        gen_plasma(rng, out, grid_x, aspect, (x0, midx, midy, y1), half);
        gen_plasma(rng, out, grid_x, aspect, (midx, x1, midy, y1), half);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::SeedableRng;

    #[test]
    fn frand_spans_zero_to_one() {
        let mut rng = rand_pcg::Pcg32::seed_from_u64(1);
        let xs: Vec<f32> = (0..20000).map(|_| frand(&mut rng)).collect();
        assert!(xs.iter().all(|x| (0.0..=1.0).contains(x)));
        assert!(xs.iter().any(|&x| x < 0.01) && xs.iter().any(|&x| x > 0.99));
    }

    #[test]
    fn uniform_pattern_is_one_zero() {
        let mut rng = rand_pcg::Pcg32::seed_from_u64(1);
        let mut out = vec![
            VertInfo {
                a: 5.0,
                c: 5.0,
                ..Default::default()
            };
            9
        ];
        blend_pattern(&mut rng, BlendPattern::Uniform, 2, 2, (1.0, 1.0), &mut out);
        assert!(out.iter().all(|v| v.a == 1.0 && v.c == 0.0));
    }

    #[test]
    fn plasma_fills_every_vertex_of_a_power_of_two_grid() {
        let mut rng = rand_pcg::Pcg32::seed_from_u64(2);
        let mut out = vec![
            VertInfo {
                c: f32::NAN,
                ..Default::default()
            };
            9 * 9
        ];
        out[0].c = 0.0;
        out[8].c = 1.0;
        out[72].c = 0.5;
        out[80].c = 0.25;
        gen_plasma(&mut rng, &mut out, 8, (1.0, 1.0), (0, 8, 0, 8), 0.25);
        assert!(out.iter().all(|v| v.c.is_finite()));
    }
}

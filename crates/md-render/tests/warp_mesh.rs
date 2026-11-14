//! Task 5.6: warp-mesh math (ComputeGridAlphaValues, milkdropfs.cpp:1681-1864)
//! and blend patterns (RandomizeBlendPattern / GenPlasma, plugin.cpp:7043-7199),
//! checked against hand-computed values.

use md_render::{
    blend_pattern, randomize_blend_pattern, BlendPattern, PerFrameVars, PerPixelVars, VertInfo,
    WarpMesh,
};
use rand_core::SeedableRng;

const EPS: f32 = 1e-5;

/// Per-frame values with no motion: zoom 1, rot 0, warp 0, stretch 1,
/// centre (0.5, 0.5), no translation.
fn identity() -> PerFrameVars {
    let mut v = PerFrameVars::from_preset(&md_preset::Preset::defaults(), 0.0);
    v.zoom = 1.0;
    v.zoomexp = 1.0;
    v.rot = 0.0;
    v.warp = 0.0;
    v.cx = 0.5;
    v.cy = 0.5;
    v.dx = 0.0;
    v.dy = 0.0;
    v.sx = 1.0;
    v.sy = 1.0;
    v
}

/// Index of grid vertex (x, y) in a mesh `gx` wide.
fn idx(gx: u32, x: u32, y: u32) -> usize {
    (y * (gx + 1) + x) as usize
}

fn assert_uv(m: &WarpMesh, n: usize, u: f32, v: f32) {
    let [mu, mv] = m.uvs()[n];
    assert!(
        (mu - u).abs() < EPS && (mv - v).abs() < EPS,
        "vertex {n}: got ({mu}, {mv}), want ({u}, {v})"
    );
}

#[test]
fn mesh_layout_matches_allocate_my_dx9_stuff() {
    let m = WarpMesh::new(4, 2, 1.0, 0.75);
    assert_eq!(m.vert_info().len(), 5 * 3);
    // pos = (x/gx*2-1, y/gy*2-1); vertex (4, 0) is the bottom-right corner.
    assert_eq!(m.positions()[idx(4, 4, 0)], [1.0, -1.0]);
    // rad = sqrt((x*ax)^2 + (y*ay)^2); ang = atan2(y*ay, x*ax), 0 at the centre.
    let vi = m.vert_info()[idx(4, 4, 2)];
    assert!((vi.rad - (1.0f32 + 0.75 * 0.75).sqrt()).abs() < EPS);
    assert!((vi.ang - 0.75f32.atan2(1.0)).abs() < EPS);
    let c = m.vert_info()[idx(4, 2, 1)];
    assert_eq!((c.rad, c.ang, c.a, c.c), (0.0, 0.0, 1.0, 0.0));
    // m_indices_list: 4 quadrants, 6 indices per cell, gx*gy*6 in total;
    // quadrant 0's first cell is (0, 0): v, v+1, v+gx+1, v+1, v+gx+1, v+gx+2.
    assert_eq!(m.indices().len(), 4 * 2 * 6);
    assert_eq!(&m.indices()[..6], &[0, 1, 5, 1, 5, 6]);
    // quadrant 1 mirrors x: its first cell is x = gx-1 = 3.
    assert_eq!(&m.indices()[12..18], &[3, 4, 8, 4, 8, 9]);
}

#[test]
fn uv_of_center_vertex_is_half_half_at_identity() {
    let mut m = WarpMesh::new(4, 4, 1.0, 1.0);
    m.compute_uvs(&identity(), None, 0.0, None);
    assert_uv(&m, idx(4, 2, 2), 0.5, 0.5);
    // Every vertex maps to its own screen position: u = x/2+.5, v = -y/2+.5.
    for (n, p) in m.positions().iter().enumerate() {
        assert_uv(&m, n, p[0] * 0.5 + 0.5, -p[1] * 0.5 + 0.5);
    }
    assert!(m.alphas().iter().all(|&a| a == 255));
    // The aspect fix is undone, so a non-square mesh is an identity too.
    let mut m = WarpMesh::new(4, 4, 1.0, 0.75);
    m.compute_uvs(&identity(), None, 0.0, None);
    assert_uv(&m, idx(4, 4, 4), 1.0, 0.0);
    assert_uv(&m, idx(4, 0, 0), 0.0, 1.0);
}

#[test]
fn zoom_scales_uv_about_center() {
    let mut m = WarpMesh::new(4, 4, 1.0, 1.0);
    let mut v = identity();
    v.zoom = 2.0;
    m.compute_uvs(&v, None, 0.0, None);
    // zoom2 = 2^(1^(rad*2-1)) = 2: u = x*0.5/2 + 0.5.
    assert_uv(&m, idx(4, 4, 4), 0.75, 0.25);
    assert_uv(&m, idx(4, 0, 0), 0.25, 0.75);
    assert_uv(&m, idx(4, 2, 2), 0.5, 0.5);
    // zoomexp: zoom2 = zoom^(zoomexp^(rad*2-1)); at the corner rad = sqrt(2).
    v.zoomexp = 2.0;
    m.compute_uvs(&v, None, 0.0, None);
    let z2 = 2.0f32.powf(2.0f32.powf(2.0f32.sqrt() * 2.0 - 1.0));
    assert_uv(&m, idx(4, 4, 4), 0.5 / z2 + 0.5, -0.5 / z2 + 0.5);
}

#[test]
fn dx_shifts_u_negative() {
    let mut m = WarpMesh::new(4, 4, 1.0, 1.0);
    let mut v = identity();
    v.dx = 0.1;
    v.dy = -0.2;
    m.compute_uvs(&v, None, 0.0, None);
    assert_uv(&m, idx(4, 2, 2), 0.4, 0.7);
    assert_uv(&m, idx(4, 4, 2), 0.9, 0.7);
}

#[test]
fn rot_rotates_about_cx_cy() {
    let mut m = WarpMesh::new(4, 4, 1.0, 1.0);
    let mut v = identity();
    v.rot = std::f64::consts::FRAC_PI_2;
    m.compute_uvs(&v, None, 0.0, None);
    // (u, v) = (1, 0.5) -> (u2, v2) = (0.5, 0) -> (0.5*cos - 0, 0.5*sin) + c.
    assert_uv(&m, idx(4, 4, 2), 0.5, 1.0);
    // About another centre: cx = cy = 0; the centre (0.5, 0.5) -> (-0.5, 0.5).
    v.cx = 0.0;
    v.cy = 0.0;
    m.compute_uvs(&v, None, 0.0, None);
    assert_uv(&m, idx(4, 2, 2), -0.5, 0.5);
}

#[test]
fn stretch_and_warp_terms_follow_the_cpp() {
    let mut m = WarpMesh::new(4, 4, 1.0, 1.0);
    let mut v = identity();
    v.sx = 2.0;
    v.sy = 0.5;
    m.compute_uvs(&v, None, 0.0, None);
    // u = (1 - 0.5)/2 + 0.5 = 0.75; v = (0 - 0.5)/0.5 + 0.5 = -0.5.
    assert_uv(&m, idx(4, 4, 4), 0.75, -0.5);

    let mut v = identity();
    v.warp = 1.0;
    v.warp_anim_speed = 1.0;
    v.warp_scale = 1.0;
    let t = 2.0f32;
    m.compute_uvs(&v, None, t, None);
    let (x, y) = (1.0f32, 1.0f32);
    let f = [
        11.68 + 4.0 * (t * 1.413 + 10.0).cos(),
        8.77 + 3.0 * (t * 1.113 + 7.0).cos(),
        10.54 + 3.0 * (t * 1.233 + 3.0).cos(),
        11.49 + 4.0 * (t * 0.933 + 5.0).cos(),
    ];
    let w = 0.0035f32;
    let u = 1.0
        + w * (t * 0.333 + (x * f[0] - y * f[3])).sin()
        + w * (t * 0.753 - (x * f[1] - y * f[2])).cos();
    let vv = 0.0
        + w * (t * 0.375 - (x * f[2] + y * f[1])).cos()
        + w * (t * 0.825 + (x * f[0] + y * f[3])).sin();
    assert_uv(&m, idx(4, 4, 4), u, vv);
}

#[test]
fn per_pixel_runner_sees_inputs_and_overrides_outputs() {
    let mut m = WarpMesh::new(2, 2, 1.0, 0.5);
    let mut seen = Vec::new();
    let mut run = |p: &mut PerPixelVars| {
        seen.push((p.x, p.y, p.rad, p.zoom));
        p.dx = 0.25;
    };
    m.compute_uvs(&identity(), Some(&mut run), 0.0, None);
    // x = px*0.5*ax + 0.5, y = py*-0.5*ay + 0.5 (vertex 0 is (-1, -1)).
    assert_eq!(seen.len(), 9);
    assert_eq!(seen[0].0, 0.0);
    assert_eq!(seen[0].1, 0.75);
    assert_eq!(seen[4], (0.5, 0.5, 0.0, 1.0));
    assert_uv(&m, idx(2, 1, 1), 0.25, 0.5);
}

#[test]
fn blend_mix_uses_a_t_plus_c() {
    let mut m = WarpMesh::new(2, 2, 1.0, 1.0);
    m.compute_uvs(&identity(), None, 0.0, None);
    let mut old = identity();
    old.dx = 0.2;
    // mix2 = clamp(a*t + c): 2*0.5 - 0.5 = 0.5 everywhere but vertex 0,
    // which clamps (4*0.5 + 0 = 2 -> 1: the new preset's UV, alpha 255).
    let mut info = vec![
        VertInfo {
            rad: 0.0,
            ang: 0.0,
            a: 2.0,
            c: -0.5
        };
        9
    ];
    info[0].a = 4.0;
    info[0].c = 0.0;
    m.compute_uvs(&old, None, 0.0, Some((&info, 0.5)));
    // tu = tu_new*mix2 + u_old*(1-mix2) = 0.5*0.5 + 0.3*0.5.
    assert_uv(&m, idx(2, 1, 1), 0.4, 0.5);
    assert_eq!(m.alphas()[4], 127); // (DWORD)(0.5*255)
    assert_uv(&m, 0, 0.0, 1.0);
    assert_eq!(m.alphas()[0], 255);
}

/// `band` back from `a = (1 + band) / band`, and `t` from `c = (t - 1) / band`.
fn band_and_t(v: &VertInfo) -> (f32, f32) {
    let band = 1.0 / (v.a - 1.0);
    (band, v.c * band + 1.0)
}

#[test]
fn plasma_pattern_is_normalized_to_unit_range() {
    let mut rng = rand_pcg::Pcg32::seed_from_u64(11);
    let (gx, gy) = (16u32, 12u32);
    let mut out = vec![VertInfo::default(); ((gx + 1) * (gy + 1)) as usize];
    blend_pattern(
        &mut rng,
        BlendPattern::Plasma,
        gx,
        gy,
        (1.0, 0.75),
        &mut out,
    );
    let (band, _) = band_and_t(&out[0]);
    assert!((0.12..=0.25 + 1e-6).contains(&band), "band {band}");
    let ts: Vec<f32> = out.iter().map(|v| band_and_t(v).1).collect();
    let min = ts.iter().copied().fold(f32::MAX, f32::min);
    let max = ts.iter().copied().fold(f32::MIN, f32::max);
    assert!(min.abs() < 1e-4 && (max - 1.0).abs() < 1e-4, "{min}..{max}");
    assert!(out.iter().all(|v| (v.a - out[0].a).abs() < 1e-6));
}

#[test]
fn wipe_and_radial_patterns_follow_the_cpp() {
    let mut rng = rand_pcg::Pcg32::seed_from_u64(3);
    let (gx, gy) = (8u32, 8u32);
    let n = ((gx + 1) * (gy + 1)) as usize;
    let mut out = vec![VertInfo::default(); n];
    blend_pattern(&mut rng, BlendPattern::Radial, gx, gy, (1.0, 1.0), &mut out);
    // t = sqrt(dx^2 + dy^2) * 1.41421 from the centre (or 1 - t).
    let (_, t_centre) = band_and_t(&out[idx(gx, 4, 4)]);
    let (_, t_corner) = band_and_t(&out[0]);
    let outside_in = t_centre.abs() < 1e-3;
    if outside_in {
        // The C++ multiplies by its literal 1.41421 (= sqrt 2 to 1e-5).
        assert!((t_corner - 1.0).abs() < 1e-3);
    } else {
        assert!((t_centre - 1.0).abs() < 1e-3);
    }
    blend_pattern(&mut rng, BlendPattern::Wipe, gx, gy, (1.0, 1.0), &mut out);
    // t is affine in (x, y): the centre is 0.5 for every angle.
    let (band, t) = band_and_t(&out[idx(gx, 4, 4)]);
    assert!((0.1..=0.3 + 1e-6).contains(&band));
    assert!((t - 0.5).abs() < 1e-4);
    // randomize_blend_pattern picks one of the three.
    for seed in 0..8 {
        let mut rng = rand_pcg::Pcg32::seed_from_u64(seed);
        randomize_blend_pattern(&mut rng, gx, gy, (1.0, 1.0), &mut out);
        assert!(out.iter().all(|v| v.a > 1.0));
    }
}

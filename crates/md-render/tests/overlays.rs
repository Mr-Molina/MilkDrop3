//! Task 6.5: motion vectors, borders and the darken-centre fan
//! (`DrawMotionVectors`, `ReversePropagatePoint`, `DrawSprites`), the
//! vertices on the CPU and pixels on a real (headless) device.
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::{
    build_borders, build_darken_center, build_motion_vectors, fan_to_list, gpu_or_skip,
    read_texture_level, reverse_propagate_point, Border, CustomWaveDraw, CustomWavePoint,
    FrameInput, MotionMesh, MotionVectorVars, PerFrameVars, RenderSettings, Renderer, RotParams,
    SoundData, Texture2d, Vertex, WarpMesh,
};

fn identity(grid: u32) -> WarpMesh {
    WarpMesh::new(grid, grid, 1.0, 1.0)
}

fn mv(x: f64, y: f64) -> MotionVectorVars {
    MotionVectorVars {
        x,
        y,
        dx: 0.0,
        dy: 0.0,
        l: 1.0,
        r: 1.0,
        g: 0.5,
        b: 0.0,
        a: 1.0,
    }
}

fn lines(v: &MotionVectorVars, mesh: &WarpMesh, tex: u32) -> (usize, Vec<Vertex>) {
    let m = MotionMesh {
        grid: mesh.grid(),
        uvs: mesh.uvs(),
        tex: (tex, tex),
    };
    let mut out = Vec::new();
    let n = build_motion_vectors(v, &m, &mut out);
    assert_eq!(out.len(), n * 2);
    (n, out)
}

#[test]
fn mv_count_matches_grid() {
    let mesh = identity(8);
    // milkdropfs.cpp:1231-1247: f = (i + 0.25) / (n + frac + 0.25 - 1);
    // the last index of an integral count lands on exactly 1.0, which the
    // `< 0.9999` test (1236, 1247) skips: 4 x 3 gives 3 columns x 2 rows.
    assert_eq!(lines(&mv(4.0, 3.0), &mesh, 64).0, 6);
    // A fractional count widens the grid (1242): 4.5 x 3 -> denominators
    // 3.75 / 2.25 -> 4 columns (the last is 3.25/3.75 = 0.867) x 2 rows.
    assert_eq!(lines(&mv(4.5, 3.0), &mesh, 64).0, 8);
    // The fraction is clamped to 0..1 and a count below 1 draws nothing
    // (1180): (int)0.9 == 0.
    assert_eq!(lines(&mv(0.9, 3.0), &mesh, 64).0, 0);
    assert_eq!(lines(&mv(4.0, 0.0), &mesh, 64).0, 0);
    assert_eq!(lines(&mv(-3.0, 3.0), &mesh, 64).0, 0);
    // Counts are capped at 64 x 48 (1177-1178): 63 x 47 lines survive the
    // 1.0 test.
    assert_eq!(lines(&mv(100.0, 100.0), &mesh, 64).0, 63 * 47);
    // mv_dx / mv_dy move the grid and points leaving (0.0001, 0.9999) go:
    // 4 x 3 columns sit at 0.0769, 0.3846, 0.6923 (1.0 is skipped): +0.2
    // gives 0.2769, 0.5846, 0.8923 (3 kept); -0.1 loses the first but
    // brings the skipped 1.0 back to 0.9 (3 kept); +0.4 keeps two.
    let m = MotionVectorVars {
        dx: 0.2,
        ..mv(4.0, 3.0)
    };
    assert_eq!(lines(&m, &mesh, 64).0, 6);
    let m = MotionVectorVars {
        dx: -0.1,
        ..mv(4.0, 3.0)
    };
    assert_eq!(lines(&m, &mesh, 64).0, 6);
    let m = MotionVectorVars {
        dx: 0.4,
        ..mv(4.0, 3.0)
    };
    assert_eq!(lines(&m, &mesh, 64).0, 4);
    // Rows move with mv_dy the other way (fy -= dy2, 1234): rows at 0.111,
    // 0.556, 1.0 minus 0.6 leave only 0.4 (-0.489 and -0.044 are out).
    let m = MotionVectorVars {
        dy: 0.6,
        ..mv(4.0, 3.0)
    };
    assert_eq!(lines(&m, &mesh, 64).0, 3);
}

#[test]
fn mv_zero_length_emits_nothing() {
    // The brief's name, the C++'s behaviour: "nothing" is only true below
    // the alpha gate (1159). A zero-length vector (mv_l = 0, or an
    // identity mesh) is not dropped: it becomes (min_len, min_len), one
    // texel in x and in y (1256-1282), so it is still drawn.
    let mesh = identity(8);
    let a0 = MotionVectorVars {
        a: 0.000_9,
        ..mv(4.0, 3.0)
    };
    assert_eq!(lines(&a0, &mesh, 64).0, 0);
    let a0 = MotionVectorVars {
        a: f64::NAN,
        ..mv(4.0, 3.0)
    };
    assert_eq!(lines(&a0, &mesh, 64).0, 0);
    let l0 = MotionVectorVars {
        l: 0.0,
        ..mv(4.0, 3.0)
    };
    let (n, v) = lines(&l0, &mesh, 64);
    assert_eq!(n, 6);
    for pair in v.chunks(2) {
        let d = [
            pair[1].pos[0] - pair[0].pos[0],
            pair[1].pos[1] - pair[0].pos[1],
        ];
        // 1/64 of the texture = 2/64 in clip space, in both axes.
        assert!(
            (d[0] - 2.0 / 64.0).abs() < 1e-6 && (d[1] - 2.0 / 64.0).abs() < 1e-6,
            "{d:?}"
        );
        assert_eq!(pair[0].color, [255, 127, 0, 255]);
    }
    // A long vector is kept as is: shift every UV by 0.1 (tu - 0.1), mv_l 1.
    let mut uvs = mesh.uvs().to_vec();
    for uv in &mut uvs {
        uv[0] -= 0.1;
    }
    let m = MotionMesh {
        grid: mesh.grid(),
        uvs: &uvs,
        tex: (64, 64),
    };
    let mut out = Vec::new();
    build_motion_vectors(&mv(4.0, 3.0), &m, &mut out);
    let d = (out[1].pos[0] - out[0].pos[0]) / 2.0;
    // tu - 0.1 + 0.5/64 - fx: the half-texel offset is added on read.
    assert!((d - (-0.1 + 0.5 / 64.0)).abs() < 1e-5, "{d}");
    // Between mv_l * len and min_len: shorter vectors are stretched to one
    // texel, along their own direction.
    let m = MotionVectorVars {
        l: 0.01,
        ..mv(4.0, 3.0)
    };
    let mut out = Vec::new();
    build_motion_vectors(
        &m,
        &MotionMesh {
            grid: mesh.grid(),
            uvs: &uvs,
            tex: (64, 64),
        },
        &mut out,
    );
    let (dx, dy) = (
        (out[1].pos[0] - out[0].pos[0]) / 2.0,
        (out[1].pos[1] - out[0].pos[1]) / 2.0,
    );
    assert!(
        (dx.hypot(dy) - 1.0 / 64.0).abs() < 1e-6 && dx < 0.0,
        "{dx} {dy}"
    );
}

#[test]
fn reverse_propagate_is_the_bilinear_lookup() {
    // 2 x 2 grid, 3 x 3 vertices, row-major (stride 3). Point (0.3, 0.6):
    // x0 = (int)0.6 = 0, dx = 0.6; y0 = (int)1.2 = 1, dy = 0.2; the cell
    // corners are vertices 3, 4, 6, 7 with weights 0.32, 0.48, 0.08, 0.12.
    let mut uvs = vec![[0.0f32; 2]; 9];
    uvs[3] = [0.1, 0.2];
    uvs[4] = [0.5, 0.3];
    uvs[6] = [0.7, 0.4];
    uvs[7] = [0.9, 0.8];
    let m = MotionMesh {
        grid: (2, 2),
        uvs: &uvs,
        tex: (100, 50),
    };
    // + the half texel (0.005, 0.01) on every corner:
    // tu = 0.32*0.105 + 0.48*0.505 + 0.08*0.705 + 0.12*0.905 = 0.441
    // tv = 0.32*0.21 + 0.48*0.31 + 0.08*0.41 + 0.12*0.81 = 0.346
    let (fx2, fy2) = reverse_propagate_point(&m, 0.3, 0.6).unwrap();
    assert!((fx2 - 0.441).abs() < 1e-5, "{fx2}");
    assert!((fy2 - (1.0 - 0.346)).abs() < 1e-5, "{fy2}");
    // A vertex itself (dx = dy = 0) returns its own UV: (0, 0.5) is the
    // corner of vertex 3 -> (0.1 + 0.005, 1 - 0.21).
    let (fx2, fy2) = reverse_propagate_point(&m, 0.0, 0.5).unwrap();
    assert!(
        (fx2 - 0.105).abs() < 1e-6 && (fy2 - 0.79).abs() < 1e-6,
        "{fx2} {fy2}"
    );
    // Outside the grid (x1 > grid, negative, NaN): false in the C++ (1444-1451).
    assert!(reverse_propagate_point(&m, 1.0, 0.5).is_none());
    assert!(reverse_propagate_point(&m, 0.5, 1.0).is_none());
    assert!(reverse_propagate_point(&m, -1.0, 0.5).is_none());
    assert!(reverse_propagate_point(&m, 0.5, -1.0).is_none());
    // (int) truncates toward zero, so -0.1 * 2 = -0.2 is cell 0 with a
    // negative fraction: the C++ extrapolates rather than failing.
    assert!(reverse_propagate_point(&m, -0.1, 0.5).is_some());
    assert!(reverse_propagate_point(&m, f32::NAN, 0.5).is_none());
}

fn border(size: f64, r: f64, a: f64) -> Border {
    Border {
        size,
        r,
        g: 0.0,
        b: 0.0,
        a,
    }
}

#[test]
fn outer_border_is_four_quads() {
    let mut out = Vec::new();
    let quads = build_borders(border(0.1, 1.0, 1.0), border(0.1, 0.0, 0.0), &mut out);
    assert_eq!((quads, out.len()), (4, 16));
    // First quad (3378-3385): inner = 1 - 0.1, outer = 1.
    let p: Vec<[f32; 2]> = out[..4].iter().map(|v| [v.pos[0], v.pos[1]]).collect();
    assert_eq!(p, [[0.9, 0.9], [1.0, 1.0], [1.0, -1.0], [0.9, -0.9]]);
    assert!(out[..4].iter().all(|v| v.color == [255, 0, 0, 255]));
    // The next quads are the first rotated by 90 degrees (x, y) -> (-y, x),
    // with the float rounding of cosf(1.5707964).
    for (q, rot) in [(1, [-1.0f32, 1.0]), (2, [-1.0, -1.0]), (3, [1.0, -1.0])] {
        let v = out[q * 4 + 1].pos;
        assert!(
            (v[0] - rot[0]).abs() < 1e-5 && (v[1] - rot[1]).abs() < 1e-5,
            "{q} {v:?}"
        );
    }
    // Inner too: 8 quads, its ring between 1 - ob - ib and 1 - ob, in its
    // own colour; the alpha gate is `a > 0.001` (3368).
    out.clear();
    let ib = Border {
        size: 0.2,
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 0.5,
    };
    assert_eq!(build_borders(border(0.1, 1.0, 1.0), ib, &mut out), 8);
    let p: Vec<[f32; 2]> = out[16..20].iter().map(|v| [v.pos[0], v.pos[1]]).collect();
    assert_eq!(p, [[0.7, 0.7], [0.9, 0.9], [0.9, -0.9], [0.7, -0.7]]);
    assert_eq!(out[16].color, [0, 255, 0, 127]);
    out.clear();
    assert_eq!(
        build_borders(border(0.1, 1.0, 0.001), border(0.1, 1.0, -1.0), &mut out),
        0
    );
    assert!(out.is_empty());
}

#[test]
fn darken_center_fan_vertex_count_matches_cpp() {
    // 3310-3347 (3344): WFVERTEX v3[6], DrawPrimitiveUP(TRIANGLEFAN, 4).
    let mut out = Vec::new();
    build_darken_center(0.5, &mut out);
    assert_eq!(out.len(), 6);
    assert_eq!(fan_to_list(&out).len(), 12);
    // Centre alpha (int)(3/32*255) = 23, the rest 0, all black.
    assert_eq!(out[0].color, [0, 0, 0, 23]);
    assert!(out[1..].iter().all(|v| v.color == [0, 0, 0, 0]));
    let p: Vec<[f32; 2]> = out.iter().map(|v| [v.pos[0], v.pos[1]]).collect();
    assert_eq!(
        p,
        [
            [0.0, 0.0],
            [-0.025, 0.0],
            [0.0, -0.05],
            [0.025, 0.0],
            [0.0, 0.05],
            [-0.025, 0.0]
        ]
    );
}

#[test]
fn overlay_vars_come_from_the_post_equation_values() {
    let mut v = PerFrameVars::default();
    (v.mv_x, v.mv_l, v.ob_size, v.ib_a, v.ob_r) = (12.0, 0.9, 0.25, 0.75, 0.5);
    let m = MotionVectorVars::from_vars(&v);
    assert_eq!((m.x, m.l), (12.0, 0.9));
    assert_eq!((Border::outer(&v).size, Border::outer(&v).r), (0.25, 0.5));
    assert_eq!(Border::inner(&v).a, 0.75);
}

// --- GPU ---

const W: u32 = 96;
const H: u32 = 96;

fn vars(p: &md_preset::Preset) -> PerFrameVars {
    let mut v = PerFrameVars::from_preset(p, 0.0);
    (v.zoom, v.zoomexp, v.rot, v.warp) = (1.0, 1.0, 0.0, 0.0);
    (v.cx, v.cy, v.dx, v.dy, v.sx, v.sy) = (0.5, 0.5, 0.0, 0.0, 1.0, 1.0);
    (v.decay, v.wrap, v.gamma, v.echo_alpha, v.shader) = (1.0, 1.0, 1.0, 0.0, 0.0);
    // The helper keeps the frame clean: no built-in wave, no overlays; a
    // test turns on the one it checks.
    v.wave_a = 0.0;
    (v.mv_a, v.ob_a, v.ib_a, v.darken_center) = (0.0, 0.0, 0.0, 0.0);
    // Preset defaults colour them 0.25 grey; start from black.
    (v.ob_r, v.ob_g, v.ob_b, v.ib_r, v.ib_g, v.ib_b) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    v
}

/// The warp shader writes `grey`, so only drawn overlays show in the warp
/// target.
fn grey_warp_preset(grey: &str) -> md_preset::Preset {
    let mut p = md_preset::Preset::defaults();
    (p.warp_ps, p.comp_ps) = (2, 2);
    p.warp_shader = format!("shader_body {{ ret = {grey}; }}");
    p.comp_shader = md_render::DEFAULT_COMP_PS.to_string();
    p
}

/// The no-shader warp: copies the source (decay 1).
fn copy_warp_preset() -> md_preset::Preset {
    let mut p = md_preset::Preset::defaults();
    (p.warp_ps, p.comp_ps) = (0, 0);
    p.warp_shader.clear();
    p.comp_shader.clear();
    p
}

fn px(out: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [out[i], out[i + 1], out[i + 2], out[i + 3]]
}

/// The brightest pixel (by `ch`) of the rows around the middle of column `x`.
fn column_max(out: &[u8], x: u32, ch: usize) -> [u8; 4] {
    (H / 2 - 4..H / 2 + 4)
        .map(|y| px(out, x, y))
        .max_by_key(|p| p[ch])
        .unwrap()
}

/// Two frames (the first clears the warp source, 971-981) of `preset` with
/// `v`, `waves` in the second; the warp target of the second frame.
fn run(
    gpu: &md_render::Gpu,
    preset: &md_preset::Preset,
    v: &PerFrameVars,
    waves: &[CustomWaveDraw],
) -> Vec<u8> {
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).unwrap();
    let pg = r.load_preset(preset).unwrap();
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    r.mesh_mut().compute_uvs(v, None, 0.0, None);
    let target = Texture2d::new_target(
        r.gpu(),
        W,
        H,
        md_render::wgpu::TextureFormat::Rgba8Unorm,
        false,
    );
    // Frame 0: no overlays of its own that survive (the clear), and the
    // mesh UVs it leaves are the ones frame 1's motion vectors read.
    let quiet = PerFrameVars {
        mv_a: 0.0,
        ..v.clone()
    };
    let warm = FrameInput::new(preset, &pg, &quiet, &rot, &sound);
    let ctx = r.begin_frame(&warm);
    r.present(ctx, &target.view);
    let input = FrameInput {
        custom_waves: waves,
        ..FrameInput::new(preset, &pg, v, &rot, &sound)
    };
    let ctx = r.begin_frame(&input);
    r.present(ctx, &target.view);
    read_texture_level(r.gpu(), &r.vs(Renderer::other_vs(r.warp_target())).tex, 0)
}

fn count_red(out: &[u8]) -> usize {
    out.chunks(4)
        .filter(|p| p[0] > 100 && p[1] < 60 && p[2] < 60)
        .count()
}

#[test]
fn borders_land_pixels_in_main() {
    let Some(gpu) = gpu_or_skip("borders_land_pixels_in_main") else {
        return;
    };
    let p = grey_warp_preset("0");
    let mut v = vars(&p);
    (v.ob_size, v.ob_r, v.ob_a) = (0.1, 1.0, 1.0);
    (v.ib_size, v.ib_g, v.ib_a) = (0.2, 1.0, 1.0);
    let out = run(&gpu, &p, &v, &[]);
    // Outer band |x| in 0.9..1 -> columns 91.2..96 (and 0..4.8); inner band
    // 0.7..0.9 -> columns 81.6..91.2.
    let c = px(&out, 94, H / 2);
    assert!(c[0] > 250 && c[1] < 5, "outer {c:?}");
    let c = px(&out, 2, H / 2);
    assert!(c[0] > 250 && c[1] < 5, "outer left {c:?}");
    let c = px(&out, 86, H / 2);
    assert!(c[1] > 250 && c[0] < 5, "inner {c:?}");
    let c = px(&out, 12, H / 2);
    assert!(c[1] > 250 && c[0] < 5, "inner left {c:?}");
    assert_eq!(&px(&out, W / 2, H / 2)[..3], &[0, 0, 0]);
    // The top and bottom bands too.
    assert!(px(&out, W / 2, 1)[0] > 250 && px(&out, W / 2, H - 2)[0] > 250);
    // Off: nothing.
    let off = run(&gpu, &p, &vars(&p), &[]);
    assert_eq!(count_red(&off), 0);
}

#[test]
fn motion_vectors_land_pixels_in_main_before_the_warp() {
    let Some(gpu) = gpu_or_skip("motion_vectors_land_pixels_in_main_before_the_warp") else {
        return;
    };
    // The no-shader warp copies its source, so lines drawn into the source
    // (the C++'s m_lpVS[0], 965) come out in the warp target.
    let p = copy_warp_preset();
    let mut v = vars(&p);
    (v.mv_x, v.mv_y, v.mv_l, v.mv_r, v.mv_g, v.mv_b, v.mv_a) = (6.0, 6.0, 1.0, 1.0, 0.0, 0.0, 1.0);
    // Every vertex's content moves 0.1 in x: the vectors are 0.1 long.
    v.dx = 0.1;
    let out = run(&gpu, &p, &v, &[]);
    assert!(count_red(&out) > 30, "{}", count_red(&out));
    let mut off = v.clone();
    off.mv_a = 0.0;
    assert_eq!(count_red(&run(&gpu, &p, &off, &[])), 0);
    // A warp shader that ignores its source erases them: they are drawn
    // before the warp, not after it.
    let black = grey_warp_preset("0");
    let mut vb = vars(&black);
    (vb.mv_x, vb.mv_y, vb.mv_r, vb.mv_a) = (6.0, 6.0, 1.0, 1.0);
    assert_eq!(count_red(&run(&gpu, &black, &vb, &[])), 0);
}

fn white_line() -> CustomWaveDraw {
    let p = |x| CustomWavePoint {
        x,
        y: 0.5,
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    CustomWaveDraw {
        points: vec![p(0.1), p(0.9)],
        ..CustomWaveDraw::default()
    }
}

#[test]
fn darken_center_lands_pixels_in_main() {
    let Some(gpu) = gpu_or_skip("darken_center_lands_pixels_in_main") else {
        return;
    };
    let p = grey_warp_preset("0.5");
    let mut v = vars(&p);
    v.darken_center = 1.0;
    let out = run(&gpu, &p, &v, &[]);
    let far = px(&out, 10, 10)[0];
    assert!(far.abs_diff(128) <= 1, "{far}");
    // Black at alpha 23/255 -> 128 * (1 - 23/255) = 116.5 at the centre,
    // falling to the field value 2.4 px out.
    let c = px(&out, W / 2, H / 2)[0].min(px(&out, W / 2 - 1, H / 2 - 1)[0]);
    assert!(c < far - 5 && c > 100, "{c} vs {far}");
    assert_eq!(px(&out, W / 2 + 4, H / 2)[0], far, "the fan is 0.05 wide");
    v.darken_center = 0.0;
    let out = run(&gpu, &p, &v, &[]);
    assert!(px(&out, W / 2, H / 2)[0].abs_diff(128) <= 1);
}

#[test]
fn darken_center_draws_after_the_wave_and_before_the_borders() {
    let Some(gpu) = gpu_or_skip("darken_center_frame_order") else {
        return;
    };
    // DrawCustomWaves / DrawWave (1055-1056) come before DrawSprites
    // (1057): a white custom wave through the centre is darkened by the fan
    // (it would stay 255 if the fan came first), and covered by a border
    // that spans the whole screen (ob_size 1), which `DrawSprites` draws
    // after the fan.
    let p = grey_warp_preset("0");
    let mut v = vars(&p);
    let waves = [white_line()];
    let plain = column_max(&run(&gpu, &p, &v, &waves), W / 2, 0)[0];
    assert_eq!(plain, 255);
    v.darken_center = 1.0;
    let dark = column_max(&run(&gpu, &p, &v, &waves), W / 2, 0)[0];
    assert!(dark < 245 && dark > 200, "darkened wave {dark}");
    (v.ob_size, v.ob_a) = (1.0, 1.0);
    let covered = column_max(&run(&gpu, &p, &v, &waves), W / 2, 0)[0];
    assert!(covered < 5, "borders over the wave: {covered}");
    // The fan is drawn before the borders. A half-alpha green border over
    // a grey field with the fan: fan first gives
    // 0.5 * green + 0.5 * fan(grey); border first would give
    // fan(0.5 * green + 0.5 * grey), a few levels apart at the centre.
    let g = grey_warp_preset("0.5");
    let mut v = vars(&g);
    (v.ob_size, v.ob_g, v.ob_a) = (1.0, 1.0, 0.5);
    let fan_only = {
        let mut f = vars(&g);
        f.darken_center = 1.0;
        run(&gpu, &g, &f, &[])
    };
    v.darken_center = 1.0;
    let both = run(&gpu, &g, &v, &[]);
    let (x, y) = (W / 2 - 1, H / 2 - 1);
    let alpha = 1.0 - f32::from(px(&fan_only, x, y)[0]) / f32::from(px(&fan_only, 5, 5)[0]);
    let grey = f32::from(px(&fan_only, 5, 5)[1]);
    let fan_first = 127.5 + 0.5 * f32::from(px(&fan_only, x, y)[1]);
    let border_first = (127.5 + 0.5 * grey) * (1.0 - alpha);
    let got = f32::from(px(&both, x, y)[1]);
    assert!(
        (got - fan_first).abs() < (got - border_first).abs() && (got - fan_first).abs() < 1.6,
        "got {got}, fan first {fan_first}, border first {border_first}"
    );
}

//! Task 6.7a: every fixed-function 2D draw runs under the C++'s single
//! projection `D3DXMatrixOrthoLH(2, -2, 0, 1)` (code/vis_milk2/support.cpp:179,
//! textmgr.cpp:238), so a vertex at C++ `(x, y)` lands at clip `(x, -y)`.
//! One off-centre GPU test per draw kind pins where it lands in the
//! readback (row 0 is the top of the image), plus the D3D9 pixel-centre
//! rule after the flip.
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::{
    gpu_or_skip, read_texture_level, BlendMode, CustomWaveDraw, CustomWavePoint, DrawBatch,
    DynamicVertexBuffer, FixedMode, FrameInput, Gpu, PerFrameVars, PipelineCache, PipelineKey,
    PresetGpu, RenderSettings, Renderer, RotParams, SamplerKind, ShapeFrameVars, SoundData,
    Texture2d, Topo, Vertex, DEFAULT_COMP_PS,
};

use md_render::wgpu;

const W: u32 = 96;
const H: u32 = 96;

fn px(out: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [out[i], out[i + 1], out[i + 2], out[i + 3]]
}

/// The rows of a `W`-wide image holding a pixel that satisfies `f`.
fn rows(out: &[u8], f: impl Fn([u8; 4]) -> bool) -> Vec<u32> {
    let h = out.len() as u32 / (W * 4);
    (0..h)
        .filter(|&y| (0..W).any(|x| f(px(out, W, x, y))))
        .collect()
}

fn red(p: [u8; 4]) -> bool {
    p[0] > 100 && p[1] < 60 && p[2] < 60
}

fn mean(v: &[u32]) -> f64 {
    v.iter().map(|&y| f64::from(y)).sum::<f64>() / v.len().max(1) as f64
}

/// Identity motion, decay 1, nothing drawn but what a test turns on.
fn vars(p: &md_preset::Preset) -> PerFrameVars {
    let mut v = PerFrameVars::from_preset(p, 0.0);
    (v.zoom, v.zoomexp, v.rot, v.warp) = (1.0, 1.0, 0.0, 0.0);
    (v.cx, v.cy, v.dx, v.dy, v.sx, v.sy) = (0.5, 0.5, 0.0, 0.0, 1.0, 1.0);
    (v.decay, v.wrap, v.gamma, v.echo_alpha, v.shader) = (1.0, 1.0, 1.0, 0.0, 0.0);
    (v.wave_a, v.wave_additive, v.wave_brighten) = (0.0, 0.0, 0.0);
    (v.mv_a, v.ob_a, v.ib_a, v.darken_center) = (0.0, 0.0, 0.0, 0.0);
    (v.ob_r, v.ob_g, v.ob_b, v.ib_r, v.ib_g, v.ib_b) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    v
}

/// A preset whose warp shader writes `body`'s value (the default comp).
fn shader_preset(ret: &str) -> md_preset::Preset {
    let mut p = md_preset::Preset::defaults();
    (p.warp_ps, p.comp_ps) = (2, 2);
    p.warp_shader = format!("shader_body {{ ret = {ret}; }}");
    p.comp_shader = DEFAULT_COMP_PS.to_string();
    p
}

/// The no-shader warp (copies its source at decay 1) and composite.
fn no_shader_preset() -> md_preset::Preset {
    let mut p = md_preset::Preset::defaults();
    (p.warp_ps, p.comp_ps) = (0, 0);
    p.warp_shader.clear();
    p.comp_shader.clear();
    p
}

/// What one test frame draws besides the per-frame variables.
#[derive(Default)]
struct Extra<'a> {
    shapes: &'a [ShapeFrameVars],
    waves: &'a [CustomWaveDraw],
}

/// Two frames (the first clears the warp source, mfs:971-981, and leaves
/// the mesh UVs the second frame's motion vectors read); returns the
/// texture the second frame drew into (main, the next warp source).
fn run(gpu: &Gpu, preset: &md_preset::Preset, v: &PerFrameVars, extra: Extra) -> Vec<u8> {
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).unwrap();
    let pg = r.load_preset(preset).unwrap();
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    r.mesh_mut().compute_uvs(v, None, 0.0, None);
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    let quiet = PerFrameVars {
        mv_a: 0.0,
        ..v.clone()
    };
    let ctx = r.begin_frame(&FrameInput::new(preset, &pg, &quiet, &rot, &sound));
    r.present(ctx, &target.view);
    let input = FrameInput {
        shapes: extra.shapes,
        custom_waves: extra.waves,
        ..FrameInput::new(preset, &pg, v, &rot, &sound)
    };
    let ctx = r.begin_frame(&input);
    r.present(ctx, &target.view);
    read_texture_level(r.gpu(), &r.vs(Renderer::other_vs(r.warp_target())).tex, 0)
}

#[test]
fn fixed_quad_lands_on_its_d3d9_pixel() {
    let Some(gpu) = gpu_or_skip("fixed_quad_lands_on_its_d3d9_pixel") else {
        return;
    };
    // D3D9 under ortho(2, -2): screen = ((1 + x) W/2, (1 + y) H/2), pixel
    // (c, r) sampled at its integer centre. A quad from c - 0.5 to c + 0.5
    // (and r - 0.5 to r + 0.5) covers exactly pixel (c, r) in D3D9; the
    // flip plus the +0.5 px raster shift must give the same pixel here.
    let (w, h) = (8u32, 8u32);
    let (c, r) = (2.0f32, 1.0f32);
    let ex = |p: f32, n: u32| 2.0 * p / n as f32 - 1.0;
    let white = [255; 4];
    let vert = |x: f32, y: f32| Vertex {
        pos: [x, y, 0.0],
        color: white,
        ..Vertex::default()
    };
    let (x0, x1) = (ex(c - 0.5, w), ex(c + 0.5, w));
    let (y0, y1) = (ex(r - 0.5, h), ex(r + 0.5, h));
    let quad = [vert(x0, y0), vert(x1, y0), vert(x0, y1), vert(x1, y1)];
    let target = Texture2d::new_target(&gpu, w, h, wgpu::TextureFormat::Rgba8Unorm, false);
    let mut cache = PipelineCache::new();
    let mut vbuf = DynamicVertexBuffer::new(&gpu, 4);
    let mut enc = gpu.device().create_command_encoder(&Default::default());
    {
        let load = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
        let mut batch = DrawBatch::begin(&gpu, &mut cache, &mut vbuf, &mut enc, &target, load);
        let key = PipelineKey::new(
            FixedMode::Color,
            BlendMode::Off,
            Topo::TriStrip,
            target.format,
        );
        batch.draw(key, &quad, None, SamplerKind::PointClamp);
        batch.finish();
    }
    gpu.queue().submit([enc.finish()]);
    let img = read_texture_level(&gpu, &target.tex, 0);
    let lit: Vec<(u32, u32)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| px(&img, w, x, y)[0] > 0)
        .collect();
    assert_eq!(lit, [(2, 1)]);
}

#[test]
fn wave_mode_6_offset_up_lands_in_the_top_half() {
    let Some(gpu) = gpu_or_skip("wave_mode_6_offset_up_lands_in_the_top_half") else {
        return;
    };
    // Mode 6 at wave_mystery 0 is a horizontal line offset by fWavePosX
    // along the perpendicular (mfs:3047, 3057-3060), so wave_x (not wave_y)
    // moves it: 0.75 -> y 0.5, negated by DrawWave (mfs:3229), flipped
    // back by the ortho projection: a quarter of the way down.
    let p = shader_preset("0");
    let mut v = vars(&p);
    (v.wave_mode, v.wave_a, v.wave_x, v.wave_mystery) = (6.0, 1.0, 0.75, 0.0);
    (v.wave_r, v.wave_g, v.wave_b) = (1.0, 0.0, 0.0);
    let lit = rows(&run(&gpu, &p, &v, Extra::default()), red);
    assert!(!lit.is_empty(), "no wave");
    assert!(lit.iter().all(|&y| y.abs_diff(H / 4) <= 2), "{lit:?}");
}

#[test]
fn shape_at_y_075_lands_in_the_top_half() {
    let Some(gpu) = gpu_or_skip("shape_at_y_075_lands_in_the_top_half") else {
        return;
    };
    // Centre `var_pf_y*-2+1` = -0.5 (mfs:2300), clip +0.5.
    let shape = ShapeFrameVars {
        x: 0.5,
        y: 0.75,
        rad: 0.1,
        sides: 4.0,
        r: 1.0,
        a: 1.0,
        r2: 1.0,
        a2: 1.0,
        tex_zoom: 1.0,
        ..ShapeFrameVars::default()
    };
    let p = shader_preset("0");
    let out = run(
        &gpu,
        &p,
        &vars(&p),
        Extra {
            shapes: &[shape],
            ..Extra::default()
        },
    );
    let lit = rows(&out, red);
    assert!(!lit.is_empty(), "no shape");
    assert!(lit.iter().all(|&y| y < H / 2), "{lit:?}");
    assert!((mean(&lit) - f64::from(H) / 4.0).abs() < 2.0, "{lit:?}");
}

#[test]
fn custom_wave_at_y_075_lands_in_the_top_half() {
    let Some(gpu) = gpu_or_skip("custom_wave_at_y_075_lands_in_the_top_half") else {
        return;
    };
    // `var_pp_y*-2+1` (mfs:2613) = -0.5, clip +0.5.
    let pt = |x| CustomWavePoint {
        x,
        y: 0.75,
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let wave = CustomWaveDraw {
        points: vec![pt(0.1), pt(0.9)],
        ..CustomWaveDraw::default()
    };
    let p = shader_preset("0");
    let out = run(
        &gpu,
        &p,
        &vars(&p),
        Extra {
            waves: &[wave],
            ..Extra::default()
        },
    );
    let lit = rows(&out, red);
    assert!(!lit.is_empty(), "no wave");
    assert!(lit.iter().all(|&y| y.abs_diff(H / 4) <= 1), "{lit:?}");
}

#[test]
fn motion_vectors_sit_high_and_mv_dy_moves_them_up() {
    let Some(gpu) = gpu_or_skip("motion_vectors_sit_high_and_mv_dy_moves_them_up") else {
        return;
    };
    // mv_y = 2: rows at fy = 0.25 / 1.25 = 0.2 and 1.25 / 1.25 = 1, the
    // second skipped (fy < 0.9999 fails), `fy -= mv_dy` (mfs:1231-1236),
    // y = fy*2-1 (mfs:1286): fy = 0.2 is a fifth of the way down, and
    // mv_dy = 0.1 moves the rows up to 0.1 and (now drawn) 0.9. The
    // no-shader warp copies the lines drawn into its source (mfs:965).
    let p = no_shader_preset();
    let mut v = vars(&p);
    (v.mv_x, v.mv_y, v.mv_l, v.mv_r, v.mv_g, v.mv_b, v.mv_a) = (6.0, 2.0, 1.0, 1.0, 0.0, 0.0, 1.0);
    v.dx = 0.1; // the vectors are 0.1 long, horizontal
    let low = rows(&run(&gpu, &p, &v, Extra::default()), red);
    v.mv_dy = 0.1;
    let high = rows(&run(&gpu, &p, &v, Extra::default()), red);
    assert!(!low.is_empty() && !high.is_empty(), "{low:?} {high:?}");
    assert!(low.iter().all(|&y| y.abs_diff(H / 5) <= 2), "{low:?}");
    let near = |y: u32, f: f64| (f64::from(y) - f * f64::from(H)).abs() <= 2.0;
    assert!(
        high.iter().all(|&y| near(y, 0.1) || near(y, 0.9)),
        "{high:?}"
    );
    assert!(high.iter().any(|&y| near(y, 0.1)), "{high:?}");
}

#[test]
fn outer_border_edges_follow_d3d9_pixel_centres() {
    let Some(gpu) = gpu_or_skip("outer_border_edges_follow_d3d9_pixel_centres") else {
        return;
    };
    // ob_size 0.1: the top band is y in (0.9, 1) before the projection,
    // screen rows [0, 4.8), and D3D9 lights the rows whose integer centre
    // is inside: 0..=4 at the top, 92..=95 at the bottom ([91.2, 96)). A
    // missing or wrong-signed raster shift moves both edges by one row.
    let p = shader_preset("0");
    let mut v = vars(&p);
    (v.ob_size, v.ob_r, v.ob_a) = (0.1, 1.0, 1.0);
    let out = run(&gpu, &p, &v, Extra::default());
    let col: Vec<bool> = (0..H).map(|y| red(px(&out, W, W / 2, y))).collect();
    let want: Vec<bool> = (0..H).map(|y| y <= 4 || y >= 92).collect();
    assert_eq!(col, want);
    let row: Vec<bool> = (0..W).map(|x| red(px(&out, W, x, H / 2))).collect();
    let want: Vec<bool> = (0..W).map(|x| x <= 4 || x >= 92).collect();
    assert_eq!(row, want);
}

#[test]
fn darken_center_is_centred() {
    let Some(gpu) = gpu_or_skip("darken_center_is_centred") else {
        return;
    };
    // The fan's centre vertex is at (0, 0) (mfs:3310-3345): D3D9 pixel
    // (W/2, H/2) samples it exactly, and the diamond is symmetric about it.
    let p = shader_preset("0.5");
    let mut v = vars(&p);
    v.darken_center = 1.0;
    let out = run(&gpu, &p, &v, Extra::default());
    let at = |x: u32, y: u32| px(&out, W, x, y)[0];
    let (cx, cy) = (W / 2, H / 2);
    let darkest = (0..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .min_by_key(|&(x, y)| at(x, y))
        .unwrap();
    assert_eq!(darkest, (cx, cy));
    for k in 1..4 {
        assert_eq!(at(cx, cy - k), at(cx, cy + k), "vertical {k}");
        assert_eq!(at(cx - k, cy), at(cx + k, cy), "horizontal {k}");
    }
}

#[test]
fn custom_title_at_y_025_lands_in_the_top_half() {
    let Some(gpu) = gpu_or_skip("custom_title_at_y_025_lands_in_the_top_half") else {
        return;
    };
    // ShowSongTitleAnim's custom-message branch moves the mesh to
    // `dy = fY*2-1` (mfs:4612-4631; :4651-4653's "final flip on y" is
    // commented out), so fY = 0.25 is clip +0.5 after the projection, as
    // LaunchCustomMessage's 0..1 placement reads on screen.
    let (w, h) = (128u32, 128u32);
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(w, h), 3).unwrap();
    r.set_title_font_bundled(md_render::md_text::FontSpec {
        face: "Arial".into(),
        size_px: 16,
        bold: false,
        italic: false,
        antialiased: true,
    });
    assert!(r.render_title_texture("TTTT"));
    let st = r.supertext_mut();
    st.is_song_title = false;
    (st.font_size, st.x, st.y, st.growth, st.fade_time) = (50.0, 0.5, 0.25, 1.0, 0.2);
    (st.start_time, st.duration) = (0.0, 10.0);
    let preset = md_preset::Preset::defaults();
    let pg = PresetGpu {
        warp: None,
        comp: None,
    };
    let v = vars(&preset);
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    r.mesh_mut().compute_uvs(&v, None, 0.0, None);
    let mut input = FrameInput::new(&preset, &pg, &v, &rot, &sound);
    (input.now, input.time) = (5.0, 5.0);
    let target = Texture2d::new_target(r.gpu(), w, h, wgpu::TextureFormat::Rgba8Unorm, false);
    let ctx = r.begin_frame(&input);
    r.present(ctx, &target.view);
    let out = read_texture_level(r.gpu(), &target.tex, 0);
    let lit: Vec<u32> = (0..h)
        .filter(|&y| (0..w).any(|x| px(&out, w, x, y)[0] > 60))
        .collect();
    assert!(!lit.is_empty(), "no title");
    assert!(lit.iter().all(|&y| y < h / 2), "{lit:?}");
}

/// Top half: red rising with x; bottom half: blue.
fn halves() -> Vec<u8> {
    (0..H)
        .flat_map(|y| {
            (0..W).flat_map(move |x| {
                if y < H / 2 {
                    [(x * 2) as u8 + 40, 0, 0, 255]
                } else {
                    [0, 0, 200, 255]
                }
            })
        })
        .collect()
}

#[test]
fn warp_and_composite_keep_the_image_upright_on_every_path() {
    let Some(gpu) = gpu_or_skip("warp_and_composite_keep_the_image_upright_on_every_path") else {
        return;
    };
    // Every warp × composite path (preset shaders with their own built-in
    // VS, and the fixed-function ones) with zoom 1 / rot 0: an image with a
    // red top and a blue bottom comes out unflipped, both in main after
    // the warp and on screen.
    let want = halves();
    for (warp_ps, comp_ps) in [(2, 2), (0, 0), (2, 0), (0, 2)] {
        let mut p = md_preset::Preset::defaults();
        (p.warp_ps, p.comp_ps) = (warp_ps, comp_ps);
        p.warp_shader = if warp_ps > 0 {
            md_preset::gen_warp_shader(1.0, true)
        } else {
            String::new()
        };
        p.comp_shader = if comp_ps > 0 {
            DEFAULT_COMP_PS.to_string()
        } else {
            String::new()
        };
        let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).unwrap();
        let pg = r.load_preset(&p).unwrap();
        assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
        let v = vars(&p);
        let rot = [RotParams::default(); 24];
        let sound = SoundData::default();
        r.mesh_mut().compute_uvs(&v, None, 0.0, None);
        let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
        let ctx = r.begin_frame(&FrameInput::new(&p, &pg, &v, &rot, &sound));
        r.present(ctx, &target.view);
        let src = r.vs(Renderer::other_vs(r.warp_target()));
        gpu.queue().write_texture(
            src.tex.as_image_copy(),
            &want,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(W * 4),
                rows_per_image: Some(H),
            },
            src.tex.size(),
        );
        let ctx = r.begin_frame(&FrameInput::new(&p, &pg, &v, &rot, &sound));
        r.present(ctx, &target.view);
        let main = read_texture_level(r.gpu(), &r.vs(Renderer::other_vs(r.warp_target())).tex, 0);
        let screen = read_texture_level(r.gpu(), &target.tex, 0);
        for (what, img) in [("main", &main), ("screen", &screen)] {
            let path = format!("warp_ps {warp_ps} comp_ps {comp_ps} {what}");
            let top = px(img, W, 40, 8);
            assert!(top[0].abs_diff(120) <= 2 && top[2] == 0, "{path}: {top:?}");
            let bottom = px(img, W, 40, H - 8);
            assert!(
                bottom[0] == 0 && bottom[2].abs_diff(200) <= 1,
                "{path}: {bottom:?}"
            );
        }
    }
}

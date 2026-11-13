//! Task 6.2: the built-in waveform (`DrawWave`), geometry on the CPU and
//! pixels on a real (headless) device.
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::{
    build_wave, gpu_or_skip, read_texture_level, FrameInput, PerFrameVars, RenderSettings,
    Renderer, RotParams, SoundData, Texture2d, Topo, Vertex, WaveBlend, WaveGeometry, WaveParams,
    WaveView, DEFAULT_COMP_PS, DEFAULT_WARP_PS,
};

fn params() -> WaveParams {
    WaveParams {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.8,
        x: 0.5,
        y: 0.5,
        mystery: 0.0,
        use_dots: false,
        thick: false,
        additive: false,
        maximize_color: false,
        mod_alpha_by_volume: false,
        mod_alpha_start: 0.5,
        mod_alpha_end: 1.5,
        scale: 1.0,
        smoothing: 0.75,
    }
}

fn view(tex_w: u32) -> WaveView {
    WaveView {
        aspect: (1.0, 1.0),
        time: 0.0,
        tex: (tex_w, tex_w),
    }
}

fn build(
    mode: i32,
    blend: Option<WaveBlend>,
    sound: &SoundData,
    p: &WaveParams,
    tex_w: u32,
) -> (WaveGeometry, Vec<Vertex>) {
    let mut out = Vec::new();
    let g = build_wave(mode, blend, sound, p, &view(tex_w), &mut out);
    (g, out)
}

#[test]
fn mode_0_circle_vertex_count_is_samples_plus_one() {
    let s = SoundData::default();
    // nVerts = 480 / 2 = 240, plus the closing duplicate when not blending
    // (milkdropfs.cpp:2852-2857) = 241; SmoothWave makes 2n - 1 = 481.
    let (g, v) = build(0, None, &s, &params(), 1024);
    assert_eq!(v.len(), 2 * 241 - 1);
    assert_eq!(g.draws, vec![0..481]);
    assert_eq!(g.topo, Topo::LineStrip);
    // The circle closes: first and last vertex coincide.
    assert_eq!(v[0].pos, v[480].pos);
    // While blending (same mode, so no second wave) there is no duplicate.
    let b = WaveBlend {
        old_mode: 0,
        progress: 0.5,
    };
    let (_, v) = build(0, Some(b), &s, &params(), 1024);
    assert_eq!(v.len(), 2 * 240 - 1);
}

#[test]
fn mode_6_line_spans_width() {
    let s = SoundData::default();
    let (_, v) = build(6, None, &s, &params(), 1024);
    // Silence, wave_x = 0.5: a horizontal line through the centre, its end
    // points clipped to +/-1.1 (milkdropfs.cpp:3097-3148), 240 samples.
    let min = v.iter().map(|v| v.pos[0]).fold(f32::MAX, f32::min);
    let max = v.iter().map(|v| v.pos[0]).fold(f32::MIN, f32::max);
    assert!((min + 1.1).abs() < 1e-4, "{min}");
    assert!(max > 1.0, "{max}");
    assert!(v.iter().all(|v| v.pos[1].abs() < 1e-4));
    assert_eq!(v.len(), 2 * 240 - 1);
    // A narrow texture caps the sample count at tex_w / 3 (3032-3033).
    let (_, v) = build(6, None, &s, &params(), 90);
    assert_eq!(v.len(), 2 * 30 - 1);
}

#[test]
fn alpha_modulation_clamps() {
    let mut p = params();
    p.mod_alpha_by_volume = true;
    let alpha = |rel: f32| {
        let s = SoundData {
            imm_rel: [rel; 3],
            ..SoundData::default()
        };
        let (g, v) = build(2, None, &s, &p, 64);
        (!g.draws.is_empty()).then(|| v[0].color[3])
    };
    // vol = 3 * 0.333 * rel; (vol - 0.5) / (1.5 - 0.5) * 0.8 at rel 1:
    // 0.499 * 0.8 = 0.3992 -> 101.
    assert_eq!(alpha(1.0), Some(101));
    // Far above the end: the factor passes 1 and alpha clamps to 1.
    assert_eq!(alpha(3.0), Some(255));
    // Below the start: negative alpha clamps to 0, nothing is drawn (< 0.004).
    assert_eq!(alpha(0.0), None);
}

#[test]
fn maximize_color_scales_max_channel_to_one() {
    let mut p = params();
    (p.r, p.g, p.b) = (0.2, 0.5, 0.1);
    p.maximize_color = true;
    let (_, v) = build(2, None, &SoundData::default(), &p, 64);
    assert_eq!(v[0].color[..3], [102, 255, 51]);
    // Off: unchanged.
    p.maximize_color = false;
    let (_, v) = build(2, None, &SoundData::default(), &p, 64);
    assert_eq!(v[0].color[..3], [51, 127, 25]);
    // Near-black (max <= 0.01) is left alone.
    (p.r, p.g, p.b) = (0.005, 0.0, 0.0);
    p.maximize_color = true;
    let (_, v) = build(2, None, &SoundData::default(), &p, 64);
    assert_eq!(v[0].color[..3], [1, 0, 0]);
}

#[test]
fn thick_quadruples_vertices() {
    let s = SoundData::default();
    let (_, one) = build(6, None, &s, &params(), 1024);
    let mut p = params();
    p.thick = true;
    let (g, four) = build(6, None, &s, &p, 1024);
    assert_eq!(four.len(), 4 * one.len());
    assert_eq!(g.draws.len(), 4);
    // Cumulative offsets: +x, then +y on top, then -x (3224-3228).
    let n = one.len();
    let (dx, dy) = (2.0 / 1024.0, 2.0 / 1024.0);
    let d = |k: usize| {
        [
            four[k * n].pos[0] - one[0].pos[0],
            four[k * n].pos[1] - one[0].pos[1],
        ]
    };
    assert_eq!(d(0), [0.0, 0.0]);
    assert!((d(1)[0] - dx).abs() < 1e-6 && d(1)[1].abs() < 1e-6);
    assert!((d(2)[0] - dx).abs() < 1e-6 && (d(2)[1] - dy).abs() < 1e-6);
    assert!(d(3)[0].abs() < 1e-6 && (d(3)[1] - dy).abs() < 1e-6);
    // Textures narrower than 512 never fatten (3259).
    let (_, small) = build(6, None, &s, &p, 256);
    assert_eq!(small.len(), 2 * 85 - 1);
    // Dots fatten the same way, and are points.
    p.thick = false;
    p.use_dots = true;
    let (g, dots) = build(6, None, &s, &p, 1024);
    assert_eq!((dots.len(), g.topo), (4 * one.len(), Topo::Points));
}

#[test]
fn mode_7_breaks_into_two_channels() {
    let s = SoundData::default();
    let (g, v) = build(7, None, &s, &params(), 1024);
    // Two 240-sample channels, each smoothed to 479, drawn separately.
    assert_eq!(g.draws, vec![0..479, 479..958]);
    assert_eq!(v.len(), 958);
}

#[test]
fn blend_mixes_both_modes_by_cosine_interp() {
    let s = SoundData::default();
    let at = |t: f32| {
        build(
            0,
            Some(WaveBlend {
                old_mode: 6,
                progress: t,
            }),
            &s,
            &params(),
            1024,
        )
    };
    // mix = CosineInterp(progress): 0 is the old wave entirely, 1 the new.
    let (g0, v0) = at(0.0);
    let (_, v1) = at(1.0);
    let (_, new) = build(
        0,
        Some(WaveBlend {
            old_mode: 0,
            progress: 1.0,
        }),
        &s,
        &params(),
        1024,
    );
    assert_eq!(g0.draws.len(), 1);
    // At 0 every y is the old (centre) line's.
    assert!(v0.iter().all(|v| v.pos[1].abs() < 1e-3));
    // At 1 the circle (blend pass-through of the new wave, no duplicate).
    for (a, b) in v1.iter().zip(&new) {
        assert!((a.pos[0] - b.pos[0]).abs() < 1e-5 && (a.pos[1] - b.pos[1]).abs() < 1e-5);
    }
    // Alpha mixes too: same params, so unchanged.
    assert_eq!(v0[0].color[3], v1[0].color[3]);
    // Old mode 7's break carries over into the blended wave.
    let (g, _) = build(
        0,
        Some(WaveBlend {
            old_mode: 7,
            progress: 0.5,
        }),
        &s,
        &params(),
        1024,
    );
    assert_eq!(g.draws.len(), 2);
}

// --- GPU ---

const W: u32 = 96;
const H: u32 = 96;

fn vars(p: &md_preset::Preset, mode: f64) -> PerFrameVars {
    let mut v = PerFrameVars::from_preset(p, 0.0);
    (v.zoom, v.zoomexp, v.rot, v.warp) = (1.0, 1.0, 0.0, 0.0);
    (v.cx, v.cy, v.dx, v.dy, v.sx, v.sy) = (0.5, 0.5, 0.0, 0.0, 1.0, 1.0);
    (v.decay, v.wrap, v.gamma, v.echo_alpha, v.shader) = (1.0, 1.0, 1.0, 0.0, 0.0);
    (v.wave_mode, v.wave_a, v.wave_additive, v.wave_brighten) = (mode, 1.0, 0.0, 0.0);
    v
}

fn preset_with_shaders() -> md_preset::Preset {
    let mut p = md_preset::Preset::defaults();
    (p.warp_ps, p.comp_ps) = (2, 2);
    p.warp_shader = DEFAULT_WARP_PS.to_string();
    p.comp_shader = DEFAULT_COMP_PS.to_string();
    p
}

fn lit(px: &[u8], x: u32, y: u32) -> bool {
    let i = ((y * W + x) * 4) as usize;
    px[i] > 100 && px[i + 1] > 100 && px[i + 2] > 100
}

#[test]
fn mode_6_line_lands_pixels_in_main() {
    let Some(gpu) = gpu_or_skip("mode_6_line_lands_pixels_in_main") else {
        return;
    };
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).unwrap();
    let preset = preset_with_shaders();
    let pg = r.load_preset(&preset).unwrap();
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let v = vars(&preset, 6.0);
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    r.mesh_mut().compute_uvs(&v, None, 0.0, None);
    let input = FrameInput::new(&preset, &pg, &v, &rot, &sound);
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    let ctx = r.begin_frame(&input);
    r.present(ctx, &target.view);
    // present swapped: the texture the wave was drawn into is now the warp source.
    let out = read_texture_level(r.gpu(), &r.vs(Renderer::other_vs(r.warp_target())).tex, 0);
    // Silence, wave_x = 0.5: a horizontal line through the vertical
    // centre (y = 0 flips to 0), spanning the width; the rest is black.
    let row = |y: u32| (0..W).filter(|&x| lit(&out, x, y)).count();
    let lit_rows: Vec<u32> = (0..H).filter(|&y| row(y) > 0).collect();
    assert!(!lit_rows.is_empty(), "no pixels drawn");
    assert!(
        lit_rows.iter().all(|y| y.abs_diff(H / 2) <= 1),
        "{lit_rows:?}"
    );
    let best = lit_rows.iter().map(|&y| row(y)).max().unwrap();
    assert!(best as u32 >= W - 4, "line covers {best} of {W}");
    assert!(!lit(&out, W / 2, H / 4));
}

#[test]
fn blend_draws_the_old_mode_too() {
    let Some(gpu) = gpu_or_skip("blend_draws_the_old_mode_too") else {
        return;
    };
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).unwrap();
    let mut old = preset_with_shaders();
    old.wave_mode = 6;
    let new = preset_with_shaders();
    let old_pg = r.load_preset(&old).unwrap();
    let pg = r.load_preset(&new).unwrap();
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let v = vars(&new, 0.0);
    let old_v = vars(&old, 6.0);
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    let (gx, gy) = r.mesh().grid();
    let pattern = vec![
        md_render::VertInfo {
            rad: 0.0,
            ang: 0.0,
            a: 0.0,
            c: 0.25
        };
        ((gx + 1) * (gy + 1)) as usize
    ];
    r.mesh_mut().compute_uvs(&v, None, 0.0, None);
    r.mesh_mut()
        .compute_uvs(&old_v, None, 0.0, Some((&pattern, 0.0)));
    // blend_t = 0: CosineInterp(0) = 0, the wave is the old mode's line
    // (its alpha is the same variable), not the new mode's circle.
    let input = FrameInput {
        old_preset: Some(&old),
        old_vars: Some(&old_v),
        old_gpu: Some(&old_pg),
        blend_t: 0.0,
        ..FrameInput::new(&new, &pg, &v, &rot, &sound)
    };
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    let ctx = r.begin_frame(&input);
    r.present(ctx, &target.view);
    let out = read_texture_level(r.gpu(), &r.vs(Renderer::other_vs(r.warp_target())).tex, 0);
    assert!(lit(&out, W / 2, H / 2), "old line missing at the centre");
    assert!(lit(&out, W / 4, H / 2));
    // The circle's top (radius 0.5 above the centre) is absent.
    assert!(!lit(&out, W / 2, H / 4));
}

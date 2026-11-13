//! Task 6.4: custom waves (`DrawCustomWaves`), the source values and
//! geometry on the CPU and pixels on a real (headless) device.
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::{
    build_custom_wave, custom_wave_inputs, gpu_or_skip, read_texture_level, CustomWaveDraw,
    CustomWaveFrame, CustomWavePoint, CustomWaveView, FrameInput, PerFrameVars, RenderSettings,
    Renderer, RotParams, SoundData, Texture2d, Topo, Vertex,
};

fn frame() -> CustomWaveFrame {
    CustomWaveFrame {
        samples: 4.0,
        sep: 0,
        scaling: 1.0,
        smoothing: 0.0,
        wave_scale: 1.0,
        spectrum: false,
        use_dots: false,
    }
}

fn sound() -> SoundData {
    let mut s = SoundData::default();
    for i in 0..576 {
        s.wave[0][i] = i as f32;
        s.wave[1][i] = 1000.0 + i as f32;
    }
    for i in 0..512 {
        s.spectrum[0][i] = i as f32;
        s.spectrum[1][i] = 2000.0 + i as f32;
    }
    s
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-5 * b.abs().max(1.0)
}

#[test]
fn samples_clamped_to_480_or_512() {
    // F41: milkdropfs.cpp:2520 caps the samples at 512 for a spectrum and
    // NUM_WAVEFORM_SAMPLES = 480 otherwise (the C++'s later `min(512)` at
    // 2549 would let a waveform read 16 values before its array).
    let s = sound();
    let n = |f: CustomWaveFrame| custom_wave_inputs(&f, &s).len();
    assert_eq!(
        n(CustomWaveFrame {
            samples: 1000.0,
            ..frame()
        }),
        480
    );
    assert_eq!(
        n(CustomWaveFrame {
            samples: 1000.0,
            spectrum: true,
            ..frame()
        }),
        512
    );
    assert_eq!(
        n(CustomWaveFrame {
            samples: 480.0,
            ..frame()
        }),
        480
    );
    assert_eq!(
        n(CustomWaveFrame {
            samples: 300.9,
            ..frame()
        }),
        300
    );
    // Fewer than two samples draws nothing (2551), except one dot.
    assert_eq!(
        n(CustomWaveFrame {
            samples: 1.0,
            ..frame()
        }),
        0
    );
    assert_eq!(
        n(CustomWaveFrame {
            samples: 1.0,
            use_dots: true,
            ..frame()
        }),
        1
    );
    assert_eq!(
        n(CustomWaveFrame {
            samples: 0.0,
            use_dots: true,
            ..frame()
        }),
        0
    );
    // (int) of NaN or an out-of-range double is INT_MIN: nothing.
    assert_eq!(
        n(CustomWaveFrame {
            samples: f64::NAN,
            ..frame()
        }),
        0
    );
    assert_eq!(
        n(CustomWaveFrame {
            samples: 3e9,
            ..frame()
        }),
        0
    );
}

#[test]
fn spectrum_uses_0_15_multiplier() {
    // 2555: mult = 0.15 * scaling * wave_scale for a spectrum; t =
    // (512 - sep) / n = 128 (2562) picks bins 0, 128, 256, 384; smoothing 0
    // makes mix1 = 0 so the values pass through.
    let s = sound();
    let f = CustomWaveFrame {
        spectrum: true,
        ..frame()
    };
    let v = custom_wave_inputs(&f, &s);
    for (j, bin) in [0.0f32, 128.0, 256.0, 384.0].into_iter().enumerate() {
        assert!(close(v[j].0, bin * 0.15), "{j}: {:?}", v[j]);
        assert!(close(v[j].1, (2000.0 + bin) * 0.15), "{j}: {:?}", v[j]);
    }
    // The waveform uses 0.004, and scaling * wave_scale multiply in.
    let f = CustomWaveFrame {
        scaling: 2.0,
        wave_scale: 3.0,
        ..frame()
    };
    let w = custom_wave_inputs(&f, &s);
    assert!(close(w[0].0, 238.0 * 0.004 * 2.0 * 3.0), "{:?}", w[0]);
}

#[test]
fn sep_offsets_right_channel() {
    // 2560-2561: j0 = (480 - n)/2 - sep/2, j1 = (480 - n)/2 + sep/2 for a
    // waveform, so the right channel reads sep samples later than the left.
    let s = sound();
    let f = CustomWaveFrame {
        sep: 2,
        scaling: 250.0,
        ..frame()
    };
    let v = custom_wave_inputs(&f, &s);
    // mult = 0.004 * 250 = 1; (480 - 4)/2 = 238; j0 = 237, j1 = 239.
    for (j, p) in v.iter().enumerate() {
        assert!(close(p.0, 237.0 + j as f32), "{j}: {p:?}");
        assert!(close(p.1, 1000.0 + 239.0 + j as f32), "{j}: {p:?}");
    }
    // A spectrum ignores sep for its offsets (j0 = j1 = 0) but not for t.
    let f = CustomWaveFrame {
        spectrum: true,
        sep: 128,
        ..frame()
    };
    let v = custom_wave_inputs(&f, &s);
    // t = (512 - 128) / 4 = 96.
    assert!(
        close(v[1].0, 96.0 * 0.15) && close(v[1].1, 2096.0 * 0.15),
        "{:?}",
        v[1]
    );
}

#[test]
fn inputs_match_hand_computed_smoothing() {
    // smoothing * 0.98 = 0.25 -> mix1 = sqrt(0.25) = 0.5 = mix2 (2563).
    // Data 238..242 read as d = [238, 239, 240, 241] (j0 = 238, sep 0);
    // use scaling 250 so mult = 1 and shift by 238 to keep it small.
    let mut s = SoundData::default();
    for (j, d) in [1.0f32, 2.0, 3.0, 4.0].into_iter().enumerate() {
        s.wave[0][238 + j] = d;
        s.wave[1][238 + j] = 10.0 * d;
    }
    let f = CustomWaveFrame {
        scaling: 250.0,
        smoothing: 0.25 / 0.98,
        ..frame()
    };
    let v = custom_wave_inputs(&f, &s);
    // Forward (2566-2572): [1, 1.5, 2.25, 3.125]; backward (2574-2578):
    // j=2: 2.6875, j=1: 2.09375, j=0: 1.546875.
    let want = [1.546875f32, 2.09375, 2.6875, 3.125];
    for j in 0..4 {
        assert!(close(v[j].0, want[j]), "{j}: {:?}", v[j]);
        assert!(close(v[j].1, want[j] * 10.0), "{j}: {:?}", v[j]);
        // sample = j / (n - 1) (2592-2595).
        assert!(close(v[j].2, j as f32 / 3.0));
    }
}

fn pt(x: f64, y: f64) -> CustomWavePoint {
    CustomWavePoint {
        x,
        y,
        r: 1.0,
        g: 0.5,
        b: 0.0,
        a: 1.0,
    }
}

fn view() -> CustomWaveView {
    CustomWaveView {
        aspect: (1.0, 1.0),
        tex: (512, 512),
        alpha_mult: 1.0,
    }
}

fn wave(n: usize) -> CustomWaveDraw {
    CustomWaveDraw {
        points: (0..n)
            .map(|i| pt(0.1 + 0.2 * i as f64, 0.5 + 0.1 * (i % 2) as f64))
            .collect(),
        ..CustomWaveDraw::default()
    }
}

#[test]
fn thick_quadruples() {
    // Lines: SmoothWave doubles the points (2n - 1 = 5 for 3), and thick
    // draws four cumulative copies offset +x, +y, -x (2654-2665).
    let mut out = Vec::new();
    let g = build_custom_wave(&wave(3), &view(), &mut out);
    assert_eq!(
        (g.topo, g.verts_per_copy, g.copies),
        (Topo::LineStrip, 5, 1)
    );
    assert_eq!(out.len(), 5);
    let base = out.clone();
    out.clear();
    let thick = CustomWaveDraw {
        thick: true,
        ..wave(3)
    };
    let g = build_custom_wave(&thick, &view(), &mut out);
    assert_eq!((g.verts_per_copy, g.copies), (5, 4));
    assert_eq!(out.len(), 20);
    let inc = 2.0 / 512.0;
    let off = [[0.0, 0.0], [inc, 0.0], [inc, inc], [0.0, inc]];
    for (c, o) in off.iter().enumerate() {
        for j in 0..5 {
            let (a, b) = (out[c * 5 + j].pos, base[j].pos);
            assert!((a[0] - b[0] - o[0]).abs() < 1e-6 && (a[1] - b[1] - o[1]).abs() < 1e-6);
        }
    }
    // Dots: no smoothing, one copy; thick only grows the point size, which
    // is 2 from 1024 up (2650).
    let dots = CustomWaveDraw {
        use_dots: true,
        ..wave(3)
    };
    out.clear();
    let g = build_custom_wave(&dots, &view(), &mut out);
    assert_eq!(
        (g.topo, g.verts_per_copy, g.copies, g.point_size),
        (Topo::Points, 3, 1, 1.0)
    );
    out.clear();
    let dots = CustomWaveDraw {
        use_dots: true,
        thick: true,
        ..wave(3)
    };
    let g = build_custom_wave(
        &dots,
        &CustomWaveView {
            tex: (1024, 1024),
            ..view()
        },
        &mut out,
    );
    assert_eq!((g.copies, g.point_size), (1, 3.0));
}

#[test]
fn positions_colours_and_alpha_follow_the_cpp() {
    // x = (px*2 - 1) / aspect_x, y = (py * -2 + 1) / aspect_y (2612-2613);
    // colour bytes are (int)(c*255) & 0xFF, alpha times alpha_mult.
    let w = CustomWaveDraw {
        points: vec![
            CustomWavePoint {
                x: 0.75,
                y: 0.25,
                r: 1.0,
                g: 0.5,
                b: 1.5,
                a: 0.5,
            },
            CustomWavePoint {
                x: 0.0,
                y: 1.0,
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        ],
        use_dots: true,
        ..CustomWaveDraw::default()
    };
    let v = CustomWaveView {
        aspect: (0.5, 0.8),
        alpha_mult: 0.5,
        ..view()
    };
    let mut out = Vec::new();
    build_custom_wave(&w, &v, &mut out);
    assert!(close(out[0].pos[0], 0.5 / 0.5) && close(out[0].pos[1], 0.5 / 0.8));
    assert!(close(out[1].pos[0], -1.0 / 0.5) && close(out[1].pos[1], -1.0 / 0.8));
    // 0.5*255*0.5 = 63.75 -> 63; b = 1.5*255 = 382 -> 382 & 0xFF = 126.
    assert_eq!(out[0].color, [255, 127, 126, 63]);
    // Non-finite colours are INT_MIN & 0xFF = 0.
    let w = CustomWaveDraw {
        points: vec![CustomWavePoint {
            r: f64::NAN,
            a: 1.0,
            ..CustomWavePoint::default()
        }],
        use_dots: true,
        ..CustomWaveDraw::default()
    };
    out.clear();
    build_custom_wave(&w, &view(), &mut out);
    assert_eq!(out[0].color, [0, 0, 0, 255]);
    // A lone point without dots draws nothing.
    let w = CustomWaveDraw {
        points: vec![pt(0.5, 0.5)],
        ..CustomWaveDraw::default()
    };
    out.clear();
    assert_eq!(build_custom_wave(&w, &view(), &mut out).copies, 0);
    assert!(out.is_empty());
    let _: Option<Vertex> = out.first().copied();
}

// --- GPU ---

const W: u32 = 96;
const H: u32 = 96;

fn vars(p: &md_preset::Preset) -> PerFrameVars {
    let mut v = PerFrameVars::from_preset(p, 0.0);
    (v.zoom, v.zoomexp, v.rot, v.warp) = (1.0, 1.0, 0.0, 0.0);
    (v.cx, v.cy, v.dx, v.dy, v.sx, v.sy) = (0.5, 0.5, 0.0, 0.0, 1.0, 1.0);
    (v.decay, v.wrap, v.gamma, v.echo_alpha, v.shader) = (1.0, 1.0, 1.0, 0.0, 0.0);
    v.wave_a = 0.0;
    v
}

/// The warp shader writes black, so only drawn waves show in the warp
/// target.
fn black_warp_preset() -> md_preset::Preset {
    let mut p = md_preset::Preset::defaults();
    (p.warp_ps, p.comp_ps) = (2, 2);
    p.warp_shader = "shader_body { ret = 0; }".to_string();
    p.comp_shader = md_render::DEFAULT_COMP_PS.to_string();
    p
}

fn px(out: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [out[i], out[i + 1], out[i + 2], out[i + 3]]
}

/// The brightest pixel (by `ch`) of the rows around the middle of column
/// `x`: a 1 px line sits on a pixel boundary, so look at a few rows.
fn column_max(out: &[u8], x: u32, ch: usize) -> [u8; 4] {
    (H / 2 - 4..H / 2 + 4)
        .map(|y| px(out, x, y))
        .max_by_key(|p| p[ch])
        .unwrap()
}

/// Draw one frame of `waves` (and `old_waves` while blending) and return
/// the warp target the waves were drawn into.
fn run(
    gpu: &md_render::Gpu,
    waves: &[CustomWaveDraw],
    old_waves: &[CustomWaveDraw],
    blend_t: Option<f32>,
) -> Vec<u8> {
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).unwrap();
    let new = black_warp_preset();
    let new_pg = r.load_preset(&new).unwrap();
    let old_pg = r.load_preset(&new).unwrap();
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let v = vars(&new);
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    r.mesh_mut().compute_uvs(&v, None, 0.0, None);
    let target = Texture2d::new_target(
        r.gpu(),
        W,
        H,
        md_render::wgpu::TextureFormat::Rgba8Unorm,
        false,
    );
    // The first frame after creation clears the warp source (971-981).
    let warm = FrameInput::new(&new, &new_pg, &v, &rot, &sound);
    let ctx = r.begin_frame(&warm);
    r.present(ctx, &target.view);
    let mut input = FrameInput {
        custom_waves: waves,
        old_custom_waves: old_waves,
        ..FrameInput::new(&new, &new_pg, &v, &rot, &sound)
    };
    if let Some(t) = blend_t {
        input.old_preset = Some(&new);
        input.old_vars = Some(&v);
        input.old_gpu = Some(&old_pg);
        input.blend_t = t;
    }
    let ctx = r.begin_frame(&input);
    r.present(ctx, &target.view);
    // present swapped: the texture the waves were drawn into is now the
    // warp source.
    read_texture_level(r.gpu(), &r.vs(Renderer::other_vs(r.warp_target())).tex, 0)
}

fn line(r: f64, g: f64, a: f64) -> CustomWaveDraw {
    let p = |x| CustomWavePoint {
        x,
        y: 0.5,
        r,
        g,
        b: 0.0,
        a,
    };
    CustomWaveDraw {
        points: vec![p(0.1), p(0.9)],
        ..CustomWaveDraw::default()
    }
}

#[test]
fn custom_wave_lands_pixels_in_main() {
    let Some(gpu) = gpu_or_skip("custom_wave_lands_pixels_in_main") else {
        return;
    };
    let thick = CustomWaveDraw {
        thick: true,
        ..line(1.0, 0.0, 1.0)
    };
    let out = run(&gpu, &[thick], &[], None);
    let c = column_max(&out, W / 2, 0);
    assert!(c[0] > 240 && c[1] < 10 && c[2] < 10, "{c:?}");
    // Away from the line, and beyond its ends, the black warp output.
    assert_eq!(&px(&out, W / 2, 10)[..3], &[0, 0, 0]);
    assert_eq!(&column_max(&out, 3, 0)[..3], &[0, 0, 0]);
    // Dots land too: thick dots are 2 px squares around the point.
    let dots = CustomWaveDraw {
        use_dots: true,
        thick: true,
        ..line(0.0, 1.0, 1.0)
    };
    let out = run(&gpu, &[dots], &[], None);
    let c = column_max(&out, (0.1 * f64::from(W)) as u32 + 1, 1);
    assert!(c[1] > 240 && c[0] < 10, "{c:?}");
    // No waves: nothing drawn.
    let empty = run(&gpu, &[], &[], None);
    assert_eq!(&column_max(&empty, W / 2, 0)[..3], &[0, 0, 0]);
}

#[test]
fn old_waves_draw_only_while_blending_scaled_by_progress() {
    let Some(gpu) = gpu_or_skip("old_waves_draw_only_while_blending_scaled_by_progress") else {
        return;
    };
    // Not blending: old waves are not drawn (num_reps == 1, 2507).
    let out = run(&gpu, &[], &[line(1.0, 0.0, 1.0)], None);
    assert_eq!(&column_max(&out, W / 2, 0)[..3], &[0, 0, 0]);
    // Blending at blend_t = 0: new waves have alpha_mult 0 (2513), the old
    // ones 1.
    let out = run(
        &gpu,
        &[line(0.0, 1.0, 1.0)],
        &[line(1.0, 0.0, 1.0)],
        Some(0.0),
    );
    let c = column_max(&out, W / 2, 0);
    assert!(c[0] > 240 && c[1] < 10, "{c:?}");
    // At blend_t = 1 only the new one shows.
    let out = run(
        &gpu,
        &[line(0.0, 1.0, 1.0)],
        &[line(1.0, 0.0, 1.0)],
        Some(1.0),
    );
    let c = column_max(&out, W / 2, 1);
    assert!(c[1] > 240 && c[0] < 10, "{c:?}");
    // Halfway: both at half alpha, the old drawn after the new.
    let out = run(
        &gpu,
        &[line(0.0, 1.0, 1.0)],
        &[line(1.0, 0.0, 1.0)],
        Some(0.5),
    );
    let c = column_max(&out, W / 2, 0);
    assert!(c[0].abs_diff(128) <= 3 && c[1].abs_diff(64) <= 3, "{c:?}");
}

#[test]
fn additive_selects_one_instead_of_inv_src_alpha() {
    let Some(gpu) = gpu_or_skip("custom_wave_additive") else {
        return;
    };
    // A red line, then a half-alpha green one on top: SRCALPHA/INVSRCALPHA
    // keeps half the red, SRCALPHA/ONE all of it (2647-2648).
    let green = |additive| CustomWaveDraw {
        additive,
        ..line(0.0, 1.0, 0.5)
    };
    let over = run(&gpu, &[line(1.0, 0.0, 1.0), green(false)], &[], None);
    let c = column_max(&over, W / 2, 0);
    assert!(c[0].abs_diff(128) <= 3 && c[1].abs_diff(127) <= 3, "{c:?}");
    let add = run(&gpu, &[line(1.0, 0.0, 1.0), green(true)], &[], None);
    let c = column_max(&add, W / 2, 0);
    assert!(c[0] > 250 && c[1].abs_diff(127) <= 3, "{c:?}");
}

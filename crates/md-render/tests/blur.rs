//! Task 6.1: the blur chain on a real (headless) device. Every GPU test
//! asks [`gpu_or_skip`] (`MD_REQUIRE_GPU=1` makes a missing adapter fail).

use md_render::{
    gpu_or_skip, read_texture_level, FrameInput, Gpu, PerFrameVars, PresetGpu, RenderSettings,
    Renderer, RotParams, SoundData, Texture2d, DEFAULT_WARP_PS,
};

fn upload_solid(gpu: &Gpu, t: &Texture2d, rgba: [u8; 4]) {
    let data: Vec<u8> = (0..t.w * t.h).flat_map(|_| rgba).collect();
    gpu.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &t.tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(t.w * 4),
            rows_per_image: Some(t.h),
        },
        wgpu::Extent3d {
            width: t.w,
            height: t.h,
            depth_or_array_layers: 1,
        },
    );
    md_render::generate_mips(gpu, t);
}

fn vars(ranges: [(f64, f64); 3], edge_darken: f64) -> PerFrameVars {
    let mut v = PerFrameVars::from_preset(&md_preset::Preset::defaults(), 0.0);
    (v.blur1_min, v.blur1_max) = ranges[0];
    (v.blur2_min, v.blur2_max) = ranges[1];
    (v.blur3_min, v.blur3_max) = ranges[2];
    v.blur1_edge_darken = edge_darken;
    v
}

/// Run `blur_passes` alone in one submit.
fn run_blur(r: &mut Renderer, v: &PerFrameVars, highest: u8) {
    let mut enc = r
        .gpu()
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    r.blur_passes(&mut enc, v, highest);
    r.gpu().queue().submit([enc.finish()]);
}

fn warp_source(r: &Renderer) -> &Texture2d {
    r.vs(Renderer::other_vs(r.warp_target()))
}

#[test]
fn blur_texture_sizes_halve_per_level_and_are_even() {
    let Some(gpu) = gpu_or_skip("blur_texture_sizes_halve_per_level_and_are_even") else {
        return;
    };
    let r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(800, 600), 1).unwrap();
    assert_eq!((r.settings().tex_w, r.settings().tex_h), (800, 608));
    // plugin.cpp:1986-2007: halve when i is even or i < 2, then width
    // ((w+3)/16)*16 and height ((h+3)/4)*4. Presets see textures 1, 3, 5.
    let want = [
        (400, 304), // ½
        (192, 152), // ¼  (blur1)
        (96, 76),   // ⅛
        (96, 76),   // ⅛  (blur2)
        (48, 40),   // 1/16
        (48, 40),   // 1/16 (blur3)
    ];
    for (i, &(w, h)) in want.iter().enumerate() {
        let t = r.blur_texture(i);
        assert_eq!((t.w, t.h), (w, h), "blur texture {i}");
        assert_eq!((t.w % 16, t.h % 4), (0, 0), "blur texture {i}");
    }
}

#[test]
fn blur_of_constant_image_is_constant_after_scale_bias() {
    let Some(gpu) = gpu_or_skip("blur_of_constant_image_is_constant_after_scale_bias") else {
        return;
    };
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(96, 64), 1).unwrap();
    upload_solid(&gpu, warp_source(&r), [128, 128, 128, 255]);
    // Level 1 range [0.2, 0.7]: scale 2, bias -0.4, so 128/255 -> 0.6.
    // Levels 2 and 3 repeat it, so their stage is the identity.
    let v = vars([(0.2, 0.7); 3], 0.0);
    run_blur(&mut r, &v, 3);
    let c = 128.0 / 255.0;
    let want = ((c * 2.0 - 0.4) * 255.0f32).round() as i32;
    for i in 0..6 {
        let t = r.blur_texture(i);
        let px = read_texture_level(r.gpu(), &t.tex, 0);
        for p in px.chunks_exact(4) {
            for ch in &p[..3] {
                assert!(
                    (i32::from(*ch) - want).abs() <= 2,
                    "blur texture {i}: {ch} vs {want}"
                );
            }
            assert_eq!(p[3], 255);
        }
    }

    // The default range [0, 1] is the identity; edges stay put with no
    // edge darkening.
    let v = vars([(0.0, 1.0); 3], 0.0);
    run_blur(&mut r, &v, 1);
    let px = read_texture_level(r.gpu(), &r.blur_texture(1).tex, 0);
    assert!(px.chunks_exact(4).all(|p| p[0].abs_diff(128) <= 2));

    // Edge darkening only touches the borders of the first vertical pass.
    let v = vars([(0.0, 1.0); 3], 1.0);
    run_blur(&mut r, &v, 1);
    let t = r.blur_texture(1);
    let px = read_texture_level(r.gpu(), &t.tex, 0);
    let at = |x: u32, y: u32| px[((y * t.w + x) * 4) as usize];
    assert!(at(t.w / 2, t.h / 2).abs_diff(128) <= 2);
    assert!(at(0, t.h / 2) < 64, "left edge {}", at(0, t.h / 2));
}

#[test]
fn collapsed_range_renders_without_panicking_and_is_deterministic() {
    let Some(gpu) = gpu_or_skip("collapsed_range_renders_without_panicking_and_is_deterministic")
    else {
        return;
    };
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(96, 64), 1).unwrap();
    upload_solid(&gpu, warp_source(&r), [200, 100, 50, 255]);
    let v = vars([(0.5, 0.5); 3], 0.25);
    run_blur(&mut r, &v, 3);
    let a = read_texture_level(r.gpu(), &r.blur_texture(5).tex, 0);
    run_blur(&mut r, &v, 3);
    let b = read_texture_level(r.gpu(), &r.blur_texture(5).tex, 0);
    assert_eq!(a, b);
}

#[test]
fn no_passes_when_unused() {
    let Some(gpu) = gpu_or_skip("no_passes_when_unused") else {
        return;
    };
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(96, 64), 1).unwrap();
    upload_solid(&gpu, warp_source(&r), [255; 4]);
    for i in 0..6 {
        upload_solid(&gpu, r.blur_texture(i), [7, 7, 7, 255]);
    }
    let v = vars([(0.0, 1.0); 3], 0.0);
    let untouched = |r: &Renderer, i: usize| {
        read_texture_level(r.gpu(), &r.blur_texture(i).tex, 0)
            .chunks_exact(4)
            .all(|p| p[..3] == [7, 7, 7])
    };
    run_blur(&mut r, &v, 0);
    assert!((0..6).all(|i| untouched(&r, i)), "0 levels ran a pass");
    // 2 * highest_used passes: level 1 draws textures 0 and 1 only.
    run_blur(&mut r, &v, 1);
    assert!(!untouched(&r, 0) && !untouched(&r, 1));
    assert!((2..6).all(|i| untouched(&r, i)));
    run_blur(&mut r, &v, 2);
    assert!(!untouched(&r, 2) && !untouched(&r, 3));
    assert!(untouched(&r, 4) && untouched(&r, 5));
}

/// The frame loop runs the chain when a shader samples a blur texture, and
/// only then.
#[test]
fn frame_runs_blur_for_presets_that_sample_it() {
    let Some(gpu) = gpu_or_skip("frame_runs_blur_for_presets_that_sample_it") else {
        return;
    };
    let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(96, 64), 1).unwrap();
    let mut p = md_preset::Preset::defaults();
    (p.warp_ps, p.comp_ps) = (2, 2);
    p.warp_shader = DEFAULT_WARP_PS.to_string();
    p.comp_shader = "shader_body { ret = GetBlur2(uv); }".to_string();
    let used = r.load_preset(&p).expect("load");
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    assert_eq!(used.highest_blur(), 2);
    p.comp_shader = "shader_body { ret = 0; }".to_string();
    let unused = r.load_preset(&p).expect("load");
    assert_eq!(unused.highest_blur(), 0);

    let target = Texture2d::new_target(r.gpu(), 96, 64, wgpu::TextureFormat::Rgba8Unorm, false);
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    let v = vars([(0.0, 1.0); 3], 0.0);
    let go = |r: &mut Renderer, pg: &PresetGpu| {
        for i in 0..6 {
            upload_solid(r.gpu(), r.blur_texture(i), [7, 7, 7, 255]);
        }
        upload_solid(r.gpu(), warp_source(r), [255; 4]);
        r.mesh_mut().compute_uvs(&v, None, 0.0, None);
        let input = FrameInput::new(&p, pg, &v, &rot, &sound);
        let ctx = r.begin_frame(&input);
        r.present(ctx, &target.view);
        (0..6)
            .map(|i| {
                !read_texture_level(r.gpu(), &r.blur_texture(i).tex, 0)
                    .chunks_exact(4)
                    .all(|p| p[..3] == [7, 7, 7])
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(go(&mut r, &used), [true, true, true, true, false, false]);
    assert_eq!(go(&mut r, &unused), [false; 6]);
}

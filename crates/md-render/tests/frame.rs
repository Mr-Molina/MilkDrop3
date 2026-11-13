//! Task 5.7: the frame loop (`begin_frame` → warp → hooks → composite →
//! `present` → swap) on a real (headless) device, the composite grid, the
//! no-shader composite's gamma passes, composing into a target of another
//! format, and resize clamping (F56).
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::{
    comp_grid, gpu_or_skip, read_texture_level, FrameInput, Gpu, PerFrameVars, PresetGpu,
    RenderSettings, Renderer, RotParams, SoundData, Texture2d, COMP_GRID_X, COMP_GRID_Y,
    DEFAULT_COMP_PS, DEFAULT_WARP_PS,
};

const W: u32 = 96;
const H: u32 = 64;

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

/// Motion that maps every pixel onto itself, decay 1, no echo, gamma 1,
/// no hue shader, no built-in wave (the default preset's wave, alpha 0.8,
/// would otherwise draw into every frame, Task 6.2) and no motion vectors
/// (the default preset's `mv_a` is 1 with a 12 x 9 grid, Task 6.5).
fn identity_vars(p: &md_preset::Preset) -> PerFrameVars {
    let mut v = PerFrameVars::from_preset(p, 0.0);
    (v.zoom, v.zoomexp, v.rot, v.warp) = (1.0, 1.0, 0.0, 0.0);
    (v.cx, v.cy, v.dx, v.dy, v.sx, v.sy) = (0.5, 0.5, 0.0, 0.0, 1.0, 1.0);
    (v.decay, v.wrap, v.gamma, v.echo_alpha) = (1.0, 1.0, 1.0, 0.0);
    v.shader = 0.0;
    v.wave_a = 0.0;
    v.mv_a = 0.0;
    v
}

/// Run one frame of `preset`/`pg` into `target` and return its bytes.
fn frame(
    r: &mut Renderer,
    preset: &md_preset::Preset,
    pg: &PresetGpu,
    vars: &PerFrameVars,
    target: &Texture2d,
) -> Vec<u8> {
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    r.mesh_mut().compute_uvs(vars, None, 0.0, None);
    let input = FrameInput::new(preset, pg, vars, &rot, &sound);
    let ctx = r.begin_frame(&input);
    r.present(ctx, &target.view);
    read_texture_level(r.gpu(), &target.tex, 0)
}

fn mean_rgb(bytes: &[u8]) -> f64 {
    let sum: u64 = bytes
        .chunks_exact(4)
        .map(|p| u64::from(p[0]) + u64::from(p[1]) + u64::from(p[2]))
        .sum();
    sum as f64 / (bytes.len() / 4 * 3) as f64
}

fn renderer(gpu: &Gpu) -> Renderer {
    Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).expect("renderer")
}

/// The source of the next warp pass (the C++'s `m_lpVS[0]`).
fn warp_source(r: &Renderer) -> &Texture2d {
    r.vs(Renderer::other_vs(r.warp_target()))
}

#[test]
fn comp_grid_has_768_vertices() {
    let g = comp_grid((W, H), (W, H));
    assert_eq!((COMP_GRID_X, COMP_GRID_Y), (32, 24));
    assert_eq!(g.vertices.len(), 768);
    // (FCGSX-2)*(FCGSY-2)*2 triangles (plugin.cpp:2120-2152).
    assert_eq!(g.indices.len(), 30 * 22 * 2 * 3);
    assert!(g.indices.iter().all(|&i| (i as usize) < 768));
    // Corners: (u, v) = (0, 0) top-left at x = -1 - 1/W (the -0.5 px).
    let v0 = g.vertices[0];
    assert_eq!(v0.uv[..2], [0.0, 0.0]);
    assert!((v0.pos[0] - (-1.0 - 1.0 / W as f32)).abs() < 1e-6);
    assert!((v0.pos[1] - (1.0 + 1.0 / H as f32)).abs() < 1e-6);
    let last = g.vertices[767];
    assert_eq!(last.uv[..2], [1.0, 1.0]);
}

#[test]
fn two_frames_of_default_preset_produce_decaying_image() {
    let Some(gpu) = gpu_or_skip("two_frames_of_default_preset_produce_decaying_image") else {
        return;
    };
    let mut r = renderer(&gpu);
    let mut preset = md_preset::Preset::defaults();
    (preset.warp_ps, preset.comp_ps) = (2, 2);
    preset.warp_shader = DEFAULT_WARP_PS.to_string();
    preset.comp_shader = DEFAULT_COMP_PS.to_string();
    let pg = r.load_preset(&preset).expect("load");
    assert!(pg.warp.is_some() && pg.comp.is_some());
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let vars = identity_vars(&preset);
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);

    // Frame 0 clears the old VS (m_nFramesSinceResize == 0) and draws
    // black, even though the source was painted white before it.
    upload_solid(r.gpu(), warp_source(&r), [255; 4]);
    let f0 = frame(&mut r, &preset, &pg, &vars, &target);
    assert_eq!(mean_rgb(&f0), 0.0);
    // Paint white into the next warp source: frame 1 shows 1 - 0.004,
    // frame 2 another 0.004 lower.
    upload_solid(r.gpu(), warp_source(&r), [255; 4]);
    let f1 = frame(&mut r, &preset, &pg, &vars, &target);
    let f2 = frame(&mut r, &preset, &pg, &vars, &target);
    let (m1, m2) = (mean_rgb(&f1), mean_rgb(&f2));
    eprintln!("mean frame 1 {m1}, frame 2 {m2}");
    assert!(
        (m1 - (255.0 - 0.004 * 255.0)).abs() < 1.0,
        "frame 1 mean {m1}"
    );
    let drop = m1 - m2;
    assert!((drop - 0.004 * 255.0).abs() < 0.5, "drop {drop}");
    // Alpha of the presented image is opaque.
    assert!(f2.chunks_exact(4).all(|p| p[3] == 255));
}

/// A MilkDrop 1 preset (no shaders) composited with `gamma` over a solid
/// `grey` source.
fn no_shader_frame(gpu: &Gpu, grey: u8, gamma: f64, format: wgpu::TextureFormat) -> Vec<u8> {
    let mut r = renderer(gpu);
    no_shader_run(
        &mut r,
        &[grey, grey, grey, 255],
        |v| v.gamma = gamma,
        format,
    )
}

/// Run a MilkDrop 1 preset (no shaders) for one frame, upload `source`
/// (one texel repeated, or a full `W`×`H` RGBA8 image) to the warp
/// source, apply `set` to the identity vars and return the next frame.
fn no_shader_run(
    r: &mut Renderer,
    source: &[u8],
    set: impl Fn(&mut PerFrameVars),
    format: wgpu::TextureFormat,
) -> Vec<u8> {
    let preset = md_preset::Preset::defaults();
    let pg = PresetGpu {
        warp: None,
        comp: None,
    };
    let mut vars = identity_vars(&preset);
    set(&mut vars);
    let target = Texture2d::new_target(r.gpu(), W, H, format, false);
    frame(r, &preset, &pg, &vars, &target);
    let src = warp_source(r);
    if source.len() == 4 {
        upload_solid(r.gpu(), src, [source[0], source[1], source[2], source[3]]);
    } else {
        upload(r.gpu(), src, source);
    }
    frame(r, &preset, &pg, &vars, &target)
}

#[test]
fn no_shader_path_applies_gamma_passes() {
    let Some(gpu) = gpu_or_skip("no_shader_path_applies_gamma_passes") else {
        return;
    };
    // 0.25 grey (64): gamma 1 keeps it, gamma 2 adds one more pass.
    let g1 = no_shader_frame(&gpu, 64, 1.0, wgpu::TextureFormat::Rgba8Unorm);
    let g2 = no_shader_frame(&gpu, 64, 2.0, wgpu::TextureFormat::Rgba8Unorm);
    let (m1, m2) = (mean_rgb(&g1), mean_rgb(&g2));
    eprintln!("gamma 1: {m1}, gamma 2: {m2}");
    assert!((m1 - 64.0).abs() < 1.0, "gamma 1 mean {m1}");
    assert!((m2 - 128.0).abs() < 1.5, "gamma 2 mean {m2}");
    // 2.5: two full passes and a half one; 0.75 grey saturates.
    let g25 = no_shader_frame(&gpu, 64, 2.5, wgpu::TextureFormat::Rgba8Unorm);
    assert!((mean_rgb(&g25) - 160.0).abs() < 2.0, "{}", mean_rgb(&g25));
    let sat = no_shader_frame(&gpu, 192, 2.0, wgpu::TextureFormat::Rgba8Unorm);
    assert!(sat.chunks_exact(4).all(|p| p[..3] == [255; 3]));
}

#[test]
fn composite_into_a_bgra_target_matches_rgba() {
    let Some(gpu) = gpu_or_skip("composite_into_a_bgra_target_matches_rgba") else {
        return;
    };
    let rgba = no_shader_frame(&gpu, 100, 1.0, wgpu::TextureFormat::Rgba8Unorm);
    let bgra = no_shader_frame(&gpu, 100, 1.0, wgpu::TextureFormat::Bgra8Unorm);
    let swizzled: Vec<u8> = bgra
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0], p[3]])
        .collect();
    assert_eq!(rgba, swizzled);
    assert!((mean_rgb(&rgba) - 100.0).abs() < 1.0);
    // A shader comp into Bgra8 works too (pipelines target the
    // intermediate, not the surface).
    let mut r = renderer(&gpu);
    let mut preset = md_preset::Preset::defaults();
    (preset.warp_ps, preset.comp_ps) = (2, 2);
    preset.warp_shader = md_preset::gen_warp_shader(1.0, true);
    preset.comp_shader = DEFAULT_COMP_PS.to_string();
    let pg = r.load_preset(&preset).expect("load");
    let vars = identity_vars(&preset);
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Bgra8Unorm, false);
    frame(&mut r, &preset, &pg, &vars, &target);
    upload_solid(r.gpu(), warp_source(&r), [10, 20, 200, 255]);
    let out = frame(&mut r, &preset, &pg, &vars, &target);
    let px = &out[((H / 2 * W + W / 2) * 4) as usize..][..4];
    assert!(
        px[0].abs_diff(200) <= 1 && px[1].abs_diff(20) <= 1 && px[2].abs_diff(10) <= 1,
        "{px:?}"
    );
}

#[test]
fn renderer_resize_clamps_to_device_limits() {
    let Some(gpu) = gpu_or_skip("renderer_resize_clamps_to_device_limits") else {
        return;
    };
    let max = gpu.device().limits().max_texture_dimension_2d;
    // Zero and oversized sizes at creation clamp to 1..=max.
    let r0 = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(0, 0), 1).expect("0x0");
    // Auto texture size: the window, snapped up to 16 (plugin.cpp:1888).
    assert_eq!(r0.window(), (1, 1));
    assert_eq!((r0.vs(0).w, r0.vs(0).h), (16, 16));
    drop(r0);

    let mut r = renderer(&gpu);
    let mut preset = md_preset::Preset::defaults();
    (preset.warp_ps, preset.comp_ps) = (2, 2);
    preset.warp_shader = md_preset::gen_warp_shader(1.0, true);
    preset.comp_shader = DEFAULT_COMP_PS.to_string();
    let pg = r.load_preset(&preset).expect("load");
    let vars = identity_vars(&preset);

    r.resize(0, 0);
    assert_eq!(r.window(), (1, 1));
    assert_eq!((r.vs(0).w, r.vs(0).h), (16, 16));
    r.resize(u32::MAX, 3);
    assert_eq!(r.window(), (max, 3));
    assert!(r.vs(0).w <= max && r.vs(0).h <= max);
    assert!((0..6).all(|i| r.blur_texture(i).w <= max));

    // Back to a new normal size: the preset loaded before the resizes
    // still draws (its bind groups are rebuilt for the new textures).
    r.resize(64, 48);
    assert_eq!((r.vs(0).w, r.vs(0).h), (64, 48));
    let target = Texture2d::new_target(r.gpu(), 64, 48, wgpu::TextureFormat::Rgba8Unorm, false);
    frame(&mut r, &preset, &pg, &vars, &target);
    upload_solid(r.gpu(), warp_source(&r), [0, 128, 255, 255]);
    let out = frame(&mut r, &preset, &pg, &vars, &target);
    let px = &out[((24 * 64 + 32) * 4) as usize..][..4];
    assert!(
        px[1].abs_diff(128) <= 1 && px[2].abs_diff(255) <= 1,
        "{px:?}"
    );
    // A resize to the same size keeps the textures but still clears the
    // warp source on the next frame (m_nFramesSinceResize = 0).
    r.resize(64, 48);
    upload_solid(r.gpu(), warp_source(&r), [255; 4]);
    let out = frame(&mut r, &preset, &pg, &vars, &target);
    assert_eq!(mean_rgb(&out), 0.0, "cleared after resize");
}

/// Top half: red rising with x; bottom half: blue.
fn halves() -> Vec<u8> {
    let mut out = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            out.extend_from_slice(&if y < H / 2 {
                [(x * 2) as u8 + 40, 0, 0, 255]
            } else {
                [0, 0, 200, 255]
            });
        }
    }
    out
}

fn upload(gpu: &Gpu, t: &Texture2d, rgba: &[u8]) {
    gpu.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &t.tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
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

#[test]
fn composite_preserves_the_image_and_its_orientation() {
    let Some(gpu) = gpu_or_skip("composite_preserves_the_image_and_its_orientation") else {
        return;
    };
    let want = halves();
    let px = |b: &[u8], x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        [b[i], b[i + 1], b[i + 2], b[i + 3]]
    };
    // Shader composite (`ret = tex2D(sampler_main, uv)`): the grid's -0.5
    // px offset plus the raster shift sample every texel at its centre, so
    // the identity warp + composite reproduce the image exactly.
    let mut r = renderer(&gpu);
    let mut preset = md_preset::Preset::defaults();
    (preset.warp_ps, preset.comp_ps) = (2, 2);
    preset.warp_shader = md_preset::gen_warp_shader(1.0, true);
    preset.comp_shader = DEFAULT_COMP_PS.to_string();
    let pg = r.load_preset(&preset).expect("load");
    let vars = identity_vars(&preset);
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    frame(&mut r, &preset, &pg, &vars, &target);
    upload(r.gpu(), warp_source(&r), &want);
    let out = frame(&mut r, &preset, &pg, &vars, &target);
    let worst = out
        .iter()
        .zip(&want)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert!(worst <= 1, "shader composite differs by {worst}");

    // No-shader composite: the C++'s one-pixel-larger quad filters a
    // little, but the image stays upright and in place.
    let mut r = renderer(&gpu);
    let preset = md_preset::Preset::defaults();
    let pg = PresetGpu {
        warp: None,
        comp: None,
    };
    let vars = identity_vars(&preset);
    frame(&mut r, &preset, &pg, &vars, &target);
    upload(r.gpu(), warp_source(&r), &want);
    let out = frame(&mut r, &preset, &pg, &vars, &target);
    let top = px(&out, 40, 8);
    assert!(top[0].abs_diff(120) <= 2 && top[2] == 0, "{top:?}");
    let bottom = px(&out, 40, H - 8);
    assert!(bottom[0] == 0 && bottom[2].abs_diff(200) <= 1, "{bottom:?}");
}

/// A preset with the identity warp shader and composite `comp_body`
/// (`None`: no composite shader).
fn blend_preset(comp_body: Option<&str>) -> md_preset::Preset {
    let mut p = md_preset::Preset::defaults();
    p.warp_ps = 2;
    p.warp_shader = md_preset::gen_warp_shader(1.0, true);
    match comp_body {
        Some(b) => {
            p.comp_ps = 2;
            p.comp_shader = b.to_string();
        }
        None => p.comp_ps = 0,
    }
    p
}

/// Blend from `old` to `new` over a white source with a constant mesh
/// blend alpha of 63 (mix 0.25); returns the centre pixel.
fn blend_frame(gpu: &Gpu, old: &md_preset::Preset, new: &md_preset::Preset) -> [u8; 4] {
    let mut r = renderer(gpu);
    let old_pg = r.load_preset(old).expect("old");
    let new_pg = r.load_preset(new).expect("new");
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let old_vars = identity_vars(old);
    let new_vars = identity_vars(new);
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    frame(&mut r, new, &new_pg, &new_vars, &target);
    upload_solid(r.gpu(), warp_source(&r), [255; 4]);
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
    r.mesh_mut().compute_uvs(&new_vars, None, 0.0, None);
    r.mesh_mut()
        .compute_uvs(&old_vars, None, 0.0, Some((&pattern, 0.5)));
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    let input = FrameInput {
        old_preset: Some(old),
        old_vars: Some(&old_vars),
        old_gpu: Some(&old_pg),
        blend_t: 0.5,
        ..FrameInput::new(new, &new_pg, &new_vars, &rot, &sound)
    };
    let ctx = r.begin_frame(&input);
    r.present(ctx, &target.view);
    let out = read_texture_level(r.gpu(), &target.tex, 0);
    let i = ((H / 2 * W + W / 2) * 4) as usize;
    [out[i], out[i + 1], out[i + 2], out[i + 3]]
}

#[test]
fn blend_composites_old_and_new_by_the_mesh_alpha() {
    let Some(gpu) = gpu_or_skip("blend_composites_old_and_new_by_the_mesh_alpha") else {
        return;
    };
    let black = "shader_body { ret = 0; }";
    // Both composite shaders: old (black, blending off) then new (white,
    // SrcAlpha/InvSrcAlpha with alpha a = 63/255): 1 * a + 0 * (1 - a).
    let px = blend_frame(
        &gpu,
        &blend_preset(Some(black)),
        &blend_preset(Some(DEFAULT_COMP_PS)),
    );
    assert!(px[0].abs_diff(63) <= 1, "both shaders: {px:?}");
    // THA FUNKY REVERSAL: the fixed composite (white) first, then the OLD
    // black shader with InvSrcAlpha/SrcAlpha: 0 * (1 - a) + 1 * a.
    let px = blend_frame(&gpu, &blend_preset(Some(black)), &blend_preset(None));
    assert!(px[0].abs_diff(63) <= 1, "funky reversal: {px:?}");
    // Only the new preset has a shader: fixed (white), then the new black
    // shader: 0 * a + 1 * (1 - a).
    let px = blend_frame(&gpu, &blend_preset(None), &blend_preset(Some(black)));
    assert!(px[0].abs_diff(192) <= 1, "new shader only: {px:?}");
    assert_eq!(px[3], 255);
}

/// Upload a solid `Rgba16Float` colour (`f16` bits per channel).
fn upload_solid_f16(gpu: &Gpu, t: &Texture2d, rgba: [u16; 4]) {
    let data: Vec<u8> = (0..t.w * t.h)
        .flat_map(|_| {
            rgba.iter()
                .flat_map(|c| c.to_le_bytes())
                .collect::<Vec<_>>()
        })
        .collect();
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
            bytes_per_row: Some(t.w * 8),
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

#[test]
fn composite_saturates_at_8_bits_with_16_bit_textures() {
    let Some(gpu) = gpu_or_skip("composite_saturates_at_8_bits_with_16_bit_textures") else {
        return;
    };
    let settings = RenderSettings {
        tex_bits: 16,
        ..RenderSettings::new(W, H)
    };
    let mut r = Renderer::new(gpu.device(), gpu.queue(), settings, 3).expect("renderer");
    assert_eq!(r.vs(0).format, wgpu::TextureFormat::Rgba16Float);
    assert_eq!(r.comp_texture().format, wgpu::TextureFormat::Rgba8Unorm);
    let preset = md_preset::Preset::defaults();
    let pg = PresetGpu {
        warp: None,
        comp: None,
    };
    let mut vars = identity_vars(&preset);
    // 0.75 grey at gamma 2 is 1.5, saturated to 1.0 by the 8-bit back
    // buffer; brighten (invert, square, invert) then gives
    // 1 - (1 - 1)^2 = 1.0 -> 255. Unclamped it would be
    // 1 - (1 - 1.5)^2 = 0.75 -> 191.
    (vars.gamma, vars.brighten) = (2.0, 1.0);
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    frame(&mut r, &preset, &pg, &vars, &target);
    upload_solid_f16(r.gpu(), warp_source(&r), [0x3A00, 0x3A00, 0x3A00, 0x3C00]);
    let out = frame(&mut r, &preset, &pg, &vars, &target);
    assert!(
        out.chunks_exact(4).all(|p| p[..3] == [255; 3]),
        "{:?}",
        &out[..4]
    );

    // A 16-bit preset composite shader also draws into the 8-bit target.
    let mut p = md_preset::Preset::defaults();
    (p.warp_ps, p.comp_ps) = (2, 2);
    p.warp_shader = md_preset::gen_warp_shader(1.0, true);
    p.comp_shader = "shader_body { ret = tex2D(sampler_main, uv).xyz * 2; }".to_string();
    let pg = r.load_preset(&p).expect("load");
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let vars = identity_vars(&p);
    frame(&mut r, &p, &pg, &vars, &target);
    upload_solid_f16(r.gpu(), warp_source(&r), [0x3800, 0x3800, 0x3800, 0x3C00]);
    let out = frame(&mut r, &p, &pg, &vars, &target);
    assert!(
        out.chunks_exact(4).all(|p| p[..3] == [255; 3]),
        "{:?}",
        &out[..4]
    );
}

#[test]
fn no_shader_echo_zooms_and_flips() {
    let Some(gpu) = gpu_or_skip("no_shader_echo_zooms_and_flips") else {
        return;
    };
    // Red where u < 0.4, black elsewhere. Echo alpha 0.5, zoom 2: layer 0
    // samples u, layer 1 samples 0.25 + 0.5 u (flipped: 1 - that); each is
    // weighted by (int)(0.5 * 255) = 127.
    let img: Vec<u8> = (0..W * H)
        .flat_map(|i| {
            if (i % W) * 10 < W * 4 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 0, 255]
            }
        })
        .collect();
    let red_at = |out: &[u8], x: u32| out[((H / 2 * W + x) * 4) as usize];
    let run = |orient: f64| {
        let mut r = renderer(&gpu);
        no_shader_run(
            &mut r,
            &img,
            |v| (v.echo_alpha, v.echo_zoom, v.echo_orient) = (0.5, 2.0, orient),
            wgpu::TextureFormat::Rgba8Unorm,
        )
    };
    let out = run(0.0);
    // u = 0.1: both layers red; u = 0.34: layer 1 at 0.42 is black (a
    // zoom of 1 would keep it red); u = 0.9: both black.
    assert!(red_at(&out, 10).abs_diff(254) <= 2, "{}", red_at(&out, 10));
    assert!(red_at(&out, 33).abs_diff(127) <= 2, "{}", red_at(&out, 33));
    assert_eq!(red_at(&out, 86), 0);
    // Orientation 1 flips layer 1 in u: u = 0.1 -> 0.7 (black),
    // u = 0.9 -> 0.3 (red).
    let out = run(1.0);
    assert!(red_at(&out, 10).abs_diff(127) <= 2, "{}", red_at(&out, 10));
    assert!(red_at(&out, 86).abs_diff(127) <= 2, "{}", red_at(&out, 86));
}

#[test]
fn no_shader_filters_follow_d3d9_blending() {
    let Some(gpu) = gpu_or_skip("no_shader_filters_follow_d3d9_blending") else {
        return;
    };
    // Source d = 64/255, gamma 1; each filter draws white quads, every
    // result rounded to 8 bits as the back buffer does.
    let run = |set: fn(&mut PerFrameVars)| {
        let mut r = renderer(&gpu);
        let out = no_shader_run(
            &mut r,
            &[64, 64, 64, 255],
            set,
            wgpu::TextureFormat::Rgba8Unorm,
        );
        out[((H / 2 * W + W / 2) * 4) as usize]
    };
    // invert: INVDESTCOLOR/ZERO -> 1 - d = 191.
    let invert = run(|v| v.invert = 1.0);
    assert!(invert.abs_diff(191) <= 1, "invert {invert}");
    // darken: ZERO/DESTCOLOR -> d * d = 64*64/255 = 16.
    let darken = run(|v| v.darken = 1.0);
    assert!(darken.abs_diff(16) <= 1, "darken {darken}");
    // solarize: ZERO/INVDESTCOLOR -> d(1-d) = 48, then DESTCOLOR/ONE ->
    // 1*48 + 48 = 96.
    let solarize = run(|v| v.solarize = 1.0);
    assert!(solarize.abs_diff(96) <= 1, "solarize {solarize}");
    // brighten: invert 191, square 191*191/255 = 143, invert 112.
    let brighten = run(|v| v.brighten = 1.0);
    assert!(brighten.abs_diff(112) <= 1, "brighten {brighten}");
}

/// Task 6.1a: the main ping-pong textures have one mip level, as in the C++
/// (plugin.cpp:1904: `D3DUSAGE_AUTOGENMIPMAP` is commented out), at every
/// `tex_bits` and size mode, also after `resize`.
#[test]
fn main_textures_are_single_level() {
    let Some(gpu) = gpu_or_skip("main_textures_are_single_level") else {
        return;
    };
    for tex_bits in [8u8, 16] {
        for tex_size in [
            md_render::TexSize::Fixed,
            md_render::TexSize::Auto,
            md_render::TexSize::AutoPow2,
        ] {
            let mut s = RenderSettings::new(W, H);
            s.tex_bits = tex_bits;
            s.tex_size = tex_size;
            let mut r = Renderer::new(gpu.device(), gpu.queue(), s, 3).expect("renderer");
            let check = |r: &Renderer, when: &str| {
                for i in 0..2 {
                    let t = r.vs(i);
                    assert_eq!(t.mips, 1, "{tex_bits} {tex_size:?} vs[{i}] {when}");
                    assert_eq!(t.tex.mip_level_count(), 1, "{tex_bits} {tex_size:?} {when}");
                }
            };
            check(&r, "after new");
            r.resize(100, 70);
            check(&r, "after resize");
        }
    }
}

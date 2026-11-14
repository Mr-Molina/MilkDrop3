//! Task 5.6: the warp pass on a real (headless) device — the fixed-function
//! path (MilkDrop 1 presets: decay as the vertex colour) and the preset
//! warp-shader path, plus a blend (pass 0 old, pass 1 new with mix alpha).
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::{
    gpu_or_skip, mesh_aspect, read_texture_level, FrameShaderInputs, Gpu, PerFrameVars, PresetGpu,
    PresetShaderInputs, RenderSettings, Renderer, RotParams, Texture2d, VertInfo, WarpMesh,
    WarpPreset,
};

const W: u32 = 96;
const H: u32 = 64;

/// A checkerboard of 8-pixel squares (opaque black / white, a red tint on
/// the white so channels differ).
fn checkerboard() -> Vec<u8> {
    let mut out = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let on = ((x / 8) + (y / 8)) % 2 == 0;
            out.extend_from_slice(if on {
                &[255, 230, 210, 255]
            } else {
                &[0, 0, 0, 255]
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
}

fn identity_vars(decay: f64) -> PerFrameVars {
    let mut v = PerFrameVars::from_preset(&md_preset::Preset::defaults(), 0.0);
    (v.zoom, v.zoomexp, v.rot, v.warp) = (1.0, 1.0, 0.0, 0.0);
    (v.cx, v.cy, v.dx, v.dy, v.sx, v.sy) = (0.5, 0.5, 0.0, 0.0, 1.0, 1.0);
    v.decay = decay;
    v.wrap = 1.0;
    v
}

fn frame() -> FrameShaderInputs {
    FrameShaderInputs {
        time: 1.0,
        fps: 60.0,
        frame: 60,
        progress: 0.1,
        imm_rel: [1.0; 3],
        avg_rel: [1.0; 3],
        rand_frame: [0.5; 4],
        window: (W, H),
    }
}

/// A preset whose warp shader is the generated `ret *= decay` shader.
fn shader_preset(decay: f32) -> md_preset::Preset {
    let mut p = md_preset::Preset::defaults();
    p.warp_ps = 2;
    p.warp_shader = md_preset::gen_warp_shader(decay, true);
    p
}

fn image(bytes: Vec<u8>) -> md_render::RgbaImage {
    md_render::RgbaImage::from_raw(W, H, bytes).expect("image size")
}

fn dssim(a: &md_render::RgbaImage, b: &md_render::RgbaImage) -> f64 {
    1.0 / md_testkit::ssim(a, b).max(1e-12) - 1.0
}

/// Upload the checkerboard to the warp source, run one warp pass, and
/// return the target's mip 0 and its level count.
fn run_warp(
    r: &mut Renderer,
    new: &WarpPreset,
    old: Option<&WarpPreset>,
    mesh: &WarpMesh,
) -> (Vec<u8>, u32) {
    let gpu = r.gpu().clone();
    let target = r.warp_target();
    upload(&gpu, r.vs(Renderer::other_vs(target)), &checkerboard());
    let mut enc = gpu.device().create_command_encoder(&Default::default());
    r.warp_pass(&mut enc, &frame(), new, old, mesh);
    gpu.queue().submit([enc.finish()]);
    let t = &r.vs(target).tex;
    (read_texture_level(&gpu, t, 0), r.vs(target).mips)
}

#[test]
fn identity_warp_preserves_image() {
    let Some(gpu) = gpu_or_skip("identity_warp_preserves_image") else {
        return;
    };
    let mut r =
        Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 7).expect("renderer");
    // The C++ draws from VS[1]'s counterpart into the warp target: VS[0]
    // from VS[1] until the frame loop swaps them.
    assert_eq!(r.warp_target(), 0);
    let (ax, ay) = mesh_aspect(W, H);
    let mut mesh = WarpMesh::new(64, 64, ax, ay);
    let vars = identity_vars(1.0);
    mesh.compute_uvs(&vars, None, 0.0, None);
    let rot = [RotParams::default(); 24];
    let shader_inputs = PresetShaderInputs {
        vars: &vars,
        rot: &rot,
        rand_preset: [0.25; 4],
        time_since_preset_start: 1.0,
    };
    let want = image(checkerboard());

    // MilkDrop 1 preset: fixed-function warp, decay 1 -> white vertex colour.
    let fixed = PresetGpu {
        warp: None,
        comp: None,
    };
    let preset = WarpPreset {
        gpu: &fixed,
        shader: shader_inputs,
    };
    let (mip0, levels) = run_warp(&mut r, &preset, None, &mesh);
    let d = dssim(&image(mip0), &want);
    eprintln!("fixed-function identity warp: dssim {d}");
    assert!(d < 0.001, "fixed-function identity warp: dssim {d}");
    assert_eq!(levels, 1, "the warp target has no mips (as in the C++)");

    // Preset warp shader `ret = tex2D(sampler_main, uv).xyz * 1.00`.
    let pg = r.load_preset(&shader_preset(1.0)).expect("load");
    assert!(pg.warp.is_some() && r.last_errors().is_empty());
    let preset = WarpPreset {
        gpu: &pg,
        shader: shader_inputs,
    };
    let (mip0, _) = run_warp(&mut r, &preset, None, &mesh);
    let d = dssim(&image(mip0), &want);
    eprintln!("shader identity warp: dssim {d}");
    assert!(d < 0.001, "shader identity warp: dssim {d}");

    // The end-of-frame flip makes this target the next warp's source.
    r.swap_vs();
    assert_eq!(r.warp_target(), 1);
    let (mip0, _) = run_warp(&mut r, &preset, None, &mesh);
    assert!(dssim(&image(mip0), &want) < 0.001);
}

#[test]
fn blend_draws_old_then_new_with_mix_alpha() {
    let Some(gpu) = gpu_or_skip("blend_draws_old_then_new_with_mix_alpha") else {
        return;
    };
    let mut r =
        Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 7).expect("renderer");
    let (ax, ay) = mesh_aspect(W, H);
    let mut mesh = WarpMesh::new(32, 32, ax, ay);
    let new_vars = identity_vars(1.0);
    let old_vars = identity_vars(0.5);
    // Constant mix2 = 0.5 everywhere: vertex alpha (DWORD)(0.5*255) = 127.
    let info = vec![
        VertInfo {
            rad: 0.0,
            ang: 0.0,
            a: 0.0,
            c: 0.5
        };
        33 * 33
    ];
    mesh.compute_uvs(&new_vars, None, 0.0, None);
    mesh.compute_uvs(&old_vars, None, 0.0, Some((&info, 0.3)));
    let rot = [RotParams::default(); 24];
    let inputs = |vars| PresetShaderInputs {
        vars,
        rot: &rot,
        rand_preset: [0.0; 4],
        time_since_preset_start: 0.0,
    };
    // Old: MilkDrop 1 preset; the fixed path uses the NEW state's decay
    // (milkdropfs.cpp:1918), so make that 0.5 here.
    let mut new_vars = new_vars;
    new_vars.decay = 0.5;
    let fixed = PresetGpu {
        warp: None,
        comp: None,
    };
    let pg = r.load_preset(&shader_preset(1.0)).expect("load");
    let old = WarpPreset {
        gpu: &fixed,
        shader: inputs(&old_vars),
    };
    let new = WarpPreset {
        gpu: &pg,
        shader: inputs(&new_vars),
    };
    let (mip0, _) = run_warp(&mut r, &new, Some(&old), &mesh);
    // Pass 0 (fixed, no blending): white * 127/255. Pass 1 (shader, alpha
    // 127/255): 1*a + 0.498*(1-a) = 0.748 -> 191. Black stays black.
    let px = |x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        [mip0[i], mip0[i + 1], mip0[i + 2]]
    };
    let white = px(4, 4);
    assert!(white[0].abs_diff(191) <= 2, "{white:?}");
    assert_eq!(px(12, 4), [0, 0, 0]);
}

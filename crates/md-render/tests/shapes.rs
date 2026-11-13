//! Task 6.3: custom shapes (`DrawCustomShapes`), geometry on the CPU and
//! pixels on a real (headless) device.
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::{
    build_shape, gpu_or_skip, read_texture_level, FrameInput, PerFrameVars, RenderSettings,
    Renderer, RotParams, ShapeFrameVars, ShapeView, SoundData, Texture2d, Vertex,
};

fn shape() -> ShapeFrameVars {
    ShapeFrameVars {
        x: 0.5,
        y: 0.5,
        rad: 0.3,
        ang: 0.0,
        tex_ang: 0.0,
        tex_zoom: 1.0,
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
        r2: 1.0,
        g2: 0.0,
        b2: 0.0,
        a2: 1.0,
        border_r: 1.0,
        border_g: 1.0,
        border_b: 1.0,
        border_a: 0.0,
        sides: 4.0,
        ..ShapeFrameVars::default()
    }
}

fn view() -> ShapeView {
    ShapeView {
        aspect_y: 1.0,
        tex: (512, 512),
        alpha_mult: 1.0,
    }
}

fn build(v: &ShapeFrameVars) -> (Vec<Vertex>, Vec<Vertex>) {
    let (mut fill, mut border) = (Vec::new(), Vec::new());
    build_shape(v, &view(), &mut fill, &mut border);
    (fill, border)
}

#[test]
fn sides_clamped_to_100() {
    // milkdropfs.cpp:2262-2264: (int)sides clamped to [3, 100]; the fan
    // has sides + 2 vertices.
    assert_eq!(
        build(&ShapeFrameVars {
            sides: 500.0,
            ..shape()
        })
        .0
        .len(),
        102
    );
    assert_eq!(
        build(&ShapeFrameVars {
            sides: 1.0,
            ..shape()
        })
        .0
        .len(),
        5
    );
    assert_eq!(
        build(&ShapeFrameVars {
            sides: -7.0,
            ..shape()
        })
        .0
        .len(),
        5
    );
    // (int) truncates: 3.9 sides is 3.
    assert_eq!(
        build(&ShapeFrameVars {
            sides: 3.9,
            ..shape()
        })
        .0
        .len(),
        5
    );
}

#[test]
fn fan_has_sides_plus_2_vertices() {
    let (fill, _) = build(&ShapeFrameVars {
        sides: 6.0,
        ..shape()
    });
    assert_eq!(fill.len(), 8);
    // v[sides + 1] = v[1] (2297), the centre is v[0] at (2x-1, -2y+1).
    assert_eq!(fill[7], fill[1]);
    assert_eq!(fill[0].pos, [0.0, 0.0, 0.0]);
    // Ring vertex 1 is at angle PI/4 (ang = 0), radius 0.3, aspect 1.
    let c = 0.3 * std::f32::consts::FRAC_1_SQRT_2;
    assert!((fill[1].pos[0] - c).abs() < 1e-6 && (fill[1].pos[1] - c).abs() < 1e-6);
    // Aspect scales x only.
    let mut fill2 = Vec::new();
    let mut border = Vec::new();
    let vw = ShapeView {
        aspect_y: 0.75,
        ..view()
    };
    build_shape(&shape(), &vw, &mut fill2, &mut border);
    assert!((fill2[1].pos[0] - c * 0.75).abs() < 1e-6);
    assert!((fill2[1].pos[1] - c).abs() < 1e-6);
}

#[test]
fn textured_uv_center_is_half_half() {
    let (fill, _) = build(&ShapeFrameVars {
        textured: 1.0,
        ..shape()
    });
    assert_eq!(&fill[0].uv[..2], &[0.5, 0.5]);
    // Ring vertex 1: 0.5 + 0.5 cos(PI/4) / zoom * aspect_y (2285-2286).
    let c = 0.5 + 0.5 * std::f32::consts::FRAC_1_SQRT_2;
    assert!((fill[1].uv[0] - c).abs() < 1e-6 && (fill[1].uv[1] - c).abs() < 1e-6);
    // zoom 2 halves the radius in uv space.
    let (fill, _) = build(&ShapeFrameVars {
        tex_zoom: 2.0,
        ..shape()
    });
    let c = 0.5 + 0.25 * std::f32::consts::FRAC_1_SQRT_2;
    assert!((fill[1].uv[0] - c).abs() < 1e-6);
}

#[test]
fn border_closes_the_loop() {
    let v = ShapeFrameVars {
        border_a: 0.5,
        sides: 5.0,
        ..shape()
    };
    let (_, border) = build(&v);
    // LINESTRIP of `sides` segments from v2[1] (2327-2349): sides + 1 vertices.
    assert_eq!(border.len(), 6);
    assert_eq!(border[0].pos, border[5].pos);
    // Border alpha 0 (or negative) draws no border at all (2320).
    assert!(build(&shape()).1.is_empty());
}

#[test]
fn thick_border_is_four_cumulative_offset_copies() {
    let v = ShapeFrameVars {
        border_a: 1.0,
        thick: 1.0,
        sides: 3.0,
        ..shape()
    };
    let (_, b) = build(&v);
    assert_eq!(b.len(), 4 * 4);
    let (dx, dy) = (2.0 / 512.0, 2.0 / 512.0);
    let base = b[0].pos;
    let off = |c: usize| (b[4 * c].pos[0] - base[0], b[4 * c].pos[1] - base[1]);
    assert_eq!(off(0), (0.0, 0.0));
    // +x, then +y on top of it, then -x (2335-2339).
    assert!((off(1).0 - dx).abs() < 1e-6 && off(1).1.abs() < 1e-6);
    assert!((off(2).0 - dx).abs() < 1e-6 && (off(2).1 - dy).abs() < 1e-6);
    assert!(off(3).0.abs() < 1e-6 && (off(3).1 - dy).abs() < 1e-6);
}

#[test]
fn colours_truncate_wrap_and_scale_alpha() {
    // ((int)(a * 255 * alpha_mult)) & 0xFF (2270-2283): truncation, and a
    // value over 1 wraps.
    let v = ShapeFrameVars {
        a: 1.0,
        r: 0.5,
        g: 1.0,
        b: 0.2,
        a2: 0.5,
        r2: 1.01,
        ..shape()
    };
    let mut fill = Vec::new();
    let mut border = Vec::new();
    let vw = ShapeView {
        alpha_mult: 0.5,
        ..view()
    };
    build_shape(&v, &vw, &mut fill, &mut border);
    assert_eq!(fill[0].color, [127, 255, 51, 127]);
    // 1.01 * 255 = 257.55 -> 257 & 0xFF = 1; 0.5 * 255 * 0.5 = 63.
    assert_eq!(fill[1].color, [1, 0, 0, 63]);
    assert!(fill[1..].iter().all(|p| p.color == fill[1].color));
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

/// The warp shader writes black, so only shapes (and what they sample)
/// show in the warp target.
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

/// Draw one frame of `shapes` (and `old_shapes` while blending) and return
/// the warp target the shapes were drawn into.
fn frame(
    gpu: &md_render::Gpu,
    prefill_src: Option<[u8; 4]>,
    shapes: &[ShapeFrameVars],
    old_shapes: &[ShapeFrameVars],
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
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    // The first frame after creation clears the warp source (971-981), so
    // run an empty frame before filling it.
    let warm = FrameInput::new(&new, &new_pg, &v, &rot, &sound);
    let ctx = r.begin_frame(&warm);
    r.present(ctx, &target.view);
    if let Some(c) = prefill_src {
        // The warp source (the C++'s m_lpVS[0]).
        let src = r.vs(Renderer::other_vs(r.warp_target()));
        let data: Vec<u8> = (0..W * H).flat_map(|_| c).collect();
        let t = Texture2d::from_rgba8(r.gpu(), W, H, &data, false);
        let mut enc = gpu.device().create_command_encoder(&Default::default());
        enc.copy_texture_to_texture(
            t.tex.as_image_copy(),
            src.tex.as_image_copy(),
            wgpu_extent(W, H),
        );
        gpu.queue().submit([enc.finish()]);
    }
    let mut input = FrameInput {
        shapes,
        old_shapes,
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
    // present swapped: the texture the shapes were drawn into is now the
    // warp source.
    read_texture_level(r.gpu(), &r.vs(Renderer::other_vs(r.warp_target())).tex, 0)
}

fn wgpu_extent(w: u32, h: u32) -> md_render::wgpu::Extent3d {
    md_render::wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    }
}

use md_render::wgpu;

#[test]
fn filled_shape_lands_pixels_in_main() {
    let Some(gpu) = gpu_or_skip("filled_shape_lands_pixels_in_main") else {
        return;
    };
    let out = frame(&gpu, None, &[shape()], &[], None);
    // A red square-ish diamond-free quad (4 sides at PI/4) of radius 0.3
    // around the centre; outside it the black warp output.
    let c = px(&out, W / 2, H / 2);
    assert!(c[0] > 240 && c[1] < 10 && c[2] < 10, "{c:?}");
    assert_eq!(&px(&out, 3, 3)[..3], &[0, 0, 0]);
    assert_eq!(&px(&out, W - 4, H / 2)[..3], &[0, 0, 0]);
    // No shapes: nothing drawn.
    let empty = frame(&gpu, None, &[], &[], None);
    assert_eq!(&px(&empty, W / 2, H / 2)[..3], &[0, 0, 0]);
}

#[test]
fn textured_shape_samples_the_warp_source() {
    let Some(gpu) = gpu_or_skip("textured_shape_samples_the_warp_source") else {
        return;
    };
    // The C++ samples m_lpVS[0] (2330), the warp source, while drawing into
    // m_lpVS[1]. The warp shader here writes black, so sampling the target
    // instead would give black.
    let v = ShapeFrameVars {
        textured: 1.0,
        r: 1.0,
        g: 1.0,
        b: 1.0,
        r2: 1.0,
        g2: 1.0,
        b2: 1.0,
        ..shape()
    };
    let out = frame(&gpu, Some([200, 100, 50, 255]), &[v], &[], None);
    let c = px(&out, W / 2, H / 2);
    assert!(
        c[0].abs_diff(200) <= 2 && c[1].abs_diff(100) <= 2 && c[2].abs_diff(50) <= 2,
        "{c:?}"
    );
    assert_eq!(&px(&out, 3, 3)[..3], &[0, 0, 0]);
}

#[test]
fn old_shapes_draw_only_while_blending_scaled_by_progress() {
    let Some(gpu) = gpu_or_skip("old_shapes_draw_only_while_blending_scaled_by_progress") else {
        return;
    };
    // Not blending: old shapes are not drawn (num_reps == 1, 2217).
    let out = frame(&gpu, None, &[], &[shape()], None);
    assert_eq!(&px(&out, W / 2, H / 2)[..3], &[0, 0, 0]);
    // Blending at blend_t = 0: new shapes have alpha_mult 0, old ones 1
    // (2224-2225).
    let out = frame(
        &gpu,
        None,
        &[ShapeFrameVars {
            g: 1.0,
            g2: 1.0,
            ..shape()
        }],
        &[shape()],
        Some(0.0),
    );
    let c = px(&out, W / 2, H / 2);
    assert!(c[0] > 240 && c[1] < 10, "{c:?}");
    // At blend_t = 1 only the new one shows.
    let out = frame(
        &gpu,
        None,
        &[ShapeFrameVars {
            g: 1.0,
            r: 0.0,
            g2: 1.0,
            r2: 0.0,
            ..shape()
        }],
        &[shape()],
        Some(1.0),
    );
    let c = px(&out, W / 2, H / 2);
    assert!(c[1] > 240 && c[0] < 10, "{c:?}");
}

//! Task 5.3: fixed-function pipelines, blend states, fan and point
//! conversion on a real (headless) device.
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::{
    fan_to_list, gpu_or_skip, points_to_quads, read_texture_level, BlendMode, DrawBatch,
    DynamicVertexBuffer, FixedMode, Gpu, PipelineCache, PipelineKey, SamplerKind, Texture2d, Topo,
    Vertex,
};

const FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn v(x: f32, y: f32, rgba: [u8; 4]) -> Vertex {
    Vertex {
        pos: [x, y, 0.0],
        color: rgba,
        ..Default::default()
    }
}

fn vt(x: f32, y: f32, rgba: [u8; 4], u: f32, t: f32) -> Vertex {
    Vertex {
        uv: [u, t, 0.0, 0.0],
        ..v(x, y, rgba)
    }
}

fn key(mode: FixedMode, blend: BlendMode, topo: Topo) -> PipelineKey {
    PipelineKey::new(mode, blend, topo, FMT)
}

/// Clear a `w`×`h` RGBA8 target to `clear` (bytes), run `f` in a batch and
/// read the target back.
fn render(gpu: &Gpu, w: u32, h: u32, clear: [u8; 4], f: impl FnOnce(&mut DrawBatch)) -> Vec<u8> {
    let target = Texture2d::new_target(gpu, w, h, FMT, false);
    let mut cache = PipelineCache::new();
    let mut vbuf = DynamicVertexBuffer::new(gpu, 4);
    let mut enc = gpu.device().create_command_encoder(&Default::default());
    let c = |i: usize| f64::from(clear[i]) / 255.0;
    let load = wgpu::LoadOp::Clear(wgpu::Color {
        r: c(0),
        g: c(1),
        b: c(2),
        a: c(3),
    });
    {
        let mut batch = DrawBatch::begin(gpu, &mut cache, &mut vbuf, &mut enc, &target, load);
        f(&mut batch);
        batch.finish();
    }
    gpu.queue().submit([enc.finish()]);
    read_texture_level(gpu, &target.tex, 0)
}

fn px(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    img[i..i + 4].try_into().unwrap()
}

fn assert_close(got: [u8; 4], want: [u8; 4], what: &str) {
    for c in 0..4 {
        assert!(
            (i32::from(got[c]) - i32::from(want[c])).abs() <= 1,
            "{what}: got {got:?}, want {want:?}"
        );
    }
}

/// A fullscreen quad as a triangle strip.
fn fullscreen(rgba: [u8; 4]) -> [Vertex; 4] {
    [
        vt(-1.0, 1.0, rgba, 0.0, 0.0),
        vt(1.0, 1.0, rgba, 1.0, 0.0),
        vt(-1.0, -1.0, rgba, 0.0, 1.0),
        vt(1.0, -1.0, rgba, 1.0, 1.0),
    ]
}

#[test]
fn color_triangle_renders_expected_pixels() {
    let Some(gpu) = gpu_or_skip("color_triangle_renders_expected_pixels") else {
        return;
    };
    let red = [255, 0, 0, 255];
    let img = render(&gpu, 64, 64, [0, 0, 0, 255], |b| {
        let tri = [v(-0.5, -0.5, red), v(0.5, -0.5, red), v(0.0, 0.5, red)];
        b.draw(
            key(FixedMode::Color, BlendMode::Off, Topo::TriList),
            &tri,
            None,
            SamplerKind::LinearClamp,
        );
    });
    assert_eq!(px(&img, 64, 32, 40), red);
    assert_eq!(px(&img, 64, 2, 2), [0, 0, 0, 255]);
}

#[test]
fn additive_blend_saturates() {
    let Some(gpu) = gpu_or_skip("additive_blend_saturates") else {
        return;
    };
    let img = render(&gpu, 8, 8, [204, 204, 100, 255], |b| {
        b.draw(
            key(FixedMode::Color, BlendMode::Additive, Topo::TriStrip),
            &fullscreen([128, 128, 100, 128]),
            None,
            SamplerKind::LinearClamp,
        );
    });
    // ONE/ONE: 204 + 128 saturates at 255; 100 + 100 = 200; alpha 255.
    for (x, y) in [(0, 0), (4, 4), (7, 7)] {
        assert_eq!(px(&img, 8, x, y), [255, 255, 200, 255]);
    }
}

/// One fullscreen draw of src `S = (200, 100, 50, 64)` over a target
/// cleared to `D = (102, 153, 51, 204)`, per blend mode. Expected values by
/// hand from the D3D9 formula `out = min(1, S*Fs + D*Fd)` per channel, the
/// same factors on alpha (no SEPARATEALPHABLENDENABLE); in bytes with
/// s = S/255, d = D/255, sa = 64/255 ≈ 0.251.
#[test]
fn every_blend_mode_matches_d3d9_formula() {
    let Some(gpu) = gpu_or_skip("every_blend_mode_matches_d3d9_formula") else {
        return;
    };
    let (src, dst) = ([200, 100, 50, 64], [102, 153, 51, 204]);
    let cases: [(BlendMode, [u8; 4]); 13] = [
        // Off: S.
        (BlendMode::Off, [200, 100, 50, 64]),
        // SRCALPHA/INVSRCALPHA: S*sa + D*(1-sa) = (126.6, 139.7, 50.7, 168.9).
        (BlendMode::Alpha, [127, 140, 51, 169]),
        // SRCALPHA/ONE: S*sa + D = (152.2, 178.1, 63.5, 220.1).
        (BlendMode::AlphaAdditive, [152, 178, 64, 220]),
        // ONE/ONE: S + D = (302 -> 255, 253, 101, 268 -> 255).
        (BlendMode::Additive, [255, 253, 101, 255]),
        // ZERO/SRCCOLOR: D*s = (80, 60, 10, 51.2).
        (BlendMode::Multiply, [80, 60, 10, 51]),
        // ONE/ZERO: S.
        (BlendMode::Replace, [200, 100, 50, 64]),
        // INVSRCALPHA/SRCALPHA: S*(1-sa) + D*sa = (175.4, 113.3, 50.3, 99.1).
        (BlendMode::InvAlpha, [175, 113, 50, 99]),
        // SRCCOLOR/INVSRCCOLOR: S*s + D*(1-s) = (178.9, 132.2, 50.8, 168.9).
        (BlendMode::SrcColor, [179, 132, 51, 169]),
        // INVDESTCOLOR/ZERO: S*(1-d) = (120, 40, 40, 12.8).
        (BlendMode::InvDestColor, [120, 40, 40, 13]),
        // ZERO/DESTCOLOR: D*d = (40.8, 91.8, 10.2, 163.2).
        (BlendMode::DestColor, [41, 92, 10, 163]),
        // ZERO/INVDESTCOLOR: D*(1-d) = (61.2, 61.2, 40.8, 40.8).
        (BlendMode::ZeroInvDestColor, [61, 61, 41, 41]),
        // DESTCOLOR/ONE: S*d + D = (182, 213, 61, 255.2 -> 255).
        (BlendMode::DestColorAdd, [182, 213, 61, 255]),
        // ZERO/INVSRCCOLOR: D*(1-s) = (22, 93, 41, 152.8).
        (BlendMode::ZeroInvSrcColor, [22, 93, 41, 153]),
    ];
    for (mode, want) in cases {
        let img = render(&gpu, 4, 4, dst, |b| {
            b.draw(
                key(FixedMode::Color, mode, Topo::TriStrip),
                &fullscreen(src),
                None,
                SamplerKind::LinearClamp,
            );
        });
        assert_close(px(&img, 4, 1, 2), want, &format!("{mode:?}"));
    }
}

#[test]
fn fan_conversion_produces_n_minus_2_triangles() {
    let c = [255, 255, 255, 255];
    let fan: Vec<Vertex> = (0..6).map(|i| v(i as f32, 0.0, c)).collect();
    let list = fan_to_list(&fan);
    assert_eq!(list.len(), 3 * (6 - 2));
    for (t, tri) in list.chunks(3).enumerate() {
        assert_eq!(tri, [fan[0], fan[t + 1], fan[t + 2]]);
    }
    assert!(fan_to_list(&fan[..2]).is_empty());

    let Some(gpu) = gpu_or_skip("fan_conversion_produces_n_minus_2_triangles") else {
        return;
    };
    // A D3DPT_TRIANGLEFAN quad (as the C++'s borders, milkdropfs.cpp:3344)
    // covers the whole target.
    let g = [0, 255, 0, 255];
    let quad = [
        v(-1.0, -1.0, g),
        v(1.0, -1.0, g),
        v(1.0, 1.0, g),
        v(-1.0, 1.0, g),
    ];
    let img = render(&gpu, 8, 8, [0, 0, 0, 255], |b| {
        b.draw(
            key(FixedMode::Color, BlendMode::Off, Topo::TriFan),
            &quad,
            None,
            SamplerKind::LinearClamp,
        );
    });
    assert!(img.chunks(4).all(|p| p == g), "fan quad fills the target");
}

#[test]
fn points_become_quads() {
    let p = v(0.25, -0.5, [1, 2, 3, 4]);
    let quads = points_to_quads(&[p, p], 3.0, (16, 8));
    assert_eq!(quads.len(), 12, "two triangles per point");
    let (hx, hy) = (3.0 / 16.0, 3.0 / 8.0);
    for q in &quads {
        assert!(((q.pos[0] - 0.25).abs() - hx).abs() < 1e-6, "{q:?}");
        assert!(((q.pos[1] + 0.5).abs() - hy).abs() < 1e-6, "{q:?}");
        assert_eq!(q.color, p.color);
    }

    let Some(gpu) = gpu_or_skip("points_become_quads") else {
        return;
    };
    // D3D9 puts pixel (5, 7) of a 16x16 target at NDC (2*5/16 - 1,
    // 1 - 2*7/16); a point of size S covers the S x S pixels around it.
    let w = [255, 255, 255, 255];
    let at = v(2.0 * 5.0 / 16.0 - 1.0, 1.0 - 2.0 * 7.0 / 16.0, w);
    for (size, lo, hi) in [(None, (5, 7), (5, 7)), (Some(3.0), (4, 6), (6, 8))] {
        let img = render(&gpu, 16, 16, [0, 0, 0, 255], |b| {
            if let Some(s) = size {
                b.set_point_size(s);
            }
            b.draw(
                key(FixedMode::Color, BlendMode::Off, Topo::Points),
                &[at],
                None,
                SamplerKind::LinearClamp,
            );
        });
        for y in 0..16 {
            for x in 0..16 {
                let inside = (lo.0..=hi.0).contains(&x) && (lo.1..=hi.1).contains(&y);
                let want = if inside { w } else { [0, 0, 0, 255] };
                assert_eq!(px(&img, 16, x, y), want, "size {size:?} pixel ({x},{y})");
            }
        }
    }
}

#[test]
fn textured_modes_follow_d3d9_stage_states() {
    let Some(gpu) = gpu_or_skip("textured_modes_follow_d3d9_stage_states") else {
        return;
    };
    let tex = Texture2d::from_rgba8(&gpu, 1, 1, &[255, 128, 64, 32], false);
    let color = [128, 255, 255, 200];
    let cases = [
        // MODULATE / SELECTARG1 DIFFUSE: (tex.rgb * color.rgb, color.a).
        (FixedMode::TexModulate, [128, 128, 64, 200]),
        // MODULATE / MODULATE: tex * color; alpha 32 * 200 / 255 = 25.1.
        (FixedMode::TexModulateAlpha, [128, 128, 64, 25]),
        // SELECTARG1 TEXTURE.
        (FixedMode::TexSelect, [255, 128, 64, 32]),
        // No texture: the colour.
        (FixedMode::Color, color),
    ];
    for (mode, want) in cases {
        let img = render(&gpu, 4, 4, [0, 0, 0, 0], |b| {
            b.draw(
                key(mode, BlendMode::Off, Topo::TriStrip),
                &fullscreen(color),
                Some(&tex),
                SamplerKind::PointClamp,
            );
        });
        for (x, y) in [(0, 0), (3, 3)] {
            assert_close(px(&img, 4, x, y), want, &format!("{mode:?}"));
        }
    }
}

#[test]
fn lines_draw_one_pixel_wide() {
    let Some(gpu) = gpu_or_skip("lines_draw_one_pixel_wide") else {
        return;
    };
    // A horizontal D3D9 line along pixel row 3 of an 8x8 target.
    let w = [255, 255, 255, 255];
    let y = 1.0 - 2.0 * 3.0 / 8.0;
    let img = render(&gpu, 8, 8, [0, 0, 0, 255], |b| {
        b.draw(
            key(FixedMode::Color, BlendMode::Off, Topo::LineStrip),
            &[v(-1.0, y, w), v(1.0, y, w)],
            None,
            SamplerKind::LinearClamp,
        );
    });
    for row in 0..8 {
        let lit = (0..8).filter(|&x| px(&img, 8, x, row) == w).count();
        assert_eq!(lit > 0, row == 3, "row {row} lit {lit}");
    }
}

#[test]
fn pipeline_cache_reuses_pipelines() {
    let Some(gpu) = gpu_or_skip("pipeline_cache_reuses_pipelines") else {
        return;
    };
    let mut cache = PipelineCache::new();
    assert!(cache.is_empty());
    let a = cache
        .get(&gpu, key(FixedMode::Color, BlendMode::Alpha, Topo::TriList))
        .clone();
    let b = cache
        .get(&gpu, key(FixedMode::Color, BlendMode::Alpha, Topo::TriFan))
        .clone();
    let c = cache
        .get(&gpu, key(FixedMode::Color, BlendMode::Alpha, Topo::Points))
        .clone();
    assert_eq!(a, b, "fans draw with the list pipeline");
    assert_eq!(a, c, "points draw with the list pipeline");
    assert_eq!(cache.len(), 1);
    cache.get(
        &gpu,
        key(FixedMode::TexModulate, BlendMode::Alpha, Topo::TriList),
    );
    cache.get(
        &gpu,
        key(FixedMode::Color, BlendMode::Additive, Topo::LineStrip),
    );
    assert_eq!(cache.len(), 3);
    let tex = Texture2d::from_rgba8(&gpu, 1, 1, &[0; 4], false);
    let g1 = cache.texture_bind_group(&gpu, &tex.view, SamplerKind::LinearWrap);
    let g2 = cache.texture_bind_group(&gpu, &tex.view, SamplerKind::LinearWrap);
    let g3 = cache.texture_bind_group(&gpu, &tex.view, SamplerKind::PointWrap);
    assert_eq!(g1, g2);
    assert_ne!(g1, g3);
}

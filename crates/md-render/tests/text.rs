//! Task 6.6: the text pass (`CTextManager`, textmgr.cpp) — queued text and
//! dark boxes drawn over the composite, the calc rect, the flag masking and
//! the draw target.
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::md_text::{Align, FontSpec, Rect, TextFlags};
use md_render::{
    gpu_or_skip, read_texture_level, FontId, FrameInput, Gpu, PerFrameVars, PresetGpu,
    RenderSettings, Renderer, RotParams, SoundData, TextQueue, Texture2d,
};

const W: u32 = 160;
const H: u32 = 64;
const WHITE: [u8; 4] = [255; 4];

fn fonts() -> [FontSpec; 6] {
    std::array::from_fn(|_| FontSpec {
        face: "Arial".into(),
        size_px: 20,
        bold: false,
        italic: false,
        antialiased: true,
    })
}

fn queue() -> TextQueue {
    TextQueue::with_bundled_fonts(fonts())
}

fn renderer(gpu: &Gpu) -> Renderer {
    Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).expect("renderer")
}

/// An all-black frame (no shaders, no wave, no motion vectors, cleared
/// source) with `queue` flushed over the composite; returns the output.
fn frame_with_text(r: &mut Renderer, q: &mut TextQueue) -> Vec<u8> {
    let preset = md_preset::Preset::defaults();
    let pg = PresetGpu {
        warp: None,
        comp: None,
    };
    let mut vars = PerFrameVars::from_preset(&preset, 0.0);
    (vars.zoom, vars.zoomexp, vars.rot, vars.warp) = (1.0, 1.0, 0.0, 0.0);
    (vars.cx, vars.cy, vars.dx, vars.dy, vars.sx, vars.sy) = (0.5, 0.5, 0.0, 0.0, 1.0, 1.0);
    (vars.decay, vars.wrap, vars.gamma, vars.echo_alpha) = (1.0, 1.0, 1.0, 0.0);
    (vars.shader, vars.wave_a, vars.mv_a) = (0.0, 0.0, 0.0);
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    r.mesh_mut().compute_uvs(&vars, None, 0.0, None);
    let input = FrameInput::new(&preset, &pg, &vars, &rot, &sound);
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    let mut ctx = r.begin_frame(&input);
    r.flush_text(&mut ctx, q);
    r.present(ctx, &target.view);
    read_texture_level(r.gpu(), &target.tex, 0)
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn non_black(p: [u8; 4]) -> bool {
    p[0] > 0 || p[1] > 0 || p[2] > 0
}

#[test]
fn text_pass_draws_visible_glyphs() {
    let Some(gpu) = gpu_or_skip("text_pass_draws_visible_glyphs") else {
        return;
    };
    let mut r = renderer(&gpu);
    let mut q = queue();
    let calc = q.draw(
        FontId::Simple,
        "MilkDrop",
        Rect::new(8, 8, 150, 60),
        TextFlags::default(),
        WHITE,
        None,
    );
    assert!(
        calc.right - calc.left > 40 && calc.bottom - calc.top >= 16,
        "{calc:?}"
    );
    let img = frame_with_text(&mut r, &mut q);
    let (mut inside, mut outside) = (0, 0);
    for y in 0..H {
        for x in 0..W {
            if non_black(px(&img, x, y)) {
                let (xi, yi) = (x as i32, y as i32);
                if (calc.left..calc.right).contains(&xi) && (calc.top..calc.bottom).contains(&yi) {
                    inside += 1;
                } else {
                    outside += 1;
                }
            }
        }
    }
    assert!(inside > 30, "glyph pixels inside the calc rect: {inside}");
    assert_eq!(outside, 0, "non-black pixels outside {calc:?}");
    // Pure white exists (full coverage), and the output is opaque.
    assert!(img.chunks_exact(4).any(|p| p[..3] == [255; 3]));
    assert!(img.chunks_exact(4).all(|p| p[3] == 255));
    assert!(q.is_empty(), "flush drains the queue");
}

#[test]
fn dark_box_precedes_text() {
    let Some(gpu) = gpu_or_skip("dark_box_precedes_text") else {
        return;
    };
    let mut r = renderer(&gpu);
    let mut q = queue();
    let red = [200, 0, 0, 255];
    let calc = q.draw(
        FontId::Simple,
        "MilkDrop",
        Rect::new(8, 8, 150, 60),
        TextFlags::default(),
        WHITE,
        Some(red),
    );
    let img = frame_with_text(&mut r, &mut q);
    // The box covers the calc rect exactly (textmgr.cpp:153-156: its rect is
    // the text's calc rect; D3D9 fill rules: [left, right) x [top, bottom)).
    assert_eq!(px(&img, calc.left as u32, calc.top as u32), red);
    assert_eq!(px(&img, calc.right as u32 - 1, calc.bottom as u32 - 1), red);
    assert_eq!(px(&img, calc.right as u32, calc.top as u32), [0, 0, 0, 255]);
    assert_eq!(
        px(&img, calc.left as u32, calc.bottom as u32),
        [0, 0, 0, 255]
    );
    // Text over the box: white-ish glyph pixels exist (a box drawn after
    // the text would leave only 200-red), and every pixel is red-to-white.
    let mut bright = 0;
    for y in calc.top..calc.bottom {
        for x in calc.left..calc.right {
            let p = px(&img, x as u32, y as u32);
            assert!(p[0] >= 200, "{p:?} at {x},{y}");
            if p[1] > 200 && p[2] > 200 {
                bright += 1;
            }
        }
    }
    assert!(bright > 20, "bright glyph pixels over the box: {bright}");
}

#[test]
fn later_box_covers_earlier_text() {
    // Messages draw in queue order (textmgr.cpp:601-621), so a box queued
    // after some text covers it; only a text's own box is swapped before it.
    let Some(gpu) = gpu_or_skip("later_box_covers_earlier_text") else {
        return;
    };
    let mut r = renderer(&gpu);
    let mut q = queue();
    let calc = q.draw(
        FontId::Simple,
        "MilkDrop",
        Rect::new(8, 8, 150, 60),
        TextFlags::default(),
        WHITE,
        None,
    );
    assert!(calc.right > calc.left && calc.bottom > calc.top, "{calc:?}");
    q.draw_box(calc, [0, 0, 255, 255]);
    let img = frame_with_text(&mut r, &mut q);
    for y in calc.top..calc.bottom {
        for x in calc.left..calc.right {
            assert_eq!(px(&img, x as u32, y as u32), [0, 0, 255, 255]);
        }
    }
}

fn width(q: &mut TextQueue, text: &str, flags: TextFlags, rect: Rect) -> i32 {
    let r = q.calc_rect(FontId::Simple, text, rect, flags);
    r.right - r.left
}

#[test]
fn ampersand_prefix_is_processed_and_ellipsis_is_ignored() {
    let mut q = queue();
    let wide = Rect::new(0, 0, 1000, 100);
    let none = TextFlags::default();
    // `&` prefix processing is ON: a single & is removed, && is one &.
    let plain = width(&mut q, "AB", none, wide);
    assert!(plain > 0);
    assert_eq!(width(&mut q, "A&B", none, wide), plain);
    assert!(width(&mut q, "A&&B", none, wide) > plain);
    // DT_NOPREFIX is stripped before drawing (textmgr.cpp:122, 174), so
    // asking for it changes nothing.
    let no_prefix = TextFlags {
        no_prefix: true,
        ..none
    };
    assert_eq!(width(&mut q, "A&B", no_prefix, wide), plain);
    // So are DT_WORD_ELLIPSIS and DT_END_ELLIPSIS: an overlong line is
    // neither truncated nor given dots.
    let long = "A fairly long line of text that cannot fit";
    let narrow = Rect::new(0, 0, 40, 100);
    let full = width(&mut q, long, none, narrow);
    assert!(full > 40);
    let ell = TextFlags {
        word_ellipsis: true,
        end_ellipsis: true,
        single_line: true,
        no_prefix: true,
        ..none
    };
    assert_eq!(width(&mut q, long, ell, narrow), full);
    // The drawn glyph count is the full line's too.
    let before = q.calc_rect(FontId::Simple, long, narrow, ell);
    let drawn = q.draw(FontId::Simple, long, narrow, ell, WHITE, None);
    assert_eq!(drawn, before);
}

#[test]
fn calc_rect_keeps_origin_and_ignores_alignment() {
    // DT_CALCRECT: left/top unchanged, right = left + widest line, bottom =
    // top + lines * line height, for any alignment (the C++ only reads
    // right - left; the queued rect is then the tight rect, textmgr.cpp:148).
    let mut q = queue();
    let rect = Rect::new(10, 5, 300, 90);
    let left = q.calc_rect(FontId::Simple, "Hi", rect, TextFlags::default());
    for align in [Align::Center, Align::Right] {
        let f = TextFlags {
            align,
            ..TextFlags::default()
        };
        assert_eq!(q.calc_rect(FontId::Simple, "Hi", rect, f), left);
    }
    assert_eq!((left.left, left.top), (10, 5));
    assert_eq!(left.bottom - left.top, 20);
    // Two lines are twice as tall without single_line, one with it.
    let two = q.calc_rect(FontId::Simple, "a\nb", rect, TextFlags::default());
    assert_eq!(two.bottom - two.top, 40);
    let one = TextFlags {
        single_line: true,
        ..TextFlags::default()
    };
    assert_eq!(
        q.calc_rect(FontId::Simple, "a\nb", rect, one).bottom - 5,
        20
    );
    // calc_rect queues nothing (the DT_CALCRECT path returns directly,
    // textmgr.cpp:127-128).
    assert!(q.is_empty());
}

#[test]
fn text_lands_on_the_output_not_in_the_main_texture() {
    let Some(gpu) = gpu_or_skip("text_lands_on_the_output_not_in_the_main_texture") else {
        return;
    };
    let mut r = renderer(&gpu);
    let mut q = queue();
    q.draw(
        FontId::Tooltip,
        "MilkDrop",
        Rect::new(8, 8, 150, 60),
        TextFlags::default(),
        WHITE,
        Some([0, 0, 0, 255]),
    );
    let img = frame_with_text(&mut r, &mut q);
    assert!(img
        .chunks_exact(4)
        .any(|p| non_black([p[0], p[1], p[2], p[3]])));
    // Neither ping-pong main texture received any of it (the swap after
    // present leaves both on the renderer).
    for i in 0..2 {
        let t = r.vs(i);
        let main = read_texture_level(r.gpu(), &t.tex, 0);
        assert!(
            main.chunks_exact(4).all(|p| p[..3] == [0, 0, 0]),
            "main texture {i} holds text"
        );
    }
    // And it does not persist: the next frame (queue drained) is black.
    let next = frame_with_text(&mut r, &mut q);
    assert!(next.chunks_exact(4).all(|p| p[..3] == [0, 0, 0]));
}

#[test]
fn glyph_texels_land_one_to_one_on_pixels() {
    // The half-pixel convention: every pixel of a glyph quad shows exactly
    // the atlas texel at its offset (white text over black, so the output
    // equals the coverage), not a bilinear mix of neighbours.
    let Some(gpu) = gpu_or_skip("glyph_texels_land_one_to_one_on_pixels") else {
        return;
    };
    let mut r = renderer(&gpu);
    let mut q = queue();
    let rect = Rect::new(7, 9, 150, 60);
    q.draw(
        FontId::Simple,
        "Rag",
        rect,
        TextFlags::default(),
        WHITE,
        None,
    );
    let img = frame_with_text(&mut r, &mut q);
    // The same layout from a separate atlas of the same font.
    let spec = fonts()[0].clone();
    let mut atlas =
        md_render::md_text::Atlas::new(&spec, &md_render::md_text::fonts::bundled_database());
    let l = md_render::md_text::layout(&mut atlas, "Rag", rect, TextFlags::default());
    let (tex, tw, th) = atlas.texture_rgba();
    let mut checked = 0;
    for g in &l.quads {
        for y in g.y0 as u32..g.y1 as u32 {
            for x in g.x0 as u32..g.x1 as u32 {
                let tx = (g.u0 * tw as f32).round() as u32 + (x - g.x0 as u32);
                let ty = (g.v0 * th as f32).round() as u32 + (y - g.y0 as u32);
                let cov = tex[((ty * tw + tx) * 4 + 3) as usize];
                let got = px(&img, x, y)[0];
                assert!(got.abs_diff(cov) <= 1, "({x},{y}): {got} vs coverage {cov}");
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "{checked}");
}

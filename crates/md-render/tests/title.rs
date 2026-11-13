//! Task 6.7: the song title texture (`RenderStringToTitleTexture`,
//! milkdropfs.cpp:230-458) and its animation (`ShowSongTitleAnim`,
//! :4477-4713), drawn into the main texture when it ends (:1059-1067) and
//! over the composite while it runs (:1123-1129).
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_render::md_text::FontSpec;
use md_render::{
    gpu_or_skip, read_texture_level, FrameInput, Gpu, PerFrameVars, PresetGpu, RenderSettings,
    Renderer, RotParams, SoundData, Texture2d, SONG_TITLE_DURATION,
};

const W: u32 = 160;
const H: u32 = 64;

fn spec() -> FontSpec {
    FontSpec {
        face: "Arial".into(),
        size_px: 16,
        bold: false,
        italic: false,
        antialiased: true,
    }
}

fn renderer(gpu: &Gpu) -> Renderer {
    let mut r =
        Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).expect("renderer");
    r.set_title_font_bundled(spec());
    r
}

/// One all-black frame (no shaders, no wave, no motion vectors) at `now`
/// with the given decay; returns the output. `extra` may record more draws.
fn frame(
    r: &mut Renderer,
    now: f32,
    decay: f64,
    extra: impl FnOnce(&mut Renderer, &mut md_render::FrameCtx),
) -> Vec<u8> {
    let preset = md_preset::Preset::defaults();
    let pg = PresetGpu {
        warp: None,
        comp: None,
    };
    let mut vars = PerFrameVars::from_preset(&preset, 0.0);
    (vars.zoom, vars.zoomexp, vars.rot, vars.warp) = (1.0, 1.0, 0.0, 0.0);
    (vars.cx, vars.cy, vars.dx, vars.dy, vars.sx, vars.sy) = (0.5, 0.5, 0.0, 0.0, 1.0, 1.0);
    (vars.decay, vars.wrap, vars.gamma, vars.echo_alpha) = (decay, 1.0, 1.0, 0.0);
    (vars.shader, vars.wave_a, vars.mv_a) = (0.0, 0.0, 0.0);
    let rot = [RotParams::default(); 24];
    let sound = SoundData::default();
    r.mesh_mut().compute_uvs(&vars, None, 0.0, None);
    let mut input = FrameInput::new(&preset, &pg, &vars, &rot, &sound);
    input.now = now;
    input.time = now;
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    let mut ctx = r.begin_frame(&input);
    extra(r, &mut ctx);
    r.present(ctx, &target.view);
    read_texture_level(r.gpu(), &target.tex, 0)
}

fn lit(img: &[u8]) -> usize {
    img.chunks_exact(4)
        .filter(|p| p[..3].iter().any(|&c| c > 0))
        .count()
}

#[test]
fn title_anim_at_progress_half_is_visible() {
    let Some(gpu) = gpu_or_skip("title_anim_at_progress_half_is_visible") else {
        return;
    };
    let mut r = renderer(&gpu);
    assert!(r.start_song_title("Rag", 0.0));
    let now = 0.5 * SONG_TITLE_DURATION;
    let img = frame(&mut r, now, 1.0, |_, _| {});
    assert!(lit(&img) > 40, "lit pixels at progress 0.5: {}", lit(&img));
    assert!(img.chunks_exact(4).all(|p| p[3] == 255));
    // Still running: the next frame draws it again.
    assert!(r.supertext().start_time >= 0.0);
}

#[test]
fn title_texture_holds_glyph_coverage_for_a_known_string() {
    let Some(gpu) = gpu_or_skip("title_texture_holds_glyph_coverage_for_a_known_string") else {
        return;
    };
    let mut r = renderer(&gpu);
    assert!(
        !r.render_title_texture(""),
        "empty text renders nothing (:235)"
    );
    assert!(r.render_title_texture("Rag"));
    let s = r.settings();
    let tx = s.tex_w.max(s.tex_h);
    let tex = r.title_texture().expect("allocated").clone();
    // plugin.cpp:2177-2178: max(tex_w, tex_h) wide, a quarter as high.
    assert_eq!((tex.w, tex.h), (tx, tx / 4));
    let img = read_texture_level(r.gpu(), &tex.tex, 0);
    let (w, h) = (tex.w as usize, tex.h as usize);
    let used = r.supertext().font_size_used as usize;
    assert!(used > 0);
    let (mut x0, mut x1, mut y0, mut y1, mut n) = (w, 0, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            let p = &img[(y * w + x) * 4..][..4];
            // Opaque black clear (:268-286); glyph edges blend alpha with the same
            // factors as colour (no SEPARATEALPHABLENDENABLE), so only the
            // untouched texels are asserted opaque.
            if p[0] == 0 {
                assert_eq!(p[3], 255, "clear at {x},{y}");
            } else {
                (x0, x1, y0, y1, n) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y), n + 1);
            }
        }
    }
    assert!(n > 50, "glyph pixels: {n}");
    // Centred (DT_CENTER, :443-447) in a band of the font height around Y/2.
    let cx = (x0 + x1) / 2;
    assert!(cx.abs_diff(w / 2) <= w / 16, "centre {cx} of {w}");
    assert!(
        y0 >= h / 2 - used / 2 && y1 < h / 2 + used / 2 + 1,
        "rows {y0}..{y1}"
    );
    assert!(
        img.chunks_exact(4).any(|p| p[0] > 200),
        "near-full coverage exists"
    );
}

#[test]
fn anim_draws_nothing_outside_its_active_range() {
    let Some(gpu) = gpu_or_skip("anim_draws_nothing_outside_its_active_range") else {
        return;
    };
    // Never started: black (start_time -1 is the off state, plugin.cpp:1117).
    let mut r = renderer(&gpu);
    assert_eq!(r.supertext().start_time, -1.0);
    assert_eq!(lit(&frame(&mut r, 5.0, 1.0, |_, _| {})), 0);

    // Started, finishing frame (progress >= 1): burnt into main, so it
    // shows and persists through the feedback with decay 1 ...
    let mut r = renderer(&gpu);
    assert!(r.start_song_title("Rag", 0.0));
    let end = frame(&mut r, SONG_TITLE_DURATION + 0.3, 1.0, |_, _| {});
    assert!(lit(&end) > 40);
    assert_eq!(
        r.supertext().start_time,
        -1.0,
        "off after the last frame (:1128)"
    );
    let kept = frame(&mut r, SONG_TITLE_DURATION + 0.4, 1.0, |_, _| {});
    assert!(lit(&kept) > 40, "burnt into the main texture");
    // ... and once off nothing is drawn any more: with decay 0 the burnt
    // image is gone and no screen-mode draw replaces it.
    let gone = frame(&mut r, SONG_TITLE_DURATION + 0.5, 0.0, |_, _| {});
    assert_eq!(lit(&gone), 0);
}

#[test]
fn same_inputs_give_identical_pixels() {
    let Some(gpu) = gpu_or_skip("same_inputs_give_identical_pixels") else {
        return;
    };
    let run = |now: f32| {
        let mut r = renderer(&gpu);
        assert!(r.render_title_texture("Rag"));
        r.supertext_mut().font_size = 16.0;
        frame(&mut r, now, 1.0, |r, ctx| r.draw_title_anim(ctx, 0.1, now))
    };
    let (a, b) = (run(3.25), run(3.25));
    assert_eq!(a, b);
    assert!(lit(&a) > 0);
    // The warp wobble follows the clock (GetTime(), :4555-4560), not a seed.
    assert_ne!(a, run(7.5));
}

/// Lit pixels in each row of the lit bounding box, top to bottom.
fn row_spans(img: &[u8], w: usize) -> Vec<usize> {
    let rows: Vec<Vec<usize>> = img
        .chunks_exact(w * 4)
        .map(|r| (0..w).filter(|&x| r[x * 4] > 60).collect())
        .collect();
    let first = rows.iter().position(|r| !r.is_empty()).expect("lit");
    let last = rows.iter().rposition(|r| !r.is_empty()).expect("lit");
    rows[first..=last].iter().map(Vec::len).collect()
}

#[test]
fn title_is_upright_on_screen_and_burnt_into_main() {
    let Some(gpu) = gpu_or_skip("title_is_upright_on_screen_and_burnt_into_main") else {
        return;
    };
    // `T`s: a wide bar on top of a narrow stem. The fixed-function ortho
    // projection flips y (support.cpp:179), so the mesh's first row (the
    // texture's top) is the top of the screen.
    let upright = |img: &[u8]| {
        let spans = row_spans(img, 512);
        assert!(spans.len() >= 6, "text height {}", spans.len());
        let k = spans.len() / 3;
        let top: usize = spans[..k].iter().sum();
        let bottom: usize = spans[spans.len() - k..].iter().sum();
        assert!(top > 2 * bottom, "top {top} vs bottom {bottom}: {spans:?}");
    };
    let size = |gpu: &Gpu| {
        let mut r = Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(512, 256), 3)
            .expect("renderer");
        r.set_title_font_bundled(spec());
        assert!(r.start_song_title("TTTTTTTT", 0.0));
        r
    };
    let big = |r: &mut Renderer, now: f32| {
        let target =
            Texture2d::new_target(r.gpu(), 512, 256, wgpu::TextureFormat::Rgba8Unorm, false);
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
        let mut input = FrameInput::new(&preset, &pg, &vars, &rot, &sound);
        input.now = now;
        let ctx = r.begin_frame(&input);
        r.present(ctx, &target.view);
        read_texture_level(r.gpu(), &target.tex, 0)
    };
    let mut r = size(&gpu);
    upright(&big(&mut r, 0.9 * SONG_TITLE_DURATION)); // over the composite
    let mut r = size(&gpu);
    big(&mut r, SONG_TITLE_DURATION + 0.1); // burn-in frame
    upright(&big(&mut r, SONG_TITLE_DURATION + 0.2)); // carried by main
}

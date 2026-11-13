//! Task 6.8: image sprites (`DrawUserSprites`, milkdropfs.cpp:3432-3777;
//! `LaunchSprite`, plugin.cpp:8567-8713; `texmgr::LoadTex`, texmgr.cpp:70-135).
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use image::{Rgba, RgbaImage};
use md_render::md_text::{FontSpec, Rect, TextFlags};
use md_render::wgpu;
use md_render::{
    gpu_or_skip, read_texture_level, FontId, FrameCtx, FrameInput, Gpu, PerFrameVars, PresetGpu,
    RenderSettings, Renderer, RotParams, SoundData, SpriteFrame, SpriteVars, TextQueue, Texture2d,
    TextureId, SONG_TITLE_DURATION,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const W: u32 = 160;
const H: u32 = 64;

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("md-render-sprites-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn save(dir: &Path, name: &str, img: &RgbaImage) -> PathBuf {
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

fn solid(w: u32, h: u32, c: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba(c))
}

fn renderer(gpu: &Gpu) -> Renderer {
    let mut r =
        Renderer::new(gpu.device(), gpu.queue(), RenderSettings::new(W, H), 3).expect("renderer");
    r.set_title_font_bundled(FontSpec {
        face: "Arial".into(),
        size_px: 16,
        bold: false,
        italic: false,
        antialiased: true,
    });
    r
}

fn load(gpu: &Gpu, r: &mut Renderer, path: &Path, key: u32) -> TextureId {
    r.textures_mut().load_sprite(gpu, path, key).expect("load")
}

fn sprite(id: TextureId, f: impl FnOnce(&mut SpriteVars)) -> SpriteFrame {
    let mut v = SpriteVars::default();
    f(&mut v);
    SpriteFrame::from_vars(&v, id)
}

/// One all-black frame (no shaders, no wave, decay 1, identity warp) at
/// `now`; `extra` records more draws between `begin_frame` and `present`.
fn frame(r: &mut Renderer, now: f32, extra: impl FnOnce(&mut Renderer, &mut FrameCtx)) -> Vec<u8> {
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
    input.time = now;
    let target = Texture2d::new_target(r.gpu(), W, H, wgpu::TextureFormat::Rgba8Unorm, false);
    let mut ctx = r.begin_frame(&input);
    extra(r, &mut ctx);
    r.present(ctx, &target.view);
    read_texture_level(r.gpu(), &target.tex, 0)
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 150 && p[1] < 60 && p[2] < 60
}

/// (rows, columns) of the pixels satisfying `f`.
fn extent(img: &[u8], f: impl Fn([u8; 4]) -> bool) -> (Vec<u32>, Vec<u32>) {
    let mut rows = Vec::new();
    let mut cols = Vec::new();
    for y in 0..H {
        for x in 0..W {
            if f(px(img, x, y)) {
                rows.push(y);
                cols.push(x);
            }
        }
    }
    (rows, cols)
}

fn mean(v: &[u32]) -> f64 {
    v.iter().map(|&y| f64::from(y)).sum::<f64>() / v.len().max(1) as f64
}

const RED: [u8; 4] = [255, 0, 0, 255];

#[test]
fn colorkey_becomes_alpha_at_load_time() {
    let Some(gpu) = gpu_or_skip("colorkey_becomes_alpha_at_load_time") else {
        return;
    };
    let dir = temp_dir("colorkey");
    let mut img = RgbaImage::new(4, 1);
    img.put_pixel(0, 0, Rgba([0x10, 0x20, 0x30, 255])); // the key
    img.put_pixel(1, 0, Rgba([0x10, 0x20, 0x31, 255])); // one blue step off
    img.put_pixel(2, 0, Rgba([0x30, 0x20, 0x10, 255])); // channels swapped
    img.put_pixel(3, 0, Rgba([0, 0, 0, 255])); // black, not the key
    let path = save(&dir, "key.png", &img);
    let mut r = renderer(&gpu);
    let id = load(&gpu, &mut r, &path, 0x0010_2030);
    let t = r.textures().texture(id).expect("sprite texture");
    assert_eq!((t.w, t.h), (4, 1));
    let rgba = read_texture_level(&gpu, &t.tex, 0);
    assert_eq!(
        rgba[0..4],
        [0, 0, 0, 0],
        "inside the key: transparent black"
    );
    assert_eq!(rgba[4..8], [0x10, 0x20, 0x31, 255], "outside the key");
    assert_eq!(rgba[8..12], [0x30, 0x20, 0x10, 255]);
    assert_eq!(rgba[12..16], [0, 0, 0, 255]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Collects the messages of WARN events.
#[derive(Clone, Default)]
struct Warnings(Arc<Mutex<Vec<String>>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Warnings {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        struct V(String);
        impl tracing::field::Visit for V {
            fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn std::fmt::Debug) {
                self.0.push_str(&format!("{}={v:?} ", f.name()));
            }
        }
        if *event.metadata().level() == tracing::Level::WARN {
            let mut v = V(String::new());
            event.record(&mut v);
            self.0.lock().unwrap().push(v.0);
        }
    }
}

#[test]
fn sprite_with_a_missing_image_is_not_created_and_logs_once_per_name() {
    use tracing_subscriber::layer::SubscriberExt;
    let Some(gpu) =
        gpu_or_skip("sprite_with_a_missing_image_is_not_created_and_logs_once_per_name")
    else {
        return;
    };
    let dir = temp_dir("missing");
    let corrupt = dir.join("corrupt.png");
    std::fs::write(&corrupt, b"not an image").unwrap();
    let absent = dir.join("no_such_sprite_anywhere.png");
    let warnings = Warnings::default();
    let subscriber = tracing_subscriber::registry().with(warnings.clone());
    let mut r = renderer(&gpu);
    tracing::subscriber::with_default(subscriber, || {
        // LaunchSprite returns false and creates no sprite (plugin.cpp:8667,
        // 8670-8712; texmgr.cpp:115-123): no id comes back.
        for _ in 0..3 {
            assert!(r.textures_mut().load_sprite(&gpu, &absent, 0).is_err());
            assert!(r.textures_mut().load_sprite(&gpu, &corrupt, 0).is_err());
        }
    });
    let logs = warnings.0.lock().unwrap().clone();
    for name in ["no_such_sprite_anywhere", "corrupt.png"] {
        let hits = logs.iter().filter(|m| m.contains(name)).count();
        assert_eq!(hits, 1, "{name}: {logs:?}");
    }
    // A frame naming a sprite that no longer exists (KillTex, texmgr) draws
    // nothing: DrawUserSprites skips slots without a surface (mfs:3474).
    let ok = save(&dir, "ok.png", &solid(8, 8, RED));
    let id = load(&gpu, &mut r, &ok, 0x00ff_00ff);
    r.textures_mut().kill_sprite(id);
    let s = sprite(id, |_| {});
    let img = frame(&mut r, 0.0, |r, ctx| {
        assert_eq!(r.draw_sprites(ctx, &[s]), 0)
    });
    assert!(img.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sprite_orientation_follows_the_cpp_without_a_y_flip() {
    let Some(gpu) = gpu_or_skip("sprite_orientation_follows_the_cpp_without_a_y_flip") else {
        return;
    };
    let dir = temp_dir("orient");
    let path = save(&dir, "red.png", &solid(8, 8, RED));
    let mut r = renderer(&gpu);
    let id = load(&gpu, &mut r, &path, 0x00ff_00ff);
    // `y = var_y * 2 - 1` and the ortho(2, -2) projection flips it once
    // (support.cpp:179), the C++'s "flip y" line being commented out
    // (mfs:3618-3619): var_y 0 is the TOP of the screen, so 0.6 is low. (The
    // window-aspect step multiplies y by 160/64 after the translation,
    // mfs:3598-3606, so 0.75 would land off this wide screen; 0.6 -> 0.5.)
    let mut at = |x: f64, y: f64| {
        let s = sprite(id, |v| {
            (v.x, v.y, v.sx, v.sy, v.burn) = (x, y, 0.1, 0.1, 0.0)
        });
        let img = frame(&mut r, 0.0, |r, ctx| {
            assert_eq!(r.draw_sprites(ctx, &[s]), 1)
        });
        extent(&img, is_red)
    };
    let (rows, cols) = at(0.75, 0.6);
    assert!(!rows.is_empty());
    assert!(
        mean(&rows) > f64::from(H) * 0.6,
        "var_y 0.6 is low: {}",
        mean(&rows)
    );
    assert!(
        mean(&cols) > f64::from(W) * 0.6,
        "var_x 0.75 is right: {}",
        mean(&cols)
    );
    let (rows, cols) = at(0.25, 0.4);
    assert!(
        mean(&rows) < f64::from(H) * 0.4,
        "var_y 0.4 is high: {}",
        mean(&rows)
    );
    assert!(
        mean(&cols) < f64::from(W) * 0.4,
        "var_x 0.25 is left: {}",
        mean(&cols)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn repeat_wraps_with_the_inherited_stage_zero_sampler() {
    let Some(gpu) = gpu_or_skip("repeat_wraps_with_the_inherited_stage_zero_sampler") else {
        return;
    };
    let dir = temp_dir("repeat");
    let mut img = solid(2, 2, [0, 255, 0, 255]);
    for y in 0..2 {
        img.put_pixel(0, y, Rgba(RED));
    }
    let path = save(&dir, "rg.png", &img);
    let mut r = renderer(&gpu);
    let id = load(&gpu, &mut r, &path, 0x00ff_00ff);
    // repeatx 4 over the full-width quad: u spans [-1.5, 2.5], four
    // periods; the no-shader composite leaves WRAP on stage 0 (wrap = 1 >
    // the snap point).
    let s = sprite(id, |v| (v.repeatx, v.burn) = (4.0, 0.0));
    let out = frame(&mut r, 0.0, |r, ctx| {
        r.draw_sprites(ctx, &[s]);
    });
    let mut changes = 0;
    let mut last = None;
    for x in 0..W {
        let p = px(&out, x, H / 2);
        let (r, g) = (i32::from(p[0]), i32::from(p[1]));
        let now = if r > g + 60 {
            Some(true)
        } else if g > r + 60 {
            Some(false)
        } else {
            None
        };
        if now.is_some() && last.is_some() && now != last {
            changes += 1;
        }
        if now.is_some() {
            last = now;
        }
    }
    assert!(changes >= 6, "colour changes along the row: {changes}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn burn_draws_into_the_warp_target_and_survives_the_next_frame() {
    let Some(gpu) = gpu_or_skip("burn_draws_into_the_warp_target_and_survives_the_next_frame")
    else {
        return;
    };
    let dir = temp_dir("burn");
    let path = save(&dir, "red.png", &solid(8, 8, RED));
    let run = |burn: f64| {
        let mut r = renderer(&gpu);
        let id = load(&gpu, &mut r, &path, 0x00ff_00ff);
        let s = sprite(id, |v| (v.sx, v.sy, v.burn, v.done) = (0.2, 0.2, burn, 1.0));
        // Frame 1 draws the sprite; frame 2 draws none: the burn-in is in
        // the main texture (m_lpVS[1], the next frame's warp source,
        // mfs:3508-3521), decay 1 keeps it.
        let first = frame(&mut r, 0.0, |r, ctx| {
            r.draw_sprites(ctx, &[s]);
        });
        let second = frame(&mut r, 1.0, |_, _| {});
        (
            extent(&first, is_red).0.len(),
            extent(&second, is_red).0.len(),
        )
    };
    let (shown, kept) = run(1.0);
    assert!(
        shown > 100,
        "the sprite is on screen in its own frame: {shown}"
    );
    assert!(
        kept > 50,
        "burnt-in sprite remains on the next frame: {kept}"
    );
    let (shown, kept) = run(0.0);
    assert!(shown > 100, "{shown}");
    assert_eq!(kept, 0, "an unburnt sprite does not persist");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sprites_draw_over_the_title_and_under_the_text() {
    let Some(gpu) = gpu_or_skip("sprites_draw_over_the_title_and_under_the_text") else {
        return;
    };
    let dir = temp_dir("order");
    let blue = [0, 0, 200, 255];
    let path = save(&dir, "blue.png", &solid(8, 8, blue));
    let spec = FontSpec {
        face: "Arial".into(),
        size_px: 20,
        bold: false,
        italic: false,
        antialiased: true,
    };
    let now = 0.5 * SONG_TITLE_DURATION;
    let mut r = renderer(&gpu);
    assert!(r.start_song_title("Rag", 0.0));
    let id = load(&gpu, &mut r, &path, 0x00ff_00ff);

    // Baseline: the title alone lights pixels that are not the sprite's blue.
    let title_only = frame(&mut r, now, |_, _| {});
    assert!(
        title_only
            .chunks_exact(4)
            .any(|p| p[..3].iter().any(|&c| c > 0)),
        "the title is visible without the sprite"
    );

    // A decal sprite covering the whole screen (blendmode 1), then text,
    // queued BEFORE the sprites are recorded: the order is structural.
    let mut r = renderer(&gpu);
    assert!(r.start_song_title("Rag", 0.0));
    let id2 = load(&gpu, &mut r, &path, 0x00ff_00ff);
    assert_eq!(id, id2);
    let mut q = TextQueue::with_bundled_fonts(std::array::from_fn(|_| spec.clone()));
    let calc = q.draw(
        FontId::Simple,
        "Hi",
        Rect::new(60, 8, 150, 60),
        TextFlags::default(),
        [255; 4],
        None,
    );
    let s = sprite(id2, |v| {
        (v.sx, v.sy, v.blendmode, v.burn) = (2.0, 2.0, 1.0, 0.0)
    });
    let out = frame(&mut r, now, |r, ctx| {
        r.flush_text(ctx, &mut q);
        assert_eq!(r.draw_sprites(ctx, &[s]), 1);
    });
    let mut text_px = 0;
    for y in 0..H {
        for x in 0..W {
            let p = px(&out, x, y);
            let (xi, yi) = (x as i32, y as i32);
            let in_text =
                (calc.left..calc.right).contains(&xi) && (calc.top..calc.bottom).contains(&yi);
            if in_text {
                text_px += usize::from(p[0] > 100);
            } else {
                assert_eq!(p, blue, "({x},{y}): the sprite hides the title");
            }
        }
    }
    assert!(text_px > 10, "text drawn over the sprite: {text_px}");
    let _ = std::fs::remove_dir_all(&dir);
}

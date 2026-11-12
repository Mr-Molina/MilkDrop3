//! Task 5.5: preset shader pipelines, sampler resolution and the shader
//! cache, on a real (headless) device with presets from the corpus.
//!
//! Every GPU test asks [`gpu_or_skip`]: without an adapter it prints a note
//! and returns early, unless `MD_REQUIRE_GPU=1`, where it fails.

use md_hlsl::ShaderKind;
use md_render::{
    gpu_or_skip, read_texture_level, BlendMode, Gpu, PresetGpu, RenderError, RenderSettings,
    Renderer, ShaderProgram, Texture2d, Vertex,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;

const W: u32 = 64;
const H: u32 = 48;

fn preset(rel: &str) -> md_preset::Preset {
    let path = md_testkit::presets_dir().join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    md_preset::parse_milk(&text).unwrap_or_else(|e| panic!("{path:?}: {e:?}"))
}

fn settings() -> RenderSettings {
    RenderSettings::new(W, H)
}

fn renderer(gpu: &Gpu, settings: RenderSettings) -> Renderer {
    let mut r = Renderer::new(gpu.device(), gpu.queue(), settings, 7).expect("renderer");
    r.textures_mut()
        .scan_dir(&md_testkit::presets_dir().join("textures"));
    r
}

/// A fresh, empty temporary directory unique to `name`.
fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("md-render-shc-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Clear mip 0 of `t` to `rgba` (0..1).
fn clear(gpu: &Gpu, t: &Texture2d, rgba: [f64; 4]) {
    let view = t.level_view(0);
    let mut enc = gpu.device().create_command_encoder(&Default::default());
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: rgba[0],
                    g: rgba[1],
                    b: rgba[2],
                    a: rgba[3],
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    gpu.queue().submit([enc.finish()]);
}

/// A fullscreen quad (two triangles), white, uv = (0,0) top-left.
fn quad() -> Vec<Vertex> {
    let v = |x: f32, y: f32| Vertex {
        pos: [x, y, 0.0],
        color: [255; 4],
        uv: [
            (x + 1.0) * 0.5,
            (1.0 - y) * 0.5,
            (x + 1.0) * 0.5,
            (1.0 - y) * 0.5,
        ],
        rad_ang: [0.0, 0.0],
    };
    let (a, b, c, d) = (v(-1.0, 1.0), v(1.0, 1.0), v(-1.0, -1.0), v(1.0, -1.0));
    vec![a, b, c, c, b, d]
}

/// Draw `prog` over all of mip 0 of `target` with `main` (a ping-pong
/// index) bound as `sampler_main`, and read the target back.
fn draw(gpu: &Gpu, prog: &ShaderProgram, main: usize, target: &Texture2d) -> Vec<u8> {
    let bytes = md_render::vertices_to_bytes(&quad());
    let vbuf = gpu
        .device()
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
    let view = target.level_view(0);
    let mut enc = gpu.device().create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        assert!(prog.bind(&mut pass, main, BlendMode::Off));
        pass.set_vertex_buffer(0, vbuf.slice(..));
        pass.draw(0..6, 0..1);
    }
    gpu.queue().submit([enc.finish()]);
    read_texture_level(gpu, &target.tex, 0)
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    img[i..i + 4].try_into().unwrap()
}

fn near(a: u8, b: u8, tol: u8) -> bool {
    a.abs_diff(b) <= tol
}

fn programs(p: &PresetGpu) -> (&ShaderProgram, &ShaderProgram) {
    (
        p.warp.as_ref().expect("warp program"),
        p.comp.as_ref().expect("comp program"),
    )
}

#[test]
fn default_warp_preset_builds_pipelines() {
    let Some(gpu) = gpu_or_skip("default_warp_preset_builds_pipelines") else {
        return;
    };
    let mut r = renderer(&gpu, settings());
    let p = r
        .load_preset(&preset("ps2/upgraded_default.milk"))
        .expect("load");
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let (warp, comp) = programs(&p);
    assert!(!warp.is_fallback() && !comp.is_fallback());
    // Only sampler_main is bound, and texsize_* is not referenced.
    for prog in [warp, comp] {
        assert_eq!(prog.samplers.len(), 1);
        assert_eq!(prog.samplers[0].root, "main");
        assert!(prog.texsize_values.is_empty());
    }
    assert_eq!(p.highest_blur(), 0);
    // The warp shader decays: ret = main * 0.97 (vertex alpha 1).
    clear(&gpu, r.vs(1), [1.0, 0.5, 0.0, 1.0]);
    let img = draw(&gpu, warp, 1, r.vs(0));
    let c = px(&img, W / 2, H / 2);
    assert!(
        near(c[0], 247, 2) && near(c[1], 124, 2) && c[2] == 0,
        "{c:?}"
    );
    assert_eq!(c[3], 255);

    // A MilkDrop 1 preset (PSVERSION 0) has no preset shaders.
    let p = r
        .load_preset(&preset("classic/drift_tunnel.milk"))
        .expect("load");
    assert!(p.warp.is_none() && p.comp.is_none());
    // max_ps = None disables preset shaders altogether (m_nMaxPSVersion 0).
    let mut s = settings();
    s.max_ps = md_hlsl::PsVersion::None;
    let mut r = renderer(&gpu, s);
    let p = r
        .load_preset(&preset("ps2/upgraded_default.milk"))
        .expect("load");
    assert!(p.warp.is_none() && p.comp.is_none());
}

#[test]
fn shader_error_falls_back_and_records() {
    let Some(gpu) = gpu_or_skip("shader_error_falls_back_and_records") else {
        return;
    };
    let mut r = renderer(&gpu, settings());
    let mut pr = preset("ps2/upgraded_default.milk");
    pr.warp_shader = "shader_body\n{\n    ret = no_such_variable;\n}\n".to_string();
    let p = r.load_preset(&pr).expect("load");
    let (warp, comp) = programs(&p);
    assert!(warp.is_fallback() && !comp.is_fallback());
    match r.last_errors() {
        [RenderError::ShaderFallback { which, error }] => {
            assert_eq!(*which, ShaderKind::Warp);
            assert_eq!(error.line, 3, "{error}");
        }
        e => panic!("{e:?}"),
    }
    // The fallback is warp_ps.fx: ret = main - 0.004.
    clear(&gpu, r.vs(0), [0.5, 0.5, 0.5, 1.0]);
    let img = draw(&gpu, warp, 0, r.vs(1));
    let c = px(&img, 3, 3);
    assert!(near(c[0], 127, 1), "{c:?}");

    // ps_4_0 cannot be compiled (ledger Task 4.6): the comp falls back.
    let mut pr = preset("ps2/upgraded_default.milk");
    pr.comp_ps = 5;
    let p = r.load_preset(&pr).expect("load");
    assert!(p.comp.as_ref().unwrap().is_fallback());
    assert!(!p.warp.as_ref().unwrap().is_fallback());
    assert!(matches!(
        r.last_errors(),
        [RenderError::ShaderFallback {
            which: ShaderKind::Comp,
            ..
        }]
    ));
    // last_errors is per load.
    r.load_preset(&preset("ps2/upgraded_default.milk"))
        .expect("load");
    assert!(r.last_errors().is_empty());
}

#[test]
fn sampler_main_binds_other_ping_pong_texture() {
    let Some(gpu) = gpu_or_skip("sampler_main_binds_other_ping_pong_texture") else {
        return;
    };
    let mut r = renderer(&gpu, settings());
    let mut pr = preset("ps2/upgraded_default.milk");
    pr.warp_shader = "shader_body { ret = tex2D(sampler_main, uv).xyz; }".to_string();
    let p = r.load_preset(&pr).expect("load");
    let warp = p.warp.as_ref().unwrap();
    clear(&gpu, r.vs(0), [0.0, 0.0, 1.0, 1.0]);
    clear(&gpu, r.vs(1), [1.0, 0.0, 0.0, 1.0]);
    // Drawing into VS[0] samples VS[1] and vice versa.
    for target in 0..2 {
        let main = Renderer::other_vs(target);
        assert_eq!(main, 1 - target);
        let img = draw(&gpu, warp, main, r.vs(target));
        let want = if main == 1 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        };
        assert_eq!(px(&img, 10, 10), want, "target {target}");
        // Restore the overwritten texture for the next iteration.
        let c = if target == 0 {
            [0.0, 0.0, 1.0, 1.0]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };
        clear(&gpu, r.vs(target), c);
    }
}

#[test]
fn cache_hit_skips_translation() {
    let Some(gpu) = gpu_or_skip("cache_hit_skips_translation") else {
        return;
    };
    let dir = temp_dir("hit");
    let mut s = settings();
    s.shader_cache_dir = Some(dir.clone());
    let pr = preset("ps2/upgraded_default.milk");

    // Memory cache (no disk cache): the second load translates nothing.
    let mut r = renderer(&gpu, settings());
    r.load_preset(&pr).expect("load");
    assert_eq!(r.shader_cache().translations(), 2);
    r.load_preset(&pr).expect("load");
    assert_eq!(r.shader_cache().translations(), 2);
    assert_eq!(r.shader_cache().disk_hits(), 0);

    // The first load with a disk cache writes one entry per shader.
    let mut r = renderer(&gpu, s.clone());
    r.load_preset(&pr).expect("load");
    assert_eq!(r.shader_cache().translations(), 2);
    let files: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert_eq!(files.len(), 2, "one disk entry per shader");

    // Disk cache: a fresh renderer (empty memory cache) translates nothing.
    let mut r2 = renderer(&gpu, s.clone());
    let p = r2.load_preset(&pr).expect("load");
    assert_eq!(r2.shader_cache().translations(), 0);
    assert_eq!(r2.shader_cache().disk_hits(), 2);
    assert!(r2.last_errors().is_empty());
    let (warp, _) = programs(&p);
    assert_eq!(warp.samplers[0].root, "main");

    // A corrupt entry is ignored and regenerated.
    for f in std::fs::read_dir(&dir).unwrap() {
        std::fs::write(f.unwrap().path(), "garbage\n").unwrap();
    }
    let mut r3 = renderer(&gpu, s);
    r3.load_preset(&pr).expect("load");
    assert_eq!(r3.shader_cache().translations(), 2);
    assert!(r3.last_errors().is_empty());
    let mut r4 = renderer(&gpu, {
        let mut s = settings();
        s.shader_cache_dir = Some(dir.clone());
        s
    });
    r4.load_preset(&pr).expect("load");
    assert_eq!(r4.shader_cache().translations(), 0, "rewritten entries hit");

    // An entry written by another translator build (different fingerprint
    // in its header) is ignored and regenerated with the current one.
    let fp = format!("translator {}", md_hlsl::TRANSLATOR_FINGERPRINT);
    for f in std::fs::read_dir(&dir).unwrap() {
        let path = f.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(&fp), "header carries the fingerprint");
        let other = format!("translator {}", "0".repeat(64));
        std::fs::write(&path, text.replacen(&fp, &other, 1)).unwrap();
    }
    let mut s5 = settings();
    s5.shader_cache_dir = Some(dir.clone());
    let mut r5 = renderer(&gpu, s5);
    r5.load_preset(&pr).expect("load");
    assert_eq!(r5.shader_cache().translations(), 2);
    assert_eq!(r5.shader_cache().disk_hits(), 0);
    for f in std::fs::read_dir(&dir).unwrap() {
        let text = std::fs::read_to_string(f.unwrap().path()).unwrap();
        assert!(
            text.contains(&fp),
            "regenerated with the current fingerprint"
        );
    }
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
fn missing_user_texture_binds_fallback_and_logs_once() {
    use tracing_subscriber::layer::SubscriberExt;
    let Some(gpu) = gpu_or_skip("missing_user_texture_binds_fallback_and_logs_once") else {
        return;
    };
    let warnings = Warnings::default();
    let subscriber = tracing_subscriber::registry().with(warnings.clone());
    let pr = preset("edge/missing_texture.milk");
    let mut r = renderer(&gpu, settings());
    let p = tracing::subscriber::with_default(subscriber, || {
        let p = r.load_preset(&pr).expect("load");
        // Loading it again (and again) does not log again.
        r.load_preset(&pr).expect("load");
        r.load_preset(&pr).expect("load");
        p
    });
    // The preset still loads with its own shaders.
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let (_, comp) = programs(&p);
    assert!(!comp.is_fallback());
    // `sampler_no_such_texture_anywhere`: the `no_` prefix is peeled.
    assert_eq!(comp.samplers[1].root, "such_texture_anywhere");
    let logs = warnings.0.lock().unwrap().clone();
    let hits: Vec<_> = logs
        .iter()
        .filter(|m| m.contains("such_texture_anywhere"))
        .collect();
    assert_eq!(hits.len(), 1, "{logs:?}");
    // `texsize_no_such_texture_anywhere` is declared but never read, so it
    // is not bound.
    assert!(comp.texsize_values.is_empty());
    // Renders with the opaque black 1x1 fallback: main (grey 0.5) +
    // black * 0.2 = main (a white fallback would give 0.7 -> 179).
    clear(&gpu, r.vs(1), [0.5, 0.5, 0.5, 1.0]);
    let img = draw(&gpu, comp, 1, r.vs(0));
    let c = px(&img, 20, 20);
    assert!(
        near(c[0], 128, 1) && near(c[1], 128, 1) && near(c[2], 128, 1),
        "{c:?}"
    );
}

#[test]
fn user_noise_and_random_textures_resolve() {
    let Some(gpu) = gpu_or_skip("user_noise_and_random_textures_resolve") else {
        return;
    };
    let mut r = renderer(&gpu, settings());
    // User texture: texsize_mytex reports the 64x64 image.
    let p = r
        .load_preset(&preset("ps2/user_texture.milk"))
        .expect("load");
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let comp = p.comp.as_ref().unwrap();
    let roots: Vec<_> = comp.samplers.iter().map(|s| s.root.as_str()).collect();
    assert!(roots.contains(&"mytex"), "{roots:?}");
    let k = comp_texsize(comp, "mytex");
    assert_eq!(comp.texsize_values[k], [64.0, 64.0, 1.0 / 64.0, 1.0 / 64.0]);

    // 3D noise binds the volume (a 2D fallback would fail bind group
    // validation against the texture_3d binding).
    let p = r
        .load_preset(&preset("ps3/noise_volume.milk"))
        .expect("load");
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let warp = p.warp.as_ref().unwrap();
    assert!(warp.samplers.iter().any(|s| s.dims == 3));

    // randNN picks among the textures directory (only mytex here).
    let p = r.load_preset(&preset("ps2/rand_tiles.milk")).expect("load");
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    assert!(p.comp.is_some() && p.warp.is_some());

    // blur1..3 bind the placeholder blur textures and raise highest_blur.
    let p = r
        .load_preset(&preset("ps2x/blur3_bloom.milk"))
        .expect("load");
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    assert!(p.highest_blur() >= 1);

    // texsize_*: blur and noise sizes resolve; texsize_main never did in
    // the C++ (the VS is not a named texture) and is zero.
    let mut pr = preset("ps2/upgraded_default.milk");
    pr.comp_shader = "float4 texsize_blur2;\nfloat4 texsize_main;\nshader_body {\n ret = GetBlur2(uv) + texsize_blur2.x + texsize_noise_hq.x\n\
                      + texsize_main.x + texsize_noisevol_lq.y;\n}"
        .to_string();
    let p = r.load_preset(&pr).expect("load");
    assert!(r.last_errors().is_empty(), "{:?}", r.last_errors());
    let comp = p.comp.as_ref().unwrap();
    let (bw, bh) = md_render::blur_texture_sizes(W, H)[3];
    assert_eq!(r.blur_texture(3).w, bw);
    let (bw, bh) = (bw as f32, bh as f32);
    assert_eq!(
        comp.texsize_values[comp_texsize(comp, "blur2")],
        [bw, bh, 1.0 / bw, 1.0 / bh]
    );
    assert_eq!(
        comp.texsize_values[comp_texsize(comp, "noise_hq")][..2],
        [256.0, 256.0]
    );
    assert_eq!(
        comp.texsize_values[comp_texsize(comp, "noisevol_lq")][..2],
        [32.0, 32.0]
    );
    assert_eq!(comp.texsize_values[comp_texsize(comp, "main")], [0.0; 4]);
    assert_eq!(p.highest_blur(), 2);
}

fn comp_texsize(prog: &ShaderProgram, root: &str) -> usize {
    prog.texsize_names
        .iter()
        .position(|n| md_hlsl::texsize_root(n) == Some(root))
        .unwrap_or_else(|| panic!("no texsize_{root} in {:?}", prog.texsize_names))
}

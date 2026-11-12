//! Task 5.4: texture file lookup and decoding, random picks, and the
//! texture store on a real (headless) device.
//!
//! GPU tests ask [`gpu_or_skip`]: without an adapter they print a note and
//! return early, unless `MD_REQUIRE_GPU=1` (CI), where they fail. Test
//! images are written to a temporary directory with the `image` crate.

use image::{ImageFormat, Rgba, RgbaImage};
use md_render::noise::{dwords_to_rgba8, noise_2d};
use md_render::{
    gpu_or_skip, load_image, locate_texture, pick_random_texture, read_texture_level,
    scan_texture_names, ImageError, RandPicks, TextureError, TextureStore, TEXTURE_EXTS,
};
use rand_core::SeedableRng;
use std::path::{Path, PathBuf};

/// A fresh, empty temporary directory unique to `name`.
fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("md-render-tex-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn solid(w: u32, h: u32, c: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba(c))
}

/// Write `img` to `path` in `fmt` (alpha dropped for formats without it).
fn write(path: &Path, img: &RgbaImage, fmt: ImageFormat) {
    match fmt {
        ImageFormat::Jpeg => image::DynamicImage::ImageRgba8(img.clone())
            .to_rgb8()
            .save_with_format(path, fmt)
            .unwrap(),
        _ => img.save_with_format(path, fmt).unwrap(),
    }
}

/// An uncompressed A8R8G8B8 DDS of one colour.
fn write_dds(path: &Path, w: u32, h: u32, rgba: [u8; 4]) {
    let mut dds = ddsfile::Dds::new_d3d(ddsfile::NewD3dParams {
        height: h,
        width: w,
        depth: None,
        format: ddsfile::D3DFormat::A8R8G8B8,
        mipmap_levels: None,
        caps2: None,
    })
    .unwrap();
    let [r, g, b, a] = rgba;
    for px in dds.data.chunks_exact_mut(4) {
        px.copy_from_slice(&[b, g, r, a]);
    }
    dds.write(&mut std::fs::File::create(path).unwrap())
        .unwrap();
}

fn touch(path: &Path) {
    std::fs::write(path, b"x").unwrap();
}

fn rng(seed: u64) -> rand_pcg::Pcg32 {
    rand_pcg::Pcg32::seed_from_u64(seed)
}

#[test]
fn extension_search_order() {
    // F32: texture_exts in plugin.cpp:2847.
    assert_eq!(TEXTURE_EXTS, ["jpg", "dds", "png", "tga", "bmp", "dib"]);
    let tex = temp_dir("order-tex");
    let preset = temp_dir("order-preset");
    for ext in ["dib", "bmp", "tga", "png", "dds"] {
        touch(&tex.join(format!("x.{ext}")));
    }
    touch(&preset.join("x.jpg"));
    let dirs = [tex.as_path(), preset.as_path()];
    // Extension-major: presets/x.jpg beats textures/x.dds.
    assert_eq!(locate_texture(&dirs, "x"), Some(preset.join("x.jpg")));
    std::fs::remove_file(preset.join("x.jpg")).unwrap();
    for ext in ["dds", "png", "tga", "bmp", "dib"] {
        assert_eq!(
            locate_texture(&dirs, "x"),
            Some(tex.join(format!("x.{ext}")))
        );
        std::fs::remove_file(tex.join(format!("x.{ext}"))).unwrap();
    }
    assert_eq!(locate_texture(&dirs, "x"), None);
    // Within one extension the textures dir comes first.
    touch(&tex.join("y.png"));
    touch(&preset.join("y.png"));
    assert_eq!(locate_texture(&dirs, "y"), Some(tex.join("y.png")));
}

#[test]
fn texture_names_match_case_insensitively() {
    let tex = temp_dir("case");
    touch(&tex.join("Heart.JPG"));
    assert_eq!(
        locate_texture(&[&tex], "heart"),
        Some(tex.join("Heart.JPG"))
    );
    assert_eq!(
        locate_texture(&[&tex], "HEART"),
        Some(tex.join("Heart.JPG"))
    );
    assert_eq!(locate_texture(&[&tex], "hear"), None);
}

#[test]
fn load_image_decodes_every_format() {
    let dir = temp_dir("formats");
    let img = solid(3, 2, [200, 100, 50, 255]);
    for (name, fmt) in [
        ("a.png", ImageFormat::Png),
        ("a.jpg", ImageFormat::Jpeg),
        ("a.tga", ImageFormat::Tga),
        ("a.bmp", ImageFormat::Bmp),
        ("a.dib", ImageFormat::Bmp),
        // D3DX sniffs content: a PNG named .jpg still loads.
        ("png_named.jpg", ImageFormat::Png),
    ] {
        let p = dir.join(name);
        write(&p, &img, fmt);
        let got = load_image(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(got.dimensions(), (3, 2), "{name}");
        for px in got.pixels() {
            for (c, want) in px.0.iter().zip([200u8, 100, 50, 255]) {
                assert!(c.abs_diff(want) <= 4, "{name}: {:?}", px.0);
            }
        }
    }
    // TGA keeps alpha.
    let p = dir.join("alpha.tga");
    write(&p, &solid(2, 2, [1, 2, 3, 77]), ImageFormat::Tga);
    assert_eq!(load_image(&p).unwrap().get_pixel(1, 1).0, [1, 2, 3, 77]);
}

#[test]
fn load_image_reads_uncompressed_dds_and_rejects_bc() {
    let dir = temp_dir("dds");
    let p = dir.join("a.dds");
    write_dds(&p, 4, 2, [10, 20, 30, 40]);
    let img = load_image(&p).unwrap();
    assert_eq!(img.dimensions(), (4, 2));
    assert!(img.pixels().all(|px| px.0 == [10, 20, 30, 40]));

    let mut bc = ddsfile::Dds::new_d3d(ddsfile::NewD3dParams {
        height: 4,
        width: 4,
        depth: None,
        format: ddsfile::D3DFormat::DXT1,
        mipmap_levels: None,
        caps2: None,
    })
    .unwrap();
    bc.data.fill(0);
    let p = dir.join("bc.dds");
    bc.write(&mut std::fs::File::create(&p).unwrap()).unwrap();
    assert!(
        matches!(load_image(&p), Err(ImageError::Unsupported { .. })),
        "{:?}",
        load_image(&p)
    );
    assert!(matches!(
        load_image(&dir.join("none.png")),
        Err(ImageError::Io { .. })
    ));
    std::fs::write(dir.join("junk.png"), b"not an image").unwrap();
    assert!(load_image(&dir.join("junk.png")).is_err());
}

#[test]
fn random_names_and_prefix_picks() {
    let tex = temp_dir("rand");
    for f in [
        "b.png",
        "SmallTiled_1.jpg",
        "smalltiled_2.tga",
        "a.jpg",
        "a.png",
        "notes.txt",
    ] {
        touch(&tex.join(f));
    }
    std::fs::create_dir(tex.join("sub.jpg")).unwrap();
    let names = scan_texture_names(&tex);
    // Extension chopped, duplicates kept, case-insensitive order, no dirs.
    assert_eq!(names, ["a", "a", "b", "SmallTiled_1", "smalltiled_2"]);

    let mut r = rng(7);
    for _ in 0..50 {
        let p = pick_random_texture(&names, "SMALLtiled", &mut r).unwrap();
        assert!(p.to_lowercase().starts_with("smalltiled"), "{p}");
    }
    assert_eq!(pick_random_texture(&names, "zzz", &mut r), None);
    assert_eq!(pick_random_texture(&[], "", &mut r), None);
    // Same seed, same picks.
    let picks = |seed| {
        let mut r = rng(seed);
        (0..8)
            .map(|_| pick_random_texture(&names, "", &mut r).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(picks(1), picks(1));
}

#[test]
fn texsize_rand_follows_the_slot_pick() {
    let names: Vec<String> = ["smalltiled_x", "big"].map(String::from).to_vec();
    let mut r = rng(3);
    let mut picks = RandPicks::new();
    assert_eq!(
        picks.resolve_sampler(&names, "rand01_smalltiled", &mut r),
        Some("smalltiled_x".to_string())
    );
    assert_eq!(
        picks.resolve_sampler(&names, "rand02_nothing", &mut r),
        None
    );
    assert_eq!(
        picks.resolve_sampler(&names, "mytex", &mut r),
        Some("mytex".into())
    );
    // By slot, whatever the suffix on the texsize name.
    assert_eq!(picks.texsize_root("rand01"), Some("smalltiled_x"));
    assert_eq!(picks.texsize_root("rand01_other"), Some("smalltiled_x"));
    assert_eq!(picks.texsize_root("rand02"), None);
    assert_eq!(picks.texsize_root("rand03_q"), Some("rand03"));
    assert_eq!(picks.texsize_root("rand16"), Some("rand16"));
    assert_eq!(picks.texsize_root("mytex"), Some("mytex"));
}

#[test]
fn noise_lq_is_256_with_9_mips() {
    let Some(gpu) = gpu_or_skip("noise_lq_is_256_with_9_mips") else {
        return;
    };
    let store = TextureStore::new(&gpu, 42);
    let lq = store.noise2d("noise_lq").unwrap();
    assert_eq!((lq.w, lq.h, lq.mips), (256, 256, 9));
    assert_eq!(lq.format, wgpu::TextureFormat::Rgba8Unorm);
    for (name, size, mips) in [
        ("noise_lq_lite", 32, 6),
        ("noise_mq", 256, 9),
        ("noise_hq", 256, 9),
    ] {
        let t = store.noise2d(name).unwrap();
        assert_eq!((t.w, t.h, t.mips), (size, size, mips), "{name}");
    }
    for name in ["noisevol_lq", "noisevol_hq"] {
        let v = store.noise3d(name).unwrap();
        assert_eq!((v.w, v.h, v.d, v.mips), (32, 32, 32, 1), "{name}");
    }
    assert!(store.noise2d("noisevol_lq").is_none() && store.noise3d("noise_lq").is_none());
    assert_eq!(store.size_of("NOISE_HQ"), Some((256, 256)));
    assert_eq!(store.size_of("noisevol_hq"), Some((32, 32)));
    // Level 0 holds the CPU generation (the first texture drawn from the
    // seeded stream) in RGBA byte order.
    let want = dwords_to_rgba8(&noise_2d(&mut rng(42), 256, 1));
    assert_eq!(read_texture_level(&gpu, &lq.tex, 0), want);
}

#[test]
fn missing_file_returns_none() {
    let Some(gpu) = gpu_or_skip("missing_file_returns_none") else {
        return;
    };
    let tex = temp_dir("missing");
    std::fs::write(tex.join("broken.png"), b"not a png").unwrap();
    let mut store = TextureStore::new(&gpu, 1);
    store.scan_dir(&tex);
    assert!(store.get(&gpu, "nope").is_none());
    assert!(matches!(
        store.load(&gpu, "nope"),
        Err(TextureError::NotFound { .. })
    ));
    assert!(store.get(&gpu, "broken").is_none());
    assert!(matches!(
        store.load(&gpu, "broken"),
        Err(TextureError::Image(_))
    ));
    assert_eq!(store.size_of("nope"), None);
    assert_eq!(store.usage(), (0, 0));
}

#[test]
fn get_loads_with_mips_from_textures_then_preset_dir() {
    let Some(gpu) = gpu_or_skip("get_loads_with_mips_from_textures_then_preset_dir") else {
        return;
    };
    let tex = temp_dir("get-tex");
    let preset = temp_dir("get-preset");
    write(
        &tex.join("Pic.png"),
        &solid(8, 4, [255, 0, 0, 255]),
        ImageFormat::Png,
    );
    write_dds(&tex.join("pic.dds"), 8, 4, [0, 0, 255, 255]);
    write(
        &preset.join("pic.jpg"),
        &solid(16, 16, [0, 255, 0, 255]),
        ImageFormat::Jpeg,
    );
    write(
        &preset.join("only_here.tga"),
        &solid(2, 2, [9, 9, 9, 9]),
        ImageFormat::Tga,
    );
    let mut store = TextureStore::new(&gpu, 1);
    store.scan_dir(&tex);
    store.set_preset_dir(Some(&preset));

    // jpg wins over dds and png, even from the preset dir.
    let t = store.get(&gpu, "PIC").unwrap();
    assert_eq!((t.w, t.h, t.mips), (16, 16, 5));
    let px = read_texture_level(&gpu, &t.tex, 0);
    assert!(px[0] < 8 && px[1] > 247 && px[2] < 8, "{:?}", &px[..4]);
    assert_eq!(store.size_of("pic"), Some((16, 16)));
    assert_eq!(store.usage(), (1, 16 * 16 * 4 + 16384));
    // Cached: the same texture again.
    let id = store.load(&gpu, "pic").unwrap();
    assert_eq!(store.load(&gpu, "Pic").unwrap(), id);
    assert_eq!(store.usage().0, 1);

    let t = store.get(&gpu, "only_here").unwrap();
    assert_eq!(read_texture_level(&gpu, &t.tex, 0), vec![9; 16]);
    // Noise names resolve to the generated textures.
    assert_eq!(store.get(&gpu, "noise_mq").unwrap().w, 256);
}

#[test]
fn store_lru_eviction_respects_budget() {
    let Some(gpu) = gpu_or_skip("store_lru_eviction_respects_budget") else {
        return;
    };
    let tex = temp_dir("lru");
    for n in ["a", "b", "c"] {
        write(
            &tex.join(format!("{n}.png")),
            &solid(4, 4, [1, 2, 3, 4]),
            ImageFormat::Png,
        );
    }
    let mut store = TextureStore::new(&gpu, 1);
    store.scan_dir(&tex);
    store.set_budget(2, u64::MAX);
    let a = store.load(&gpu, "a").unwrap();
    store.preset_loaded();
    let b = store.load(&gpu, "b").unwrap();
    store.preset_loaded();
    store.preset_loaded();
    let c = store.load(&gpu, "c").unwrap();
    assert!(store.texture(a).is_none(), "LRU texture a evicted");
    assert!(store.texture(b).is_some() && store.texture(c).is_some());
    assert_eq!(store.usage().0, 2);
    assert_eq!(store.size_of("a"), None);
    // Reloading a evicts b (now the least recently used).
    store.preset_loaded();
    store.preset_loaded();
    store.load(&gpu, "c").unwrap();
    let a2 = store.load(&gpu, "a").unwrap();
    assert_ne!(a2, a);
    assert!(store.texture(b).is_none() && store.texture(a2).is_some());
}

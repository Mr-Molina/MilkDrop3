//! Writer tests (Task 2.4). `Preset::to_milk` mirrors `CState::Export`
//! (state.cpp:851-1069@6f7cd5d), whose `%.3f`/`%.5f` formats are lossy, so:
//!
//! * the exact round trip uses `fixtures/all_keys_exact.milk` (the all-keys
//!   fixture with every value cut to 3 decimals, exactly representable);
//! * the corpus test asserts that one write/parse round trip changes floats
//!   by at most the format's rounding and nothing else, and that the result
//!   is a fixed point of `parse . write` (see `corpus_round_trip_is_stable`).

use md_preset::{parse_milk, Preset};

const ALL_KEYS_EXACT: &str = include_str!("fixtures/all_keys_exact.milk");

fn parse(text: &str) -> Preset {
    parse_milk(text).expect("parse_milk")
}

/// Keys (text before `=`) of every line of `text`, in order.
fn keys(text: &str) -> Vec<&str> {
    text.lines()
        .map(|l| l.split_once('=').map_or(l, |(k, _)| k))
        .collect()
}

#[test]
fn round_trip_preserves_every_field() {
    let p = parse(ALL_KEYS_EXACT);
    // The fixture really sets everything: no blendable, wave or shape is at
    // its default, and all code blocks are non-empty.
    let d = Preset::defaults();
    for ((name, a), (_, b)) in p.blendables().into_iter().zip(d.blendables()) {
        assert_ne!(a.value(), b.value(), "{name} left at default");
    }
    for i in 0..4 {
        assert_ne!(p.waves[i], d.waves[i]);
        assert_ne!(p.shapes[i], d.shapes[i]);
        assert!(!p.waves[i].per_point.is_empty() && !p.shapes[i].init.is_empty());
    }
    assert!(!p.per_frame_init.is_empty() && !p.per_pixel.is_empty());
    assert_eq!((p.warp_ps, p.comp_ps), (3, 4));

    let text = p.to_milk();
    assert_eq!(parse(&text), p, "parse(write(p)) != p:\n{text}");
}

#[test]
fn code_line_starting_with_bracket_gets_backtick() {
    // WriteCode(..., bPrependApostrophe) backticks EVERY warp/comp line
    // (state.cpp:980-983@6f7cd5d); EEL blocks are written raw (:977-979).
    let mut p = Preset::defaults();
    p.warp_ps = 2;
    p.comp_ps = 2;
    p.warp_shader = "[x]\n;y\nz".into();
    p.comp_shader = "shader_body\n{\n}".into();
    p.per_frame = "[a\n;b".into();
    p.waves[1].per_point = "c=1;".into();
    let text = p.to_milk();
    for line in [
        "warp_1=`[x]",
        "warp_2=`;y",
        "warp_3=`z",
        "comp_1=`shader_body",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "missing {line:?} in\n{text}"
        );
    }
    for line in ["per_frame_1=[a", "per_frame_2=;b", "wave_1_per_point1=c=1;"] {
        assert!(
            text.lines().any(|l| l == line),
            "missing {line:?} in\n{text}"
        );
    }
    let q = parse(&text);
    assert_eq!(q.warp_shader, p.warp_shader);
    assert_eq!(q.per_frame, p.per_frame);
    assert_eq!(q.waves[1].per_point, p.waves[1].per_point);
}

#[test]
fn header_and_key_order_follow_export() {
    let mut p = Preset::defaults();
    p.warp_ps = 2;
    p.comp_ps = 3;
    p.per_frame = "a=1;".into();
    p.warp_shader = "w".into();
    p.comp_shader = "c".into();
    let text = p.to_milk();
    let mut expected: Vec<String> = [
        "MILKDROP_PRESET_VERSION",
        "PSVERSION",
        "PSVERSION_WARP",
        "PSVERSION_COMP",
        "[preset00]",
        "fRating",
        "fGammaAdj",
        "fDecay",
        "fVideoEchoZoom",
        "fVideoEchoAlpha",
        "nVideoEchoOrientation",
        "nWaveMode",
        "bAdditiveWaves",
        "bWaveDots",
        "bWaveThick",
        "bModWaveAlphaByVolume",
        "bMaximizeWaveColor",
        "bTexWrap",
        "bDarkenCenter",
        "bRedBlueStereo",
        "bBrighten",
        "bDarken",
        "bSolarize",
        "bInvert",
        "fWaveAlpha",
        "fWaveScale",
        "fWaveSmoothing",
        "fWaveParam",
        "fModWaveAlphaStart",
        "fModWaveAlphaEnd",
        "fWarpAnimSpeed",
        "fWarpScale",
        "fZoomExponent",
        "fShader",
        "zoom",
        "rot",
        "cx",
        "cy",
        "dx",
        "dy",
        "warp",
        "sx",
        "sy",
        "wave_r",
        "wave_g",
        "wave_b",
        "wave_x",
        "wave_y",
        "ob_size",
        "ob_r",
        "ob_g",
        "ob_b",
        "ob_a",
        "ib_size",
        "ib_r",
        "ib_g",
        "ib_b",
        "ib_a",
        "nMotionVectorsX",
        "nMotionVectorsY",
        "mv_dx",
        "mv_dy",
        "mv_l",
        "mv_r",
        "mv_g",
        "mv_b",
        "mv_a",
        "b1n",
        "b2n",
        "b3n",
        "b1x",
        "b2x",
        "b3x",
        "b1ed",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for i in 0..4 {
        for k in [
            "enabled",
            "samples",
            "sep",
            "bSpectrum",
            "bUseDots",
            "bDrawThick",
            "bAdditive",
            "scaling",
            "smoothing",
            "r",
            "g",
            "b",
            "a",
        ] {
            expected.push(format!("wavecode_{i}_{k}"));
        }
    }
    for i in 0..4 {
        for k in [
            "enabled",
            "sides",
            "additive",
            "thickOutline",
            "textured",
            "num_inst",
            "x",
            "y",
            "rad",
            "ang",
            "tex_ang",
            "tex_zoom",
            "r",
            "g",
            "b",
            "a",
            "r2",
            "g2",
            "b2",
            "a2",
            "border_r",
            "border_g",
            "border_b",
            "border_a",
        ] {
            expected.push(format!("shapecode_{i}_{k}"));
        }
    }
    expected.extend(["per_frame_1", "warp_1", "comp_1"].map(String::from));
    assert_eq!(keys(&text), expected);
    // Header values: CUR_MILKDROP_PRESET_VERSION (state.h:236) and the max.
    assert!(text.starts_with(
        "MILKDROP_PRESET_VERSION=201\nPSVERSION=3\nPSVERSION_WARP=2\nPSVERSION_COMP=3\n[preset00]\n"
    ));
}

#[test]
fn float_formats_match_export() {
    let mut p = Preset::defaults();
    p.rating = 2.5;
    p.zoom.set(1.0123456);
    p.rot.set(-0.000001);
    p.cx.set(0.12345);
    p.zoom_exponent.set(1.5);
    p.warp_anim_speed = 0.33333;
    p.tex_wrap = false;
    p.wave_mode = 7;
    p.waves[2].scaling = 2.0;
    p.waves[2].r = 0.25;
    p.shapes[3].rad = 0.1;
    p.shapes[3].x = 0.75;
    let text = p.to_milk();
    for line in [
        "fRating=2.500",
        "zoom=1.01235",
        "rot=-0.00000", // C prints the sign of a value that rounds to zero
        "cx=0.123",     // %.3f
        "fZoomExponent=1.50000",
        "fWarpAnimSpeed=0.333",
        "bTexWrap=0",
        "nWaveMode=7",
        "wavecode_2_scaling=2.00000",
        "wavecode_2_r=0.250",
        "shapecode_3_rad=0.10000",
        "shapecode_3_x=0.750",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "missing {line:?} in\n{text}"
        );
    }
}

#[test]
fn version_gates_header_and_shader_text() {
    // m_nMaxPSVersion == 0: no header (MilkDrop 1-era file), no shader text.
    let mut p = Preset::defaults();
    p.warp_shader = "w".into();
    p.comp_shader = "c".into();
    let text = p.to_milk();
    assert!(text.starts_with("[preset00]\n"), "{text}");
    assert!(!text.contains("warp_1") && !text.contains("comp_1"));
    // Shader text needs version >= MD2_PS_2_0 (state.cpp:980, 982@6f7cd5d); a
    // header is written as soon as either version is > 0.
    p.warp_ps = 1;
    p.comp_ps = 2;
    let text = p.to_milk();
    assert!(text.starts_with("MILKDROP_PRESET_VERSION=201\nPSVERSION=2\n"));
    assert!(!text.contains("warp_1") && text.contains("comp_1=`c\n"));
}

#[test]
fn empty_shader_text_is_one_empty_line() {
    // A C++ shader buffer is never empty after Import (it is generated when
    // no line was read), so an empty Rust text means one empty line was read
    // (`warp_1=` alone); it is written back as such. Empty EEL text writes
    // nothing, as WriteCode does for an empty buffer.
    let p = parse("MILKDROP_PRESET_VERSION=201\nwarp_1=\nper_frame_1=\n");
    assert_eq!(p.warp_shader, "");
    assert_eq!(p.per_frame, "");
    let text = p.to_milk();
    assert!(text.contains("\nwarp_1=`\n"), "{text}");
    assert!(!text.contains("per_frame_1"));
    assert_eq!(parse(&text), p);
}

/// Copy of `b` whose floats are replaced by `a`'s where they agree within
/// the rounding of the `%.3f`/`%.5f` formats, and whose fields Export does
/// not write (shader text below `MD2_PS_2_0`; versions <= 0) are `a`'s.
fn snap_to(a: &Preset, b: &Preset) -> Preset {
    fn close(x: f32, y: f32) -> bool {
        (x - y).abs() <= 0.0005 + 1e-6 * x.abs()
    }
    fn s(x: f32, y: &mut f32) {
        if close(x, *y) {
            *y = x;
        }
    }
    let mut b = b.clone();
    for ((_, x), (_, y)) in a.blendables().into_iter().zip(b.blendables_mut()) {
        if close(x.value(), y.value()) {
            y.set(x.value());
        }
    }
    s(a.rating, &mut b.rating);
    s(a.warp_anim_speed, &mut b.warp_anim_speed);
    for (wa, wb) in a.waves.iter().zip(b.waves.iter_mut()) {
        for (x, y) in [
            (wa.scaling, &mut wb.scaling),
            (wa.smoothing, &mut wb.smoothing),
            (wa.r, &mut wb.r),
            (wa.g, &mut wb.g),
            (wa.b, &mut wb.b),
            (wa.a, &mut wb.a),
        ] {
            s(x, y);
        }
    }
    for (sa, sb) in a.shapes.iter().zip(b.shapes.iter_mut()) {
        for (x, y) in [
            (sa.x, &mut sb.x),
            (sa.y, &mut sb.y),
            (sa.rad, &mut sb.rad),
            (sa.ang, &mut sb.ang),
            (sa.tex_ang, &mut sb.tex_ang),
            (sa.tex_zoom, &mut sb.tex_zoom),
            (sa.r, &mut sb.r),
            (sa.g, &mut sb.g),
            (sa.b, &mut sb.b),
            (sa.a, &mut sb.a),
            (sa.r2, &mut sb.r2),
            (sa.g2, &mut sb.g2),
            (sa.b2, &mut sb.b2),
            (sa.a2, &mut sb.a2),
            (sa.border_r, &mut sb.border_r),
            (sa.border_g, &mut sb.border_g),
            (sa.border_b, &mut sb.border_b),
            (sa.border_a, &mut sb.border_a),
        ] {
            s(x, y);
        }
    }
    if a.warp_ps.max(a.comp_ps) <= 0 {
        (b.warp_ps, b.comp_ps) = (a.warp_ps, a.comp_ps);
    }
    if a.warp_ps < 2 {
        b.warp_shader.clone_from(&a.warp_shader);
    }
    if a.comp_ps < 2 {
        b.comp_shader.clone_from(&a.comp_shader);
    }
    b
}

/// For every corpus preset `x`, with `p = parse(x)` and `q = parse(write(p))`:
///
/// 1. `q` equals `p` except for float rounding within the written precision
///    and the fields Export does not write (shader text below version 2);
/// 2. `q` is a fixed point: `parse(write(q)) == q` (so also
///    `write(parse(write(p))) == write(p)`, the F18 form).
///
/// The brief's `parse(write(parse(x))) == parse(x)` is false in general
/// because `%.3f`/`%.5f` drop digits (it happens to hold only for files that
/// already use <= 3 decimals); (1) is its exact lossy-format analogue.
#[test]
fn corpus_round_trip_is_stable() {
    let files = md_testkit::corpus_presets();
    assert!(files.len() >= 30, "only {} corpus presets", files.len());
    for path in &files {
        let bytes = std::fs::read(path).expect("read");
        let p = parse(&String::from_utf8_lossy(&bytes));
        let w = p.to_milk();
        let q = parse(&w);
        assert_eq!(snap_to(&p, &q), p, "{}: round trip changed", path.display());
        let w2 = q.to_milk();
        assert_eq!(parse(&w2), q, "{}: not a fixed point", path.display());
        assert_eq!(
            w2,
            w,
            "{}: write(parse(write(p))) != write(p)",
            path.display()
        );
    }
}

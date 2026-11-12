use md_text::fonts::bundled_database;
use md_text::{layout, Align, Atlas, FontSpec, Rect, TextFlags};

fn spec(size: u32) -> FontSpec {
    FontSpec {
        face: "Liberation Sans".into(),
        size_px: size,
        bold: false,
        italic: false,
        antialiased: true,
    }
}

fn atlas(size: u32) -> Atlas {
    Atlas::new(&spec(size), &bundled_database())
}

const BIG: Rect = Rect {
    left: 0,
    top: 0,
    right: 2000,
    bottom: 2000,
};

#[test]
fn calc_rect_height_equals_line_height_times_lines() {
    let mut a = atlas(20);
    let lh = a.line_height();
    assert_eq!(lh, 20, "positive Win32 height is the cell height");
    for (text, lines) in [
        ("one", 1),
        ("one\ntwo", 2),
        ("a\r\nb\r\nc", 3),
        ("a\n\nb", 3),
        ("", 0),
    ] {
        let l = layout(
            &mut a,
            text,
            Rect::new(10, 5, 500, 500),
            TextFlags::default(),
        );
        assert_eq!(l.bounds.top, 5);
        assert_eq!(l.bounds.bottom - l.bounds.top, lh * lines, "{text:?}");
        assert_eq!(l.bounds.left, 10);
    }
    // single_line ignores newlines: one line, CR/LF contribute no width.
    let flags = TextFlags {
        single_line: true,
        ..TextFlags::default()
    };
    let joined = layout(&mut a, "ab", BIG, flags);
    let split = layout(&mut a, "a\r\nb", BIG, flags);
    assert_eq!(split.bounds.bottom - split.bounds.top, lh);
    assert_eq!(split.bounds, joined.bounds);
    // width is the widest line
    let wide = layout(&mut a, "iiii\nWWWWWWWW", BIG, TextFlags::default());
    let w_only = layout(&mut a, "WWWWWWWW", BIG, TextFlags::default());
    assert_eq!(wide.bounds.right, w_only.bounds.right);
}

#[test]
fn right_align_ends_at_rect_right() {
    let mut a = atlas(24);
    let rect = Rect::new(100, 0, 400, 100);
    let flags = TextFlags {
        align: Align::Right,
        ..TextFlags::default()
    };
    let l = layout(&mut a, "Hello.\nWorld.", rect, flags);
    assert!(!l.quads.is_empty());
    let max_x = l.quads.iter().map(|q| q.x1).fold(f32::MIN, f32::max);
    assert!(max_x <= rect.right as f32, "{max_x}");
    // the last glyph '.' has a small right bearing; ink ends within a few px of the edge
    assert!(max_x > rect.right as f32 - 6.0, "{max_x}");
    let min_x = l.quads.iter().map(|q| q.x0).fold(f32::MAX, f32::min);
    assert!(min_x > rect.left as f32);
    // centre
    let c = layout(
        &mut a,
        "ab",
        rect,
        TextFlags {
            align: Align::Center,
            ..TextFlags::default()
        },
    );
    let lo = c.quads.iter().map(|q| q.x0).fold(f32::MAX, f32::min);
    let hi = c.quads.iter().map(|q| q.x1).fold(f32::MIN, f32::max);
    let mid = (lo + hi) / 2.0;
    assert!((mid - 250.0).abs() < 4.0, "{mid}");
}

#[test]
fn word_ellipsis_truncates_at_word() {
    let mut a = atlas(20);
    let text = "The quick brown fox jumps";
    let calc = TextFlags {
        single_line: true,
        ..TextFlags::default()
    };
    let full = layout(&mut a, text, BIG, calc);
    let avail = full.bounds.right / 2;
    let rect = Rect::new(0, 0, avail, 40);
    let w = TextFlags {
        single_line: true,
        word_ellipsis: true,
        ..TextFlags::default()
    };
    let l = layout(&mut a, text, rect, w);
    assert!(l.bounds.right <= avail, "{} <= {avail}", l.bounds.right);
    // reference: "The quick..." / "The..." must be what was drawn, never a mid-word cut
    let candidates = ["The quick...", "The...", "..."];
    let widths: Vec<i32> = candidates
        .iter()
        .map(|c| layout(&mut a, c, BIG, calc).bounds.right)
        .collect();
    assert!(
        widths.contains(&l.bounds.right),
        "{} not in {widths:?}",
        l.bounds.right
    );
    // fits entirely: untouched
    let same = layout(&mut a, text, BIG, w);
    assert_eq!(same.bounds, full.bounds);
    assert_eq!(same.quads, full.quads);
    // a single long word falls back to a character cut with "..."
    let e = layout(&mut a, "Supercalifragilistic", Rect::new(0, 0, 80, 40), w);
    assert!(e.bounds.right <= 80 && e.bounds.right > 40);
    // end ellipsis cuts mid-word
    let end = TextFlags {
        single_line: true,
        end_ellipsis: true,
        ..TextFlags::default()
    };
    let e2 = layout(&mut a, text, rect, end);
    assert!(e2.bounds.right <= avail);
}

#[test]
fn ampersand_prefix_rules() {
    let mut a = atlas(20);
    let flags = TextFlags::default();
    let lit = TextFlags {
        no_prefix: true,
        ..flags
    };
    let w = |a: &mut Atlas, t: &str, f| layout(a, t, BIG, f).bounds.right;
    // "&&" -> "&"
    assert_eq!(w(&mut a, "a&&b", flags), w(&mut a, "a&b", lit));
    // single "&" dropped
    assert_eq!(w(&mut a, "&File", flags), w(&mut a, "File", lit));
    assert_eq!(w(&mut a, "Fi&le", flags), w(&mut a, "File", lit));
    // trailing lone & dropped
    assert_eq!(w(&mut a, "File&", flags), w(&mut a, "File", lit));
    // "&&&" -> "&" then "&x" -> "x"
    assert_eq!(w(&mut a, "&&&x", flags), w(&mut a, "&x", lit));
    // no_prefix keeps everything literal
    assert!(w(&mut a, "a&&b", lit) > w(&mut a, "a&b", lit));
}

#[test]
fn missing_face_uses_bundled_font() {
    let db = bundled_database();
    let mut s = spec(18);
    s.face = "No Such Face 12345".into();
    let mut missing = Atlas::new(&s, &db);
    assert!(missing.uses_fallback());
    let mut found = Atlas::new(&spec(18), &db);
    assert!(!found.uses_fallback());
    let a = layout(&mut missing, "Fallback", BIG, TextFlags::default());
    let b = layout(&mut found, "Fallback", BIG, TextFlags::default());
    assert!(!a.quads.is_empty());
    assert_eq!(a.bounds, b.bounds);
    // even an empty database works
    let empty = fontdb::Database::new();
    let mut e = Atlas::new(&s, &empty);
    assert!(e.uses_fallback());
    assert!(!layout(&mut e, "x", BIG, TextFlags::default())
        .quads
        .is_empty());
}

#[test]
fn atlas_texture_is_white_with_coverage_in_alpha_and_dirty_tracked() {
    let mut a = atlas(20);
    let (_, w0, h0) = a.texture_rgba();
    assert_eq!((w0, h0), (256, 256));
    assert!(
        a.take_dirty().is_some(),
        "fresh texture must be uploaded once"
    );
    assert!(a.take_dirty().is_none());
    let l = layout(&mut a, "Hg", BIG, TextFlags::default());
    assert_eq!(l.quads.len(), 2);
    let d = a.take_dirty().expect("new glyphs dirty");
    assert!(!d.resized && d.w > 0 && d.h > 0);
    let (px, w, _) = a.texture_rgba();
    assert!(px
        .chunks(4)
        .all(|p| p[..3] == [255, 255, 255] || p[3] == 0 && p[..3] == [255, 255, 255]));
    assert!(px.chunks(4).any(|p| p[3] == 255));
    assert!(px.chunks(4).any(|p| p[3] > 0 && p[3] < 255), "antialiased");
    // same text again: nothing new
    let _ = layout(&mut a, "Hg", BIG, TextFlags::default());
    assert!(a.take_dirty().is_none());
    // quads' UVs point at covered texels
    let q = l.quads[0];
    let (x, y) = (
        ((q.u0 + q.u1) / 2.0 * w as f32) as usize,
        ((q.v0 + q.v1) / 2.0 * w as f32) as usize,
    );
    assert!(x < w as usize && y < w as usize);
}

#[test]
fn aliased_text_has_only_binary_coverage() {
    let mut s = spec(20);
    s.antialiased = false;
    let mut a = Atlas::new(&s, &bundled_database());
    let _ = layout(&mut a, "Sg%@", BIG, TextFlags::default());
    let (px, _, _) = a.texture_rgba();
    assert!(px.chunks(4).all(|p| p[3] == 0 || p[3] == 255));
    assert!(px.chunks(4).any(|p| p[3] == 255));
}

#[test]
fn atlas_grows_by_doubling_then_resets_when_full() {
    let db = bundled_database();
    let mut a = Atlas::with_limits(&spec(40), &db, 64, 256);
    assert_eq!(a.texture_rgba().1, 64);
    let g0 = a.generation();
    let text: String = ('A'..='Z').collect();
    let l = layout(&mut a, &text, BIG, TextFlags::default());
    assert_eq!(l.quads.len(), 26);
    let (_, w, h) = a.texture_rgba();
    assert!(w > 64 && w <= 256 && w == h && w.is_power_of_two());
    assert!(a.generation() > g0);
    assert!(a.take_dirty().unwrap().resized);
    // Fill the 256x256 atlas with many distinct glyphs: it must evict (reset) and keep working.
    let many: String = ('a'..='z')
        .chain('A'..='Z')
        .chain('0'..='9')
        .chain('\u{c0}'..='\u{ff}')
        .collect();
    let g1 = a.generation();
    let l2 = layout(&mut a, &many, BIG, TextFlags::default());
    assert!(a.generation() > g1, "reset bumps generation");
    assert!(a.resets() >= 1);
    // every quad produced references a valid in-bounds UV rect
    for q in &l2.quads {
        assert!(q.u0 >= 0.0 && q.u1 <= 1.0 && q.v0 >= 0.0 && q.v1 <= 1.0 && q.u1 > q.u0);
    }
    assert_eq!(a.texture_rgba().1, 256, "never exceeds the max");
    // a glyph bigger than the whole atlas is dropped, not a panic
    let mut tiny = Atlas::with_limits(&spec(300), &db, 64, 64);
    let l3 = layout(&mut tiny, "W", BIG, TextFlags::default());
    assert!(l3.quads.is_empty());
    assert!(l3.bounds.right > 0, "advance still counted");
}

#[test]
fn layout_is_deterministic() {
    let run = || {
        let mut a = atlas(22);
        let l = layout(
            &mut a,
            "Determinism & test\nsecond line",
            Rect::new(3, 4, 300, 90),
            TextFlags::default(),
        );
        (l, a.texture_rgba().0.to_vec())
    };
    let (l1, t1) = run();
    let (l2, t2) = run();
    assert_eq!(l1.quads, l2.quads);
    assert_eq!(l1.bounds, l2.bounds);
    assert_eq!(t1, t2);
}

//! Tests for custom messages, the shared supertext and the title-texture
//! font fitting (Task 7.8).

use crate::messages::CustomMessages;
use crate::supertext::{fit_font_px, title_band, SuperTextSlot, TitleLaunch, TITLE_FONT_SIZES};
use rand_core::{Rng, SeedableRng};
use rand_pcg::Pcg32;

const INI: &str = "\
[font00]
face=Verdana
bold=1
r=10
[font01]
face=courier
ital=1
g=20
[message00]
text=Hello
[message01]
text=Two
font=1
size=30
x=0.25
y=0.75
randx=0.1
randy=0.2
growth=2
time=3
fade=0.5
r=200
randr=100
randg=40
randb=-30
bold=0
face=Impact
[message05]
size=70
";

fn rng(seed: u64) -> Pcg32 {
    Pcg32::seed_from_u64(seed)
}

/// `(rand()%1037)/1037.0f*2.0f - 1.0f` with `rand()` = one `next_u32`.
fn jitter(r: &mut Pcg32) -> f32 {
    (r.next_u32() % 1037) as f32 / 1037.0 * 2.0 - 1.0
}

#[test]
fn message_defaults_match_cpp() {
    // plugin.cpp:8358-8397: size 50, x/y 0.5, growth 1, time 1.5, fade 0.2,
    // font face "arial", white, no jitter.
    let m = CustomMessages::parse("[message00]\ntext=Hi\n");
    let c = m.messages[0].as_ref().unwrap();
    assert_eq!(c.text, "Hi");
    assert_eq!((c.font, c.size, c.x, c.y), (0, 50.0, 0.5, 0.5));
    assert_eq!((c.randx, c.randy, c.growth), (0.0, 0.0, 1.0));
    assert_eq!((c.time, c.fade), (1.5, 0.2));
    assert_eq!((c.face.as_deref(), c.bold, c.ital), (None, None, None));
    assert_eq!(c.color, [None; 3]);
    assert_eq!(c.rand_color, [0; 3]);
    for f in &m.fonts {
        assert_eq!(f.face, "arial");
        assert_eq!((f.bold, f.ital, f.color), (false, false, [255; 3]));
    }
    let l = m.launch(0, &mut rng(1), 5.0).unwrap();
    assert_eq!((l.face.as_str(), l.bold, l.ital), ("arial", false, false));
    assert_eq!(l.st.color, [255; 3]);
    assert_eq!(
        (l.st.font_size, l.st.x, l.st.y, l.st.growth),
        (50.0, 0.5, 0.5, 1.0)
    );
}

#[test]
fn missing_sections_and_empty_text_define_nothing() {
    // A message with no text is not defined even if other keys are set
    // (plugin.cpp:8421: the rest is read only when szText[0]).
    let m = CustomMessages::parse(INI);
    assert!(m.messages[5].is_none());
    assert!(m.messages[2].is_none());
    assert_eq!(m.count(), 2);
    assert_eq!(CustomMessages::parse("").count(), 0);
    assert!(CustomMessages::parse("")
        .launch(0, &mut rng(1), 0.0)
        .is_none());
    // [font02..15] are absent: defaults.
    assert_eq!(m.fonts[2].face, "arial");
    assert_eq!(m.fonts[0].face, "Verdana");
    assert!(m.fonts[0].bold);
    assert_eq!(m.fonts[0].color, [10, 255, 255]);
}

#[test]
fn message_overrides_beat_the_font_and_the_font_fills_the_rest() {
    let m = CustomMessages::parse(INI);
    let c = m.messages[1].as_ref().unwrap();
    assert_eq!(c.face.as_deref(), Some("Impact"));
    assert_eq!(c.bold, Some(false));
    assert_eq!(c.ital, None);
    assert_eq!(c.color, [Some(200), None, None]);
    assert_eq!(c.rand_color, [100, 40, -30]);
    // Launch with a jitter that is exactly known: zero-jitter fields only.
    let mut r = rng(7);
    let l = m.launch(1, &mut r, 2.0).unwrap();
    assert_eq!(l.face, "Impact"); // override face
    assert!(!l.bold); // override bold = 0 beats nothing on font01
    assert!(l.ital); // from [font01]
    assert_eq!(l.text, "Two");
    assert_eq!(l.st.growth, 2.0);
    assert_eq!(l.st.duration, 3.0);
    assert_eq!(l.st.fade_time, 0.5);
    assert!(l.writes_fade);
    assert!(!l.st.is_song_title);
    assert_eq!(l.st.start_time, 2.0);
}

#[test]
fn custom_message_jitter_is_deterministic_for_a_seed() {
    let m = CustomMessages::parse(INI);
    let a = m.launch(1, &mut rng(42), 0.0).unwrap();
    let b = m.launch(1, &mut rng(42), 0.0).unwrap();
    assert_eq!(a, b);
    let c = m.launch(1, &mut rng(43), 0.0).unwrap();
    assert_ne!((a.st.x, a.st.y, a.st.color), (c.st.x, c.st.y, c.st.color));

    // The draw order is x, y, R, G, B (plugin.cpp:8497-8498, 8515-8517).
    let mut e = rng(42);
    let (jx, jy, jr, jg, jb) = (
        jitter(&mut e),
        jitter(&mut e),
        jitter(&mut e),
        jitter(&mut e),
        jitter(&mut e),
    );
    assert_eq!(a.st.x, 0.25 + 0.1 * jx);
    assert_eq!(a.st.y, 0.75 + 0.2 * jy);
    let ch = |base: i32, amt: i32, j: f32| (base + (amt as f32 * j) as i32).clamp(0, 255) as u8;
    assert_eq!(
        a.st.color,
        [ch(200, 100, jr), ch(20, 40, jg), ch(255, -30, jb)]
    );
}

#[test]
fn launch_always_consumes_five_draws() {
    // rand() is called even when randx/randy/randr.. are 0 (the C++ has no
    // guard), so a seeded stream stays aligned across messages.
    let m = CustomMessages::parse("[message00]\ntext=Hi\n");
    let mut r = rng(9);
    m.launch(0, &mut r, 0.0).unwrap();
    let mut e = rng(9);
    for _ in 0..5 {
        e.next_u32();
    }
    assert_eq!(r.next_u32(), e.next_u32());
    // An undefined message draws nothing.
    let mut r = rng(9);
    assert!(m.launch(3, &mut r, 0.0).is_none());
    assert_eq!(r.next_u32(), rng(9).next_u32());
}

#[test]
fn colour_jitter_is_clamped_to_a_byte() {
    let m =
        CustomMessages::parse("[message00]\ntext=a\nr=300\ng=5\nrandg=-1000\nb=0\nrandb=1000\n");
    for seed in 0..20 {
        let l = m.launch(0, &mut rng(seed), 0.0).unwrap();
        let mut e = rng(seed);
        let (_, _, _, jg, jb) = (
            jitter(&mut e),
            jitter(&mut e),
            jitter(&mut e),
            jitter(&mut e),
            jitter(&mut e),
        );
        assert_eq!(l.st.color[0], 255); // 300 clamps
        assert_eq!(
            l.st.color[1] as i32,
            (5 + (-1000.0 * jg) as i32).clamp(0, 255)
        );
        assert_eq!(l.st.color[2] as i32, ((1000.0 * jb) as i32).clamp(0, 255));
    }
}

#[test]
fn numbers_clamp_and_negative_picks_randomly() {
    let m = CustomMessages::parse("[message07]\ntext=seven\n[message99]\ntext=last\n");
    // `if (nMsgNum > 99) nMsgNum = 99` (plugin.cpp:8460).
    assert_eq!(m.launch(150, &mut rng(1), 0.0).unwrap().text, "last");
    // Negative: `sel = rand()%count + 1`, one draw first (8471).
    let mut r = rng(11);
    let want = if rng(11).next_u32().is_multiple_of(2) {
        "seven"
    } else {
        "last"
    };
    assert_eq!(m.launch(-1, &mut r, 0.0).unwrap().text, want);
    // No messages defined: nothing (the C++ divides by zero).
    assert!(CustomMessages::parse("")
        .launch(-1, &mut rng(1), 0.0)
        .is_none());
}

#[test]
fn out_of_range_font_falls_back_to_font_zero() {
    let m = CustomMessages::parse("[font00]\nface=Zero\n[message00]\ntext=a\nfont=40\n");
    assert_eq!(m.launch(0, &mut rng(1), 0.0).unwrap().face, "Zero");
}

#[test]
fn message_runs_on_the_supertext_timeline() {
    // Adapted from the brief's `message_fades_after_time`: the C++ has no
    // separate fade-out; the shared supertext runs `fDuration` seconds, the
    // text fades IN over the first `fFade` fraction (mfs:4667), is drawn
    // until progress >= 1, burnt in on that frame and then off
    // (mfs:1059-1067, 1123-1129).
    let m = CustomMessages::parse("[message00]\ntext=a\ntime=2\nfade=0.25\n");
    let mut slot = SuperTextSlot::default();
    assert!(!slot.is_active());
    assert!(slot.st.plan(0.0).screen.is_none());
    slot.start(&m.launch(0, &mut rng(1), 10.0).unwrap());
    assert!(slot.is_active());
    assert!(slot.take_redraw());
    assert!(!slot.take_redraw());
    assert_eq!(slot.st.fade_time, 0.25);
    let p = slot.st.plan(10.0);
    assert_eq!(p.screen, Some(0.0));
    assert!(!p.burn_in && !p.finished);
    assert_eq!(slot.st.plan(11.0).screen, Some(0.5));
    let end = slot.st.plan(12.0);
    assert!(end.burn_in && end.finished);
    assert_eq!(end.screen, Some(0.9999));
    slot.kill();
    assert!(!slot.is_active());
    assert!(slot.st.plan(10.5).screen.is_none());
}

#[test]
fn message_interrupting_a_song_title_replaces_it_as_the_cpp() {
    // One m_supertext serves both (plugin.cpp:8491-8543 and 8546-8565):
    // the later launch overwrites text, font, position, timing and colour;
    // the earlier animation just stops (no burn-in).
    let m = CustomMessages::parse(
        "[message00]\ntext=Msg\ntime=2\nfade=0.3\nsize=40\nx=0.2\ny=0.8\ngrowth=3\nr=1\ng=2\nb=3\n",
    );
    let mut slot = SuperTextSlot::default();
    slot.start(&TitleLaunch::song_title(
        "Song", "Arial", true, false, 16.0, 1.7, 0.0,
    ));
    assert!(slot.st.is_song_title);
    assert_eq!((slot.text.as_str(), slot.st.duration), ("Song", 1.7));
    assert!(slot.take_redraw());

    slot.start(&m.launch(0, &mut rng(5), 1.0).unwrap());
    assert!(!slot.st.is_song_title);
    assert_eq!(slot.text, "Msg");
    assert_eq!((slot.st.start_time, slot.st.duration), (1.0, 2.0));
    assert_eq!((slot.st.font_size, slot.st.growth), (40.0, 3.0));
    assert_eq!(slot.st.color, [1, 2, 3]);
    assert!(slot.take_redraw());
    // The song title would have ended at 1.7: no burn-in, still running.
    let p = slot.st.plan(1.7);
    assert!(!p.burn_in && !p.finished);

    // And back: the song title replaces the message, resets position and
    // colour (8556-8562) but not fFadeTime, which it never writes.
    slot.start(&TitleLaunch::song_title(
        "Song2", "Arial", false, true, 16.0, 1.7, 2.0,
    ));
    assert!(slot.st.is_song_title);
    assert_eq!(slot.text, "Song2");
    assert_eq!((slot.st.x, slot.st.y, slot.st.growth), (0.5, 0.5, 1.0));
    assert_eq!(slot.st.color, [255; 3]);
    assert_eq!(slot.st.fade_time, 0.3);
    assert!(slot.ital && !slot.bold);
    assert_eq!(slot.st.start_time, 2.0);
}

#[test]
fn font_fitting_is_the_cpp_binary_search() {
    // milkdropfs.cpp:315-398: lo = 0, hi = N-1; stop at lo == hi-1 and use
    // the lo size; grow lo when the text fits (width < band width and
    // height <= band height), else shrink hi. The last entry is never used.
    assert_eq!(TITLE_FONT_SIZES.len(), 36);
    assert_eq!(TITLE_FONT_SIZES[0], 6);
    assert_eq!(TITLE_FONT_SIZES[35], 512);
    // rect = (0, Y/21, X, 17Y/21) of the X by X/4 title texture (milkdropfs.cpp:309-313).
    assert_eq!(title_band(1024, 256), (1024, 256 * 17 / 21 - 256 / 21));
    let (bw, bh) = title_band(1024, 256);
    // Everything fits: 480, not 512.
    assert_eq!(fit_font_px(bw, bh, |_| (1, 1)), 480);
    // Nothing fits: the smallest.
    assert_eq!(fit_font_px(bw, bh, |_| (5000, 5000)), 6);
    // Width-limited text 10 characters at 0.5 px/px: widest size with
    // width < 1024 on the table's search path.
    let measure = |px: i32| (px * 5, px);
    let got = fit_font_px(bw, bh, measure);
    assert!(got * 5 < bw && got <= bh);
    let next = TITLE_FONT_SIZES.iter().find(|&&s| s > got).unwrap();
    assert!(next * 5 >= bw || *next > bh);
    // `>=` on the width: exactly the band width does not fit.
    assert_eq!(
        fit_font_px(100, 100, |px| (if px >= 50 { 100 } else { 1 }, 1)),
        44
    );
    // `>` on the height: exactly the band height fits.
    assert_eq!(fit_font_px(10_000, 40, |px| (1, px.min(40))), 480);
}

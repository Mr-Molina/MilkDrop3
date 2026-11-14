//! Tests of what the UI queues (Task 7.7): the overlays' strings, fonts and
//! corners, read off the `TextQueue` with no GPU.

use md_render::md_text::{FontSpec, Rect};
use md_render::{FontId, QueuedMessage, TextQueue, DARK_TRANSLUCENT_BOX, FONT_SLOTS};

use crate::commands::Command;
use crate::ui::{ScreenLayout, UiAction, UiMode};
use crate::ui_tests::{ctx, Harness};

pub(crate) const W: i32 = 1280;
pub(crate) const H: i32 = 720;
pub(crate) const FONT: i32 = 16;

pub(crate) fn queue() -> TextQueue {
    let spec = FontSpec {
        face: "Arial".into(),
        size_px: FONT as u32,
        bold: false,
        italic: false,
        antialiased: false,
    };
    TextQueue::with_bundled_fonts(std::array::from_fn::<_, FONT_SLOTS, _>(|_| spec.clone()))
}

pub(crate) fn layout() -> ScreenLayout {
    ScreenLayout::uniform(W, H, FONT)
}

/// A drawn text message.
#[derive(Clone, Debug)]
pub(crate) struct Drawn {
    pub font: FontId,
    pub text: String,
    pub rect: Rect,
    pub color: [u8; 4],
}

pub(crate) fn texts(q: &TextQueue) -> Vec<Drawn> {
    q.messages()
        .iter()
        .filter_map(|m| match m {
            QueuedMessage::Text {
                font,
                text,
                rect,
                color,
                ..
            } => Some(Drawn {
                font: *font,
                text: text.clone(),
                rect: *rect,
                color: *color,
            }),
            _ => None,
        })
        .collect()
}

pub(crate) fn find<'a>(t: &'a [Drawn], needle: &str) -> Vec<&'a Drawn> {
    t.iter().filter(|d| d.text.contains(needle)).collect()
}

pub(crate) fn dark_boxes(q: &TextQueue) -> Vec<Rect> {
    q.messages()
        .iter()
        .filter_map(|m| match m {
            QueuedMessage::DarkBox { rect, color } => {
                assert_eq!(*color, DARK_TRANSLUCENT_BOX);
                Some(*rect)
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn frame(h: &mut Harness) -> (TextQueue, Vec<UiAction>) {
    let mut q = queue();
    let ctx = ctx!(h);
    let acts = h.ui.draw(&mut q, &layout(), &ctx);
    (q, acts)
}

#[test]
fn regular_mode_with_nothing_on_draws_nothing() {
    let mut h = Harness::new();
    let (q, acts) = frame(&mut h);
    assert!(q.is_empty() && acts.is_empty());
}

#[test]
fn upper_right_overlays_stack_down_from_the_corner() {
    let mut h = Harness::new();
    h.ui.show_preset_name = true;
    h.ui.show_rating = true;
    h.ui.show_fps = true;
    h.ui.show_debug = true;
    h.preset.rating = 3.0;
    let mut ctx = ctx!(h);
    ctx.fps = 59.5;
    ctx.monitor = 1.25;
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    let t = texts(&q);

    // each is drawn twice: the shadow (black, +1,+1), then white
    let lines = [
        (" current preset ", FontId::Decorative),
        (" Rating: 3 ", FontId::Decorative),
        ("fps: 59.50 ", FontId::Decorative),
        (" pf monitor: 1.2500 ", FontId::Simple),
    ];
    for (i, (s, font)) in lines.iter().enumerate() {
        let hits = find(&t, s);
        assert_eq!(hits.len(), 2, "{s:?}");
        let (shadow, white) = (hits[0], hits[1]);
        assert_eq!(shadow.color, [0, 0, 0, 255]);
        assert_eq!(white.color, [255, 255, 255, 255]);
        assert_eq!(white.font, *font);
        // right edge at width - TEXT_MARGIN, top 10 + i lines
        assert_eq!(white.rect.right, W - 10, "{s:?}");
        assert_eq!(white.rect.top, 10 + i as i32 * FONT, "{s:?}");
        assert_eq!(
            (
                shadow.rect.left - white.rect.left,
                shadow.rect.top - white.rect.top
            ),
            (1, 1)
        );
    }
    assert!(dark_boxes(&q).is_empty(), "shadows, no boxes");
}

#[test]
fn preset_name_shows_the_lock_and_the_loading_preset() {
    let mut h = Harness::new();
    h.ui.show_preset_name = true;
    let mut ctx = ctx!(h);
    ctx.locked_by_user = true;
    ctx.loading_name = Some("next one");
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    assert_eq!(find(&texts(&q), "\u{1F512} next one ").len(), 2);
    // any UI mode locks too (plugin.cpp:3922)
    h.ui.mode = UiMode::Menu;
    let ctx = ctx!(h);
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    assert!(!find(&texts(&q), "\u{1F512} current preset ").is_empty());
}

#[test]
fn rating_shows_for_a_moment_after_it_changes_and_says_when_disabled() {
    let mut h = Harness::new();
    h.preset.rating = 4.0;
    let mut ctx = ctx!(h);
    ctx.show_rating_until = h.now + 2.0;
    ctx.rating_enabled = false;
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    assert_eq!(find(&texts(&q), " Rating: 4 [disabled] ").len(), 2);
    ctx.show_rating_until = h.now - 1.0;
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    assert!(q.is_empty());
}

#[test]
fn press_f1_slides_out_of_the_lower_right_for_three_seconds() {
    let mut h = Harness::new();
    h.ui.show_press_f1 = true;
    let at = |h: &mut Harness, t: f32| {
        let mut ctx = ctx!(h);
        ctx.time_since_start = t;
        let mut q = queue();
        h.ui.draw(&mut q, &layout(), &ctx);
        texts(&q)
    };
    let early = at(&mut h, 0.0);
    assert_eq!(early.len(), 1);
    let d = &early[0];
    assert_eq!(d.text, "Press F1 for Help ");
    assert_eq!(d.font, FontId::Decorative);
    assert_eq!(d.rect.right, W - 10, "dx = 160 * (t/3)^10 is 0 at t = 0");
    assert_eq!(d.rect.bottom, H - 10);
    // later it has moved right by dx
    let late = at(&mut h, 2.9);
    let dx = (160.0f32 * (2.9f32 / 3.0).powf(10.0)) as i32;
    assert!(dx > 100);
    assert_eq!(late[0].rect.right, W - 10 + dx);
    // gone after PRESS_F1_DUR
    assert!(at(&mut h, 3.0).is_empty());
    // and off unless asked for
    h.ui.show_press_f1 = false;
    assert!(at(&mut h, 0.0).is_empty());
}

#[test]
fn help_screen_is_help_txt_in_a_box_in_the_upper_left() {
    let mut h = Harness::new();
    h.send(Command::ToggleHelp);
    let (q, _) = frame(&mut h);
    let t = texts(&q);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].font, FontId::Help);
    assert_eq!(t[0].text, crate::ui::overlays::HELP_TEXT);
    assert!(t[0].text.contains("BEATDROP2077") && t[0].text.contains("toggle fullscreen"));
    // the text is inset by PLAYLIST_INNER_MARGIN inside the dark box, which
    // starts at the left edge and the top margin
    let boxes = dark_boxes(&q);
    assert_eq!(boxes.len(), 1);
    assert_eq!((boxes[0].left, boxes[0].top), (10, 10));
    assert_eq!((t[0].rect.left, t[0].rect.top), (14, 14));
    assert_eq!(boxes[0].right - boxes[0].left, t[0].rect.width() + 8);
    assert_eq!(boxes[0].bottom - boxes[0].top, t[0].rect.height() + 8);
}

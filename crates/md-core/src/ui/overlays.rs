//! The overlays: what `MyRenderUI` (plugin.cpp:4107-5120) and
//! `RenderBuiltInTextMsgs` (pluginshell.cpp:1857-1900) put on the screen,
//! queued on a [`TextQueue`] in the C++ order.
//!
//! Layout is the C++ one: four running `y` values, one per corner, start
//! `TEXT_MARGIN` from the edges (`DrawAndDisplay`, pluginshell.cpp:1348-1353)
//! and move as text is placed. Preset name, rating, fps and the debug line
//! stack down from the upper right, each with a one pixel shadow; the prompts,
//! menu and lists stack down from the upper left over a dark translucent box;
//! tooltips sit in the lower right; the "Press F1" message slides out of the
//! lower right; toasts go last, in the upper right, so they cover the rest.
//!
//! Text in the C++ goes through `CTextManager` (rects shrunk by a
//! `DT_CALCRECT`; [`TextQueue::draw`] does the same), except the help screen
//! and "Press F1", which call the D3DX font directly (pluginshell.cpp:1870,
//! 1897): those size their own rects here. The dark box under menus, lists
//! and tooltips is `DrawDarkTranslucentBox` ([`TextQueue::draw_box`] with
//! [`DARK_TRANSLUCENT_BOX`]).

use md_render::md_text::{Align, Rect, TextFlags};
use md_render::{FontId, TextQueue, DARK_TRANSLUCENT_BOX, FONT_SLOTS};

use super::mode::UiMode;
use super::strings;
use super::{Ui, UiAction, UiCtx};

/// `TEXT_MARGIN` (defines.h:203).
pub const TEXT_MARGIN: i32 = 10;
/// `PLAYLIST_INNER_MARGIN` (defines.h:204).
pub const PLAYLIST_INNER_MARGIN: i32 = 4;
/// `MENU_COLOR` (defines.h:211).
pub const MENU_COLOR: u32 = 0xFFCCCCCC;
/// `MENU_HILITE_COLOR`.
pub const MENU_HILITE_COLOR: u32 = 0xFFFF4400;
/// `DIR_COLOR`.
pub const DIR_COLOR: u32 = 0xFF88CCFF;
/// `PLAYLIST_COLOR_PLAYING_TRACK`.
pub const PLAYLIST_COLOR_PLAYING_TRACK: u32 = 0xFFCCFF00;
/// `PLAYLIST_COLOR_HILITE_TRACK`.
pub const PLAYLIST_COLOR_HILITE_TRACK: u32 = 0xFFFF5050;
/// `PLAYLIST_COLOR_BOTH`.
pub const PLAYLIST_COLOR_BOTH: u32 = 0xFFFFCC22;
/// `PLAYLIST_COLOR_NORMAL`.
pub const PLAYLIST_COLOR_NORMAL: u32 = 0xFFCCCCCC;
/// `PRESS_F1_DUR` (defines.h:220): seconds the message stays.
pub const PRESS_F1_DUR: f32 = 3.0;
/// `PRESS_F1_EXP` (defines.h:221).
pub const PRESS_F1_EXP: f32 = 10.0;

/// The help screen, `text1.bin` ported verbatim (F45).
pub const HELP_TEXT: &str = include_str!("../../../../assets/help.txt");

const WHITE: u32 = 0xFFFFFFFF;
const BLACK: u32 = 0xFF000000;

/// An `0xAARRGGBB` colour (the C++ `DWORD`) as the queue's `[r, g, b, a]`.
pub const fn argb(c: u32) -> [u8; 4] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]
}

/// The window and the font metrics the overlays are placed with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenLayout {
    /// Client width in pixels (`m_client_width`).
    pub width: i32,
    /// Client height.
    pub height: i32,
    /// `GetFontHeight(font)`: each slot's line height in pixels, by
    /// [`FontId`] order (simple, decorative, help, playlist, song title,
    /// tooltip).
    pub font_heights: [i32; FONT_SLOTS],
}

impl ScreenLayout {
    /// A layout where every font is `font_height` pixels.
    pub fn uniform(width: i32, height: i32, font_height: i32) -> ScreenLayout {
        ScreenLayout {
            width,
            height,
            font_heights: [font_height; FONT_SLOTS],
        }
    }

    /// `GetFontHeight`.
    pub fn font_height(&self, f: FontId) -> i32 {
        self.font_heights[f as usize]
    }
}

/// The four running corner positions and the text column limits
/// (`m_upper_left_corner_y` ... `m_right_edge`, pluginshell.cpp:1348-1353).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corners {
    /// Next free `y` going down from the upper left.
    pub ul: i32,
    /// Going down from the upper right.
    pub ur: i32,
    /// Going up from the lower left.
    pub ll: i32,
    /// Going up from the lower right.
    pub lr: i32,
    /// `m_left_edge`.
    pub xl: i32,
    /// `m_right_edge`.
    pub xr: i32,
}

impl Corners {
    /// The corners of a fresh frame.
    pub fn new(lay: &ScreenLayout) -> Corners {
        Corners {
            ul: TEXT_MARGIN,
            ur: TEXT_MARGIN,
            ll: lay.height - TEXT_MARGIN,
            lr: lay.height - TEXT_MARGIN,
            xl: TEXT_MARGIN,
            xr: lay.width - TEXT_MARGIN,
        }
    }
}

/// `MTO_*` (plugin.cpp:4027-4030).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    /// `MTO_UPPER_RIGHT`.
    UpperRight,
    /// `MTO_UPPER_LEFT`.
    UpperLeft,
    /// `MTO_LOWER_RIGHT`.
    LowerRight,
    /// `MTO_LOWER_LEFT`.
    LowerLeft,
}

/// A line's flags: `DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX` and kin.
pub fn single_line() -> TextFlags {
    TextFlags {
        single_line: true,
        end_ellipsis: true,
        no_prefix: true,
        ..TextFlags::default()
    }
}

fn corner_rect(c: &Corners, corner: Corner, w: i32, h: i32) -> Rect {
    match corner {
        Corner::UpperLeft => Rect::new(c.xl, c.ul, c.xl + w, c.ul + h),
        Corner::UpperRight => Rect::new(c.xr - w, c.ur, c.xr, c.ur + h),
        Corner::LowerLeft => Rect::new(c.xl, c.ll - h, c.xl + w, c.ll),
        Corner::LowerRight => Rect::new(c.xr - w, c.lr - h, c.xr, c.lr),
    }
}

fn advance(c: &mut Corners, corner: Corner, h: i32) {
    match corner {
        Corner::UpperLeft => c.ul += h,
        Corner::UpperRight => c.ur += h,
        Corner::LowerLeft => c.ll -= h,
        Corner::LowerRight => c.lr -= h,
    }
}

/// `MyTextOut_BGCOLOR` (plugin.cpp:4037-4050): one line of `text` in a corner,
/// white, over an opaque box of `box_color` when `dark_box`.
pub fn text_out(
    q: &mut TextQueue,
    lay: &ScreenLayout,
    c: &mut Corners,
    font: FontId,
    text: &str,
    corner: Corner,
    box_color: Option<u32>,
) {
    let h = lay.font_height(font);
    let flags = TextFlags {
        single_line: corner != Corner::UpperRight,
        word_ellipsis: true,
        no_prefix: true,
        align: if corner == Corner::UpperRight {
            Align::Right
        } else {
            Align::Left
        },
        ..TextFlags::default()
    };
    let w = q
        .calc_rect(font, text, Rect::new(0, 0, c.xr - c.xl, 2048), flags)
        .width();
    let r = corner_rect(c, corner, w, h);
    q.draw(font, text, r, flags, argb(WHITE), box_color.map(argb));
    advance(c, corner, h);
}

/// `MyTextOut_Shadow` (plugin.cpp:4054-4073): the text in black one pixel down
/// and right, then in white.
pub fn text_out_shadow(
    q: &mut TextQueue,
    lay: &ScreenLayout,
    c: &mut Corners,
    font: FontId,
    text: &str,
    corner: Corner,
) {
    let h = lay.font_height(font);
    let flags = TextFlags {
        single_line: true,
        word_ellipsis: true,
        no_prefix: true,
        ..TextFlags::default()
    };
    let w = q
        .calc_rect(font, text, Rect::new(0, 0, c.xr - c.xl, 2048), flags)
        .width();
    let mut r = corner_rect(c, corner, w, h);
    r.top += 1;
    r.left += 1;
    q.draw(font, text, r, flags, argb(BLACK), None);
    r.top -= 1;
    r.left -= 1;
    q.draw(font, text, r, flags, argb(WHITE), None);
    advance(c, corner, h);
}

/// `DrawTooltip` (plugin.cpp:4006-4025): `text` in the lower-right corner
/// above `yb`, over a dark translucent box.
pub fn draw_tooltip(q: &mut TextQueue, text: &str, xr: i32, yb: i32) {
    let r = q.calc_rect(
        FontId::Tooltip,
        text,
        Rect::new(0, 0, xr - TEXT_MARGIN * 2, 2048),
        TextFlags::default(),
    );
    let bottom = yb - TEXT_MARGIN;
    let right = xr - TEXT_MARGIN;
    let r2 = Rect::new(right - r.width(), bottom - r.height(), right, bottom);
    let r3 = Rect::new(r2.left - 4, r2.top - 2, r2.right + 2, r2.bottom + 2);
    q.draw_box(r3, DARK_TRANSLUCENT_BOX);
    q.draw(
        FontId::Tooltip,
        text,
        r2,
        TextFlags::default(),
        argb(WHITE),
        None,
    );
}

impl Ui {
    /// Queue every overlay for one frame, in the C++ order, and do what the
    /// C++ does while drawing: expire toasts, page the load list, clamp and
    /// apply the mash-up. Returns the actions that causes.
    pub fn draw(&mut self, q: &mut TextQueue, lay: &ScreenLayout, ctx: &UiCtx) -> Vec<UiAction> {
        let mut out = Vec::new();
        let mut c = Corners::new(lay);
        self.prune_toasts(ctx.now);
        self.draw_built_in(q, lay, ctx, &mut c);
        self.draw_ui(q, lay, ctx, &mut c, &mut out);
        out
    }

    /// `RenderBuiltInTextMsgs` (pluginshell.cpp:1857-1900): help, then
    /// "Press F1".
    fn draw_built_in(&self, q: &mut TextQueue, lay: &ScreenLayout, ctx: &UiCtx, c: &mut Corners) {
        if self.show_help {
            let flags = TextFlags::default();
            let calc = q.calc_rect(
                FontId::Help,
                HELP_TEXT,
                Rect::new(0, 0, lay.width, lay.height),
                flags,
            );
            let (w, h) = (calc.width(), calc.height());
            let boxr = Rect::new(
                c.xl,
                c.ul,
                c.xl + w + PLAYLIST_INNER_MARGIN * 2,
                c.ul + h + PLAYLIST_INNER_MARGIN * 2,
            );
            q.draw_box(boxr, DARK_TRANSLUCENT_BOX);
            let t = Rect::new(
                boxr.left + PLAYLIST_INNER_MARGIN,
                boxr.top + PLAYLIST_INNER_MARGIN,
                boxr.right - PLAYLIST_INNER_MARGIN,
                boxr.bottom - PLAYLIST_INNER_MARGIN,
            );
            q.draw(FontId::Help, HELP_TEXT, t, flags, argb(WHITE), None);
            c.ul += h + PLAYLIST_INNER_MARGIN * 3;
        }
        if self.show_press_f1 && ctx.time_since_start < PRESS_F1_DUR {
            let dx = (160.0 * (ctx.time_since_start / PRESS_F1_DUR).powf(PRESS_F1_EXP)) as i32;
            let h = lay.font_height(FontId::Decorative);
            let flags = TextFlags {
                align: Align::Right,
                ..TextFlags::default()
            };
            let w = q
                .calc_rect(
                    FontId::Decorative,
                    strings::PRESS_F1_MSG,
                    Rect::new(c.xl, c.lr - h, c.xr + dx, c.lr),
                    flags,
                )
                .width();
            let r = Rect::new(c.xr + dx - w, c.lr - h, c.xr + dx, c.lr);
            q.draw(
                FontId::Decorative,
                strings::PRESS_F1_MSG,
                r,
                flags,
                argb(WHITE),
                None,
            );
            c.lr -= h;
        }
    }

    /// `MyRenderUI`.
    fn draw_ui(
        &mut self,
        q: &mut TextQueue,
        lay: &ScreenLayout,
        ctx: &UiCtx,
        c: &mut Corners,
        out: &mut Vec<UiAction>,
    ) {
        // 1. the upper right: preset name, rating, fps, debug
        if self.show_preset_name {
            let lock = if ctx.locked_by_user || self.mode != UiMode::Regular {
                "\u{1F512}"
            } else {
                ""
            };
            let name = ctx.loading_name.unwrap_or(ctx.preset_name);
            let s = format!("{lock} {name} ");
            text_out_shadow(q, lay, c, FontId::Decorative, &s, Corner::UpperRight);
        }
        if self.show_rating || ctx.now < ctx.show_rating_until {
            let mut s = format!(" {}: {} ", strings::RATING, ctx.preset.rating as i32);
            if !ctx.rating_enabled {
                s.push_str(strings::DISABLED);
            }
            text_out_shadow(q, lay, c, FontId::Decorative, &s, Corner::UpperRight);
        }
        if self.show_fps {
            let s = format!("{}: {:4.2} ", strings::FPS, ctx.fps);
            text_out_shadow(q, lay, c, FontId::Decorative, &s, Corner::UpperRight);
        }
        if self.show_debug {
            let s = format!(" {}: {:6.4} ", strings::PF_MONITOR, ctx.monitor);
            text_out_shadow(q, lay, c, FontId::Simple, &s, Corner::UpperRight);
        }
        // 2. the lower right: the string editor's tooltip
        if self.edit.active && ctx.show_menu_tooltips && !self.edit.tooltip.is_empty() {
            draw_tooltip(q, &self.edit.tooltip, c.xr, c.lr);
        }
        // (3. the lower left is the song title and time; no player here.)
        // 4. the upper left, by mode
        if self.edit.active {
            self.draw_edit(q, lay, ctx, c);
        } else {
            match self.mode {
                UiMode::Menu => self.draw_menu_screen(q, lay, ctx, c),
                UiMode::UpgradePixelShader => self.draw_upgrade(q, lay, ctx, c),
                UiMode::LoadDel => self.draw_delete_prompt(q, lay, ctx, c),
                UiMode::SaveOverwrite => self.draw_overwrite_prompt(q, ctx, c),
                UiMode::Mashup => self.draw_mashup(q, lay, ctx, c, out),
                UiMode::Load => self.draw_load(q, lay, ctx, c),
                _ => {}
            }
        }
        // 5. the toasts, last, so they are on top
        if !ctx.warnings_disabled {
            for t in &self.toasts {
                if ctx.now >= t.birth && ctx.now < t.expire {
                    let s = format!("{} ", t.msg);
                    let age = (ctx.now - t.birth) / (t.expire - t.birth);
                    let cr = (200.0 - 199.0 * age.powi(4)) as u32;
                    let color = if t.bold { BLACK | (cr << 16) } else { BLACK };
                    text_out(
                        q,
                        lay,
                        c,
                        FontId::Simple,
                        &s,
                        Corner::UpperRight,
                        Some(color),
                    );
                }
            }
        }
    }
}

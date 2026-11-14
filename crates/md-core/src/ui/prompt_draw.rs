//! The screens of the string editor and the `[y/N]` questions
//! (plugin.cpp:4253-4590, 4611-4704).

use md_render::md_text::{Rect, TextFlags};
use md_render::{FontId, TextQueue, DARK_TRANSLUCENT_BOX};

use super::menu::ROOT;
use super::mode::UiMode;
use super::overlays::*;
use super::strings;
use super::{Ui, UiCtx};

const BLACK: u32 = 0xFF000000;

/// `rect.top += DrawText(.., DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
/// color, bBox = true)`: a menu-coloured line over an opaque box.
fn boxed_line(q: &mut TextQueue, rect: &mut Rect, text: &str, color: u32, box_color: u32) {
    let t = q.draw(
        FontId::Simple,
        text,
        *rect,
        single_line(),
        argb(color),
        Some(argb(box_color)),
    );
    rect.top += t.height();
}

impl Ui {
    fn prompt_rect(c: &Corners) -> Rect {
        Rect::new(c.xl, c.ul, c.xr, c.ll)
    }

    /// The upgrade question (plugin.cpp:4611-4682).
    pub(super) fn draw_upgrade(
        &self,
        q: &mut TextQueue,
        _lay: &ScreenLayout,
        ctx: &UiCtx,
        c: &mut Corners,
    ) {
        let mut r = Self::prompt_rect(c);
        let (warp, comp) = (ctx.preset.warp_ps, ctx.preset.comp_ps);
        let (min, max) = (warp.min(comp), warp.max(comp));
        let mut line = |q: &mut TextQueue, s: &str| boxed_line(q, &mut r, s, MENU_COLOR, BLACK);
        if warp >= ctx.max_ps && comp >= ctx.max_ps {
            line(q, &strings::highest_ps_version(ctx.max_ps));
            line(q, strings::PRESS_ESC_TO_RETURN);
        } else if min != max {
            line(q, strings::MIXED_SHADERS);
            match min {
                0 => line(q, strings::UPGRADE_ALL_PS2),
                2 => line(q, strings::UPGRADE_ALL_PS2X),
                3 => line(q, strings::UPGRADE_ALL_PS3),
                _ => {}
            }
        } else {
            let (info, ask) = match min {
                0 => (strings::NO_PIXEL_SHADERS, strings::UPGRADE_PS2),
                2 => (strings::USES_PS2, strings::UPGRADE_PS2X),
                3 => (strings::USES_PS2X, strings::UPGRADE_PS3),
                _ => (strings::USES_PS3, strings::UPGRADE_PS4),
            };
            line(q, info);
            line(q, ask);
            line(q, strings::OLD_GPU_WARNING);
        }
        c.ul = r.top;
    }

    /// The delete question (plugin.cpp:4683-4691).
    pub(super) fn draw_delete_prompt(
        &self,
        q: &mut TextQueue,
        _lay: &ScreenLayout,
        ctx: &UiCtx,
        c: &mut Corners,
    ) {
        let mut r = Self::prompt_rect(c);
        boxed_line(q, &mut r, strings::ARE_YOU_SURE_DELETE, MENU_COLOR, BLACK);
        let name = ctx
            .library
            .entries()
            .get(self.load.pos)
            .map_or("", |e| e.name.as_str());
        boxed_line(
            q,
            &mut r,
            &strings::preset_to_delete(name),
            MENU_COLOR,
            BLACK,
        );
        c.ul = r.top;
    }

    /// The overwrite question (plugin.cpp:4692-4704).
    pub(super) fn draw_overwrite_prompt(&self, q: &mut TextQueue, ctx: &UiCtx, c: &mut Corners) {
        let mut r = Self::prompt_rect(c);
        boxed_line(q, &mut r, strings::FILE_ALREADY_EXISTS, MENU_COLOR, BLACK);
        boxed_line(
            q,
            &mut r,
            &strings::file_in_question(&self.edit.as_string()),
            MENU_COLOR,
            BLACK,
        );
        if ctx.warp_shader_lock {
            boxed_line(
                q,
                &mut r,
                strings::WARP_SHADER_WAS_LOCKED,
                0xFFFFFFFF,
                0xFFCC0000,
            );
        }
        if ctx.comp_shader_lock {
            boxed_line(
                q,
                &mut r,
                strings::COMP_SHADER_WAS_LOCKED,
                0xFFFFFFFF,
                0xFFCC0000,
            );
        }
        c.ul = r.top;
    }

    /// The string editor's screen (plugin.cpp:4253-4541).
    pub(super) fn draw_edit(
        &mut self,
        q: &mut TextQueue,
        lay: &ScreenLayout,
        ctx: &UiCtx,
        c: &mut Corners,
    ) {
        let fh = lay.font_height(FontId::Simple);
        let prompt = self.edit.prompt.clone();
        text_out(
            q,
            lay,
            c,
            FontId::Simple,
            &prompt,
            Corner::UpperLeft,
            Some(BLACK),
        );

        let label = self
            .menu
            .current()
            .cur_item()
            .map(|i| i.def.label)
            .unwrap_or("");
        let on_root = self.edit.display_as_code && self.menu.cur == ROOT;
        let is_warp = on_root && label == "[ edit warp shader ]";
        let is_comp = on_root && label == "[ edit composite shader ]";
        if is_warp || is_comp {
            let hint = if self.show_shader_help {
                strings::PRESS_F9_HIDE_QREF
            } else {
                strings::PRESS_F9_SHOW_QREF
            };
            text_out(
                q,
                lay,
                c,
                FontId::Simple,
                hint,
                Corner::UpperLeft,
                Some(BLACK),
            );
            c.ul += fh * 2 / 3;
            if self.show_shader_help {
                let flags = TextFlags {
                    single_line: true,
                    word_ellipsis: true,
                    no_prefix: true,
                    ..Default::default()
                };
                let r = q.calc_rect(
                    FontId::Simple,
                    strings::QREF_WIDEST,
                    Rect::new(0, 0, 2048, 2048),
                    flags,
                );
                let darkbox =
                    Rect::new(c.xl, c.ul - 2, c.xl + r.width(), c.ul + r.height() * 13 + 2);
                q.draw_box(darkbox, DARK_TRANSLUCENT_BOX);
                let special = if is_warp {
                    &strings::QREF_WARP
                } else {
                    &strings::QREF_COMP
                };
                for s in strings::QREF_COMMON.iter().chain(special.iter()) {
                    text_out(q, lay, c, FontId::Simple, s, Corner::UpperLeft, None);
                }
                c.ul += fh * 2 / 3;
            }
        } else if self.mode == UiMode::SaveAs && (ctx.warp_shader_lock || ctx.comp_shader_lock) {
            let msg = match (ctx.warp_shader_lock, ctx.comp_shader_lock) {
                (true, true) => strings::WARP_AND_COMP_LOCKED,
                (true, false) => strings::WARP_LOCKED,
                _ => strings::COMP_LOCKED,
            };
            text_out(
                q,
                lay,
                c,
                FontId::Simple,
                msg,
                Corner::UpperLeft,
                Some(BLACK),
            );
            c.ul += fh * 2 / 3;
        } else {
            c.ul += fh * 2 / 3;
        }

        // the text, with the cursor or the selection marked
        let blink = !self.edit.has_visible_selection() && (ctx.now * 270.0) as i32 % 100 > 50;
        let display = self.edit.display_text(blink);
        let mut rect = Rect::new(
            c.xl + PLAYLIST_INNER_MARGIN,
            c.ul + PLAYLIST_INNER_MARGIN,
            c.xr - PLAYLIST_INNER_MARGIN,
            c.ll - PLAYLIST_INNER_MARGIN,
        );
        let plain = TextFlags::default();
        if self.edit.display_as_code {
            self.draw_code(q, &display, &mut rect, c);
        } else {
            let mut boxr = Rect::new(rect.left, rect.top, rect.right, 4096);
            boxr = q.calc_rect(FontId::Simple, &format!("    {display}X"), boxr, plain);
            boxr.top -= PLAYLIST_INNER_MARGIN;
            boxr.left -= PLAYLIST_INNER_MARGIN;
            boxr.right += PLAYLIST_INNER_MARGIN;
            boxr.bottom += PLAYLIST_INNER_MARGIN;
            q.draw_box(boxr, DARK_TRANSLUCENT_BOX);
            c.ul += boxr.height() + PLAYLIST_INNER_MARGIN * 3;
            q.draw(
                FontId::Simple,
                &format!("    {display} "),
                rect,
                plain,
                argb(MENU_COLOR),
                None,
            );
        }
    }

    /// The multi-line code view: find the page the cursor line is on and the
    /// box around it, then draw its lines (plugin.cpp:4470-4541).
    fn draw_code(&mut self, q: &mut TextQueue, display: &str, rect: &mut Rect, c: &mut Corners) {
        let chars: Vec<char> = display.chars().collect();
        let cursor = self.edit.cursor;
        let plain = TextFlags::default();
        let (mut top_of_page, mut ypixels, mut page) = (0usize, 0i32, 1usize);
        let mut exit_on_next_page = false;
        let mut cut_at: Option<usize> = None;
        let mut boxr = Rect::new(rect.left, rect.top, rect.left, rect.top);
        let mut pos = 0;
        while pos < chars.len() {
            let start = pos;
            while pos < chars.len() && chars[pos] != '\n' {
                pos += 1;
            }
            let line: String = chars[start..pos].iter().collect();
            let r2 = Rect::new(rect.left, rect.top, rect.right, 4096);
            let t = q.calc_rect(FontId::Simple, &format!("   {line}X"), r2, plain);
            let h = t.height();
            ypixels += h;
            if start > cursor {
                exit_on_next_page = true;
            }
            if ypixels > rect.bottom - rect.top {
                if exit_on_next_page {
                    cut_at = Some(start);
                    break;
                }
                ypixels = h;
                top_of_page = start;
                page += 1;
                boxr = Rect::new(rect.left, rect.top, rect.left, rect.top);
            }
            boxr.bottom += h;
            boxr.right = boxr.right.max(boxr.left + t.width());
            if pos >= chars.len() {
                break;
            }
            pos += 1;
        }
        boxr.top -= PLAYLIST_INNER_MARGIN;
        boxr.left -= PLAYLIST_INNER_MARGIN;
        boxr.right += PLAYLIST_INNER_MARGIN;
        boxr.bottom += PLAYLIST_INNER_MARGIN;
        q.draw_box(boxr, DARK_TRANSLUCENT_BOX);
        c.ul += boxr.height() + PLAYLIST_INNER_MARGIN * 3;
        // plugin.cpp:4539 overwrites the tooltip with the page number, so
        // after the first frame that is what the lower right shows.
        self.edit.tooltip = strings::page(page);

        let end = cut_at.unwrap_or(chars.len());
        let (mut start, mut pos) = (top_of_page, top_of_page);
        while pos < end {
            while pos < end && chars[pos] != '\n' {
                pos += 1;
            }
            let line: String = chars[start..pos].iter().collect();
            let color = if cursor >= start && cursor <= pos {
                MENU_HILITE_COLOR
            } else {
                MENU_COLOR
            };
            let t = q.draw(
                FontId::Simple,
                &format!("   {line} "),
                *rect,
                plain,
                argb(color),
                None,
            );
            rect.top += t.height();
            if rect.top > rect.bottom {
                break;
            }
            if pos < end {
                pos += 1;
            }
            start = pos;
        }
    }
}

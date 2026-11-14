//! `CMilkMenu::DrawMenu` (menu.cpp:258-394) and the menu part of
//! `MyRenderUI` (plugin.cpp:4592-4610).

use md_render::md_text::Rect;
use md_render::{FontId, TextQueue, DARK_TRANSLUCENT_BOX};

use super::menu_tree::ItemKind;
use super::overlays::*;
use super::strings;
use super::{Ui, UiCtx};

/// `MyMenuTextOut` (menu.cpp:241-256): one line at `rect.top`, moving it
/// down; when measuring, grow `calc` instead of drawing.
fn menu_text_out(
    q: &mut TextQueue,
    text: &str,
    color: u32,
    rect: &mut Rect,
    calc: &mut Option<&mut Rect>,
) {
    match calc {
        Some(cr) => {
            let t = q.calc_rect(FontId::Simple, text, *rect, single_line_ellipsis());
            rect.top += t.height();
            cr.bottom += t.height();
            cr.right = cr.right.max(cr.left + t.width());
        }
        None => {
            let t = q.draw(
                FontId::Simple,
                text,
                *rect,
                single_line_ellipsis(),
                argb(color),
                None,
            );
            rect.top += t.height();
        }
    }
}

fn single_line_ellipsis() -> md_render::md_text::TextFlags {
    md_render::md_text::TextFlags {
        single_line: true,
        end_ellipsis: true,
        ..Default::default()
    }
}

impl Ui {
    /// `CMilkMenu::DrawMenu` of the current menu. `rect` is where it may draw
    /// (its `top` moves down as lines go); `xr`/`yb` are where tooltips go;
    /// with `calc` it measures the menu (excluding tooltips) into it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_menu(
        &self,
        q: &mut TextQueue,
        lay: &ScreenLayout,
        ctx: &UiCtx,
        mut rect: Rect,
        xr: i32,
        yb: i32,
        mut calc: Option<&mut Rect>,
    ) {
        let menu = self.menu.current();
        if let Some(cr) = calc.as_deref_mut() {
            *cr = Rect::new(rect.left, rect.top, rect.left, rect.top);
        }
        let measuring = calc.is_some();
        let tooltip = |q: &mut TextQueue, s: &str| {
            if ctx.show_menu_tooltips && !measuring {
                draw_tooltip(q, s, xr, yb);
            }
        };
        if !menu.editing {
            let fh = lay.font_height(FontId::Simple).max(1);
            let lines = (rect.bottom - rect.top - PLAYLIST_INNER_MARGIN * 2) / fh - 1;
            if lines < 1 {
                return;
            }
            let lines = lines as usize;
            let start = (menu.cur_sel / lines) * lines;
            let mut drawn = 0;
            for (i, &k) in menu.children.iter().enumerate() {
                if i >= start && i < start + lines {
                    let child = &self.menu.menus[k];
                    if child.enabled {
                        let color = if i == menu.cur_sel {
                            MENU_HILITE_COLOR
                        } else {
                            MENU_COLOR
                        };
                        menu_text_out(q, &child.name, color, &mut rect, &mut calc);
                        drawn += 1;
                    }
                    if i == menu.cur_sel {
                        tooltip(q, strings::MENU_NAV_TOOLTIP);
                    }
                }
            }
            let mut i = menu.children.len();
            for item in &menu.items {
                if drawn >= start + lines {
                    break;
                }
                if !item.enabled {
                    i += 1;
                    continue;
                }
                if i >= start {
                    let mut text = item.def.label.to_owned();
                    if item.def.kind == ItemKind::Bool {
                        let on = super::menu::field_of(item.def.target, menu.slot)
                            .is_some_and(|f| f.get(ctx.preset).as_bool());
                        text = format!(
                            "{} [{}]",
                            item.def.label,
                            if on { strings::ON } else { strings::OFF }
                        );
                    }
                    if i == menu.cur_sel {
                        menu_text_out(q, &text, MENU_HILITE_COLOR, &mut rect, &mut calc);
                        tooltip(q, item.def.tooltip);
                    } else {
                        menu_text_out(q, &text, MENU_COLOR, &mut rect, &mut calc);
                    }
                    drawn += 1;
                }
                i += 1;
            }
        } else if let Some(item) = menu.cur_item() {
            menu_text_out(
                q,
                strings::USE_UP_DOWN_ARROW_KEYS,
                MENU_COLOR,
                &mut rect,
                &mut calc,
            );
            menu_text_out(
                q,
                &strings::current_value_of(item.def.label),
                MENU_COLOR,
                &mut rect,
                &mut calc,
            );
            let v = super::menu::field_of(item.def.target, menu.slot)
                .map(|f| f.get(ctx.preset))
                .unwrap_or(super::FieldValue::Float(0.0));
            let buf = match item.def.kind {
                ItemKind::Int => format!(" {} ", v.as_i32()),
                ItemKind::Float
                | ItemKind::LogFloat
                | ItemKind::Blendable
                | ItemKind::LogBlendable => {
                    format!(" {:5.3} ", v.as_f32())
                }
                _ => " ? ".to_owned(),
            };
            menu_text_out(q, &buf, MENU_HILITE_COLOR, &mut rect, &mut calc);
            tooltip(q, item.def.tooltip);
        }
    }

    /// The menu in the upper left (plugin.cpp:4592-4610): measured, boxed, then
    /// drawn inside the box.
    pub(super) fn draw_menu_screen(
        &self,
        q: &mut TextQueue,
        lay: &ScreenLayout,
        ctx: &UiCtx,
        c: &mut Corners,
    ) {
        let mut r = Rect::new(c.xl, c.ul, c.xr, c.ll);
        let mut darkbox = Rect::default();
        self.draw_menu(q, lay, ctx, r, c.xr, c.lr, Some(&mut darkbox));
        c.ul += darkbox.height() + PLAYLIST_INNER_MARGIN * 3;
        darkbox.right += PLAYLIST_INNER_MARGIN * 2;
        darkbox.bottom += PLAYLIST_INNER_MARGIN * 2;
        q.draw_box(darkbox, DARK_TRANSLUCENT_BOX);
        r.top += PLAYLIST_INNER_MARGIN;
        r.left += PLAYLIST_INNER_MARGIN;
        r.right += PLAYLIST_INNER_MARGIN;
        r.bottom += PLAYLIST_INNER_MARGIN;
        self.draw_menu(q, lay, ctx, r, c.xr, c.lr, None);
    }
}

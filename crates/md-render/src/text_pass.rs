//! The text pass's queue (`CTextManager`, code/vis_milk2/textmgr.cpp): the
//! CPU half. [`TextQueue`] owns one glyph atlas per font slot, answers
//! `DT_CALCRECT` queries and records the messages a frame draws;
//! [`Renderer::flush_text`](crate::Renderer::flush_text)
//! ([`crate::text_draw`]) rasterizes them over the composite.
//!
//! What is mirrored from the C++ (`mfs` = milkdropfs.cpp):
//!
//! * `DrawText`/`DrawTextW` first clear `DT_WORD_ELLIPSIS | DT_END_ELLIPSIS |
//!   DT_NOPREFIX` (textmgr.cpp:122 and :174; the comment at :604 says why:
//!   in DX9 they cause no text to render), so the effective behaviour is no
//!   ellipsis and `&` prefix processing ON. [`mask_flags`] does the same,
//!   for [`TextQueue::calc_rect`] as well as [`TextQueue::draw`] (the clear
//!   is the first statement, before the `DT_CALCRECT` branch at :127).
//! * A `DT_CALCRECT` call returns the calculated rectangle and queues
//!   nothing (:127-128). A drawing call queues the message with its rect
//!   shrunk by a `DT_CALCRECT` on the same text (:148, "shrink rects on new
//!   frame's text strings"): the rect that is drawn into, and that a dark
//!   box covers, is the *tight* rect (`left`/`top` as given, `right = left +`
//!   widest line, `bottom = top +` lines * line height), so the alignment
//!   flags only matter between the lines of a multi-line text. [`draw`]
//!   returns that rect (the C++ returns its height, `bottom - top`).
//! * With a box, the box message is added after the text and swapped before
//!   it (:153-161), so it draws first and is the text's background; the box
//!   rect is the tight rect (:156). Messages draw in queue order
//!   (:601-621): a box queued later covers earlier text.
//! * Not mirrored: the message pool limits (`MAX_MSGS` 4096 messages,
//!   `MAX_MSG_CHARS`), and the dirty-rect text-surface cache (`fix_slow_text`,
//!   off by default, pluginshell.cpp:846) — the queue is drawn afresh every
//!   frame, which is what the C++ does with no text surface (:245, :268).
//!
//! Fonts are the six slots of F43 ([`FontId`]); md-render takes
//! [`md_text::FontSpec`] values (md-core converts them from md-platform's
//! `FontConfig`). The `eFontIndex` slots of the C++ are `SIMPLE_FONT`,
//! `DECORATIVE_FONT`, `HELPSCREEN_FONT`, `PLAYLIST_FONT` (shell_defines.h
//! :50-53) plus `EXTRA_1` = `TOOLTIP_FONT` and `EXTRA_2` = `SONGTITLE_FONT`
//! (defines.h:165, :173; shell_defines.h:54-55). [`FontId`] follows F43's
//! order, which lists the song title before the tooltip; the discriminants
//! are internal.
//!
//! [`draw`]: TextQueue::draw

use crate::texture::Texture2d;
use md_text::{fonts, layout, Atlas, FontSpec, Rect, TextFlags};

/// The colour `DrawDarkTranslucentBox` uses, `0xD0000000` as `[r, g, b, a]`
/// (pluginshell.cpp:1847).
pub const DARK_TRANSLUCENT_BOX: [u8; 4] = [0, 0, 0, 0xD0];

/// Number of font slots (F43).
pub const FONT_SLOTS: usize = 6;

/// The six font slots (F43).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontId {
    Simple = 0,
    Decorative = 1,
    Help = 2,
    Playlist = 3,
    SongTitle = 4,
    Tooltip = 5,
}

/// `flags &= ~(DT_WORD_ELLIPSIS | DT_END_ELLIPSIS | DT_NOPREFIX)`
/// (textmgr.cpp:122, :174).
pub fn mask_flags(flags: TextFlags) -> TextFlags {
    TextFlags {
        word_ellipsis: false,
        end_ellipsis: false,
        no_prefix: false,
        ..flags
    }
}

/// One queued message (`td_string`): a dark box (`pfont == NULL`) or text.
#[derive(Clone, Debug)]
pub(crate) enum Item {
    Box {
        rect: Rect,
        color: [u8; 4],
    },
    /// A [`TextQueue::draw_box`] box: alpha-blended, under all text.
    DarkBox {
        rect: Rect,
        color: [u8; 4],
    },
    Text {
        font: FontId,
        text: String,
        /// The tight rect (see the module docs).
        rect: Rect,
        flags: TextFlags,
        color: [u8; 4],
    },
}

/// The frame's text messages and the glyph atlases (one per [`FontId`]).
/// Use one queue per [`Renderer`](crate::Renderer): the atlas textures it
/// creates belong to that renderer's device.
pub struct TextQueue {
    atlases: Vec<Atlas>,
    items: Vec<Item>,
    /// The GPU copy of each atlas (created and updated by `flush_text`).
    pub(crate) textures: [Option<(Texture2d, u64)>; FONT_SLOTS],
}

impl TextQueue {
    /// A queue with the six fonts, matched against the system fonts with the
    /// bundled font as the fallback.
    pub fn new(specs: [FontSpec; FONT_SLOTS]) -> TextQueue {
        let db = fonts::system_database();
        TextQueue::from_atlases(specs.iter().map(|s| Atlas::new(s, &db)).collect())
    }

    /// As [`new`](Self::new) with only the bundled font (deterministic; for
    /// tests and headless runs).
    pub fn with_bundled_fonts(specs: [FontSpec; FONT_SLOTS]) -> TextQueue {
        let db = fonts::bundled_database();
        TextQueue::from_atlases(specs.iter().map(|s| Atlas::new(s, &db)).collect())
    }

    fn from_atlases(atlases: Vec<Atlas>) -> TextQueue {
        TextQueue {
            atlases,
            items: Vec::new(),
            textures: Default::default(),
        }
    }

    /// `DrawText` with `DT_CALCRECT`: the rectangle `text` would occupy
    /// (see [`md_text::layout`]). Queues nothing.
    pub fn calc_rect(&mut self, font: FontId, text: &str, rect: Rect, flags: TextFlags) -> Rect {
        let atlas = &mut self.atlases[font as usize];
        layout(atlas, text, rect, mask_flags(flags)).bounds
    }

    /// `CTextManager::DrawText`: queue `text` for this frame's pass and
    /// return its calc rect (the rect the text and its box occupy).
    /// `color` is `[r, g, b, a]`; with `dark_box` the text gets a box of that
    /// colour behind it (`bBox`, `boxColor`).
    pub fn draw(
        &mut self,
        font: FontId,
        text: &str,
        rect: Rect,
        flags: TextFlags,
        color: [u8; 4],
        dark_box: Option<[u8; 4]>,
    ) -> Rect {
        let flags = mask_flags(flags);
        let tight = self.calc_rect(font, text, rect, flags);
        if let Some(box_color) = dark_box {
            self.items.push(Item::Box {
                rect: tight,
                color: box_color,
            });
        }
        self.items.push(Item::Text {
            font,
            text: text.to_owned(),
            rect: tight,
            flags,
            color,
        });
        tight
    }

    /// `CPluginShell::DrawDarkTranslucentBox` (pluginshell.cpp:1815-1855):
    /// a translucent box (`SRCALPHA`/`INVSRCALPHA`; the C++ colour is
    /// [`DARK_TRANSLUCENT_BOX`]), drawn under *all* of the frame's queued
    /// text whatever the call order, as the C++ draws it immediately,
    /// before `DrawNow`. (The opaque box a text carries is
    /// `draw(.., dark_box)`: textmgr's own, swapped before its text.)
    pub fn draw_box(&mut self, rect: Rect, color: [u8; 4]) {
        self.items.push(Item::DarkBox { rect, color });
    }

    /// Number of queued messages (a text with a box is two).
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Empty the queue (`ClearAll`) and return its messages.
    pub(crate) fn take_items(&mut self) -> Vec<Item> {
        std::mem::take(&mut self.items)
    }

    pub(crate) fn atlas_mut(&mut self, font: FontId) -> &mut Atlas {
        &mut self.atlases[font as usize]
    }
}

//! Glyph atlas, text layout and DT_* flag emulation.
//!
//! `fontdue` rasterizes glyphs into a shelf-packed RGBA8 atlas (white RGB, coverage in
//! alpha, as `assets/shaders/text.wgsl` expects) and [`layout`] reproduces the subset of
//! Win32 `DrawText` that MilkDrop uses: `DT_SINGLELINE`, `DT_CALCRECT`, `DT_NOPREFIX`,
//! `DT_CENTER`/`DT_RIGHT`, `DT_END_ELLIPSIS`, `DT_WORD_ELLIPSIS`. See `layout.rs` for the
//! exact semantics mirrored and the documented deviations.
#![forbid(unsafe_code)]

pub mod atlas;
pub mod fonts;
pub mod layout;

pub use atlas::{Atlas, DirtyRegion};
pub use layout::layout;

/// One of MilkDrop's font slots (`m_fontinfo[i]`). `size_px` is a positive Win32
/// `lfHeight`: the *cell height* (ascent + descent) in pixels, which is also the line
/// height [`Atlas::line_height`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontSpec {
    pub face: String,
    pub size_px: u32,
    pub bold: bool,
    pub italic: bool,
    pub antialiased: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// The `DT_*` flags MilkDrop passes (every other flag is unused by the C++).
/// `end_ellipsis` is `DT_END_ELLIPSIS` (used by the menu code) and is an addition to the
/// A.3 interface; `Default` gives plain `DrawText(..., 0)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFlags {
    pub align: Align,
    pub single_line: bool,
    pub word_ellipsis: bool,
    pub end_ellipsis: bool,
    pub no_prefix: bool,
}

/// Win32 `RECT`: `right`/`bottom` exclusive, pixels, y down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }
    pub const fn width(&self) -> i32 {
        self.right - self.left
    }
    pub const fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// A glyph rectangle in target pixels (y down) and its atlas UV rectangle (`v` grows
/// downwards, like the texture rows). UVs are valid for the atlas state at the time of
/// the `layout` call: re-read them if [`Atlas::generation`] changes before drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphQuad {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub quads: Vec<GlyphQuad>,
    /// What `DT_CALCRECT` would return: `left/top` of the input rect, `right` = left +
    /// widest (ellipsized) line, `bottom` = top + lines * line height.
    pub bounds: Rect,
}

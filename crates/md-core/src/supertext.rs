//! The shared supertext: what a song title and a custom message have in
//! common in the C++, and how a custom message's text is sized into the
//! title texture.
//!
//! `m_supertext` (`td_supertext`, plugin.h:134-155; `plugin` below is
//! plugin.cpp, `mfs` is milkdropfs.cpp) is ONE record serving both: a
//! custom message launch ([`CustomMessages::launch`]) and a song-title
//! launch ([`TitleLaunch::song_title`]) overwrite the same fields, so
//! whichever starts later replaces the other outright; the replaced
//! animation just stops (no burn-in, `fStartTime` is reset). [`SuperTextSlot`]
//! is that record on the md-core side: it holds the text and font that
//! `md_render::Renderer` cannot get from [`md_render::SuperText`].
//!
//! # Handing it to md-render (Task 7.9)
//!
//! A launch is a [`TitleLaunch`]. The renderer's `start_song_title` already
//! does the song-title case. For a custom message the host
//!
//! 1. calls [`SuperTextSlot::start`] (md-core's copy, used for `kill`,
//!    `is_active` and the shared-slot rules above);
//! 2. gives the renderer `launch.st` as its supertext
//!    (`Renderer::supertext_mut()`), `launch.text` as the title text and
//!    `launch.face/bold/ital` as the font;
//! 3. when the renderer redraws the title texture (`bRedrawSuperText`,
//!    mfs:942-947) for a custom message it picks the pixel size with
//!    [`fit_font_px`] over [`TITLE_FONT_SIZES`] inside [`title_band`], measuring
//!    [`display_text`] with the face at each candidate size, draws it centred
//!    on the middle row at the chosen size, and stores the drawn height in
//!    `SuperText::font_size_used` (mfs:315-398). md-render does not have that
//!    branch yet; its `render_title_texture` handles song titles only.

use md_render::SuperText;

/// `g_title_font_sizes` (mfs:45-53), the pixel heights a custom message's
/// text may be drawn at.
pub const TITLE_FONT_SIZES: [i32; 36] = [
    6, 8, 10, 12, 14, 16, 20, 26, 32, 38, 44, 50, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128, 136,
    144, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 480, 512,
];

/// The rectangle text is fitted into for a title texture of `title_w` by
/// `title_h` pixels: `(width, height)` of `rect` with `left = 0`,
/// `top = h/21`, `bottom = h*17/21`, `right = w` (mfs:309-313). Integer
/// arithmetic.
pub fn title_band(title_w: i32, title_h: i32) -> (i32, i32) {
    (title_w, title_h * 17 / 21 - title_h / 21)
}

/// The text as drawn: `" %s "` (mfs:243), with a space at each end for
/// italic overhang and because it is centred.
pub fn display_text(text: &str) -> String {
    format!(" {text} ")
}

/// A custom message's font size (mfs:315-398): the binary search over
/// [`TITLE_FONT_SIZES`] with `lo = 0`, `hi = N - 1`. `measure(px)` is the
/// `DT_CALCRECT` size `(width, height)` of the [`display_text`] drawn
/// single-line at `px` (the rectangle's left is 0, so its right edge is the
/// width). A size is too big when `width >= band_w` or `height > band_h`
/// (`hi = mid`), otherwise `lo = mid`; the search stops when `lo == hi - 1`
/// and uses the `lo` size. The last table entry is therefore never chosen.
pub fn fit_font_px(band_w: i32, band_h: i32, mut measure: impl FnMut(i32) -> (i32, i32)) -> i32 {
    let (mut lo, mut hi) = (0usize, TITLE_FONT_SIZES.len() - 1);
    loop {
        let mid = (lo + hi) / 2;
        if lo + 1 == hi {
            return TITLE_FONT_SIZES[mid];
        }
        let (w, h) = measure(TITLE_FONT_SIZES[mid]);
        if w >= band_w || h > band_h {
            hi = mid;
        } else {
            lo = mid;
        }
    }
}

/// One launch of the supertext: everything `LaunchCustomMessage` or
/// `LaunchSongTitleAnim` writes into `m_supertext`.
#[derive(Clone, Debug, PartialEq)]
pub struct TitleLaunch {
    /// `szTextW`.
    pub text: String,
    /// `nFontFace`.
    pub face: String,
    /// `bBold`.
    pub bold: bool,
    /// `bItal`.
    pub ital: bool,
    /// The numeric fields, with `start_time` = the launch's `GetTime()`.
    /// `font_size_used` is 0: the title-texture draw sets it.
    pub st: SuperText,
    /// `fFadeTime` when the launch writes it (custom messages); a song
    /// title leaves the previous value in place (plugin.cpp:8546-8565 never
    /// assigns it).
    pub fade_time: Option<f32>,
}

impl TitleLaunch {
    /// `LaunchSongTitleAnim` (plugin.cpp:8546-8565): the `songtitle` font
    /// (`face`, `bold`, `ital`, `size` = its `nSize`), centred, no growth,
    /// white, for `duration` seconds (`m_fSongTitleAnimDuration`).
    pub fn song_title(
        text: &str,
        face: &str,
        bold: bool,
        ital: bool,
        size: f32,
        duration: f32,
        now: f32,
    ) -> TitleLaunch {
        TitleLaunch {
            text: text.to_owned(),
            face: face.to_owned(),
            bold,
            ital,
            st: SuperText {
                is_song_title: true,
                font_size: size,
                font_size_used: 0,
                x: 0.5,
                y: 0.5,
                growth: 1.0,
                fade_time: 0.0,
                color: [255; 3],
                start_time: now,
                duration,
            },
            fade_time: None,
        }
    }
}

/// `m_supertext` plus its redraw flag.
#[derive(Clone, Debug)]
pub struct SuperTextSlot {
    /// The numeric state (`fStartTime` -1 is off).
    pub st: SuperText,
    pub text: String,
    pub face: String,
    pub bold: bool,
    pub ital: bool,
    /// `bRedrawSuperText`: the title texture needs drawing.
    pub redraw: bool,
}

impl Default for SuperTextSlot {
    /// The off state (`fStartTime = -1`, plugin.cpp:1116-1117).
    fn default() -> Self {
        SuperTextSlot {
            st: SuperText::default(),
            text: String::new(),
            face: String::new(),
            bold: false,
            ital: false,
            redraw: false,
        }
    }
}

impl SuperTextSlot {
    /// Overwrite the slot with a launch (plugin.cpp:8491-8543, 8548-8564),
    /// whatever it was showing. `fFadeTime` and `nFontSizeUsed` keep their
    /// old values unless the launch writes them.
    pub fn start(&mut self, l: &TitleLaunch) {
        let (fade, used) = (self.st.fade_time, self.st.font_size_used);
        self.st = l.st.clone();
        self.st.fade_time = l.fade_time.unwrap_or(fade);
        self.st.font_size_used = used;
        self.text.clone_from(&l.text);
        self.face.clone_from(&l.face);
        self.bold = l.bold;
        self.ital = l.ital;
        self.redraw = true;
    }

    /// CTRL+T, CTRL+Y and the Delete key in message mode: `fStartTime = -1`
    /// (plugin.cpp:6053, 6200, 6216, 6225).
    pub fn kill(&mut self) {
        self.st.start_time = -1.0;
    }

    /// `fStartTime >= 0`.
    pub fn is_active(&self) -> bool {
        self.st.is_active()
    }

    /// Read and clear `bRedrawSuperText`.
    pub fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.redraw)
    }
}

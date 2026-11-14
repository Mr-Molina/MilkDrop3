//! Custom messages: `milk2_msg.ini` and `LaunchCustomMessage`
//! (`plugin` = code/vis_milk2/plugin.cpp).
//!
//! Custom messages do not draw anything themselves. Launching one fills the
//! shared supertext ([`crate::supertext`]) and the title texture shows it, so
//! [`CustomMessages::launch`] returns a [`TitleLaunch`] and the animation is
//! md-render's `draw_title_anim`. The per-message `rand()` jitter comes from
//! the caller's generator (one `next_u32()` per C++ `rand()`; the C runtime's
//! sequence is not reproduced).
//!
//! Defaults (`ReadCustomMessages`, plugin:8358-8397): every `[fontNN]`
//! (00-15) is face `arial`, not bold, not italic, white; every `[messageNN]`
//! (00-99) has size 50, `x` and `y` 0.5, no `randx/randy`, growth 1, time
//! 1.5 s, fade 0.2 (a fraction of the time), and no face, bold, italic,
//! colour or colour-jitter overrides. A missing section keeps the defaults; a
//! message is defined only when its `text` is non-empty (plugin:8421), and
//! only then are its other keys read.
//!
//! Departures: a `font=` outside 0..=15 indexes past the C++ array (undefined
//! behaviour); here it selects font 0. `text` is cut to 255 characters and
//! `face` to 127 (the `wchar_t[256]` and `[128]` fields; the C++ passes the
//! byte size as the limit, so an oversized value would overflow). `time=0`
//! or `fade=0` are kept as read.

use crate::supertext::TitleLaunch;
use md_preset::ini::Ini;
use md_render::SuperText;
use rand_core::Rng;

/// `MAX_CUSTOM_MESSAGE_FONTS` (md_defines.h:54).
pub const MAX_FONTS: usize = 16;
/// `MAX_CUSTOM_MESSAGES` (md_defines.h:55).
pub const MAX_MESSAGES: usize = 100;

/// `td_custom_msg_font`: `[fontNN]` keys `face bold ital r g b`.
#[derive(Clone, Debug, PartialEq)]
pub struct MsgFont {
    pub face: String,
    pub bold: bool,
    pub ital: bool,
    pub color: [i32; 3],
}

impl Default for MsgFont {
    fn default() -> Self {
        MsgFont {
            face: "arial".into(),
            bold: false,
            ital: false,
            color: [255; 3],
        }
    }
}

/// `td_custom_msg` of a defined message. `None` overrides fall back to the
/// message's font (`bOverride*` false).
#[derive(Clone, Debug, PartialEq)]
pub struct CustomMessage {
    pub text: String,
    /// `font` (0..=15).
    pub font: i32,
    /// `size`, 0..100 relative to the window.
    pub size: f32,
    pub x: f32,
    pub y: f32,
    /// `randx`, `randy`: the position is jittered by up to +-this.
    pub randx: f32,
    pub randy: f32,
    pub growth: f32,
    /// `time`: seconds shown (`fTime`).
    pub time: f32,
    /// `fade`: the fraction of the time spent fading in (`fFade`).
    pub fade: f32,
    pub face: Option<String>,
    pub bold: Option<bool>,
    pub ital: Option<bool>,
    /// `r g b` when present (any value but -1).
    pub color: [Option<i32>; 3],
    /// `randr randg randb`.
    pub rand_color: [i32; 3],
}

impl CustomMessage {
    fn defaults(text: String) -> CustomMessage {
        CustomMessage {
            text,
            font: 0,
            size: 50.0,
            x: 0.5,
            y: 0.5,
            randx: 0.0,
            randy: 0.0,
            growth: 1.0,
            time: 1.5,
            fade: 0.2,
            face: None,
            bold: None,
            ital: None,
            color: [None; 3],
            rand_color: [0; 3],
        }
    }
}

/// The 16 fonts and 100 messages of `milk2_msg.ini`.
#[derive(Clone, Debug)]
pub struct CustomMessages {
    pub fonts: [MsgFont; MAX_FONTS],
    pub messages: [Option<CustomMessage>; MAX_MESSAGES],
}

fn truncated(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// `GetPrivateProfileInt(.., -1)` as the override tests use it: `Some` unless
/// the key is missing or is -1.
fn override_int(ini: &Ini, sec: &str, key: &str) -> Option<i32> {
    Some(ini.get_i32(sec, key, -1)).filter(|v| *v != -1)
}

impl CustomMessages {
    /// `ReadCustomMessages` (plugin:8354-8456).
    pub fn from_ini(ini: &Ini) -> CustomMessages {
        let fonts = std::array::from_fn(|n| {
            let s = format!("font{n:02}");
            let d = MsgFont::default();
            MsgFont {
                face: ini.get(&s, "face").map_or(d.face, |f| truncated(f, 127)),
                bold: ini.get_bool(&s, "bold", d.bold),
                ital: ini.get_bool(&s, "ital", d.ital),
                color: [
                    ini.get_i32(&s, "r", 255),
                    ini.get_i32(&s, "g", 255),
                    ini.get_i32(&s, "b", 255),
                ],
            }
        });
        let messages = std::array::from_fn(|n| {
            let s = format!("message{n:02}");
            let text = truncated(ini.get(&s, "text").unwrap_or(""), 255);
            if text.is_empty() {
                return None;
            }
            let mut m = CustomMessage::defaults(text);
            m.font = ini.get_i32(&s, "font", 0);
            m.size = ini.get_f32(&s, "size", m.size);
            m.x = ini.get_f32(&s, "x", m.x);
            m.y = ini.get_f32(&s, "y", m.y);
            m.randx = ini.get_f32(&s, "randx", 0.0);
            m.randy = ini.get_f32(&s, "randy", 0.0);
            m.growth = ini.get_f32(&s, "growth", m.growth);
            m.time = ini.get_f32(&s, "time", m.time);
            m.fade = ini.get_f32(&s, "fade", m.fade);
            m.rand_color = [
                ini.get_i32(&s, "randr", 0),
                ini.get_i32(&s, "randg", 0),
                ini.get_i32(&s, "randb", 0),
            ];
            // Overrides (plugin:8440-8453): a non-empty face, and bold, ital,
            // r, g, b whenever they are not -1.
            m.face = ini
                .get(&s, "face")
                .filter(|f| !f.is_empty())
                .map(|f| truncated(f, 127));
            m.bold = override_int(ini, &s, "bold").map(|v| v != 0);
            m.ital = override_int(ini, &s, "ital").map(|v| v != 0);
            m.color = [
                override_int(ini, &s, "r"),
                override_int(ini, &s, "g"),
                override_int(ini, &s, "b"),
            ];
            Some(m)
        });
        CustomMessages { fonts, messages }
    }

    /// [`from_ini`](Self::from_ini) of the file's text.
    pub fn parse(text: &str) -> CustomMessages {
        CustomMessages::from_ini(&Ini::parse(text))
    }

    /// How many messages are defined.
    pub fn count(&self) -> usize {
        self.messages.iter().flatten().count()
    }

    /// `LaunchCustomMessage(n)` (plugin:8458-8544) at clock `now`. `n > 99`
    /// is 99; `n < 0` picks a defined message with `rand() % count + 1`
    /// (plugin:8463-8480; `None` when none is defined, where the C++ would
    /// divide by zero). `None` for an undefined message, drawing nothing.
    ///
    /// A launch draws five numbers from `rng` in this order, always (the C++
    /// calls `rand()` whatever the jitter amounts): x, y (8497-8498), then R,
    /// G, B (8515-8517); each is `(r % 1037) / 1037 * 2 - 1`. The colour is
    /// the override or the font's, plus `(int)(randc * j)`, clamped to
    /// 0..=255 (8518-8523).
    pub fn launch(&self, n: i32, rng: &mut impl Rng, now: f32) -> Option<TitleLaunch> {
        let mut n = n.min(99);
        if n < 0 {
            let count = self.count() as u32;
            if count == 0 {
                return None;
            }
            let sel = (rng.next_u32() % count) as usize + 1;
            n = self
                .messages
                .iter()
                .enumerate()
                .filter(|(_, m)| m.is_some())
                .nth(sel - 1)
                .map(|(i, _)| i as i32)?;
        }
        let m = self.messages.get(n as usize)?.as_ref()?;
        let font = usize::try_from(m.font)
            .ok()
            .and_then(|i| self.fonts.get(i))
            .unwrap_or(&self.fonts[0]);

        let mut jit = || (rng.next_u32() % 1037) as f32 / 1037.0 * 2.0 - 1.0;
        let x = m.x + m.randx * jit();
        let y = m.y + m.randy * jit();
        let mut color = [0u8; 3];
        for (i, c) in color.iter_mut().enumerate() {
            let base = m.color[i].unwrap_or(font.color[i]);
            let v = base + (m.rand_color[i] as f32 * jit()) as i32;
            *c = v.clamp(0, 255) as u8;
        }
        Some(TitleLaunch {
            text: m.text.clone(),
            face: m.face.clone().unwrap_or_else(|| font.face.clone()),
            bold: m.bold.unwrap_or(font.bold),
            ital: m.ital.unwrap_or(font.ital),
            st: SuperText {
                is_song_title: false,
                font_size: m.size,
                font_size_used: 0,
                x,
                y,
                growth: m.growth,
                fade_time: m.fade,
                color,
                start_time: now,
                duration: m.time,
            },
            writes_fade: true,
        })
    }
}

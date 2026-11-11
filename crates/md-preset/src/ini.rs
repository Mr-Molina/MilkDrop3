//! General-purpose INI reader used for `.milk` presets and `milk2.ini`.
//!
//! Two lookup models share one parse, because the C++ itself has two:
//!
//! * **Flat (preset) model** -- `flat_*` and [`Ini::code_block`]. This mirrors
//!   `_GetLineByName` in `state.cpp` (the fast reader every `.milk` load
//!   uses): section headers are ignored (preset keys such as
//!   `MILKDROP_PRESET_VERSION` even sit *before* `[preset00]`), keys are
//!   case-sensitive, the key ends at the first `=` or space, the value is the
//!   untrimmed remainder of the line up to `\r`/`\n`, and the first
//!   occurrence of a key wins.
//! * **Sectioned model** -- `get`, `get_f32`, `get_i32`, `get_bool`. This is
//!   the Win32 `GetPrivateProfile*` behaviour `milk2.ini` relies on: sections
//!   and keys match case-insensitively, whitespace around key and value is
//!   trimmed, `;` lines are comments, the first occurrence wins.
//!
//! Intentional divergence from C++ (ledger ruling): a leading UTF-8 BOM is
//! stripped. CRLF and lone `\r` line ends are accepted (as in C++), and junk
//! lines without `=`/space are ignored.

use std::collections::HashMap;

/// `LINEFEED_CONTROL_CHAR` is byte 1 in C++; Rust code blocks use `'\n'`.
const LINE_SEP: char = '\n';
/// `MAX_BIGSTRING_LEN` (state.h:52): the capacity `ReadCode` fills.
const MAX_BIGSTRING_LEN: usize = 32768;
/// `ReadCode`'s "key missing" sentinel default (state.cpp:1090).
const MISSING_SENTINEL: &str = "~!@#$";

/// A parsed INI file.
#[derive(Debug, Clone, Default)]
pub struct Ini {
    /// Section-blind entries in file order, as the C++ fast reader sees them.
    flat: Vec<(String, String)>,
    /// Lowercased section name -> `(lowercased key, trimmed value)` in order.
    /// Entries before any `[section]` header live under `""`.
    sections: HashMap<String, Vec<(String, String)>>,
}

impl Ini {
    /// Parse INI text. Never fails; unrecognised lines are skipped.
    pub fn parse(text: &str) -> Ini {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut ini = Ini::default();
        let mut section = String::new();
        // C++ splits lines on '\r' and '\n' alike and skips empty lines.
        for line in text.split(['\r', '\n']).filter(|l| !l.is_empty()) {
            // Flat model: key up to the first '=' or ' ', value is the rest.
            if let Some(i) = line.find(['=', ' ']) {
                ini.flat
                    .push((line[..i].to_string(), line[i + 1..].to_string()));
            }
            // Sectioned model.
            let t = line.trim();
            if t.is_empty() || t.starts_with(';') {
                continue;
            }
            if let Some(rest) = t.strip_prefix('[') {
                if let Some(name) = rest.strip_suffix(']') {
                    section = name.trim().to_lowercase();
                    ini.sections.entry(section.clone()).or_default();
                    continue;
                }
            }
            if let Some((k, v)) = t.split_once('=') {
                ini.sections
                    .entry(section.clone())
                    .or_default()
                    .push((k.trim().to_lowercase(), v.trim().to_string()));
            }
        }
        ini
    }

    // ---- sectioned model -------------------------------------------------

    /// Value of `key` in `section` (both case-insensitive, first wins,
    /// trimmed). Use `""` for keys before any section header.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let key = key.to_lowercase();
        self.sections
            .get(&section.to_lowercase())?
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Float value, or `default` if missing or unparsable (sscanf `%f` rules).
    pub fn get_f32(&self, s: &str, k: &str, default: f32) -> f32 {
        self.get(s, k).and_then(scan_f32).unwrap_or(default)
    }

    /// Integer value, or `default` if missing or unparsable (sscanf `%d`).
    pub fn get_i32(&self, s: &str, k: &str, default: i32) -> i32 {
        self.get(s, k).and_then(scan_i32).unwrap_or(default)
    }

    /// Boolean stored as an integer (`0` false, anything else true).
    pub fn get_bool(&self, s: &str, k: &str, default: bool) -> bool {
        self.get(s, k)
            .and_then(scan_i32)
            .map_or(default, |v| v != 0)
    }

    // ---- flat (preset) model ---------------------------------------------

    /// Raw value of `key`: section-blind, case-sensitive, first wins, value
    /// untrimmed (`_GetLineByName`).
    pub fn flat_get(&self, key: &str) -> Option<&str> {
        self.flat
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// `GetFastFloat`: `sscanf("%f")` of the raw value, else `default`.
    pub fn flat_f32(&self, key: &str, default: f32) -> f32 {
        self.flat_get(key).and_then(scan_f32).unwrap_or(default)
    }

    /// `GetFastInt`: `sscanf("%d")` of the raw value, else `default`.
    pub fn flat_i32(&self, key: &str, default: i32) -> i32 {
        self.flat_get(key).and_then(scan_i32).unwrap_or(default)
    }

    /// Bool read as `GetFastInt != 0`.
    pub fn flat_bool(&self, key: &str, default: bool) -> bool {
        self.flat_get(key)
            .and_then(scan_i32)
            .map_or(default, |v| v != 0)
    }

    /// Mirror of `ReadCode` (state.cpp:1071): collects `<prefix>1`,
    /// `<prefix>2`, ... stopping at the first missing key, stripping one
    /// leading backtick from each line, and joins the lines with `'\n'`
    /// (C++ terminates each with a control char; see [`strip_code`]).
    /// Also stops, as C++ does, when the 32768-byte buffer would overflow.
    pub fn code_block(&self, prefix: &str) -> String {
        let mut out = String::new();
        let mut char_pos = 0usize;
        let mut first = true;
        for n in 1.. {
            let Some(raw) = self.flat_get(&format!("{prefix}{n}")) else {
                break;
            };
            if raw == MISSING_SENTINEL || raw.len() + 2 >= MAX_BIGSTRING_LEN - char_pos {
                break;
            }
            let line = raw.strip_prefix('`').unwrap_or(raw);
            if !first {
                out.push(LINE_SEP);
            }
            first = false;
            out.push_str(line);
            char_pos += line.len() + 1;
        }
        out
    }
}

/// Mirror of `CState::StripLinefeedCharsAndComments` (state.cpp:1525): drops
/// the line separators with NO replacement and removes `//` and `\\` comments
/// running to the end of their own line (the separator ends the comment, so a
/// comment never swallows the next line). This is the form md-eval is fed.
pub fn strip_code(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut comment = false;
    for (i, &c) in b.iter().enumerate() {
        let next = b.get(i + 1).copied();
        if comment {
            if c == LINE_SEP as u8 {
                comment = false;
            }
        } else if (c == b'\\' && next == Some(b'\\')) || (c == b'/' && next == Some(b'/')) {
            comment = true;
        } else if c != LINE_SEP as u8 {
            out.push(c);
        }
    }
    // Only ASCII bytes are ever removed, so UTF-8 validity is preserved.
    String::from_utf8(out).unwrap_or_default()
}

/// `sscanf("%d")`: skip whitespace, optional sign, digits; trailing junk ok.
fn scan_i32(s: &str) -> Option<i32> {
    let s = s.trim_start();
    let b = s.as_bytes();
    let mut end = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let digits = end;
    while b.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == digits {
        return None;
    }
    s[..end].parse().ok()
}

/// `sscanf("%f")` (decimal forms): longest `[+-]digits[.digits][e[+-]digits]`.
fn scan_f32(s: &str) -> Option<f32> {
    let s = s.trim_start();
    let b = s.as_bytes();
    let mut end = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let mut mantissa = 0;
    while b.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
        mantissa += 1;
    }
    if b.get(end) == Some(&b'.') {
        end += 1;
        while b.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
            mantissa += 1;
        }
    }
    if mantissa == 0 {
        return None;
    }
    if matches!(b.get(end), Some(b'e' | b'E')) {
        let mut e = end + 1;
        if matches!(b.get(e), Some(b'+' | b'-')) {
            e += 1;
        }
        let ds = e;
        while b.get(e).is_some_and(u8::is_ascii_digit) {
            e += 1;
        }
        if e > ds {
            end = e;
        }
    }
    s[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_duplicate_key_wins() {
        let ini = Ini::parse("[a]\nk=1\nk=2\n[A]\nk=3\n");
        assert_eq!(ini.get("a", "k"), Some("1"));
        assert_eq!(ini.get("A", "K"), Some("1"));
        assert_eq!(ini.flat_get("k"), Some("1"));
    }

    #[test]
    fn value_may_contain_equals() {
        let ini = Ini::parse("[p]\nper_frame_1=zoom = zoom + 1;\neq=a=b=c\n");
        assert_eq!(ini.get("p", "eq"), Some("a=b=c"));
        assert_eq!(ini.flat_get("eq"), Some("a=b=c"));
        assert_eq!(ini.flat_get("per_frame_1"), Some("zoom = zoom + 1;"));
    }

    #[test]
    fn code_block_stops_at_gap() {
        let ini = Ini::parse("per_frame_1=a;\nper_frame_2=b;\nper_frame_4=d;\n");
        assert_eq!(ini.code_block("per_frame_"), "a;\nb;");
    }

    #[test]
    fn backtick_prefix_is_stripped() {
        let ini = Ini::parse("warp_1=`shader_body\nwarp_2=`{\nwarp_3=``x\nwarp_4=y\n");
        // exactly one backtick is removed; only a leading one.
        assert_eq!(ini.code_block("warp_"), "shader_body\n{\n`x\ny");
    }

    #[test]
    fn parses_crlf_bom_and_trailing_junk() {
        let text = include_str!("../../../tests/presets/edge/crlf_bom_junk.milk");
        assert!(text.starts_with('\u{feff}') && text.contains("\r\n"));
        let ini = Ini::parse(text);
        // BOM stripped: the very first key is addressable.
        assert_eq!(ini.flat_get("MILKDROP_PRESET_VERSION"), Some("201"));
        assert_eq!(ini.flat_i32("MILKDROP_PRESET_VERSION", 0), 201);
        assert_eq!(ini.get("", "milkdrop_preset_version"), Some("201"));
        assert_eq!(ini.get_f32("preset00", "fDecay", 0.0), 0.97);
        // No '\r' leaks into values.
        assert!(!ini.flat_get("per_frame_1").unwrap().contains('\r'));
        assert_eq!(ini.code_block("per_frame_").lines().count(), 2);
        assert_eq!(
            ini.code_block("warp_"),
            "shader_body\n{\n    ret = tex2D(sampler_main, uv).xyz*0.97;\n}"
        );
        // Trailing junk (comment, prose, ^Z) defines no preset keys.
        assert_eq!(ini.flat_get("rate"), None);
        assert_eq!(ini.get("preset00", "thanks"), None);
    }

    // ---- C++ lookup rules (state.cpp _GetLineByName) -----------------------

    #[test]
    fn flat_keys_are_case_sensitive_but_sectioned_are_not() {
        let ini = Ini::parse("[s]\nfDecay=0.5\n");
        assert_eq!(ini.flat_get("fdecay"), None);
        assert_eq!(ini.flat_get("fDecay"), Some("0.5"));
        assert_eq!(ini.get("S", "FDECAY"), Some("0.5"));
    }

    #[test]
    fn flat_lookup_ignores_sections() {
        let ini = Ini::parse("before=1\n[preset00]\nin=2\n[other]\nlater=3\n");
        assert_eq!(ini.flat_get("before"), Some("1"));
        assert_eq!(ini.flat_get("later"), Some("3"));
        assert_eq!(ini.get("", "before"), Some("1"));
        assert_eq!(ini.get("preset00", "before"), None);
        assert_eq!(ini.get("other", "later"), Some("3"));
    }

    #[test]
    fn flat_value_is_not_trimmed_and_space_delimits_key() {
        let ini = Ini::parse("a= x \nb = y\nc z\n");
        assert_eq!(ini.flat_get("a"), Some(" x "));
        // key ends at the first space: value of `b` is "= y".
        assert_eq!(ini.flat_get("b"), Some("= y"));
        assert_eq!(ini.flat_get("c"), Some("z"));
        // sectioned model trims.
        assert_eq!(ini.get("", "a"), Some("x"));
        assert_eq!(ini.get("", "b"), Some("y"));
    }

    #[test]
    fn lines_without_delimiter_and_blank_lines_are_skipped() {
        let ini = Ini::parse("\r\n\r\n[preset00]\nnodelim\n\nk=v\n");
        assert_eq!(ini.flat_get("nodelim"), None);
        assert_eq!(ini.flat_get("k"), Some("v"));
    }

    #[test]
    fn lone_cr_ends_a_line() {
        let ini = Ini::parse("a=1\rb=2\r");
        assert_eq!(ini.flat_get("a"), Some("1"));
        assert_eq!(ini.flat_get("b"), Some("2"));
    }

    #[test]
    fn semicolon_lines_are_comments_in_sectioned_model() {
        let ini = Ini::parse("[s]\n; k=1\n;k=2\nk=3\n");
        assert_eq!(ini.get("s", "k"), Some("3"));
        assert_eq!(ini.get("s", ";k"), None);
    }

    #[test]
    fn numeric_getters_follow_sscanf() {
        let ini = Ini::parse(
            "[s]\nf=0.25abc\ni= 7 junk\nneg=-3\nbad=x\nb1=1\nb0=0\nb2=2\nexp=1e2\ndot=.5\n",
        );
        assert_eq!(ini.get_f32("s", "f", 9.0), 0.25);
        assert_eq!(ini.get_i32("s", "i", 9), 7);
        assert_eq!(ini.get_i32("s", "neg", 9), -3);
        assert_eq!(ini.get_f32("s", "bad", 9.0), 9.0);
        assert_eq!(ini.get_i32("s", "missing", 9), 9);
        assert!(ini.get_bool("s", "b1", false));
        assert!(!ini.get_bool("s", "b0", true));
        assert!(ini.get_bool("s", "b2", false));
        assert!(ini.get_bool("s", "bad", true));
        assert_eq!(ini.get_f32("s", "exp", 0.0), 100.0);
        assert_eq!(ini.get_f32("s", "dot", 0.0), 0.5);
        // flat reader keeps leading whitespace, which sscanf skips.
        let flat = Ini::parse("i= 7\nf=  2.5\nb=1\n");
        assert_eq!(flat.flat_i32("i", 0), 7);
        assert_eq!(flat.flat_f32("f", 0.0), 2.5);
        assert!(flat.flat_bool("b", false));
        assert_eq!(flat.flat_i32("nope", 4), 4);
    }

    #[test]
    fn empty_code_line_is_present_not_a_gap() {
        let ini = Ini::parse("pf_1=a\npf_2=\npf_3=c\n");
        assert_eq!(ini.code_block("pf_"), "a\n\nc");
        assert_eq!(ini.code_block("none_"), "");
    }

    #[test]
    fn code_block_stops_when_buffer_would_overflow() {
        let long = "x".repeat(20000);
        let ini = Ini::parse(&format!("pf_1={long}\npf_2={long}\npf_3=z\n"));
        // second line does not fit in MAX_BIGSTRING_LEN; C++ stops there.
        assert_eq!(ini.code_block("pf_"), long);
    }

    // ---- strip_code (StripLinefeedCharsAndComments) ------------------------

    #[test]
    fn strip_code_joins_without_separator() {
        assert_eq!(strip_code("a=1;\nb=2;"), "a=1;b=2;");
    }

    #[test]
    fn strip_code_comment_does_not_swallow_next_line() {
        let ini = Ini::parse("pf_1=a=1; // note\npf_2=b=2;\npf_3=c=3; \\\\ other\npf_4=d=4;\n");
        let block = ini.code_block("pf_");
        assert_eq!(block, "a=1; // note\nb=2;\nc=3; \\\\ other\nd=4;");
        assert_eq!(strip_code(&block), "a=1; b=2;c=3; d=4;");
    }

    #[test]
    fn strip_code_single_slash_and_backslash_are_kept() {
        assert_eq!(strip_code("a=b/c;\nd=e\\f;"), "a=b/c;d=e\\f;");
        assert_eq!(strip_code("x=1; //"), "x=1; ");
        assert_eq!(strip_code(""), "");
    }

    #[test]
    fn strip_code_preserves_non_ascii() {
        assert_eq!(strip_code("a=1; // é\nb=\"é\";"), "a=1; b=\"é\";");
    }
}

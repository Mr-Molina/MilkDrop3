//! `DrawText`-compatible layout.
//!
//! The C++ (`textmgr.cpp` -> `ID3DXFont::DrawText`) only ever passes `DT_SINGLELINE`,
//! `DT_CALCRECT`, `DT_NOPREFIX`, `DT_CENTER`, `DT_RIGHT`, `DT_END_ELLIPSIS`,
//! `DT_WORD_ELLIPSIS` (plus no flags at all); `DT_WORDBREAK` is only ever commented out.
//! NOTE: `CTextManager::DrawText[W]` (textmgr.cpp:122/:174, comment ~:604) strips
//! `DT_WORD_ELLIPSIS | DT_END_ELLIPSIS | DT_NOPREFIX` before drawing ("not supported by
//! D3DX9"), so *effective* MilkDrop behaviour is no ellipsis and `&` prefix processing
//! ON. This API keeps the flags; the consumer (md-render text pass, Task 6.6) must apply
//! the same masking to match MilkDrop.
//!
//! Mirrored semantics:
//!
//! * Text is top-aligned, never wrapped (no `DT_WORDBREAK`). Without `single_line`,
//!   `\n` starts a new line (a lone `\r` is dropped, `\r\n` is one break; a trailing
//!   `\n` yields a final empty line). With `single_line`, `\r`/`\n` are dropped and do
//!   not break the line.
//! * Line advance = ascent + descent = the font's cell height (`Atlas::line_height`);
//!   external leading is not added. `bounds.bottom - bounds.top` = lines * line height;
//!   empty text has zero lines (`DrawText` returns 0 for ""), so zero height.
//! * `bounds` is the `DT_CALCRECT` result: `left`/`top` unchanged, `right = left +` the
//!   widest line (after ellipsis), regardless of alignment (MilkDrop only reads
//!   `right - left` from it). Quads are positioned inside the input `rect` according to
//!   `align` (Left: `rect.left`; Right: line ends at `rect.right`; Center:
//!   `rect.left + (width - line) / 2`, integer division, may be negative when the line
//!   overflows).
//! * `&` prefix handling unless `no_prefix`: `&&` -> `&`, a single `&` is removed (the
//!   following character is *not* underlined; deviation, MilkDrop's text never relies
//!   on the mnemonic underline).
//! * Ellipsis applies per line, only when the line is wider than `rect`. `end_ellipsis`
//!   keeps the longest character prefix that fits with `...` appended. `word_ellipsis`
//!   does the same but, when the cut would fall inside a word and an earlier word
//!   boundary exists, cuts there instead (trailing blanks removed) so whole words are
//!   kept; a lone overlong word is cut by character. If `...` alone is wider than the
//!   rect, as many dots as fit are shown.
//! * Glyph advances are integer pixels without kerning (GDI behaviour). `\t` is laid out
//!   as one space (`DT_EXPANDTABS` is never used); other control characters are dropped.
//! * Deviation: nothing is clipped to `rect` (`DT_NOCLIP` is not emulated either way), so
//!   overflowing lines without ellipsis extend past the rect.

use crate::{Align, Atlas, GlyphQuad, Layout, Rect, TextFlags};

fn strip_prefix(text: &str, no_prefix: bool) -> Vec<char> {
    let mut out = Vec::with_capacity(text.len());
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if c == '&' && !no_prefix {
            if it.peek() == Some(&'&') {
                it.next();
                out.push('&');
            }
            // a single '&' is dropped
        } else {
            out.push(c);
        }
    }
    out
}

fn split_lines(chars: Vec<char>, single_line: bool) -> Vec<Vec<char>> {
    if chars.is_empty() {
        return Vec::new();
    }
    let norm = |c: char| if c == '\t' { ' ' } else { c };
    let keep = |c: &char| !c.is_control() || *c == '\t';
    if single_line {
        return vec![chars.into_iter().filter(keep).map(norm).collect()];
    }
    let mut lines = vec![Vec::new()];
    for c in chars {
        if c == '\n' {
            lines.push(Vec::new());
        } else if keep(&c) {
            lines.last_mut().expect("non-empty").push(norm(c));
        }
    }
    lines
}

fn width_of(atlas: &mut Atlas, s: &[char]) -> i32 {
    s.iter()
        .fold(0i32, |acc, &c| acc.saturating_add(atlas.advance(c)))
}

fn ellipsize(atlas: &mut Atlas, line: &[char], avail: i32, word: bool) -> Vec<char> {
    if width_of(atlas, line) <= avail {
        return line.to_vec();
    }
    let dot = atlas.advance('.');
    if avail < dot * 3 {
        let k = if dot > 0 {
            (avail.max(0) / dot).min(3)
        } else {
            0
        };
        return vec!['.'; k as usize];
    }
    let budget = avail - dot * 3;
    let mut n = 0;
    let mut w = 0;
    while n < line.len() {
        let a = atlas.advance(line[n]);
        if w + a > budget {
            break;
        }
        w += a;
        n += 1;
    }
    let mut keep = n;
    if word && n < line.len() && n > 0 && line[n] != ' ' && line[n - 1] != ' ' {
        if let Some(j) = line[..n].iter().rposition(|&c| c == ' ') {
            keep = j;
        }
    }
    if word {
        while keep > 0 && line[keep - 1] == ' ' {
            keep -= 1;
        }
    }
    let mut out = line[..keep].to_vec();
    out.extend_from_slice(&['.'; 3]);
    out
}

pub fn layout(atlas: &mut Atlas, text: &str, rect: Rect, flags: TextFlags) -> Layout {
    let avail = rect.width();
    let lines = split_lines(strip_prefix(text, flags.no_prefix), flags.single_line);
    let ellipsis = flags.word_ellipsis || flags.end_ellipsis;
    let lines: Vec<Vec<char>> = lines
        .into_iter()
        .map(|l| {
            if ellipsis {
                ellipsize(atlas, &l, avail, flags.word_ellipsis)
            } else {
                l
            }
        })
        .collect();
    let widths: Vec<i32> = lines.iter().map(|l| width_of(atlas, l)).collect();
    let max_w = widths.iter().copied().max().unwrap_or(0);
    let lh = atlas.line_height();
    let bounds = Rect::new(
        rect.left,
        rect.top,
        rect.left.saturating_add(max_w),
        rect.top
            .saturating_add(lh.saturating_mul(lines.len().min(i32::MAX as usize) as i32)),
    );

    // Rasterize every visible glyph first so that the final texture size is known.
    let mut wanted: Vec<char> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &c in lines.iter().flatten() {
        if c != ' ' && seen.insert(c) {
            wanted.push(c);
        }
    }
    let mut reset_done = false;
    'retry: loop {
        for &c in &wanted {
            // On Err after the one reset the glyph is dropped (blank advance).
            if atlas.ensure(c).is_err() && !reset_done {
                atlas.reset();
                reset_done = true;
                continue 'retry;
            }
        }
        break;
    }

    let (tw, th) = atlas.tex_size();
    let (tw, th) = (tw as f32, th as f32);
    let mut quads = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let mut pen = match flags.align {
            Align::Left => rect.left,
            Align::Right => rect.right.saturating_sub(widths[i]),
            Align::Center => rect
                .left
                .saturating_add(avail.saturating_sub(widths[i]) / 2),
        };
        let base = rect
            .top
            .saturating_add(lh.saturating_mul(i.min(i32::MAX as usize) as i32))
            .saturating_add(atlas.ascent());
        for &c in line {
            if let Some(p) = atlas.placed(c) {
                let x0 = pen.saturating_add(p.left) as f32;
                let y0 = base.saturating_sub(p.top) as f32;
                quads.push(GlyphQuad {
                    x0,
                    y0,
                    x1: x0 + p.w as f32,
                    y1: y0 + p.h as f32,
                    u0: p.x as f32 / tw,
                    v0: p.y as f32 / th,
                    u1: (p.x + p.w) as f32 / tw,
                    v1: (p.y + p.h) as f32 / th,
                });
            }
            pen = pen.saturating_add(atlas.advance(c));
        }
    }
    Layout { quads, bounds }
}

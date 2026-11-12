//! Legacy source fixups (Task 4.7), applied to the preset body before
//! lexing, so presets that compiled in MilkDrop keep translating.
//!
//! The brief's candidate list was checked idiom by idiom against the
//! pipeline (one fixture test each in `tests/fixups.rs`); only idioms the
//! pipeline does not already accept get a source rewrite here:
//! - `;` after the closing `}` of `shader_body` — covered by Task 4.2 (the
//!   parser drops tokens after the block, like the C++ split).
//! - `float2 x = float2(a);` single-argument constructors — covered by
//!   Task 4.3 (`Splat`).
//! - `tex2D(...).xyz` into the `float3 ret`, and `float4` results assigned
//!   to `ret` — covered by Task 4.3 (implicit truncation).
//! - `#define` redefinition — covered by Task 4.1 (the new body replaces
//!   the old, as fxc does after its X1519 warning).
//! - identifiers that are WGSL keywords — covered by Task 4.4's `h_`
//!   prefix; `uv_orig` in comp — covered by Task 4.6's defines (preflight
//!   F28: neither gets a fixup).
//! - `//` comments ending with `\`, and comment edge cases `/*/` and `*//`
//!   — [`strip_comments`].

/// Apply every fixup. Line structure is preserved (no line is added or
/// removed), so error positions stay valid for the original body.
pub fn apply_fixups(src: &str) -> String {
    strip_comments(src)
}

/// Remove `//` and `/* */` comments exactly as `plugin.cpp::StripComments`
/// (code/vis_milk2/plugin.cpp:1342) did to the preset text before fxc saw
/// it: a two-character window `(c0, c1)` slides one character at a time, a
/// `//` (outside a block comment) ignores up to the end of the line, a
/// `/*` (outside a line comment) ignores up to the next `*/`, whose two
/// characters are then skipped. Because the window overlaps, `/*/` is a
/// complete comment and the `/` that closes `*//` also opens a line
/// comment; a block comment never closed drops the rest of the text.
///
/// Why: the preprocessor follows C and splices `\`-newline before
/// removing comments, so `// note \` would swallow the next line; in
/// MilkDrop the comment, and with it the backslash, was already gone (no
/// preset in `tests/presets/` has such a comment; fixture
/// `tests/fixtures/line_comment_backslash.hlsl`). The lexer's own comment
/// rules are C's, which differ on `/*/` and `*//`.
///
/// One deliberate difference: the C++ also drops the newlines inside a
/// block comment; here they are re-emitted just before the next newline
/// kept (or at the end), so the text around the comment is joined exactly
/// as in the C++ (`a/**/b` is `ab`) while every later line keeps its
/// number. Only text after a multi-line comment on its closing line moves
/// up, to the line the comment starts on — where fxc saw it too.
fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    if b.len() < 2 {
        return src.to_string();
    }
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut held: Vec<u8> = Vec::new(); // newlines removed with block comments
    let (mut in_line, mut in_block) = (false, false);
    let mut skip = 0;
    for (i, &c0) in b.iter().enumerate() {
        let c1 = b.get(i + 1).copied().unwrap_or(0);
        if !in_block && c0 == b'/' && c1 == b'/' {
            in_line = true;
        }
        if in_line && (c0 == b'\n' || c0 == b'\r') {
            in_line = false;
            skip = 0;
        }
        if !in_line && c0 == b'/' && c1 == b'*' {
            in_block = true;
        }
        if in_block && c0 == b'*' && c1 == b'/' {
            in_block = false;
            skip = 2;
        }
        if !in_line && !in_block {
            if skip > 0 {
                skip -= 1;
            } else {
                if c0 == b'\n' || c0 == b'\r' {
                    out.append(&mut held);
                }
                out.push(c0);
            }
        } else if in_block && (c0 == b'\n' || c0 == b'\r') {
            held.push(c0);
        }
    }
    out.append(&mut held);
    // Only ASCII ranges are removed, so `out` is still UTF-8.
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

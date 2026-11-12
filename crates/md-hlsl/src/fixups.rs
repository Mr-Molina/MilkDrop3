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
//! - `//` comments ending with `\` — [`strip_line_comments`].

/// Apply every fixup. Line structure is preserved (no line is added or
/// removed), so error positions stay valid for the original body.
pub fn apply_fixups(src: &str) -> String {
    strip_line_comments(src)
}

/// Remove `//` comments up to (not including) the end of their line, as
/// `plugin.cpp::StripComments` (code/vis_milk2/plugin.cpp:1342) did to the
/// preset text before fxc saw it. A `//` inside a `/* */` comment is left
/// alone (block comments stay for the lexer).
///
/// Why: the preprocessor follows C and splices `\`-newline before removing
/// comments, so `// note \` would swallow the next line; in MilkDrop the
/// comment, and with it the backslash, was already gone. No preset in
/// `tests/presets/` has such a comment (fixture:
/// `tests/fixtures/line_comment_backslash.hlsl`).
fn strip_line_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut in_block = false;
    let mut copied = 0; // `src[copied..i]` is pending output
    let mut i = 0;
    while i < b.len() {
        let two = |c0: u8, c1: u8| b[i] == c0 && b.get(i + 1) == Some(&c1);
        if in_block {
            if two(b'*', b'/') {
                in_block = false;
                i += 2;
            } else {
                i += 1;
            }
        } else if two(b'/', b'/') {
            out.push_str(&src[copied..i]);
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            copied = i;
        } else if two(b'/', b'*') {
            in_block = true;
            i += 2;
        } else {
            i += 1;
        }
    }
    out.push_str(&src[copied..]);
    out
}

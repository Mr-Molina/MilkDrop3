//! Character cleaning (line continuation, comments) and the tokenizer.
//!
//! Every character keeps the position it had in the original source, so the
//! preprocessor and lexer report original line/col even after comments and
//! `\`-newline joins have been removed.

use crate::token::{TokKind, Token};
use crate::TranslateError;

/// A source character with its original 1-based position.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ch {
    pub c: char,
    pub line: u32,
    pub col: u32,
}

fn err_at(ch: &Ch, msg: impl Into<String>) -> TranslateError {
    TranslateError::new(ch.line, ch.col, msg)
}

/// Apply translation phases 1-3 of the C preprocessor: join `\`-newline
/// continuations, then replace each comment by one space. Newlines that end
/// `//` comments are kept; newlines inside `/* */` comments are dropped with
/// the comment (a block comment is a single space, so it cannot end a
/// directive line).
pub(crate) fn clean(src: &str) -> Result<Vec<Ch>, TranslateError> {
    // Original positions, with "\r\n" folded to "\n" and a lone '\r' to ' '.
    let mut raw: Vec<Ch> = Vec::with_capacity(src.len());
    let (mut line, mut col) = (1u32, 1u32);
    let mut it = src.chars().peekable();
    while let Some(c) = it.next() {
        let (pl, pc) = (line, col);
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
        if c == '\r' {
            if it.peek() == Some(&'\n') {
                continue;
            }
            raw.push(Ch {
                c: ' ',
                line: pl,
                col: pc,
            });
            continue;
        }
        raw.push(Ch {
            c,
            line: pl,
            col: pc,
        });
    }
    // Line continuations.
    let mut joined: Vec<Ch> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i].c == '\\' && raw.get(i + 1).is_some_and(|n| n.c == '\n') {
            i += 2;
        } else {
            joined.push(raw[i]);
            i += 1;
        }
    }
    // Comments.
    let mut out: Vec<Ch> = Vec::with_capacity(joined.len());
    let mut i = 0;
    while i < joined.len() {
        let ch = joined[i];
        let next = joined.get(i + 1).map(|n| n.c);
        if ch.c == '/' && next == Some('/') {
            while i < joined.len() && joined[i].c != '\n' {
                i += 1;
            }
        } else if ch.c == '/' && next == Some('*') {
            let mut j = i + 2;
            loop {
                if j + 1 >= joined.len() {
                    return Err(err_at(&ch, "unterminated /* comment"));
                }
                if joined[j].c == '*' && joined[j + 1].c == '/' {
                    break;
                }
                j += 1;
            }
            out.push(Ch { c: ' ', ..ch });
            i = j + 2;
        } else {
            out.push(ch);
            i += 1;
        }
    }
    Ok(out)
}

const OPS3: [&str; 2] = ["<<=", ">>="];
const OPS2: [&str; 14] = [
    "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "==", "!=", "<=", ">=",
];
const OPS2B: [&str; 4] = ["&&", "||", "<<", ">>"];
const OPS1: &str = "+-*/%&|^~!<>=?:;,.()[]{}";

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn spelled(chars: &[Ch], at: usize, n: usize) -> String {
    chars[at..(at + n).min(chars.len())]
        .iter()
        .map(|c| c.c)
        .collect()
}

/// Lex an already-cleaned character run (no comments, no directives).
pub(crate) fn lex(chars: &[Ch]) -> Result<Vec<Token>, TranslateError> {
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        let c = ch.c;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let (kind, len) = if is_ident_start(c) {
            let mut j = i;
            while j < chars.len() && is_ident_char(chars[j].c) {
                j += 1;
            }
            (TokKind::Ident(spelled(chars, i, j - i)), j - i)
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(|n| n.c.is_ascii_digit()))
        {
            lex_number(chars, i)?
        } else if let Some(op) = match_op(chars, i) {
            (TokKind::Op(op), op.len())
        } else if c == '#' {
            return Err(err_at(
                &ch,
                "unexpected '#' (stringising and token pasting are not supported)",
            ));
        } else {
            return Err(err_at(&ch, format!("unexpected character {c:?}")));
        };
        toks.push(Token {
            kind,
            line: ch.line,
            col: ch.col,
        });
        i += len;
    }
    Ok(toks)
}

fn match_op(chars: &[Ch], i: usize) -> Option<&'static str> {
    let s3 = spelled(chars, i, 3);
    if let Some(op) = OPS3.iter().find(|o| s3 == **o) {
        return Some(op);
    }
    let s2 = spelled(chars, i, 2);
    if let Some(op) = OPS2.iter().chain(OPS2B.iter()).find(|o| s2 == **o) {
        return Some(op);
    }
    let c = chars[i].c;
    OPS1.find(c).map(|p| &OPS1[p..p + 1])
}

fn lex_number(chars: &[Ch], start: usize) -> Result<(TokKind, usize), TranslateError> {
    let at = chars[start];
    let peek = |j: usize| chars.get(j).map(|c| c.c);
    let mut j = start;
    // Hexadecimal integer.
    if peek(j) == Some('0') && matches!(peek(j + 1), Some('x' | 'X')) {
        j += 2;
        let ds = j;
        while peek(j).is_some_and(|c| c.is_ascii_hexdigit()) {
            j += 1;
        }
        let digits = spelled(chars, ds, j - ds);
        if digits.is_empty() {
            return Err(err_at(&at, "invalid hexadecimal literal"));
        }
        let v = i64::from_str_radix(&digits, 16)
            .map_err(|_| err_at(&at, "integer literal out of range"))?;
        j = skip_int_suffix(chars, j);
        check_literal_end(chars, j, &at)?;
        return Ok((TokKind::Int(v), j - start));
    }
    let mut is_float = false;
    while peek(j).is_some_and(|c| c.is_ascii_digit()) {
        j += 1;
    }
    if peek(j) == Some('.') {
        is_float = true;
        j += 1;
        while peek(j).is_some_and(|c| c.is_ascii_digit()) {
            j += 1;
        }
    }
    if matches!(peek(j), Some('e' | 'E')) {
        let mut k = j + 1;
        if matches!(peek(k), Some('+' | '-')) {
            k += 1;
        }
        if peek(k).is_some_and(|c| c.is_ascii_digit()) {
            while peek(k).is_some_and(|c| c.is_ascii_digit()) {
                k += 1;
            }
            is_float = true;
            j = k;
        }
    }
    let text = spelled(chars, start, j - start);
    if is_float {
        let v: f64 = text
            .parse()
            .map_err(|_| err_at(&at, format!("invalid floating literal {text:?}")))?;
        if matches!(peek(j), Some('f' | 'F' | 'h' | 'H')) {
            j += 1;
        }
        check_literal_end(chars, j, &at)?;
        return Ok((TokKind::Float(v), j - start));
    }
    // `1f` / `1h` are floats in HLSL.
    if matches!(peek(j), Some('f' | 'F' | 'h' | 'H')) && !peek(j + 1).is_some_and(is_ident_char) {
        let v: f64 = text.parse().map_err(|_| err_at(&at, "invalid literal"))?;
        return Ok((TokKind::Float(v), j + 1 - start));
    }
    // Leading 0 means octal when every digit allows it (C/HLSL rule).
    let radix = if text.len() > 1 && text.starts_with('0') && text.chars().all(|c| c < '8') {
        8
    } else {
        10
    };
    let v = i64::from_str_radix(&text, radix)
        .map_err(|_| err_at(&at, "integer literal out of range"))?;
    j = skip_int_suffix(chars, j);
    check_literal_end(chars, j, &at)?;
    Ok((TokKind::Int(v), j - start))
}

fn skip_int_suffix(chars: &[Ch], mut j: usize) -> usize {
    while chars
        .get(j)
        .is_some_and(|c| matches!(c.c, 'u' | 'U' | 'l' | 'L'))
    {
        j += 1;
    }
    j
}

fn check_literal_end(chars: &[Ch], j: usize, at: &Ch) -> Result<(), TranslateError> {
    if chars.get(j).is_some_and(|c| is_ident_char(c.c)) {
        return Err(err_at(at, "invalid numeric literal suffix"));
    }
    Ok(())
}

/// Tokenize source text that needs no macro expansion: comments and line
/// continuations are removed; `#` directives are an error (use
/// [`crate::preprocess_tokens`] for text with macros).
pub fn tokenize(src: &str) -> Result<Vec<Token>, TranslateError> {
    lex(&clean(src)?)
}

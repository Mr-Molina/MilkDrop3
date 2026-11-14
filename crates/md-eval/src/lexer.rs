//! Tokenizer for the EEL expression language.
//!
//! Mirrors the character-level rules of ns-eel2's preprocessor and lexer
//! (`nseel-compiler.c` preprocessing, `nseel-lextab.c` DFA): C-locale
//! whitespace, `//` and `/* */` comments (an unterminated block comment runs
//! to the end of input), identifiers `[A-Za-z_][A-Za-z0-9_]*` (a `.` is *not*
//! an identifier character in this ns-eel2 build), numbers
//! `digits[.digits]` / `.digits`, and the `$` forms `$xHEX`, `$'c'`, `$PI`,
//! `$E`, `$PHI`. Exponents (`1e3`) are accepted per the milestone spec even
//! though the ns-eel2 DFA itself would split `1e3` into `1` and `e3`.

use crate::error::CompileError;
use crate::token::Tok;

pub struct Lexer;

const OPS2: [&str; 14] = [
    "+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=", "==", "!=", "<=", ">=", "&&", "||",
];
const OPS1: [&str; 14] = [
    "+", "-", "*", "/", "%", "^", "|", "&", "=", "<", ">", "!", "?", ":",
];

fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

fn hex_val(b: u8) -> Option<u32> {
    (b as char).to_digit(16)
}

impl Lexer {
    /// Tokenize `src`. Each token carries its byte offset; the last token is
    /// always `Tok::Eof` at offset `src.len()`.
    pub fn tokenize(src: &str) -> Result<Vec<(Tok, usize)>, CompileError> {
        let b = src.as_bytes();
        let n = b.len();
        let mut out = Vec::new();
        let mut i = 0;
        while i < n {
            let c = b[i];
            if is_c_space(c) {
                i += 1;
                continue;
            }
            if c == b'/' && i + 1 < n && b[i + 1] == b'/' {
                while i < n && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
                i += 2;
                while i < n && !(b[i] == b'*' && i + 1 < n && b[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(n);
                continue;
            }
            let start = i;
            if c == b'$' {
                let (tok, end) = Self::dollar(b, i)?;
                out.push((tok, start));
                i = end;
                continue;
            }
            if c.is_ascii_digit() || c == b'.' {
                let (v, end) = Self::number(b, i);
                out.push((Tok::Num(v), start));
                i = end;
                continue;
            }
            if c.is_ascii_alphabetic() || c == b'_' {
                while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                out.push((Tok::Ident(src[start..i].to_ascii_lowercase()), start));
                continue;
            }
            let simple = match c {
                b'(' => Some(Tok::LParen),
                b')' => Some(Tok::RParen),
                b'[' => Some(Tok::LBracket),
                b']' => Some(Tok::RBracket),
                b',' => Some(Tok::Comma),
                b';' => Some(Tok::Semi),
                _ => None,
            };
            if let Some(t) = simple {
                out.push((t, start));
                i += 1;
                continue;
            }
            let rest = &src[i..];
            let op = OPS2
                .iter()
                .chain(OPS1.iter())
                .find(|o| rest.starts_with(**o));
            match op {
                Some(o) => {
                    out.push((Tok::Op(o), start));
                    i += o.len();
                }
                None => {
                    let ch = rest.chars().next().unwrap_or('?');
                    return Err(CompileError::new(
                        start,
                        format!("unexpected character {ch:?}"),
                    ));
                }
            }
        }
        out.push((Tok::Eof, n));
        Ok(out)
    }

    /// Decimal literal starting at `i` (a digit or `.`); returns value and end.
    fn number(b: &[u8], mut i: usize) -> (f64, usize) {
        let n = b.len();
        let start = i;
        let mut digits = 0;
        while i < n && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
        if i < n && b[i] == b'.' {
            i += 1;
            while i < n && b[i].is_ascii_digit() {
                i += 1;
                digits += 1;
            }
        }
        // Exponent only after a mantissa digit and when it is complete:
        // e[+-]digits (so `.e3` stays `.` then `e3`).
        if digits > 0 && i < n && (b[i] == b'e' || b[i] == b'E') {
            let mut j = i + 1;
            if j < n && (b[j] == b'+' || b[j] == b'-') {
                j += 1;
            }
            if j < n && b[j].is_ascii_digit() {
                while j < n && b[j].is_ascii_digit() {
                    j += 1;
                }
                i = j;
            }
        }
        // The text is ASCII digits / '.' / exponent, so it is valid UTF-8.
        let text = std::str::from_utf8(&b[start..i]).unwrap_or("0");
        // A lone "." is the number 0 in ns-eel2 (atof("."))
        let v = text.parse::<f64>().unwrap_or_else(|_| {
            // forms like "5." or "." that Rust rejects
            let t = text.trim_end_matches('.');
            if t.is_empty() {
                0.0
            } else {
                t.parse::<f64>().unwrap_or(0.0)
            }
        });
        (v, i)
    }

    /// `$`-prefixed constant at `i`; mirrors the preprocessor in nseel-compiler.c.
    #[allow(clippy::approx_constant)] // ns-eel2 truncates E to 2.71828183
    fn dollar(b: &[u8], i: usize) -> Result<(Tok, usize), CompileError> {
        let n = b.len();
        let at = |k: usize| -> u8 {
            if k < n {
                b[k]
            } else {
                0
            }
        };
        let up = |k: usize| at(k).to_ascii_uppercase();
        if up(i + 1) == b'X' {
            // strtoul(..., 16): optional 0x prefix, hex digits; no digits -> 0.
            let mut j = i + 2;
            if at(j) == b'0' && up(j + 1) == b'X' && hex_val(at(j + 2)).is_some() {
                j += 2;
            }
            let mut v: u64 = 0;
            while let Some(d) = hex_val(at(j)) {
                v = v.saturating_mul(16).saturating_add(u64::from(d));
                j += 1;
            }
            // C stores the strtoul result in an `unsigned int`.
            return Ok((Tok::Num(f64::from(v as u32)), j));
        }
        if at(i + 1) == b'\'' && at(i + 2) != 0 && at(i + 3) == b'\'' {
            return Ok((Tok::Num(f64::from(at(i + 2))), i + 4));
        }
        if up(i + 1) == b'P' && up(i + 2) == b'I' {
            return Ok((Tok::Num(3.141_592_653_589_793), i + 3));
        }
        if up(i + 1) == b'E' {
            return Ok((Tok::Num(2.718_281_83), i + 2));
        }
        if up(i + 1) == b'P' && up(i + 2) == b'H' && up(i + 3) == b'I' {
            return Ok((Tok::Num(1.618_033_99), i + 4));
        }
        Err(CompileError::new(i, "unexpected character '$'"))
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        Lexer::tokenize(src)
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    fn num(src: &str) -> f64 {
        match toks(src).first() {
            Some(Tok::Num(v)) => *v,
            other => panic!("expected number for {src:?}, got {other:?}"),
        }
    }

    #[test]
    fn lexes_numbers_including_hex_and_leading_dot() {
        assert_eq!(num(".5"), 0.5);
        assert_eq!(num("$x1F"), 31.0);
        assert_eq!(num("1e3"), 1000.0);
        assert_eq!(num("1"), 1.0);
        assert_eq!(num("1.5"), 1.5);
        assert_eq!(num("2.5e-1"), 0.25);
        assert_eq!(num("5."), 5.0);
    }

    #[test]
    fn exponent_must_be_complete() {
        // `1e` / `1else` are a number followed by an identifier.
        assert_eq!(
            toks("1e"),
            vec![Tok::Num(1.0), Tok::Ident("e".into()), Tok::Eof]
        );
        assert_eq!(
            toks("2x"),
            vec![Tok::Num(2.0), Tok::Ident("x".into()), Tok::Eof]
        );
    }

    #[test]
    fn exponent_needs_a_mantissa_digit() {
        // `.e3` is the number `.` (0) followed by the identifier `e3`.
        assert_eq!(
            toks(".e3"),
            vec![Tok::Num(0.0), Tok::Ident("e3".into()), Tok::Eof]
        );
        assert_eq!(num(".5e1"), 5.0);
        assert_eq!(num("5.e1"), 50.0);
    }

    #[test]
    fn hex_rules_follow_strtoul() {
        assert_eq!(num("$X1f"), 31.0);
        assert_eq!(num("$x"), 0.0);
        assert_eq!(num("$x0x10"), 16.0);
        assert_eq!(num("$xFFFFFFFF"), 4294967295.0);
        // stored in an unsigned int: wraps modulo 2^32
        assert_eq!(num("$x100000001"), 1.0);
        assert_eq!(
            toks("$x1Fg"),
            vec![Tok::Num(31.0), Tok::Ident("g".into()), Tok::Eof]
        );
    }

    #[test]
    fn named_constants_and_char_literals() {
        assert_eq!(num("$PI"), std::f64::consts::PI);
        assert_eq!(num("$pi"), std::f64::consts::PI);
        assert_eq!(num("$E"), 2.71828183);
        assert_eq!(num("$e"), 2.71828183);
        assert_eq!(num("$PHI"), 1.61803399);
        assert_eq!(num("$phi"), 1.61803399);
        assert_eq!(num("$'A'"), 65.0);
        assert_eq!(num("$' '"), 32.0);
        assert_eq!(num("$'''"), 39.0);
        assert_eq!(
            toks("$PIx"),
            vec![
                Tok::Num(std::f64::consts::PI),
                Tok::Ident("x".into()),
                Tok::Eof
            ]
        );
        assert!(Lexer::tokenize("$").is_err());
        assert!(Lexer::tokenize("$'ab'").is_err());
        assert_eq!(Lexer::tokenize("1 + $q").unwrap_err().offset, 4);
    }

    #[test]
    fn dot_is_not_an_identifier_char() {
        // ns-eel2's DFA splits `a.b`; the lone dot is the number 0.
        assert_eq!(
            toks("a.b"),
            vec![
                Tok::Ident("a".into()),
                Tok::Num(0.0),
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
        assert_eq!(toks("1.2.3"), vec![Tok::Num(1.2), Tok::Num(0.3), Tok::Eof]);
    }

    #[test]
    fn lowercases_identifiers() {
        assert_eq!(
            toks("Foo_Bar9 ZOOM"),
            vec![
                Tok::Ident("foo_bar9".into()),
                Tok::Ident("zoom".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn skips_both_comment_styles() {
        assert_eq!(
            toks("a // x = 1\n/* b ;\n c */ d"),
            vec![Tok::Ident("a".into()), Tok::Ident("d".into()), Tok::Eof]
        );
        // unterminated block comment runs to the end
        assert_eq!(toks("a /* oops"), vec![Tok::Ident("a".into()), Tok::Eof]);
        assert_eq!(toks("a // end"), vec![Tok::Ident("a".into()), Tok::Eof]);
        // `/` alone is division
        assert_eq!(
            toks("a/b"),
            vec![
                Tok::Ident("a".into()),
                Tok::Op("/"),
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn reports_offset_of_bad_char() {
        let e = Lexer::tokenize("a = 1;\n  b = @;").unwrap_err();
        assert_eq!(e.offset, 13);
        let e = Lexer::tokenize("x = {").unwrap_err();
        assert_eq!(e.offset, 4);
        // offset is a byte offset at a char boundary
        let e = Lexer::tokenize("a é").unwrap_err();
        assert_eq!(e.offset, 2);
    }

    #[test]
    fn operators_use_longest_match() {
        let src = "+ - * / % ^ | & = += -= *= /= %= |= &= ^= == != < > <= >= && || ! ? :";
        let want: Vec<&str> = src.split(' ').collect();
        let got: Vec<Tok> = toks(src);
        assert_eq!(got.len(), want.len() + 1);
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(g, &Tok::Op(w));
        }
        // no space needed
        assert_eq!(
            toks("a^=b"),
            vec![
                Tok::Ident("a".into()),
                Tok::Op("^="),
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
        assert_eq!(
            toks("!a!=b"),
            vec![
                Tok::Op("!"),
                Tok::Ident("a".into()),
                Tok::Op("!="),
                Tok::Ident("b".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn brackets_parens_and_separators() {
        assert_eq!(
            toks("a[3](,);"),
            vec![
                Tok::Ident("a".into()),
                Tok::LBracket,
                Tok::Num(3.0),
                Tok::RBracket,
                Tok::LParen,
                Tok::Comma,
                Tok::RParen,
                Tok::Semi,
                Tok::Eof
            ]
        );
    }

    #[test]
    fn offsets_and_eof() {
        let t = Lexer::tokenize("ab = 12").unwrap();
        let offs: Vec<usize> = t.iter().map(|(_, o)| *o).collect();
        assert_eq!(offs, vec![0, 3, 5, 7]);
        assert_eq!(Lexer::tokenize("").unwrap(), vec![(Tok::Eof, 0)]);
        assert_eq!(toks(" \t\r\n"), vec![Tok::Eof]);
    }
}

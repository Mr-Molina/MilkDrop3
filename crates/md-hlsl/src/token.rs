//! Token types shared by the preprocessor, lexer and (Task 4.2) parser.

use std::sync::Arc;

/// Longest identifier or numeric literal the lexer accepts, in characters;
/// longer ones are a `TranslateError`. The corpus and `include.fx` peak at 32
/// (identifier) and 17 (literal). Together with the shared `Arc<str>` text this
/// bounds the memory any one token costs, however often expansion copies it.
pub const MAX_TOKEN_LEN: usize = 256;

/// Token payload.
#[derive(Clone, Debug, PartialEq)]
pub enum TokKind {
    /// Identifier or keyword (keywords are not distinguished here). The text
    /// is shared (`Arc<str>`), so copying a token never copies its text; it is
    /// at most [`MAX_TOKEN_LEN`] characters.
    Ident(Arc<str>),
    /// Integer literal (`1`, `0x1F`, `010` octal, `1u`).
    Int(i64),
    /// Floating literal (`1.0`, `1.f`, `.5`, `1e-3`, `1.0h`); the suffix is dropped.
    Float(f64),
    /// Operator or punctuation, as its source spelling: `+ - * / % & | ^ ~ !
    /// < > = ? : ; , . ( ) [ ] { }`, `++ -- += -= *= /= %= &= |= ^= == != <=
    /// >= && || << >>`, `<<= >>=`.
    Op(&'static str),
}

/// A token with its 1-based position in the original source.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokKind,
    pub line: u32,
    pub col: u32,
}

impl Token {
    /// True if this is the operator `op`.
    pub fn is_op(&self, op: &str) -> bool {
        matches!(&self.kind, TokKind::Op(o) if *o == op)
    }

    /// The identifier text, if this is an identifier.
    pub fn ident(&self) -> Option<&str> {
        match &self.kind {
            TokKind::Ident(s) => Some(s.as_ref()),
            _ => None,
        }
    }
}

//! Token types shared by the preprocessor, lexer and (Task 4.2) parser.

/// Token payload.
#[derive(Clone, Debug, PartialEq)]
pub enum TokKind {
    /// Identifier or keyword (keywords are not distinguished here).
    Ident(String),
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
            TokKind::Ident(s) => Some(s),
            _ => None,
        }
    }
}

//! Lexical tokens of the EEL language.

/// One lexical token. Identifiers are ASCII-lowercased by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Num(f64),
    Ident(String),
    /// Operator text: one of `+ - * / % ^ | & = += -= *= /= %= |= &= ^= == != < > <= >= && || ! ? :`.
    Op(&'static str),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Eof,
}

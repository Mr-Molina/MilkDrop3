//! The translator's error type (A.3).

use std::fmt;

/// A translation failure. `line` and `col` are 1-based and refer to the
/// ORIGINAL source text handed to the preprocessor (tokens produced by a
/// macro expansion report the invocation site).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslateError {
    pub line: u32,
    pub col: u32,
    pub message: String,
}

impl TranslateError {
    pub fn new(line: u32, col: u32, message: impl Into<String>) -> Self {
        TranslateError {
            line,
            col,
            message: message.into(),
        }
    }
}

impl fmt::Display for TranslateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for TranslateError {}

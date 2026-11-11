//! Compile-time error shared by the lexer, parser and compiler.

use std::fmt;

/// An error found while compiling EEL source. `offset` is a byte offset into
/// the source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub offset: usize,
    pub message: String,
}

impl CompileError {
    pub fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            message: message.into(),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for CompileError {}

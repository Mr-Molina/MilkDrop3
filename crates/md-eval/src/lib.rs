//! EEL expression language: lexer, parser, compiler and VM.
#![forbid(unsafe_code)]

pub mod error;
pub mod lexer;
pub mod token;

pub use error::CompileError;
pub use lexer::Lexer;
pub use token::Tok;

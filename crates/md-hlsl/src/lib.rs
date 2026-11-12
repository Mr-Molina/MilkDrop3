//! HLSL subset to WGSL translator, with sampler and uniform conventions.
//!
//! Task 4.1 provides the front end: [`preprocess_tokens`] (macro expansion
//! with original-source positions) and [`tokenize`]. Task 4.2 adds the
//! [`ast`] and [`parse`].
#![forbid(unsafe_code)]

pub mod ast;
mod error;
mod lexer;
mod parser;
mod preprocess;
mod token;
mod version;

pub use error::TranslateError;
pub use lexer::tokenize;
pub use parser::parse;
pub use preprocess::{preprocess, preprocess_tokens};
pub use token::{TokKind, Token, MAX_TOKEN_LEN};
pub use version::PsVersion;

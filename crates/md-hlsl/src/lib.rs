//! HLSL subset to WGSL translator, with sampler and uniform conventions.
//!
//! Task 4.1 provides the front end: [`preprocess_tokens`] (macro expansion
//! with original-source positions) and [`tokenize`].
#![forbid(unsafe_code)]

mod error;
mod lexer;
mod preprocess;
mod token;
mod version;

pub use error::TranslateError;
pub use lexer::tokenize;
pub use preprocess::{preprocess, preprocess_tokens};
pub use token::{TokKind, Token};
pub use version::PsVersion;

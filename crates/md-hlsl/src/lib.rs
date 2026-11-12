//! HLSL subset to WGSL translator, with sampler and uniform conventions.
//!
//! Task 4.1 provides the front end: [`preprocess_tokens`] (macro expansion
//! with original-source positions) and [`tokenize`]. Task 4.2 adds the
//! [`ast`] and [`parse`]. Task 4.3 adds the type checker [`check`], producing
//! a [`typed::TypedModule`] over [`types::Ty`].
#![forbid(unsafe_code)]

pub mod ast;
mod error;
mod intrinsics;
mod lexer;
mod parser;
mod preprocess;
mod token;
mod typeck;
pub mod typed;
pub mod types;
mod version;

pub use error::TranslateError;
pub use intrinsics::{Intrinsic, SigClass};
pub use lexer::tokenize;
pub use parser::parse;
pub use preprocess::{preprocess, preprocess_tokens};
pub use token::{TokKind, Token, MAX_TOKEN_LEN};
pub use typeck::check;
pub use typed::{TypedModule, Warning};
pub use types::Ty;
pub use version::PsVersion;

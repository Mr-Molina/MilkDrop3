//! HLSL subset to WGSL translator, with sampler and uniform conventions.
//!
//! Task 4.1 provides the front end: [`preprocess_tokens`] (macro expansion
//! with original-source positions) and [`tokenize`]. Task 4.2 adds the
//! [`ast`] and [`parse`]. Task 4.3 adds the type checker [`check`], producing
//! a [`typed::TypedModule`] over [`types::Ty`]. Task 4.5 adds
//! [`resolve_bindings`] (sampler slots and `texsize_*` names). Task 4.4 adds
//! the WGSL emitter [`emit`] and the shared uniform [`layout`]. Task 4.6
//! adds the wrapper assembly and the public entry point [`translate`];
//! Task 4.7 the legacy source fixups [`apply_fixups`] it applies to the
//! preset body (including `plugin.cpp::StripComments`).
#![forbid(unsafe_code)]

pub mod ast;
mod bindings;
mod emit;
mod emit_call;
mod emit_expr;
mod emit_place;
mod emit_stmt;
mod error;
mod fixups;
mod intrinsics;
pub mod layout;
mod lexer;
mod parser;
mod preprocess;
mod token;
mod translate;
mod typeck;
pub mod typed;
pub mod types;
mod version;
mod wrap;

pub use bindings::{
    decode_sampler_name, parse_rand_root, resolve_bindings, texsize_root, Bindings, SamplerBinding,
    MAX_SAMPLERS,
};
pub use emit::{emit, ShaderKind};
pub use error::TranslateError;
pub use fixups::apply_fixups;
pub use intrinsics::{Intrinsic, SigClass};
pub use lexer::tokenize;
pub use parser::parse;
pub use preprocess::{preprocess, preprocess_tokens};
pub use token::{TokKind, Token, MAX_TOKEN_LEN};
pub use translate::{parse_wrapped, translate, validate_wgsl, ShaderInput, Translated};
pub use typeck::check;
pub use typed::{TypedModule, Warning};
pub use types::Ty;
pub use version::PsVersion;

//! EEL expression language: lexer, parser, compiler and VM.
#![forbid(unsafe_code)]

pub mod ast;
pub mod error;
pub mod lexer;
pub mod mem;
pub mod parser;
mod preproc;
pub mod token;
pub mod vm;

#[cfg(test)]
mod vm_tests;

pub use ast::{AssignOp, BinOp, Expr, UnOp};
pub use error::CompileError;
pub use lexer::Lexer;
pub use mem::{GlobalMem, MegaBuf};
pub use parser::parse;
pub use token::Tok;
pub use vm::{VarRef, Vm};

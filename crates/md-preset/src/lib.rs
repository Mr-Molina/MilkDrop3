//! The .milk preset model: parser, writer, blending and defaults.
#![forbid(unsafe_code)]

pub mod blend;
pub mod ini;

pub use blend::Blendable;
pub use ini::{strip_code, Ini};

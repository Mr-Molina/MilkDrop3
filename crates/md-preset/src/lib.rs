//! The .milk preset model: parser, writer, blending and defaults.
#![forbid(unsafe_code)]

pub mod blend;
mod defaults;
pub mod gen_shader;
pub mod ini;
mod parse;
pub mod preset;
pub mod shape;
pub mod wave;

pub use blend::Blendable;
pub use gen_shader::{gen_comp_shader, gen_warp_shader, CompShaderParams};
pub use ini::{strip_code, Ini};
pub use parse::{parse_milk, ParseError};
pub use preset::{Preset, PsVersion, MAX_CUSTOM_SHAPES, MAX_CUSTOM_WAVES};
pub use shape::ShapeData;
pub use wave::WaveData;

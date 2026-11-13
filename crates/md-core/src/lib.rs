//! Visualizer core: frame orchestration, preset management, UI state and commands.
#![forbid(unsafe_code)]

pub mod custom;
pub mod error;
pub mod per_frame;
pub mod per_pixel;
pub mod shapes;
pub mod state;
pub mod vars;
pub mod waves;

#[cfg(test)]
mod custom_eval_tests;
#[cfg(test)]
mod per_frame_tests;
#[cfg(test)]
mod state_tests;

pub use custom::{ShapeState, WaveState};
pub use error::{CoreError, ProgramError};
pub use per_frame::{blend_outputs, run_per_frame, FrameInputs};
pub use per_pixel::per_pixel_runner;
pub use shapes::eval_custom_shapes;
pub use state::{InitInputs, InitMode, PresetState};
pub use waves::eval_custom_waves;

//! Visualizer core: frame orchestration, preset management, UI state and commands.
#![forbid(unsafe_code)]

pub mod custom;
pub mod error;
pub mod state;
pub mod vars;

#[cfg(test)]
mod state_tests;

pub use custom::{ShapeState, WaveState};
pub use error::{CoreError, ProgramError};
pub use state::{InitInputs, InitMode, PresetState};

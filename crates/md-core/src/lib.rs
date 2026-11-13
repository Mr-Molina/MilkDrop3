//! Visualizer core: frame orchestration, preset management, UI state and commands.
#![forbid(unsafe_code)]

pub mod commands;
pub mod custom;
pub mod error;
pub mod history;
pub mod keymap;
pub mod library;
pub mod per_frame;
pub mod per_pixel;
pub mod shapes;
pub mod state;
pub mod switcher;
pub mod timing;
pub mod vars;
pub mod waves;

#[cfg(test)]
mod custom_eval_tests;
#[cfg(test)]
mod keymap_tests;
#[cfg(test)]
mod library_tests;
#[cfg(test)]
mod per_frame_tests;
#[cfg(test)]
mod state_tests;
#[cfg(test)]
mod switcher_tests;

pub use commands::{Command, Mods};
pub use custom::{ShapeState, WaveState};
pub use error::{CoreError, ProgramError};
pub use history::{History, PRESET_HIST_LEN};
pub use keymap::{accepts_text_char, lookup, Key, KeyChord, BINDINGS};
pub use library::{Entry, PresetLibrary};
pub use per_frame::{blend_outputs, run_per_frame, FrameInputs};
pub use per_pixel::per_pixel_runner;
pub use shapes::eval_custom_shapes;
pub use state::{InitInputs, InitMode, PresetState};
pub use switcher::{ShaderLoader, Switcher};
pub use timing::{SwitchConfig, SwitchEvent, TimerFrame, Timers};
pub use waves::eval_custom_waves;

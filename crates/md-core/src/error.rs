//! Errors of the visualizer core.

use md_eval::CompileError;

/// A compile error in one expression section of a preset. The state still
/// loads (the section's program is `None`); the UI shows these later, as
/// the C++ shows `AddError(..., ERR_PRESET)` warnings (state.cpp:1674,
/// 1705, 1717, 1739, ...).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramError {
    /// `per_frame_init`, `per_frame`, `per_pixel`, `wave<i>_init`,
    /// `wave<i>_per_frame`, `wave<i>_per_point`, `shape<i>_init` or
    /// `shape<i>_per_frame` (`i` from 0, as in the C++ messages).
    pub section: String,
    pub error: CompileError,
}

impl std::fmt::Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error in {}: {}", self.section, self.error)
    }
}

/// A failure that stops the visualizer core. Compile errors in preset
/// code are not among them (see [`ProgramError`]).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CoreError {
    /// The preset cannot be turned into a state.
    #[error("preset: {0}")]
    Preset(String),
}

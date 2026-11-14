//! The messages the host raises after the file operations the UI asks for
//! (`SavePresetAs`, `DeletePresetFile`, `RenamePresetFile`,
//! plugin.cpp:8156-8263), with the C++ duration, category and boldness, so
//! 7.9 does not have to copy the numbers. Raise one with [`Ui::raise`].

use super::strings;
use super::{ErrCategory, Ui};

/// How a message is raised (`AddError(msg, duration, category, bold)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToastSpec {
    /// The text (for delete, [`strings::preset_deleted`] makes it).
    pub msg: &'static str,
    /// Seconds.
    pub duration: f32,
    /// `ERR_*`.
    pub category: ErrCategory,
    /// `bBold`.
    pub bold: bool,
}

const fn spec(msg: &'static str, duration: f32, category: ErrCategory, bold: bool) -> ToastSpec {
    ToastSpec {
        msg,
        duration,
        category,
        bold,
    }
}

/// plugin.cpp:8165.
pub const SAVE_FAILED: ToastSpec = spec(
    strings::ERROR_UNABLE_TO_SAVE,
    6.0,
    ErrCategory::Preset,
    true,
);
/// plugin.cpp:8170.
pub const SAVE_OK: ToastSpec = spec(strings::SAVE_SUCCESSFUL, 3.0, ErrCategory::Notify, false);
/// plugin.cpp:8189.
pub const DELETE_FAILED: ToastSpec = spec(
    strings::ERROR_UNABLE_TO_DELETE,
    6.0,
    ErrCategory::Misc,
    true,
);
/// plugin.cpp:8196; the text is [`strings::preset_deleted`].
pub const DELETE_OK: ToastSpec = spec("", 3.0, ErrCategory::Notify, false);
/// plugin.cpp:8213.
pub const RENAME_EXISTS: ToastSpec =
    spec(strings::ERROR_FILE_EXISTS, 6.0, ErrCategory::Preset, true);
/// plugin.cpp:8223.
pub const RENAME_FAILED: ToastSpec = spec(
    strings::ERROR_UNABLE_TO_RENAME,
    6.0,
    ErrCategory::Misc,
    true,
);
/// plugin.cpp:8228.
pub const RENAME_OK: ToastSpec = spec(strings::RENAME_SUCCESSFUL, 3.0, ErrCategory::Notify, false);

impl Ui {
    /// `AddError` with a [`ToastSpec`]; `msg` replaces the spec's text when
    /// given (the delete message).
    pub fn raise(&mut self, s: ToastSpec, msg: Option<&str>, now: f32) {
        self.add_error(msg.unwrap_or(s.msg), s.duration, s.category, s.bold, now);
    }
}

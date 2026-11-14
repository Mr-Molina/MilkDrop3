//! Typed ids for the values the UI edits, and the values themselves.
//!
//! The C++ menu item holds a byte offset into `CState` (`m_var_offset`,
//! menu.h:69) and reads and writes through it. [`FieldId`] is the Rust
//! spelling: the menu asks the host for the current value
//! ([`FieldId::get`]) and asks for a new one with
//! [`UiAction::SetValue`](super::UiAction::SetValue); the visualizer applies
//! it with [`FieldId::set`] (Task 7.9).

use super::field_tables::{PresetField, ShapeField, WaveField};
use md_preset::Preset;

/// A value a menu reads or writes.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    /// A `bool` field (`MENUITEMTYPE_BOOL`).
    Bool(bool),
    /// An `int` field (`MENUITEMTYPE_INT`, and the integer flags of the
    /// custom waves and shapes).
    Int(i32),
    /// A `float` or `CBlendableFloat` field (its target value).
    Float(f32),
    /// A code string (`MENUITEMTYPE_STRING`), with `\n` as its line break.
    Text(String),
}

impl FieldValue {
    /// The number, as the menu's `float` arithmetic sees it.
    pub fn as_f32(&self) -> f32 {
        match self {
            FieldValue::Bool(b) => f32::from(u8::from(*b)),
            FieldValue::Int(i) => *i as f32,
            FieldValue::Float(f) => *f,
            FieldValue::Text(_) => 0.0,
        }
    }

    /// The number as an `int` (a float truncates, as the C++ cast does).
    pub fn as_i32(&self) -> i32 {
        match self {
            FieldValue::Int(i) => *i,
            other => other.as_f32() as i32,
        }
    }

    /// Non-zero is true.
    pub fn as_bool(&self) -> bool {
        match self {
            FieldValue::Bool(b) => *b,
            other => other.as_f32() != 0.0,
        }
    }

    /// The text (empty for a number).
    pub fn as_text(&self) -> String {
        match self {
            FieldValue::Text(t) => t.clone(),
            _ => String::new(),
        }
    }
}

/// Which preset value a menu item edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldId {
    /// A preset-level field.
    Preset(PresetField),
    /// A field of custom wave `slot` (`0..4`).
    Wave(u8, WaveField),
    /// A field of custom shape `slot` (`0..4`).
    Shape(u8, ShapeField),
}

impl FieldId {
    /// The current value in `p`. A slot outside the four custom waves or
    /// shapes reads as `Float(0)`.
    pub fn get(self, p: &Preset) -> FieldValue {
        match self {
            FieldId::Preset(f) => f.get(p),
            FieldId::Wave(i, f) if usize::from(i) < p.waves.len() => f.get(p, usize::from(i)),
            FieldId::Shape(i, f) if usize::from(i) < p.shapes.len() => f.get(p, usize::from(i)),
            _ => FieldValue::Float(0.0),
        }
    }

    /// Store `v` in `p` (the visualizer's side of `UiAction::SetValue`).
    pub fn set(self, p: &mut Preset, v: &FieldValue) {
        match self {
            FieldId::Preset(f) => f.set(p, v),
            FieldId::Wave(i, f) if usize::from(i) < p.waves.len() => f.set(p, usize::from(i), v),
            FieldId::Shape(i, f) if usize::from(i) < p.shapes.len() => f.set(p, usize::from(i), v),
            _ => {}
        }
    }

    /// Whether this is a code string.
    pub fn is_text(self) -> bool {
        match self {
            FieldId::Preset(f) => f.is_text(),
            FieldId::Wave(_, f) => f.is_text(),
            FieldId::Shape(_, f) => f.is_text(),
        }
    }
}

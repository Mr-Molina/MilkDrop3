//! The menu tree as data: a declarative mirror of `CPlugin::BuildMenus`
//! (plugin.cpp:6661-6871). Every `AddChildMenu` and `AddItem` call is one
//! row below, in the C++ order, with its label, type, target, `min`/`max`
//! and tooltip (the `.rc` string, [`super::menu_tips`]).
//!
//! The C++ item records a byte offset into `CState` (`m_var_offset`); here a
//! [`Target`] names the field ([`super::field`]). The callback the C++ item
//! stores (`OnUserEdited*`, plugin.cpp:786-850) is [`Recompile`]: what the
//! visualizer redoes when the text is accepted. The `wParam` of a string item
//! (the buffer size, `MAX_BIGSTRING_LEN` = 32768) is `max_len` of the
//! waitstring it opens; `lParam` of the wave/shape import and export items
//! (the slot) is the index of the custom wave or shape menu.
//!
//! Counts, from the C++ calls: preset menu 7 items and 6 sub-menus; motion
//! 12; custom shapes and custom waves 0 items and 4 sub-menus each; simple
//! waveform 17; borders and motion vectors 19; post-processing 19 (the
//! `stereo 3D` call at plugin.cpp:6802 is commented out); each custom wave
//! 18; each custom shape 28. 15 menus and 258 items.

use super::field_tables::{PresetField, ShapeField, WaveField};
use super::menu_tips as tips;
use super::mode::UiMode;

/// `MENUITEMTYPE_*` (menu.h:39-49), the edited-value kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// `MENUITEMTYPE_BOOL`: Enter toggles.
    Bool,
    /// `MENUITEMTYPE_INT`: edited by `min..=max`.
    Int,
    /// `MENUITEMTYPE_FLOAT`: edited by `(max-min) * 0.01` steps.
    Float,
    /// `MENUITEMTYPE_LOGFLOAT`: edited by factors of 1.01.
    LogFloat,
    /// `MENUITEMTYPE_BLENDABLE`: a [`Float`](Self::Float) on a blendable.
    Blendable,
    /// `MENUITEMTYPE_LOGBLENDABLE`: a [`LogFloat`](Self::LogFloat) on a blendable.
    LogBlendable,
    /// `MENUITEMTYPE_STRING`: opens the string editor (code).
    Text,
    /// `MENUITEMTYPE_UIMODE`: switches the UI mode.
    Mode,
}

/// The field an item edits; for the wave and shape menus the slot comes from
/// the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// `NULL` / the `UI_*` constant stored as the address of a mode item.
    None,
    /// A field of the preset (`m_pState->m_x`).
    Preset(PresetField),
    /// A field of custom wave `i` (`m_pState->m_wave[i].x`).
    Wave(WaveField),
    /// A field of custom shape `i` (`m_pState->m_shape[i].x`).
    Shape(ShapeField),
}

/// What the C++ callback of a string item recompiles
/// (`CState::RecompileExpressions` flags and the shader callbacks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recompile {
    /// No callback (`NULL`).
    None,
    /// `OnUserEditedPresetInit`: preset code, running the init code
    /// (`RecompileExpressions(RECOMPILE_PRESET_CODE, 1)`).
    PresetInit,
    /// `OnUserEditedPerFrame` / `OnUserEditedPerPixel` (flag 0).
    PresetCode,
    /// `OnUserEditedWavecodeInit` (flag 1).
    WaveInit,
    /// `OnUserEditedWavecode` (flag 0).
    WaveCode,
    /// `OnUserEditedShapecodeInit` (flag 1).
    ShapeInit,
    /// `OnUserEditedShapecode` (flag 0).
    ShapeCode,
    /// `OnUserEditedWarpShaders`.
    WarpShader,
    /// `OnUserEditedCompShaders`.
    CompShader,
}

/// One `AddItem` call.
#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    /// `szName` (64 chars max in the C++).
    pub label: &'static str,
    /// `type`.
    pub kind: ItemKind,
    /// The edited field.
    pub target: Target,
    /// `min`: the lower bound; for a text item the buffer size hint (256,
    /// unused by the C++ handlers) is not kept.
    pub min: f32,
    /// `max`.
    pub max: f32,
    /// `szToolTip`.
    pub tooltip: &'static str,
    /// The callback of a text item.
    pub on_edit: Recompile,
    /// The mode a [`ItemKind::Mode`] item enters (`wParam`).
    pub mode: Option<UiMode>,
    /// `wParam` of a text item: the longest string accepted.
    pub max_len: usize,
}

/// `MAX_BIGSTRING_LEN` (state.h:52): the size of every code buffer.
pub const MAX_BIGSTRING_LEN: usize = 32768;

impl ItemDef {
    pub(super) const fn base(
        label: &'static str,
        kind: ItemKind,
        target: Target,
        min: f32,
        max: f32,
        tooltip: &'static str,
    ) -> ItemDef {
        // AddItem: a log item with min == max gets the "special defaults"
        // (menu.cpp:228-233).
        let log = matches!(kind, ItemKind::LogFloat | ItemKind::LogBlendable);
        let (min, max) = if log && min == max {
            (0.01, 100.0)
        } else {
            (min, max)
        };
        ItemDef {
            label,
            kind,
            target,
            min,
            max,
            tooltip,
            on_edit: Recompile::None,
            mode: None,
            max_len: 0,
        }
    }
    pub(super) const fn boolean(label: &'static str, target: Target, tip: &'static str) -> ItemDef {
        ItemDef::base(label, ItemKind::Bool, target, 0.0, 0.0, tip)
    }
    pub(super) const fn int(
        label: &'static str,
        target: Target,
        min: f32,
        max: f32,
        tip: &'static str,
    ) -> ItemDef {
        ItemDef::base(label, ItemKind::Int, target, min, max, tip)
    }
    pub(super) const fn float(
        label: &'static str,
        target: Target,
        min: f32,
        max: f32,
        tip: &'static str,
    ) -> ItemDef {
        ItemDef::base(label, ItemKind::Float, target, min, max, tip)
    }
    pub(super) const fn log_float(
        label: &'static str,
        target: Target,
        min: f32,
        max: f32,
        tip: &'static str,
    ) -> ItemDef {
        ItemDef::base(label, ItemKind::LogFloat, target, min, max, tip)
    }
    pub(super) const fn blendable(
        label: &'static str,
        target: Target,
        min: f32,
        max: f32,
        tip: &'static str,
    ) -> ItemDef {
        ItemDef::base(label, ItemKind::Blendable, target, min, max, tip)
    }
    pub(super) const fn log_blendable(
        label: &'static str,
        target: Target,
        min: f32,
        max: f32,
        tip: &'static str,
    ) -> ItemDef {
        ItemDef::base(label, ItemKind::LogBlendable, target, min, max, tip)
    }
    pub(super) const fn text(
        label: &'static str,
        target: Target,
        on_edit: Recompile,
        tip: &'static str,
    ) -> ItemDef {
        let mut d = ItemDef::base(label, ItemKind::Text, target, 0.0, 0.0, tip);
        d.on_edit = on_edit;
        d.max_len = MAX_BIGSTRING_LEN;
        d
    }
    pub(super) const fn mode(label: &'static str, mode: UiMode, tip: &'static str) -> ItemDef {
        let mut d = ItemDef::base(label, ItemKind::Mode, Target::None, 0.0, 0.0, tip);
        d.mode = Some(mode);
        d
    }
}

/// `NUM_WAVES` (md_defines.h:51): the simple waveform modes, `0..NUM_WAVES`.
const NUM_WAVES: u32 = 8;

/// A shorthand for the table rows.
use PresetField as Pf;
use Target::Preset as Pr;

/// `m_menuPreset` items (plugin.cpp:6702-6735).
#[rustfmt::skip]
pub const PRESET_ITEMS: [ItemDef; 7] = [
    ItemDef::text("[ edit preset initialization code ]", Pr(Pf::PerFrameInit), Recompile::PresetInit, tips::EDIT_PRESET_INIT_CODE_TT),
    ItemDef::text("[ edit per_frame equations ]", Pr(Pf::PerFrame), Recompile::PresetCode, tips::EDIT_PER_FRAME_EQUATIONS_TT),
    ItemDef::text("[ edit per_vertex equations ]", Pr(Pf::PerPixel), Recompile::PresetCode, tips::EDIT_PER_VERTEX_EQUATIONS_TT),
    ItemDef::text("[ edit warp shader ]", Pr(Pf::WarpShader), Recompile::WarpShader, tips::EDIT_WARP_SHADER_TT),
    ItemDef::text("[ edit composite shader ]", Pr(Pf::CompShader), Recompile::CompShader, tips::EDIT_COMPOSITE_SHADER_TT),
    ItemDef::mode("upgrade preset's pixel shader version", UiMode::UpgradePixelShader, tips::EDIT_UPGRADE_PRESET_PS_VERSION_TT),
    ItemDef::mode("do a preset mash-up", UiMode::Mashup, tips::EDIT_DO_A_PRESET_MASH_UP_TT),
];

/// `m_menuMotion` items (plugin.cpp:6781-6792).
#[rustfmt::skip]
pub const MOTION_ITEMS: [ItemDef; 12] = [
    ItemDef::log_blendable("zoom amount", Pr(Pf::Zoom), 0.0, 0.0, tips::ZOOM_AMOUNT_TT),
    ItemDef::log_blendable(" zoom exponent", Pr(Pf::ZoomExponent), 0.0, 0.0, tips::ZOOM_EXPONENT_TT),
    ItemDef::log_blendable("warp amount", Pr(Pf::Warp), 0.0, 0.0, tips::WARP_AMOUNT_TT),
    ItemDef::log_blendable(" warp scale", Pr(Pf::WarpScale), 0.0, 0.0, tips::WARP_SCALE_TT),
    ItemDef::log_float(" warp speed", Pr(Pf::WarpAnimSpeed), 0.0, 0.0, tips::WARP_SPEED_TT),
    ItemDef::blendable("rotation amount", Pr(Pf::Rot), -1.00, 1.00, tips::ROTATION_AMOUNT_TT),
    ItemDef::blendable(" rot., center of (X)", Pr(Pf::Cx), -1.0, 2.0, tips::ROTATION_CENTER_OF_X_TT),
    ItemDef::blendable(" rot., center of (Y)", Pr(Pf::Cy), -1.0, 2.0, tips::ROTATION_CENTER_OF_Y_TT),
    ItemDef::blendable("translation (X)", Pr(Pf::Dx), -1.0, 1.0, tips::TRANSLATION_X_TT),
    ItemDef::blendable("translation (Y)", Pr(Pf::Dy), -1.0, 1.0, tips::TRANSLATION_Y_TT),
    ItemDef::log_blendable("scaling (X)", Pr(Pf::Sx), 0.0, 0.0, tips::SCALING_X_TT),
    ItemDef::log_blendable("scaling (Y)", Pr(Pf::Sy), 0.0, 0.0, tips::SCALING_Y_TT),
];

/// `m_menuWave` items (plugin.cpp:6743-6759).
#[rustfmt::skip]
pub const WAVE_ITEMS: [ItemDef; 17] = [
    ItemDef::int("wave type", Pr(Pf::WaveMode), 0.0, NUM_WAVES as f32 - 1.0, tips::WAVE_TYPE_TT),
    ItemDef::log_blendable("size", Pr(Pf::WaveScale), 0.0, 0.0, tips::SIZE_TT),
    ItemDef::blendable("smoothing", Pr(Pf::WaveSmoothing), 0.0, 0.9, tips::SMOOTH_TT),
    ItemDef::blendable("mystery parameter", Pr(Pf::WaveParam), -1.0, 1.0, tips::MYSTERY_PARAMETER_TT),
    ItemDef::blendable("position (X)", Pr(Pf::WaveX), 0.0, 1.0, tips::POSITION_X_TT),
    ItemDef::blendable("position (Y)", Pr(Pf::WaveY), 0.0, 1.0, tips::POSITION_Y_TT),
    ItemDef::blendable("color (red)", Pr(Pf::WaveR), 0.0, 1.0, tips::COLOR_RED_TT),
    ItemDef::blendable("color (green)", Pr(Pf::WaveG), 0.0, 1.0, tips::COLOR_GREEN_TT),
    ItemDef::blendable("color (blue)", Pr(Pf::WaveB), 0.0, 1.0, tips::COLOR_BLUE_TT),
    ItemDef::log_blendable("opacity", Pr(Pf::WaveAlpha), 0.001, 100.0, tips::OPACITY_TT),
    ItemDef::boolean("use dots", Pr(Pf::WaveDots), tips::USE_DOTS_TT),
    ItemDef::boolean("draw thick", Pr(Pf::WaveThick), tips::DRAW_THICK_TT),
    ItemDef::boolean("modulate opacity by volume", Pr(Pf::ModWaveAlphaByVolume), tips::MODULATE_OPACITY_BY_VOLUME_TT),
    ItemDef::blendable("modulation: transparent volume", Pr(Pf::ModWaveAlphaStart), 0.0, 2.0, tips::MODULATION_TRANSPARENT_VOLUME_TT),
    ItemDef::blendable("modulation: opaque volume", Pr(Pf::ModWaveAlphaEnd), 0.0, 2.0, tips::MODULATION_OPAQUE_VOLUME_TT),
    ItemDef::boolean("additive drawing", Pr(Pf::AdditiveWaves), tips::ADDITIVE_DRAWING_TT),
    ItemDef::boolean("color brightening", Pr(Pf::MaximizeWaveColor), tips::COLOR_BRIGHTENING_TT),
];

/// `m_menuAugment` items (plugin.cpp:6761-6779).
#[rustfmt::skip]
pub const AUGMENT_ITEMS: [ItemDef; 19] = [
    ItemDef::blendable("outer border thickness", Pr(Pf::ObSize), 0.0, 0.5, tips::OUTER_BORDER_THICKNESS_TT),
    ItemDef::blendable(" color (red)", Pr(Pf::ObR), 0.0, 1.0, tips::COLOR_RED_OUTER_TT),
    ItemDef::blendable(" color (green)", Pr(Pf::ObG), 0.0, 1.0, tips::COLOR_GREEN_OUTER_TT),
    ItemDef::blendable(" color (blue)", Pr(Pf::ObB), 0.0, 1.0, tips::COLOR_BLUE_OUTER_TT),
    ItemDef::blendable(" opacity", Pr(Pf::ObA), 0.0, 1.0, tips::OPACITY_OUTER_TT),
    ItemDef::blendable("inner border thickness", Pr(Pf::IbSize), 0.0, 0.5, tips::INNER_BORDER_THICKNESS_TT),
    ItemDef::blendable(" color (red)", Pr(Pf::IbR), 0.0, 1.0, tips::COLOR_RED_INNER_TT),
    ItemDef::blendable(" color (green)", Pr(Pf::IbG), 0.0, 1.0, tips::COLOR_GREEN_INNER_TT),
    ItemDef::blendable(" color (blue)", Pr(Pf::IbB), 0.0, 1.0, tips::COLOR_BLUE_INNER_TT),
    ItemDef::blendable(" opacity", Pr(Pf::IbA), 0.0, 1.0, tips::OPACITY_INNER_TT),
    ItemDef::blendable("motion vector opacity", Pr(Pf::MvA), 0.0, 1.0, tips::MOTION_VECTOR_OPACITY_TT),
    ItemDef::blendable(" num. mot. vectors (X)", Pr(Pf::MvX), 0.0, 64.0, tips::NUM_MOT_VECTORS_X_TT),
    ItemDef::blendable(" num. mot. vectors (Y)", Pr(Pf::MvY), 0.0, 48.0, tips::NUM_MOT_VECTORS_Y_TT),
    ItemDef::blendable(" offset (X)", Pr(Pf::MvDx), -1.0, 1.0, tips::OFFSET_X_TT),
    ItemDef::blendable(" offset (Y)", Pr(Pf::MvDy), -1.0, 1.0, tips::OFFSET_Y_TT),
    ItemDef::blendable(" trail length", Pr(Pf::MvL), 0.0, 5.0, tips::TRAIL_LENGTH_TT),
    ItemDef::blendable(" color (red)", Pr(Pf::MvR), 0.0, 1.0, tips::COLOR_RED_MOTION_VECTOR_TT),
    ItemDef::blendable(" color (green)", Pr(Pf::MvG), 0.0, 1.0, tips::COLOR_GREEN_MOTION_VECTOR_TT),
    ItemDef::blendable(" color (blue)", Pr(Pf::MvB), 0.0, 1.0, tips::COLOR_BLUE_MOTION_VECTOR_TT),
];

/// `m_menuPost` items (plugin.cpp:6794-6813).
#[rustfmt::skip]
pub const POST_ITEMS: [ItemDef; 19] = [
    ItemDef::blendable("sustain level", Pr(Pf::Decay), 0.50, 1.0, tips::SUSTAIN_LEVEL_TT),
    ItemDef::boolean("darken center", Pr(Pf::DarkenCenter), tips::DARKEN_CENTER_TT),
    ItemDef::blendable("gamma adjustment", Pr(Pf::GammaAdj), 1.0, 8.0, tips::GAMMA_ADJUSTMENT_TT),
    ItemDef::blendable("hue shader", Pr(Pf::Shader), 0.0, 1.0, tips::HUE_SHADER_TT),
    ItemDef::blendable("video echo: alpha", Pr(Pf::VideoEchoAlpha), 0.0, 1.0, tips::VIDEO_ECHO_ALPHA_TT),
    ItemDef::log_blendable(" video echo: zoom", Pr(Pf::VideoEchoZoom), 0.0, 0.0, tips::VIDEO_ECHO_ZOOM_TT),
    ItemDef::int(" video echo: orientation", Pr(Pf::VideoEchoOrientation), 0.0, 3.0, tips::VIDEO_ECHO_ORIENTATION_TT),
    ItemDef::boolean("texture wrap", Pr(Pf::TexWrap), tips::TEXTURE_WRAP_TT),
    ItemDef::boolean("filter: invert", Pr(Pf::Invert), tips::FILTER_INVERT_TT),
    ItemDef::boolean("filter: brighten", Pr(Pf::Brighten), tips::FILTER_BRIGHTEN_TT),
    ItemDef::boolean("filter: darken", Pr(Pf::Darken), tips::FILTER_DARKEN_TT),
    ItemDef::boolean("filter: solarize", Pr(Pf::Solarize), tips::FILTER_SOLARIZE_TT),
    ItemDef::float("blur1: edge darken amount", Pr(Pf::Blur1EdgeDarken), 0.0, 1.0, tips::BLUR1_EDGE_DARKEN_AMOUNT_TT),
    ItemDef::float("blur1: min color value", Pr(Pf::Blur1Min), 0.0, 1.0, tips::BLUR1_MIN_COLOR_VALUE_TT),
    ItemDef::float("blur1: max color value", Pr(Pf::Blur1Max), 0.0, 1.0, tips::BLUR1_MAX_COLOR_VALUE_TT),
    ItemDef::float("blur2: min color value", Pr(Pf::Blur2Min), 0.0, 1.0, tips::BLUR2_MIN_COLOR_VALUE_TT),
    ItemDef::float("blur2: max color value", Pr(Pf::Blur2Max), 0.0, 1.0, tips::BLUR2_MAX_COLOR_VALUE_TT),
    ItemDef::float("blur3: min color value", Pr(Pf::Blur3Min), 0.0, 1.0, tips::BLUR3_MIN_COLOR_VALUE_TT),
    ItemDef::float("blur3: max color value", Pr(Pf::Blur3Max), 0.0, 1.0, tips::BLUR3_MAX_COLOR_VALUE_TT),
];

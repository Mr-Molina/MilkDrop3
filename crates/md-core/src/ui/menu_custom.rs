//! The rows of the custom wave and custom shape menus: the `AddItem` calls
//! inside the `for (i=0; i<MAX_CUSTOM_WAVES; i++)` and `MAX_CUSTOM_SHAPES`
//! loops of `CPlugin::BuildMenus` (plugin.cpp:6815-6870). The same rows
//! serve every slot; the slot is the `lParam` / loop index `i`.

use super::field_tables::{ShapeField, WaveField};
use super::menu_tips as tips;
use super::menu_tree::{ItemDef, Recompile, Target};
use super::mode::UiMode;
use ShapeField as Sf;
use Target::{Shape as Sh, Wave as Wv};
use WaveField as Wf;

/// Items of each `m_menuWavecode[i]` (plugin.cpp:6818-6835).
#[rustfmt::skip]
pub const WAVECODE_ITEMS: [ItemDef; 18] = [
    ItemDef::boolean("enabled", Wv(Wf::Enabled), tips::ENABLED_TT),
    ItemDef::int("number of samples", Wv(Wf::Samples), 2.0, 512.0, tips::NUMBER_OF_SAMPLES_TT),
    ItemDef::int("L/R separation", Wv(Wf::Sep), 0.0, 256.0, tips::L_R_SEPARATION_TT),
    ItemDef::log_float("scaling", Wv(Wf::Scaling), 0.0, 0.0, tips::SCALING_TT),
    ItemDef::float("smoothing", Wv(Wf::Smoothing), 0.0, 1.0, tips::SMOOTHING_TT),
    ItemDef::float("color (red)", Wv(Wf::R), 0.0, 1.0, tips::COLOR_RED_TT),
    ItemDef::float("color (green)", Wv(Wf::G), 0.0, 1.0, tips::COLOR_GREEN_TT),
    ItemDef::float("color (blue)", Wv(Wf::B), 0.0, 1.0, tips::COLOR_BLUE_TT),
    ItemDef::float("opacity", Wv(Wf::A), 0.0, 1.0, tips::OPACITY_WAVE_TT),
    ItemDef::boolean("use spectrum", Wv(Wf::Spectrum), tips::USE_SPECTRUM_TT),
    ItemDef::boolean("use dots", Wv(Wf::UseDots), tips::USE_DOTS_WAVE_TT),
    ItemDef::boolean("draw thick", Wv(Wf::DrawThick), tips::DRAW_THICK_WAVE_TT),
    ItemDef::boolean("additive drawing", Wv(Wf::Additive), tips::ADDITIVE_DRAWING_WAVE_TT),
    ItemDef::mode("--export to file", UiMode::ExportWave, tips::EXPORT_TO_FILE_TT),
    ItemDef::mode("--import from file", UiMode::ImportWave, tips::IMPORT_FROM_FILE_TT),
    ItemDef::text("[ edit initialization code ]", Wv(Wf::Init), Recompile::WaveInit, tips::EDIT_INIT_CODE_TT),
    ItemDef::text("[ edit per-frame code ]", Wv(Wf::PerFrame), Recompile::WaveCode, tips::EDIT_PER_FRAME_CODE_TT),
    ItemDef::text("[ edit per-point code ]", Wv(Wf::PerPoint), Recompile::WaveCode, tips::EDIT_PER_POINT_CODE_TT),
];

/// Items of each `m_menuShapecode[i]` (plugin.cpp:6841-6868).
#[rustfmt::skip]
pub const SHAPECODE_ITEMS: [ItemDef; 28] = [
    ItemDef::boolean("enabled", Sh(Sf::Enabled), tips::ENABLED_SHAPE_TT),
    ItemDef::int("number of instances", Sh(Sf::Instances), 1.0, 1024.0, tips::NUMBER_OF_INSTANCES_TT),
    ItemDef::int("number of sides", Sh(Sf::Sides), 3.0, 100.0, tips::NUMBER_OF_SIDES_TT),
    ItemDef::boolean("draw thick", Sh(Sf::ThickOutline), tips::DRAW_THICK_SHAPE_TT),
    ItemDef::boolean("additive drawing", Sh(Sf::Additive), tips::ADDITIVE_DRAWING_SHAPE_TT),
    ItemDef::float("x position", Sh(Sf::X), 0.0, 1.0, tips::X_POSITION_TT),
    ItemDef::float("y position", Sh(Sf::Y), 0.0, 1.0, tips::Y_POSITION_TT),
    ItemDef::log_float("radius", Sh(Sf::Rad), 0.0, 0.0, tips::RADIUS_TT),
    ItemDef::float("angle", Sh(Sf::Ang), 0.0, std::f32::consts::TAU, tips::ANGLE_TT),
    ItemDef::boolean("textured", Sh(Sf::Textured), tips::TEXTURED_TT),
    ItemDef::log_float("texture zoom", Sh(Sf::TexZoom), 0.0, 0.0, tips::TEXTURE_ZOOM_TT),
    ItemDef::float("texture angle", Sh(Sf::TexAng), 0.0, std::f32::consts::TAU, tips::TEXTURE_ANGLE_TT),
    ItemDef::float("inner color (red)", Sh(Sf::R), 0.0, 1.0, tips::INNER_COLOR_RED_TT),
    ItemDef::float("inner color (green)", Sh(Sf::G), 0.0, 1.0, tips::INNER_COLOR_GREEN_TT),
    ItemDef::float("inner color (blue)", Sh(Sf::B), 0.0, 1.0, tips::INNER_COLOR_BLUE_TT),
    ItemDef::float("inner opacity", Sh(Sf::A), 0.0, 1.0, tips::INNER_OPACITY_TT),
    ItemDef::float("outer color (red)", Sh(Sf::R2), 0.0, 1.0, tips::OUTER_COLOR_RED_TT),
    ItemDef::float("outer color (green)", Sh(Sf::G2), 0.0, 1.0, tips::OUTER_COLOR_GREEN_TT),
    ItemDef::float("outer color (blue)", Sh(Sf::B2), 0.0, 1.0, tips::OUTER_COLOR_BLUE_TT),
    ItemDef::float("outer opacity", Sh(Sf::A2), 0.0, 1.0, tips::OUTER_OPACITY_TT),
    ItemDef::float("border color (red)", Sh(Sf::BorderR), 0.0, 1.0, tips::BORDER_COLOR_RED_TT),
    ItemDef::float("border color (green)", Sh(Sf::BorderG), 0.0, 1.0, tips::BORDER_COLOR_GREEN_TT),
    ItemDef::float("border color (blue)", Sh(Sf::BorderB), 0.0, 1.0, tips::BORDER_COLOR_BLUE_TT),
    ItemDef::float("border opacity", Sh(Sf::BorderA), 0.0, 1.0, tips::BORDER_OPACITY_TT),
    ItemDef::mode("--export to file", UiMode::ExportShape, tips::EXPORT_TO_FILE_SHAPE_TT),
    ItemDef::mode("--import from file", UiMode::ImportShape, tips::IMPORT_FROM_FILE_SHAPE_TT),
    ItemDef::text("[ edit initialization code ]", Sh(Sf::Init), Recompile::ShapeInit, tips::EDIT_INIT_CODE_SHAPE_TT),
    ItemDef::text("[ edit per-frame(/per-instance) code ]", Sh(Sf::PerFrame), Recompile::ShapeCode, tips::EDIT_PER_FRAME_INSTANCE_CODE_TT),
];

//! Custom shapes (`CShape`, state.h). Four per preset (`MAX_CUSTOM_SHAPES`).

use crate::ini::Ini;

/// One custom shape: the `shapecode_<i>_*` scalars and its two code blocks.
///
/// Integer fields stay `i32` because `CShape` stores them as `int`.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeData {
    /// `shapecode_<i>_enabled`.
    pub enabled: i32,
    /// `shapecode_<i>_sides`.
    pub sides: i32,
    /// `shapecode_<i>_additive`.
    pub additive: i32,
    /// `shapecode_<i>_thickOutline`.
    pub thick_outline: i32,
    /// `shapecode_<i>_textured`.
    pub textured: i32,
    /// `shapecode_<i>_num_inst` (`CShape::instances`).
    pub instances: i32,
    /// `shapecode_<i>_x`.
    pub x: f32,
    /// `shapecode_<i>_y`.
    pub y: f32,
    /// `shapecode_<i>_rad`.
    pub rad: f32,
    /// `shapecode_<i>_ang`.
    pub ang: f32,
    /// `shapecode_<i>_tex_ang`.
    pub tex_ang: f32,
    /// `shapecode_<i>_tex_zoom`.
    pub tex_zoom: f32,
    /// `shapecode_<i>_r`.
    pub r: f32,
    /// `shapecode_<i>_g`.
    pub g: f32,
    /// `shapecode_<i>_b`.
    pub b: f32,
    /// `shapecode_<i>_a`.
    pub a: f32,
    /// `shapecode_<i>_r2`.
    pub r2: f32,
    /// `shapecode_<i>_g2`.
    pub g2: f32,
    /// `shapecode_<i>_b2`.
    pub b2: f32,
    /// `shapecode_<i>_a2`.
    pub a2: f32,
    /// `shapecode_<i>_border_r`.
    pub border_r: f32,
    /// `shapecode_<i>_border_g`.
    pub border_g: f32,
    /// `shapecode_<i>_border_b`.
    pub border_b: f32,
    /// `shapecode_<i>_border_a`.
    pub border_a: f32,
    /// `shape_<i>_init<N>` lines (ReadCode text, joined with `'\n'`).
    pub init: String,
    /// `shape_<i>_per_frame<N>` lines.
    pub per_frame: String,
}

impl ShapeData {
    /// `CShape::Import` (state.cpp:1216): every key falls back to the current
    /// (already defaulted) value.
    pub(crate) fn import(&mut self, ini: &Ini, i: usize) {
        let k = |name: &str| format!("shapecode_{i}_{name}");
        self.enabled = ini.flat_i32(&k("enabled"), self.enabled);
        self.sides = ini.flat_i32(&k("sides"), self.sides);
        self.additive = ini.flat_i32(&k("additive"), self.additive);
        self.thick_outline = ini.flat_i32(&k("thickOutline"), self.thick_outline);
        self.textured = ini.flat_i32(&k("textured"), self.textured);
        self.instances = ini.flat_i32(&k("num_inst"), self.instances);
        self.x = ini.flat_f32(&k("x"), self.x);
        self.y = ini.flat_f32(&k("y"), self.y);
        self.rad = ini.flat_f32(&k("rad"), self.rad);
        self.ang = ini.flat_f32(&k("ang"), self.ang);
        self.tex_ang = ini.flat_f32(&k("tex_ang"), self.tex_ang);
        self.tex_zoom = ini.flat_f32(&k("tex_zoom"), self.tex_zoom);
        self.r = ini.flat_f32(&k("r"), self.r);
        self.g = ini.flat_f32(&k("g"), self.g);
        self.b = ini.flat_f32(&k("b"), self.b);
        self.a = ini.flat_f32(&k("a"), self.a);
        self.r2 = ini.flat_f32(&k("r2"), self.r2);
        self.g2 = ini.flat_f32(&k("g2"), self.g2);
        self.b2 = ini.flat_f32(&k("b2"), self.b2);
        self.a2 = ini.flat_f32(&k("a2"), self.a2);
        self.border_r = ini.flat_f32(&k("border_r"), self.border_r);
        self.border_g = ini.flat_f32(&k("border_g"), self.border_g);
        self.border_b = ini.flat_f32(&k("border_b"), self.border_b);
        self.border_a = ini.flat_f32(&k("border_a"), self.border_a);
        self.init = ini.code_block(&format!("shape_{i}_init"));
        self.per_frame = ini.code_block(&format!("shape_{i}_per_frame"));
    }
}

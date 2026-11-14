//! Custom waves (`CWave`, state.h). Four per preset (`MAX_CUSTOM_WAVES`).

use crate::ini::Ini;

/// One custom wave: the `wavecode_<i>_*` scalars and its three code blocks.
///
/// Integer fields stay `i32` because `CWave` stores them as `int` and the
/// per-frame equations may write any value into them.
#[derive(Clone, Debug, PartialEq)]
pub struct WaveData {
    /// `wavecode_<i>_enabled`.
    pub enabled: i32,
    /// `wavecode_<i>_samples`.
    pub samples: i32,
    /// `wavecode_<i>_sep`.
    pub sep: i32,
    /// `wavecode_<i>_bSpectrum`.
    pub spectrum: i32,
    /// `wavecode_<i>_bUseDots`.
    pub use_dots: i32,
    /// `wavecode_<i>_bDrawThick`.
    pub draw_thick: i32,
    /// `wavecode_<i>_bAdditive`.
    pub additive: i32,
    /// `wavecode_<i>_scaling`.
    pub scaling: f32,
    /// `wavecode_<i>_smoothing`.
    pub smoothing: f32,
    /// `wavecode_<i>_r`.
    pub r: f32,
    /// `wavecode_<i>_g`.
    pub g: f32,
    /// `wavecode_<i>_b`.
    pub b: f32,
    /// `wavecode_<i>_a`.
    pub a: f32,
    /// `wave_<i>_init<N>` lines (ReadCode text, joined with `'\n'`).
    pub init: String,
    /// `wave_<i>_per_frame<N>` lines.
    pub per_frame: String,
    /// `wave_<i>_per_point<N>` lines.
    pub per_point: String,
}

impl WaveData {
    /// `CWave::Import` (state.cpp:1179): every key falls back to the current
    /// (already defaulted) value. Note the code prefixes have no `_` before
    /// the line number: `wave_0_per_frame1`.
    pub(crate) fn import(&mut self, ini: &Ini, i: usize) {
        let k = |name: &str| format!("wavecode_{i}_{name}");
        self.enabled = ini.flat_i32(&k("enabled"), self.enabled);
        self.samples = ini.flat_i32(&k("samples"), self.samples);
        self.sep = ini.flat_i32(&k("sep"), self.sep);
        self.spectrum = ini.flat_i32(&k("bSpectrum"), self.spectrum);
        self.use_dots = ini.flat_i32(&k("bUseDots"), self.use_dots);
        self.draw_thick = ini.flat_i32(&k("bDrawThick"), self.draw_thick);
        self.additive = ini.flat_i32(&k("bAdditive"), self.additive);
        self.scaling = ini.flat_f32(&k("scaling"), self.scaling);
        self.smoothing = ini.flat_f32(&k("smoothing"), self.smoothing);
        self.r = ini.flat_f32(&k("r"), self.r);
        self.g = ini.flat_f32(&k("g"), self.g);
        self.b = ini.flat_f32(&k("b"), self.b);
        self.a = ini.flat_f32(&k("a"), self.a);
        self.init = ini.code_block(&format!("wave_{i}_init"));
        self.per_frame = ini.code_block(&format!("wave_{i}_per_frame"));
        self.per_point = ini.code_block(&format!("wave_{i}_per_point"));
    }
}

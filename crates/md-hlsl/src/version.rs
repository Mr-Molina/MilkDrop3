//! md-hlsl's own pixel-shader model enum (ruling F23: no runtime dependency
//! on md-preset; md-render converts with a `From` impl).

/// Pixel-shader model a preset asks for (mirrors `MD2_PS_*`).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsVersion {
    /// No preset shaders.
    None,
    /// ps_2_0.
    Ps2_0,
    /// ps_2_x (ps_2_a / ps_2_b).
    Ps2_X,
    /// ps_3_0.
    Ps3_0,
    /// ps_4_0 (not supported by MilkDrop).
    Ps4_0,
}

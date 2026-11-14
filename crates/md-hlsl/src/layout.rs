//! The `MdUniforms` uniform block shared by every translated preset shader
//! and `md-render` (Task 4.4; F29: this is the single source of truth,
//! md-render imports it instead of keeping its own copy).
//!
//! WGSL (`@group(0) @binding(0) var<uniform> U: MdUniforms`), in the field
//! order of the Milestone 4 output contract:
//!
//! | fields | WGSL type | offsets |
//! |---|---|---|
//! | `_c0`..`_c13` | `vec4<f32>` | 0, 16, … 208 |
//! | `_qa`..`_qh` | `vec4<f32>` | 224 … 336 |
//! | `rand_frame`, `rand_preset` | `vec4<f32>` | 352, 368 |
//! | `rot_s1..4`, `rot_d1..4`, `rot_f1..4`, `rot_vf1..4`, `rot_uf1..4`, `rot_rand1..4` | `mat4x3<f32>` | 384 + 64·k … 1856 |
//!
//! Total size [`MD_UNIFORMS_SIZE`] = 1920 bytes.
//!
//! Matrix convention (F24): HLSL `float4x3` (4 rows of 3) is stored
//! transposed as WGSL `mat4x3<f32>` (4 columns of `vec3`), so **WGSL column
//! `i` is HLSL row `i`**. Each column is 16-byte aligned: column `i` of a
//! matrix at offset `o` starts at `o + 16·i`, its 3 floats are the HLSL row
//! and the 4th float is padding. A `mat4x3<f32>` therefore occupies 64 bytes.
//!
//! The per-preset texture sizes live in a second block,
//! `@group(0) @binding(1) var<uniform> T: TexSizes`, whose fields are the
//! `texsize_*` names of `Bindings::texsize_names`, one `vec4<f32>` each,
//! in that order (offset `16·i`). WGSL forbids empty structs, so a shader
//! that references no `texsize_*` still declares `TexSizes` with one unused
//! `vec4<f32>` field ([`TEXSIZES_PLACEHOLDER`]): the block is always at least
//! 16 bytes (see [`texsizes_size`]).

use crate::types::{Scalar, Ty};

/// Size in bytes of the `MdUniforms` block.
pub const MD_UNIFORMS_SIZE: u64 = 1920;

/// The D3D9 rasterization shift (F29, round-1 ruling), in render-target
/// pixels: D3D9 samples pixel `i` at screen coordinate `i`, wgpu at
/// `i + 0.5`, so every built-in vertex shader moves its clip-space position
/// by `+D3D9_PIXEL_SHIFT` pixels right and down:
///
/// `clip.xy += 2 * pixel_shift * (texel_size.x, -texel_size.y) * clip.w`
///
/// with `texel_size = (1/W, 1/H)` of the W×H target, i.e. NDC `(+1/W, -1/H)`
/// for `clip.w = 1`. Every interpolated attribute then takes at wgpu's
/// pixel centre the value D3D9 gave that pixel, so vertex data is built
/// exactly as the C++ builds it (including its own half-texel UV biases).
pub const D3D9_PIXEL_SHIFT: f32 = 0.5;

/// Binding (in group 0) of the `VsParams` block every built-in vertex
/// shader in `assets/shaders/` reads (Task 4.8): `texel_size: vec2<f32>`
/// at offset 0 (the target's `1/W, 1/H`), `pixel_shift: f32` at 8 (fill
/// with [`D3D9_PIXEL_SHIFT`]; 0 disables the shift), `pad` at 12. Binding 2
/// so it never collides with the preset fragment shader's `U` (binding 0)
/// and `T` (binding 1) in the same pipeline.
pub const BUILTIN_VS_PARAMS_BINDING: u32 = 2;

/// Size in bytes of the built-in `VsParams` block.
pub const BUILTIN_VS_PARAMS_SIZE: u64 = 16;

/// Size in bytes of the `BlurUniforms` block of `blur1_ps`/`blur2_ps`
/// (`@group(0) @binding(0)`): seven `vec4<f32>` `_c0`..`_c6` mirroring the
/// D3D9 constant registers, `_cN` at offset `16·N` (Task 4.8; see
/// `assets/shaders/README.md`).
pub const BLUR_UNIFORMS_SIZE: u64 = 112;

/// Name of the unused field of an otherwise empty `TexSizes` block.
pub const TEXSIZES_PLACEHOLDER: &str = "md_unused";

/// The WGSL type of an `MdUniforms` field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniformKind {
    /// HLSL `float4`, WGSL `vec4<f32>` (16 bytes).
    Vec4,
    /// HLSL `float4x3`, WGSL `mat4x3<f32>` (64 bytes, see module docs).
    Mat4x3,
}

impl UniformKind {
    /// Size in bytes inside the block.
    pub fn size(self) -> u64 {
        match self {
            UniformKind::Vec4 => 16,
            UniformKind::Mat4x3 => 64,
        }
    }

    /// WGSL spelling.
    pub fn wgsl(self) -> &'static str {
        match self {
            UniformKind::Vec4 => "vec4<f32>",
            UniformKind::Mat4x3 => "mat4x3<f32>",
        }
    }

    /// The HLSL type a preset declares for it.
    pub(crate) fn hlsl_ty(self) -> Ty {
        match self {
            UniformKind::Vec4 => Ty::Vec(Scalar::Float, 4),
            UniformKind::Mat4x3 => Ty::Mat(Scalar::Float, 4, 3),
        }
    }
}

use UniformKind::{Mat4x3 as M, Vec4 as V};

/// The `MdUniforms` fields in block order.
pub const MD_UNIFORM_FIELDS: [(&str, UniformKind); 48] = [
    ("_c0", V),
    ("_c1", V),
    ("_c2", V),
    ("_c3", V),
    ("_c4", V),
    ("_c5", V),
    ("_c6", V),
    ("_c7", V),
    ("_c8", V),
    ("_c9", V),
    ("_c10", V),
    ("_c11", V),
    ("_c12", V),
    ("_c13", V),
    ("_qa", V),
    ("_qb", V),
    ("_qc", V),
    ("_qd", V),
    ("_qe", V),
    ("_qf", V),
    ("_qg", V),
    ("_qh", V),
    ("rand_frame", V),
    ("rand_preset", V),
    ("rot_s1", M),
    ("rot_s2", M),
    ("rot_s3", M),
    ("rot_s4", M),
    ("rot_d1", M),
    ("rot_d2", M),
    ("rot_d3", M),
    ("rot_d4", M),
    ("rot_f1", M),
    ("rot_f2", M),
    ("rot_f3", M),
    ("rot_f4", M),
    ("rot_vf1", M),
    ("rot_vf2", M),
    ("rot_vf3", M),
    ("rot_vf4", M),
    ("rot_uf1", M),
    ("rot_uf2", M),
    ("rot_uf3", M),
    ("rot_uf4", M),
    ("rot_rand1", M),
    ("rot_rand2", M),
    ("rot_rand3", M),
    ("rot_rand4", M),
];

/// Byte offset of field `name` in `MdUniforms` (case-sensitive), or `None`
/// if it is not a field.
pub fn md_uniform_offset(name: &str) -> Option<u64> {
    let mut off = 0;
    for (n, k) in MD_UNIFORM_FIELDS {
        if n == name {
            return Some(off);
        }
        off += k.size();
    }
    None
}

/// The kind of `MdUniforms` field `name`.
pub fn md_uniform_kind(name: &str) -> Option<UniformKind> {
    MD_UNIFORM_FIELDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| *k)
}

/// The WGSL declaration of `struct MdUniforms`.
pub fn md_uniforms_wgsl() -> String {
    let mut s = String::from("struct MdUniforms {\n");
    for (n, k) in MD_UNIFORM_FIELDS {
        s += &format!("    {n}: {},\n", k.wgsl());
    }
    s + "}\n"
}

/// Size in bytes of the `TexSizes` block for `n` `texsize_*` names.
pub fn texsizes_size(n: usize) -> u64 {
    16 * n.max(1) as u64
}

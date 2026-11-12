//! Uniform blocks, serialized by hand (F11) at the offsets
//! [`md_hlsl::layout`] publishes (F29: the single source of truth).
//!
//! Decision (ledger Task 4.8 note): the `VsParams` and `BlurUniforms`
//! serializers live here with `MdUniforms`, since they are pure layout;
//! filling them per pass (blur weights, scale/bias) belongs to the passes
//! that use them (Tasks 6.1 and later).

use crate::bytes::{to_array, ByteWriter};
use md_hlsl::layout::{
    texsizes_size, BLUR_UNIFORMS_SIZE, BUILTIN_VS_PARAMS_SIZE, D3D9_PIXEL_SHIFT, MD_UNIFORMS_SIZE,
    MD_UNIFORM_FIELDS,
};

/// Pack an HLSL `float4x3` (four rows of three) as the WGSL `mat4x3<f32>`
/// the translator declares (F24): **WGSL column `i` is HLSL row `i`**, each
/// column padded to 16 bytes (fourth float 0).
pub fn pack_mat4x3(hlsl_rows: [[f32; 3]; 4]) -> [[f32; 4]; 4] {
    hlsl_rows.map(|[a, b, c]| [a, b, c, 0.0])
}

/// The `MdUniforms` block every translated preset shader reads at
/// `@group(0) @binding(0)` (1920 bytes, field order of
/// [`MD_UNIFORM_FIELDS`]).
#[derive(Clone, Debug, PartialEq)]
pub struct MdUniforms {
    /// `_c0`..`_c13`.
    pub c: [[f32; 4]; 14],
    /// `_qa`..`_qh` (`q1..q32` in groups of four).
    pub q: [[f32; 4]; 8],
    pub rand_frame: [f32; 4],
    pub rand_preset: [f32; 4],
    /// The 24 `float4x3` rotation matrices as HLSL rows, in block order:
    /// `rot_s1..4`, `rot_d1..4`, `rot_f1..4`, `rot_vf1..4`, `rot_uf1..4`
    /// (k = 0..19), `rot_rand1..4` (k = 20..23).
    pub rot: [[[f32; 3]; 4]; 24],
}

// The struct covers every field of the published layout, no more.
const _: () = assert!(14 + 8 + 2 + 24 == MD_UNIFORM_FIELDS.len());

impl Default for MdUniforms {
    fn default() -> Self {
        MdUniforms {
            c: [[0.0; 4]; 14],
            q: [[0.0; 4]; 8],
            rand_frame: [0.0; 4],
            rand_preset: [0.0; 4],
            rot: [[[0.0; 3]; 4]; 24],
        }
    }
}

impl MdUniforms {
    /// Serialized size in bytes ([`MD_UNIFORMS_SIZE`]).
    pub const SIZE: u64 = MD_UNIFORMS_SIZE;

    /// Append the 1920 little-endian bytes of the block.
    pub fn write_to(&self, out: &mut Vec<u8>) {
        let start = out.len();
        let mut w = ByteWriter::new(out);
        for v in self.c.iter().chain(&self.q) {
            w.f32s(v);
        }
        w.f32s(&self.rand_frame).f32s(&self.rand_preset);
        for m in &self.rot {
            for col in pack_mat4x3(*m) {
                w.f32s(&col);
            }
        }
        debug_assert_eq!((out.len() - start) as u64, Self::SIZE);
    }

    /// The 1920 little-endian bytes of the block.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::SIZE as usize);
        self.write_to(&mut out);
        out
    }
}

/// The `VsParams` block every built-in vertex shader reads at
/// `@group(0) @binding(2)` (16 bytes; `assets/shaders/README.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VsParams {
    /// `(1/W, 1/H)` of the W×H target being drawn (unsigned).
    pub texel_size: [f32; 2],
    /// [`D3D9_PIXEL_SHIFT`]; 0 disables the D3D9 rasterization shift.
    pub pixel_shift: f32,
}

impl VsParams {
    /// Parameters for drawing into a `w`×`h` target with the D3D9 shift.
    pub fn for_target(w: u32, h: u32) -> VsParams {
        VsParams {
            texel_size: [1.0 / w.max(1) as f32, 1.0 / h.max(1) as f32],
            pixel_shift: D3D9_PIXEL_SHIFT,
        }
    }

    /// The 16 bytes of the block (`texel_size`, `pixel_shift`, pad 0).
    pub fn to_bytes(&self) -> [u8; BUILTIN_VS_PARAMS_SIZE as usize] {
        to_array(|w| {
            w.f32s(&self.texel_size).f32s(&[self.pixel_shift, 0.0]);
        })
    }
}

/// The `BlurUniforms` block of `blur1_ps`/`blur2_ps` at `@group(0)
/// @binding(0)`: `_c0`..`_c6`, `_cN` at `16·N` (112 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlurUniforms {
    pub c: [[f32; 4]; 7],
}

impl BlurUniforms {
    /// The 112 bytes of the block.
    pub fn to_bytes(&self) -> [u8; BLUR_UNIFORMS_SIZE as usize] {
        to_array(|w| {
            for v in &self.c {
                w.f32s(v);
            }
        })
    }
}

/// The `TexSizes` block (`@group(0) @binding(1)`) for the `texsize_*`
/// values in `Translated::texsize_names` order: one `vec4` each, never
/// smaller than 16 bytes ([`texsizes_size`]; the placeholder is zero).
pub fn texsizes_bytes(values: &[[f32; 4]]) -> Vec<u8> {
    let size = texsizes_size(values.len()) as usize;
    let mut out = Vec::with_capacity(size);
    let mut w = ByteWriter::new(&mut out);
    for v in values {
        w.f32s(v);
    }
    out.resize(size, 0);
    out
}

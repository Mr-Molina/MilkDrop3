//! The single vertex layout of every md-render draw (F30): it serves the
//! C++'s `MYVERTEX`, `WFVERTEX` and `SPRITEVERTEX`, unused fields zero.
//! See `assets/shaders/README.md` ("Vertex layout").

use crate::bytes::{to_array, ByteWriter};

/// One vertex: 40 bytes, `pos` @0 `Float32x3` (offset 0), `color` @1
/// `Unorm8x4` (12, bytes **R, G, B, A**), `uv` @2 `Float32x4` (16;
/// `tu, tv, tu_orig, tv_orig`), `rad_ang` @3 `Float32x2` (32).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub color: [u8; 4],
    pub uv: [f32; 4],
    pub rad_ang: [f32; 2],
}

impl Vertex {
    /// Serialized size and array stride in bytes.
    pub const SIZE: u64 = 40;

    /// The vertex attributes, matching every built-in `VsIn`.
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Unorm8x4,
            offset: 12,
            shader_location: 1,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 16,
            shader_location: 2,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 32,
            shader_location: 3,
        },
    ];

    /// The vertex buffer layout for pipelines.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: Self::SIZE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &Self::ATTRIBUTES,
    };

    /// Convert a D3D `D3DCOLOR` (`0xAARRGGBB`, stored in memory B, G, R, A)
    /// to this layout's R, G, B, A bytes. Every vertex colour the C++
    /// builds as a `D3DCOLOR` goes through here.
    pub fn color_from_d3dcolor(argb: u32) -> [u8; 4] {
        let [a, r, g, b] = argb.to_be_bytes();
        [r, g, b, a]
    }

    /// Append the 40 little-endian bytes of this vertex.
    pub fn write_to(&self, out: &mut Vec<u8>) {
        ByteWriter::new(out)
            .f32s(&self.pos)
            .bytes(&self.color)
            .f32s(&self.uv)
            .f32s(&self.rad_ang);
    }

    /// The 40 little-endian bytes of this vertex.
    pub fn to_bytes(&self) -> [u8; 40] {
        to_array(|w| {
            w.f32s(&self.pos)
                .bytes(&self.color)
                .f32s(&self.uv)
                .f32s(&self.rad_ang);
        })
    }
}

/// Serialize `verts` back to back (stride [`Vertex::SIZE`]).
pub fn vertices_to_bytes(verts: &[Vertex]) -> Vec<u8> {
    let mut out = Vec::with_capacity(verts.len() * Vertex::SIZE as usize);
    for v in verts {
        v.write_to(&mut out);
    }
    out
}

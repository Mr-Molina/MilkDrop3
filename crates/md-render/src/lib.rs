//! The wgpu renderer: resources, pipelines and all draw passes.
//!
//! Task 5.2 foundation: the [`Gpu`] context (owned device/queue clones, F6),
//! the single [`Vertex`] layout every built-in shader reads, textures with
//! full mip chains and GPU mip generation, the hand-serialized uniform
//! blocks (F11: no bytemuck, `forbid(unsafe_code)`; F29: offsets come from
//! [`md_hlsl::layout`]) and the per-frame [`DynamicVertexBuffer`].
#![forbid(unsafe_code)]

mod bytes;
mod dynbuf;
mod gpu;
mod mips;
mod readback;
mod texture;
mod uniforms;
mod vertex;

pub use dynbuf::DynamicVertexBuffer;
pub use gpu::{gpu_or_skip, headless_device, headless_gpu, Gpu};
pub use mips::generate_mips;
pub use readback::{map_read, read_texture_level};
pub use texture::{full_mip_count, Texture2d, Texture3d};
pub use uniforms::{pack_mat4x3, texsizes_bytes, BlurUniforms, MdUniforms, VsParams};
pub use vertex::{vertices_to_bytes, Vertex};

pub use wgpu;

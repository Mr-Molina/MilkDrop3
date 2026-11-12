//! The wgpu renderer: resources, pipelines and all draw passes.
//!
//! Task 5.2 foundation: the [`Gpu`] context (owned device/queue clones, F6),
//! the single [`Vertex`] layout every built-in shader reads, textures with
//! full mip chains and GPU mip generation, the hand-serialized uniform
//! blocks (F11: no bytemuck, `forbid(unsafe_code)`; F29: offsets come from
//! [`md_hlsl::layout`]) and the per-frame [`DynamicVertexBuffer`].
//!
//! Task 5.3: D3D9 fixed-function emulation — [`FixedMode`] × [`BlendMode`]
//! × [`Topo`] pipelines in a [`PipelineCache`], recorded by a [`DrawBatch`]
//! (fan and point conversion, [`SamplerKind`] samplers).
//!
//! Task 5.4: the [`TextureStore`] — disk textures found and decoded on
//! demand ([`load_image`]) under an LRU budget, the generated noise
//! textures ([`noise`]) and `randNN` picks ([`RandPicks`]).
//!
//! Task 5.5: the [`Renderer`] (ping-pong main textures, blur textures,
//! texture store, pipeline cache) and [`Renderer::load_preset`], which
//! translates the preset's shaders through the [`ShaderCache`] (memory and
//! disk), falls back to the default shaders on error, resolves every
//! sampler ([`SlotSource`]) and builds a [`ShaderProgram`] per shader.
#![forbid(unsafe_code)]

mod blend;
mod bytes;
mod dynbuf;
mod fixed;
mod gpu;
mod image_load;
mod mips;
pub mod noise;
mod pipelines;
mod preset_bind;
mod preset_gpu;
mod readback;
mod renderer;
mod shader_cache;
mod texcache;
mod texture;
mod textures;
mod uniforms;
mod vertex;

pub use blend::BlendMode;
pub use dynbuf::DynamicVertexBuffer;
pub use fixed::{fan_to_list, points_to_quads, FixedMode, SamplerKind, Topo};
pub use gpu::{gpu_or_skip, headless_device, headless_gpu, Gpu};
pub use image_load::{load_image, ImageError};
pub use mips::generate_mips;
pub use pipelines::{DrawBatch, PipelineCache, PipelineKey};
pub use preset_bind::{sampler_kind, SlotSource};
pub use preset_gpu::{PresetGpu, ShaderProgram};
pub use readback::{map_read, read_texture_level};
pub use renderer::{
    blur_texture_sizes, hlsl_ps_version, RenderError, RenderSettings, Renderer, DEFAULT_COMP_PS,
    DEFAULT_WARP_PS, INCLUDE_FX,
};
pub use shader_cache::{
    decode as decode_cache_entry, encode as encode_cache_entry, shader_key, CachedShader,
    ShaderCache, ShaderKey, CACHE_EXT, CACHE_FORMAT, MAX_MEMORY_ENTRIES,
};
pub use texcache::{TextureId, MAX_BYTES, MAX_IMAGES, TEXTURE_OVERHEAD_BYTES};
pub use texture::{full_mip_count, Texture2d, Texture3d};
pub use textures::{
    pick_random_texture, scan_texture_names, texture_candidates, RandPicks, TextureError,
    TextureStore, TEXTURE_EXTS,
};
pub use uniforms::{pack_mat4x3, texsizes_bytes, BlurUniforms, MdUniforms, VsParams};
pub use vertex::{vertices_to_bytes, Vertex};

pub use image::RgbaImage;
pub use wgpu;

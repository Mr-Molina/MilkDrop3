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
//!
//! Task 5.6: the [`WarpMesh`] (mesh set-up and per-frame UVs, with the
//! [`PerFrameVars`] / [`PerPixelVars`] / [`PerPixelRunner`] md-core fills,
//! F37), blend patterns ([`randomize_blend_pattern`]), `ApplyShaderParams`
//! ([`shader_uniforms`], shared with the composite pass) and
//! [`Renderer::warp_pass`].
//!
//! Task 5.7: the frame loop ([`Renderer::begin_frame`] with a
//! [`FrameInput`], [`Renderer::present`] into any target), the composite
//! pass ([`comp_grid`], the preset composite shaders) and its no-shader
//! path ([`fixed_comp_draws`]), and [`Renderer::resize`] (clamped, F56).
//!
//! Task 6.1: the blur chain ([`Renderer::blur_passes`], [`blur_scale_bias`]).
//!
//! Task 6.2: the built-in waveform ([`build_wave`], drawn by the frame loop).
//!
//! Task 6.3: custom shapes ([`build_shape`], [`ShapeFrameVars`] in
//! [`FrameInput::shapes`]).
//!
//! Task 6.4: custom waves ([`custom_wave_inputs`], [`build_custom_wave`],
//! [`CustomWaveDraw`] in [`FrameInput::custom_waves`]).
//!
//! Task 6.5: motion vectors, borders and the darken-centre fan
//! ([`build_motion_vectors`], [`build_borders`], [`build_darken_center`]).
#![forbid(unsafe_code)]

mod blend;
mod blendpattern;
mod blur;
mod bytes;
mod cast;
mod comp;
mod comp_fixed;
mod custom_waves;
mod custom_waves_draw;
mod dynbuf;
mod fixed;
mod frame;
mod frame_vars;
mod gpu;
mod image_load;
mod mesh;
mod mips;
pub mod noise;
mod overlays;
mod overlays_draw;
mod pipelines;
mod preset_bind;
mod preset_gpu;
mod readback;
mod renderer;
mod shader_cache;
mod shader_params;
mod shapes;
mod shapes_draw;
mod tex_size;
mod texcache;
mod text_draw;
mod text_pass;
mod texture;
mod textures;
mod uniforms;
mod vertex;
mod warp;
mod wave;
mod wave_draw;

pub use blend::BlendMode;
pub use blendpattern::{blend_pattern, frand, gen_plasma, randomize_blend_pattern, BlendPattern};
pub use blur::{blur_scale_bias, pass_uniforms, BLUR_WEIGHTS, MIN_RANGE, NUM_BLUR_TEX};
pub use comp::{
    comp_grid, comp_vertices, cosine_interp, cull_indices, hue_shades, plan_comp_passes, CompGrid,
    CompPass, COMP_GRID_X, COMP_GRID_Y,
};
pub use comp_fixed::{fixed_comp_draws, rgba_01, EchoBlend, FixedCompDraw, FixedCompParams};
pub use custom_waves::{
    build_custom_wave, custom_wave_inputs, CustomWaveDraw, CustomWaveFrame, CustomWaveGeometry,
    CustomWaveInput, CustomWavePoint, CustomWaveView,
};
pub use dynbuf::DynamicVertexBuffer;
pub use fixed::{fan_to_list, points_to_quads, FixedMode, SamplerKind, Topo};
pub use frame::{FrameCtx, FrameInput};
pub use frame_vars::PerFrameVars;
pub use gpu::{gpu_or_skip, headless_device, headless_gpu, Gpu};
pub use image_load::{load_image, ImageError};
pub use mesh::{
    mesh_aspect, PerPixelRunner, PerPixelVars, VertInfo, WarpMesh, MAX_GRID_X, MAX_GRID_Y,
};
pub use mips::{generate_mips, record_mips};
pub use overlays::{
    build_borders, build_darken_center, build_motion_vectors, reverse_propagate_point, Border,
    MotionMesh, MotionVectorVars,
};
pub use pipelines::{DrawBatch, PipelineCache, PipelineKey};
pub use preset_bind::{sampler_kind, SlotSource};
pub use preset_gpu::{PresetGpu, ShaderProgram};
pub use readback::{map_read, read_texture_level};
pub use renderer::{
    hlsl_ps_version, RenderError, RenderSettings, Renderer, DEFAULT_COMP_PS, DEFAULT_WARP_PS,
    INCLUDE_FX,
};
pub use shader_cache::{
    decode as decode_cache_entry, encode as encode_cache_entry, shader_key, CachedShader,
    ShaderCache, ShaderKey, CACHE_EXT, CACHE_FORMAT, MAX_MEMORY_ENTRIES,
};
pub use shader_params::{
    random_rot_matrix, randomize_preset_vars, safe_blur_min_max, shader_uniforms,
    FrameShaderInputs, PresetShaderInputs, RotParams,
};
pub use shapes::{build_shape, ShapeFrameVars, ShapeGeometry, ShapeView};
pub use tex_size::{blur_texture_sizes, main_texture_size, TexSize};
pub use texcache::{TextureId, MAX_BYTES, MAX_IMAGES, TEXTURE_OVERHEAD_BYTES};
pub use text_pass::{FontId, TextQueue, DARK_TRANSLUCENT_BOX, FONT_SLOTS};
pub use texture::{full_mip_count, Texture2d, Texture3d};
pub use textures::{
    pick_random_texture, scan_texture_names, texture_candidates, RandPicks, TextureError,
    TextureStore, TEXTURE_EXTS,
};
pub use uniforms::{pack_mat4x3, texsizes_bytes, BlurUniforms, MdUniforms, VsParams};
pub use vertex::{vertices_to_bytes, Vertex};
pub use warp::{
    decay_byte, plan_warp_passes, snap_point, warp_geometry, Cull, WarpGeometry, WarpPassPlan,
    WarpPreset,
};
pub use wave::{
    build_wave, scaled_wave, smooth_wave, WaveBlend, WaveGeometry, WaveParams, WaveView,
    NUM_WAVEFORM_SAMPLES, NUM_WAVES,
};

pub use image::RgbaImage;
pub use md_analysis::SoundData;
pub use md_text;
pub use wgpu;

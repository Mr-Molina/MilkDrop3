// Song-title animation (ShowSongTitleAnim, milkdropfs.cpp:4477): the
// title texture modulated by the vertex colour (D3DTOP_MODULATE defaults,
// all four channels), drawn with additive blending (ONE / ONE) by
// md-render. Task 6.7 may adjust this shader.
//
// Bindings: @group(0) @binding(2) VsParams (half-texel shift, see
// fixed_tex_modulate.wgsl; pass texel_size = 0 for pixel-aligned quads),
// @group(1) @binding(0) tex, @group(1) @binding(1) samp.

struct VsParams {
    texel_size: vec2<f32>,
    half_texel: f32,
    pad: f32,
}

@group(0) @binding(2) var<uniform> vs_params: VsParams;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) rad_ang: vec2<f32>,
}

struct FixedVsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(v: VsIn) -> FixedVsOut {
    var o: FixedVsOut;
    o.pos = vec4<f32>(v.pos, 1.0);
    o.color = v.color;
    o.uv = v.uv.xy + vs_params.half_texel * vs_params.texel_size;
    return o;
}

@fragment
fn fs_main(in: FixedVsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv) * in.color;
}

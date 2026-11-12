// D3D9 fixed-function emulation, texture x vertex colour: COLOROP and
// ALPHAOP D3DTOP_MODULATE with ARG1 = D3DTA_TEXTURE, ARG2 = D3DTA_DIFFUSE,
// i.e. output = texture sample * interpolated vertex colour (all four
// channels). Used for textured custom shapes, sprites, and the
// non-shader (MilkDrop 1) warp/comp fallbacks (SPRITEVERTEX/MYVERTEX).
//
// Half-texel (F29): uv is shifted by `half_texel * texel_size`
// (half_texel = md_hlsl::layout::D3D9_HALF_TEXEL, texel_size = signed UV
// step per render-target pixel) so wgpu's +0.5 pixel centres sample what
// D3D9's integer pixel centres sampled; md-render passes texel_size = 0 to
// disable it for draws the C++ already aligns. See README.md.

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

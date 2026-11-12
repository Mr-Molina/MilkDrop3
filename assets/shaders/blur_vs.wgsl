// Built-in blur vertex shader: hand translation of data/blur_vs.fx
// (assets/hlsl/blur_vs.fx), shared by blur1_ps and blur2_ps.
//
//   vPosProj = float4(vPosIn.xy, 1, 1);
//   uv       = uv1.xy;
//
// z = 1 is inside wgpu's 0..1 depth range (depth testing is off for blur).
// Half-texel (F29): the C++ blur quad (milkdropfs.cpp:1542-1562) has no UV
// offset, so under D3D9 pixel i samples uv = i / dst_size; wgpu samples at
// pixel centres (+0.5 px), so uv is shifted by `half_texel * texel_size`
// (half_texel = md_hlsl::layout::D3D9_HALF_TEXEL). texel_size is the signed
// UV step per destination pixel: the C++ quad maps v = 0 to the BOTTOM edge
// (NDC y = -1), so md-render passes (1/dst_w, -1/dst_h); see README.md.

struct VsParams {
    texel_size: vec2<f32>,
    half_texel: f32,
    pad: f32,
}

@group(0) @binding(2) var<uniform> vs_params: VsParams;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) rad_ang: vec2<f32>,
}

struct BlurVsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(v: VsIn) -> BlurVsOut {
    var o: BlurVsOut;
    o.pos = vec4<f32>(v.pos.xy, 1.0, 1.0);
    o.uv = v.uv.xy + vs_params.half_texel * vs_params.texel_size;
    return o;
}

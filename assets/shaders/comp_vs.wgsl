// Built-in composite vertex shader: hand translation of data/comp_vs.fx
// (assets/hlsl/comp_vs.fx). Feeds a translated preset comp fragment shader,
// so it outputs exactly the Milestone 4 `VsOut`.
//
//   vPosProj  = float4(vPosIn.x, vPosIn.y, vPosIn.z, 1);
//   _vDiffuse = vDiffuseIn;
//   _uv       = uv_in.xy;     // .xy = UVs to use (unwarped), .zw = IGNORE
//   _rad_ang  = rad_ang_in.xy;
//
// The HLSL `_uv` is a float2; the contract's `uv` is a vec4 whose .zw the
// comp fragment shader never reads (its `_uv` parameter is float2), so .zw
// is written as 0.
//
// D3D9 and wgpu share clip space and top-down texture v: nothing flips.
// Half-texel (F29): plugin.cpp:2041-2058 shifts the comp grid positions by
// -0.5 pixel so D3D9's integer pixel centres sample texel centres; under
// wgpu's +0.5 pixel centres that grid samples one half texel too far, so
// the UVs are shifted by `half_texel * texel_size`
// (half_texel = md_hlsl::layout::D3D9_HALF_TEXEL, texel_size = 1 / window size).

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

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) diffuse: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) rad_ang: vec2<f32>,
}

@vertex
fn vs_main(v: VsIn) -> VsOut {
    let off = vs_params.half_texel * vs_params.texel_size;
    var o: VsOut;
    o.pos = vec4<f32>(v.pos.x, v.pos.y, v.pos.z, 1.0);
    o.diffuse = v.color;
    o.uv = vec4<f32>(v.uv.xy + off, 0.0, 0.0);
    o.rad_ang = v.rad_ang.xy;
    return o;
}

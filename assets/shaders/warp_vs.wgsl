// Built-in warp vertex shader: hand translation of data/warp_vs.fx
// (assets/hlsl/warp_vs.fx). Feeds a translated preset warp fragment shader,
// so it outputs exactly the Milestone 4 `VsOut`.
//
//   vPosProj  = float4(vPosIn.x, vPosIn.y, vPosIn.z, 1);
//   _vDiffuse = vDiffuseIn;
//   _uv       = uv_in.xyzw;   // .xy = warped UVs, .zw = orig UVs
//   _rad_ang  = rad_ang_in.xy;
//
// D3D9 and wgpu share clip space (x right, y up, z in 0..1) and top-down
// texture v, so nothing is flipped. The only change is the D3D9 half-texel
// emulation (F29): the C++ adds +0.5 texel to both the warped UVs
// (milkdropfs.cpp:1835-1837) and the orig UVs (plugin.cpp:2299-2300) to
// hit texel centres under D3D9's integer pixel centres; wgpu samples at
// pixel centres (+0.5 px), so both UV pairs are shifted by
// `half_texel * texel_size` (half_texel = md_hlsl::layout::D3D9_HALF_TEXEL,
// texel_size = 1 / warp target size).

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
    o.uv = v.uv + vec4<f32>(off, off);
    o.rad_ang = v.rad_ang.xy;
    return o;
}

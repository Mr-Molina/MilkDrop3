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
// D3D9 and wgpu share clip space (x right, y up, z in 0..1) and top-down
// texture v, so nothing is flipped. The position goes through d3d9_raster
// (VsParams.texel_size = 1 / target size), so every attribute -- including
// the C++'s own half-texel UV biases, which md-render keeps in the vertex
// data -- interpolates exactly as under D3D9.
// Target: the window/back buffer. The comp grid keeps the C++'s -0.5 px
// position bias (plugin.cpp:2041-2058); with the +0.5 px raster shift it
// covers the target exactly as under D3D9.

struct VsParams {
    texel_size: vec2<f32>,
    pixel_shift: f32,
    pad: f32,
}

@group(0) @binding(2) var<uniform> vs_params: VsParams;

// D3D9 rasterization (F29): D3D9 samples pixel i at screen coordinate i,
// wgpu at i + 0.5, so the clip-space position moves right/down by
// `pixel_shift` (= md_hlsl::layout::D3D9_PIXEL_SHIFT = 0.5) target pixels:
// clip.xy += 2 * pixel_shift * (1/W, -1/H) * clip.w.
fn d3d9_raster(clip: vec4<f32>) -> vec4<f32> {
    let s = 2.0 * vs_params.pixel_shift * vec2<f32>(vs_params.texel_size.x, -vs_params.texel_size.y);
    return vec4<f32>(clip.xy + s * clip.w, clip.zw);
}

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
    var o: VsOut;
    o.pos = d3d9_raster(vec4<f32>(v.pos.x, v.pos.y, v.pos.z, 1.0));
    o.diffuse = v.color;
    o.uv = vec4<f32>(v.uv.xy, 0.0, 0.0);
    o.rad_ang = v.rad_ang.xy;
    return o;
}

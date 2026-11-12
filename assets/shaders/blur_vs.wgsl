// Built-in blur vertex shader: hand translation of data/blur_vs.fx
// (assets/hlsl/blur_vs.fx), shared by blur1_ps and blur2_ps.
//
//   vPosProj = float4(vPosIn.xy, 1, 1);
//   uv       = uv1.xy;
//
// z = 1 is inside wgpu's 0..1 depth range (depth testing is off for blur).
// D3D9 and wgpu share clip space (x right, y up, z in 0..1) and top-down
// texture v, so nothing is flipped. The position goes through d3d9_raster
// (VsParams.texel_size = 1 / target size), so every attribute -- including
// the C++'s own half-texel UV biases, which md-render keeps in the vertex
// data -- interpolates exactly as under D3D9.
// Target: the blur texture being written (1 / dst size). The source
// texel size the pixel shaders sample with is BlurUniforms._c0.

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

struct BlurVsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(v: VsIn) -> BlurVsOut {
    var o: BlurVsOut;
    o.pos = d3d9_raster(vec4<f32>(v.pos.xy, 1.0, 1.0));
    o.uv = v.uv.xy;
    return o;
}

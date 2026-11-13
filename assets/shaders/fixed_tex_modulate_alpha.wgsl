// D3D9 fixed-function emulation, full RGBA modulate: COLOROP
// MODULATE(DIFFUSE, TEXTURE) and ALPHAOP MODULATE(DIFFUSE, TEXTURE)
// (milkdropfs.cpp:3714-3721): output = texture * colour (all four channels).
// Used only by sprite blend mode 4 (colour-keyed sprites).
//
// Positions are the C++'s pre-projection coordinates (d3d9_ortho2d).
//
// Bindings: @group(0) @binding(2) VsParams (D3D9 raster shift; texel_size =
// 1 / target size, pixel_shift = md_hlsl::layout::D3D9_PIXEL_SHIFT),
// @group(1) @binding(0) tex, @group(1) @binding(1) samp.

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

// D3D9 fixed-function transform: every 2D fixed-function draw of the C++
// runs with WORLD and VIEW identity and PROJECTION
// D3DXMatrixOrthoLH(2, -2, 0, 1) (PrepareFor2DDrawing, support.cpp:179;
// CTextManager::DrawNow, textmgr.cpp:238), i.e. clip = (x, -y, z, 1).
// md-render's vertex data is the C++'s pre-projection data, so the y flip
// happens here, once, for every fixed-function draw. The raster shift
// above is applied after it, in clip space, as D3D9 rasterizes after the
// projection: its sign does not depend on the flip.
fn d3d9_ortho2d(p: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(p.x, -p.y, p.z, 1.0);
}

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
    o.pos = d3d9_raster(d3d9_ortho2d(v.pos));
    o.color = v.color;
    o.uv = v.uv.xy;
    return o;
}

@fragment
fn fs_main(in: FixedVsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv) * in.color;
}

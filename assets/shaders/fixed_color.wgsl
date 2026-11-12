// D3D9 fixed-function emulation, no texture: COLOROP/ALPHAOP SELECTARG1
// with ARG1 = D3DTA_DIFFUSE, i.e. output = interpolated vertex colour.
// Used for waveforms, untextured custom shapes, borders/motion vectors,
// dark boxes (WFVERTEX draws).
//
// No uniforms and no half-texel shift: there is no texture coordinate to
// correct (D3D9 vs wgpu pixel-centre differences only move edges of
// untextured geometry by half a pixel).

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) rad_ang: vec2<f32>,
}

struct FixedVsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(v: VsIn) -> FixedVsOut {
    var o: FixedVsOut;
    o.pos = vec4<f32>(v.pos, 1.0);
    o.color = v.color;
    return o;
}

@fragment
fn fs_main(in: FixedVsOut) -> @location(0) vec4<f32> {
    return in.color;
}

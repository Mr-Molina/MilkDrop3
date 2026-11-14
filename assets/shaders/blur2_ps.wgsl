// Built-in blur pass 2 (short vertical pass): hand translation of
// data/blur2_ps.fx (assets/hlsl/blur2_ps.fx). Input: blur_vs.wgsl.
//
// Constants, set per pass by the CPU (milkdropfs.cpp BlurPasses, i%2 == 1):
//   _c0 = source texsize (.xy), and inverse (.zw)
//   _c5 = w1, w2, d1, d2
//   _c6 = w_div, edge_darken_c1, edge_darken_c2, edge_darken_c3
// derived from the weights exactly as the .fx comments (and the C++) say:
//   const float w[8] = { 4.0, 3.8, 3.5, 2.9, 1.9, 1.2, 0.7, 0.3 };
//   const float w1 = w[0]+w[1] + w[2]+w[3];
//   const float w2 = w[4]+w[5] + w[6]+w[7];
//   const float d1 = 0 + 2*((w[2]+w[3])/w1);
//   const float d2 = 2 + 2*((w[6]+w[7])/w2);
//   const float w_div = 1.0/((w1+w2)*2);
// _c6.yzw = (1 - edge_darken, edge_darken, 5.0) on the first vertical pass
// (i == 1) and (1.0, 0.0, 5.0) afterwards (no darkening).
//
// The block mirrors the D3D9 constant registers c0..c6 (shared layout with
// blur1_ps; this pass reads _c0, _c5, _c6).

struct BlurUniforms {
    _c0: vec4<f32>,
    _c1: vec4<f32>,
    _c2: vec4<f32>,
    _c3: vec4<f32>,
    _c4: vec4<f32>,
    _c5: vec4<f32>,
    _c6: vec4<f32>,
}

@group(0) @binding(0) var<uniform> blur: BlurUniforms;
@group(1) @binding(0) var tex_main: texture_2d<f32>;
@group(1) @binding(1) var samp_main: sampler;

struct BlurVsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

fn main3(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(tex_main, samp_main, uv).xyz;
}

@fragment
fn fs_main(in: BlurVsOut) -> @location(0) vec4<f32> {
    let srctexsize = blur._c0;
    let w1 = blur._c5.x;
    let w2 = blur._c5.y;
    let d1 = blur._c5.z;
    let d2 = blur._c5.w;
    let edge_darken_c1 = blur._c6.y;
    let edge_darken_c2 = blur._c6.z;
    let edge_darken_c3 = blur._c6.w;
    let w_div = blur._c6.x;

    // note: if you just take one sample at exactly uv.xy, you get an avg of 4 pixels.
    let uv2 = in.uv.xy + srctexsize.zw * vec2<f32>(1.0, 0.0); // + moves blur UP, LEFT by TWO-pixel increments! (since texture is 1/2 the size of blur1_ps)

    var blur3 =
          (main3(uv2 + vec2<f32>(0.0,  d1 * srctexsize.w))
         + main3(uv2 + vec2<f32>(0.0, -d1 * srctexsize.w))) * w1
        + (main3(uv2 + vec2<f32>(0.0,  d2 * srctexsize.w))
         + main3(uv2 + vec2<f32>(0.0, -d2 * srctexsize.w))) * w2;
    blur3 *= w_div;

    // tone it down at the edges:  (only happens on 1st X pass!)
    var t = min(min(in.uv.x, in.uv.y), 1.0 - max(in.uv.x, in.uv.y));
    t = sqrt(t);
    t = edge_darken_c1 + edge_darken_c2 * saturate(t * edge_darken_c3);
    blur3 *= t;

    return vec4<f32>(blur3, 1.0);
}

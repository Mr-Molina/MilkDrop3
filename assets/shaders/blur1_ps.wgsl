// Built-in blur pass 1 (long horizontal pass): hand translation of
// data/blur1_ps.fx (assets/hlsl/blur1_ps.fx). Input: blur_vs.wgsl.
//
// Constants, set per pass by the CPU (milkdropfs.cpp BlurPasses, i%2 == 0):
//   _c0 = source texsize (.xy), and inverse (.zw)
//   _c1 = w1..w4
//   _c2 = d1..d4
//   _c3 = scale, bias, w_div, 0
// derived from the weights exactly as the .fx comments (and the C++) say:
//   const float w[8] = { 4.0, 3.8, 3.5, 2.9, 1.9, 1.2, 0.7, 0.3 };  <- user can specify these
//   const float w1 = w[0] + w[1];
//   const float w2 = w[2] + w[3];
//   const float w3 = w[4] + w[5];
//   const float w4 = w[6] + w[7];
//   const float d1 = 0 + 2*w[1]/w1;
//   const float d2 = 2 + 2*w[3]/w2;
//   const float d3 = 4 + 2*w[5]/w3;
//   const float d4 = 6 + 2*w[7]/w4;
//   const float w_div = 0.5/(w1+w2+w3+w4);
//
// The block mirrors the D3D9 constant registers c0..c6 (shared layout with
// blur2_ps; this pass reads _c0.._c3).

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
    let w1 = blur._c1.x;
    let w2 = blur._c1.y;
    let w3 = blur._c1.z;
    let w4 = blur._c1.w;
    let d1 = blur._c2.x;
    let d2 = blur._c2.y;
    let d3 = blur._c2.z;
    let d4 = blur._c2.w;
    let fscale = blur._c3.x;
    let fbias = blur._c3.y;
    let w_div = blur._c3.z;

    // note: if you just take one sample at exactly uv.xy, you get an avg of 4 pixels.
    let uv2 = in.uv.xy + srctexsize.zw * vec2<f32>(1.0, 1.0); // + moves blur UP, LEFT by 1-pixel increments

    var blur3 =
          (main3(uv2 + vec2<f32>( d1 * srctexsize.z, 0.0))
         + main3(uv2 + vec2<f32>(-d1 * srctexsize.z, 0.0))) * w1
        + (main3(uv2 + vec2<f32>( d2 * srctexsize.z, 0.0))
         + main3(uv2 + vec2<f32>(-d2 * srctexsize.z, 0.0))) * w2
        + (main3(uv2 + vec2<f32>( d3 * srctexsize.z, 0.0))
         + main3(uv2 + vec2<f32>(-d3 * srctexsize.z, 0.0))) * w3
        + (main3(uv2 + vec2<f32>( d4 * srctexsize.z, 0.0))
         + main3(uv2 + vec2<f32>(-d4 * srctexsize.z, 0.0))) * w4;
    blur3 *= w_div;

    blur3 = blur3 * fscale + fbias;

    return vec4<f32>(blur3, 1.0);
}

//! Task 4.4: one test per WGSL emission rule (brief list + F24/F26). Each
//! emits `include.fx` + globals + `void PS(<warp params>) { body }`,
//! validates the module with naga and checks exact snippets.
//! Run with `cargo test -p md-hlsl --test emit`.

mod common;

use common::{emit_ok, has, try_emit};

#[test]
fn float_vectors_are_vec_f32() {
    let w = emit_ok("", "float3 a = float3(1, 2, 3); float2 b = a.xy;");
    has(
        &w,
        &[
            "var h_a: vec3<f32> = vec3<f32>(1.0, 2.0, 3.0);",
            "var h_b: vec2<f32> = h_a.xy;",
        ],
    );
}

#[test]
fn float_matrices_are_stored_transposed_as_mat_r_x_c() {
    let w = emit_ok(
        "",
        "float4x3 m = rot_s1; float2x3 n = float2x3(1, 2, 3, 4, 5, 6);",
    );
    has(
        &w,
        &[
            "var h_m: mat4x3<f32> = U.rot_s1;",
            "var h_n: mat2x3<f32> = mat2x3<f32>(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);",
            "    rot_s1: mat4x3<f32>,",
        ],
    );
}

#[test]
fn mul_vector_matrix_is_matrix_times_vector() {
    let w = emit_ok("", "float4 v = _c0; float3 r = mul(v, rot_s1);");
    has(&w, &["var h_r: vec3<f32> = (U.rot_s1 * h_v);"]);
}

#[test]
fn mul_matrix_vector_is_vector_times_matrix() {
    let w = emit_ok(
        "",
        "float3 v = 1; float4 r = mul(rot_s1, v); float3x3 a = 1; float3x2 b = 2; \
         float3x2 c = mul(a, b); float2 d = mul(_c0.xy, _c1.xy); float3 e = mul(2, v);",
    );
    has(
        &w,
        &[
            "var h_r: vec4<f32> = (h_v * U.rot_s1);",
            "var h_c: mat3x2<f32> = (h_b * h_a);",
            "dot(U._c0.xy, U._c1.xy)",
            "(2.0 * h_v)",
        ],
    );
}

#[test]
fn tex2d_is_texture_sample_on_slot() {
    let w = emit_ok("", "_return_value = tex2D(sampler_main, _uv.xy);");
    has(
        &w,
        &[
            "@group(1) @binding(0) var tex_0: texture_2d<f32>;",
            "@group(1) @binding(1) var samp_0: sampler;",
            "(*h__return_value) = textureSample(tex_0, samp_0, h__uv.xy);",
        ],
    );
}

#[test]
fn tex2dbias_splits_coordinate_and_bias() {
    let w = emit_ok(
        "",
        "float4 c = _c0; float4 a = tex2Dbias(sampler_main, c); \
         float4 b = tex2Dbias(sampler_main, float4(_uv.xy, 0, 1));",
    );
    has(
        &w,
        &[
            "textureSampleBias(tex_0, samp_0, h_c.xy, h_c.w)",
            "let md_t0 = vec4<f32>(h__uv.xy, 0.0, 1.0);",
            "textureSampleBias(tex_0, samp_0, md_t0.xy, md_t0.w)",
        ],
    );
}

#[test]
fn tex2dlod_is_texture_sample_level() {
    let w = emit_ok("", "float4 c = _c0; float4 a = tex2Dlod(sampler_main, c);");
    has(&w, &["textureSampleLevel(tex_0, samp_0, h_c.xy, h_c.w)"]);
}

#[test]
fn tex3d_samples_a_texture_3d() {
    let w = emit_ok("", "float4 a = tex3D(sampler_noisevol_hq, _uv.xyz);");
    has(
        &w,
        &[
            "@group(1) @binding(0) var tex_0: texture_3d<f32>;",
            "textureSample(tex_0, samp_0, h__uv.xyz)",
        ],
    );
}

#[test]
fn saturate_is_clamp_with_splat_bounds() {
    let w = emit_ok(
        "",
        "float a = saturate(_c0.x); float3 b = saturate(_c0.xyz);",
    );
    has(
        &w,
        &[
            "clamp(U._c0.x, 0.0, 1.0)",
            "clamp(U._c0.xyz, vec3<f32>(0.0), vec3<f32>(1.0))",
        ],
    );
}

#[test]
fn lerp_is_mix() {
    let w = emit_ok("", "float3 a = lerp(_c0.xyz, _c1.xyz, 0.25);");
    has(&w, &["mix(U._c0.xyz, U._c1.xyz, vec3<f32>(0.25))"]);
}

#[test]
fn frac_is_fract() {
    let w = emit_ok("", "float2 a = frac(_uv.xy * 4);");
    has(&w, &["fract((h__uv.xy * vec2<f32>(4.0)))"]);
}

#[test]
fn fmod_is_truncated_remainder() {
    let w = emit_ok(
        "",
        "float a = _c0.x; float b = _c0.y; float c = fmod(a, b);",
    );
    has(&w, &["var h_c: f32 = (h_a % h_b);"]);
}

#[test]
fn atan2_is_atan2() {
    let w = emit_ok("", "float a = atan2(_c0.y, _c0.x);");
    has(&w, &["atan2(U._c0.y, U._c0.x)"]);
}

#[test]
fn rsqrt_is_inverse_sqrt() {
    let w = emit_ok("", "float a = rsqrt(_c0.x);");
    has(&w, &["inverseSqrt(U._c0.x)"]);
}

#[test]
fn ddx_ddy_are_dpdx_dpdy() {
    let w = emit_ok("", "float2 a = ddx(_uv.xy) + ddy(_uv.xy);");
    has(&w, &["(dpdx(h__uv.xy) + dpdy(h__uv.xy))"]);
}

#[test]
fn log10_divides_natural_logs() {
    let w = emit_ok("", "float a = log10(_c0.x);");
    has(&w, &["(log(U._c0.x) / log(10.0))"]);
}

#[test]
fn sincos_becomes_two_statements() {
    let w = emit_ok("", "float s, c; sincos(_c0.x, s, c);");
    has(&w, &["h_s = sin(U._c0.x);\n    h_c = cos(U._c0.x);"]);
}

#[test]
fn round_step_smoothstep_pass_through() {
    let w = emit_ok(
        "",
        "float a = round(_c0.x); float b = step(_c0.x, _c0.y); \
         float c = smoothstep(0, 1, _c0.z);",
    );
    has(
        &w,
        &[
            "round(U._c0.x)",
            "step(U._c0.x, U._c0.y)",
            "smoothstep(0.0, 1.0, U._c0.z)",
        ],
    );
}

#[test]
fn sign_returns_int_like_hlsl() {
    let w = emit_ok("", "int s = sign(_c0.x); int3 v = sign(_c0.xyz);");
    has(
        &w,
        &[
            "var h_s: i32 = i32(sign(U._c0.x));",
            "var h_v: vec3<i32> = vec3<i32>(sign(U._c0.xyz));",
        ],
    );
}

#[test]
fn any_all_take_bool_vectors() {
    let w = emit_ok("", "bool a = any(_c0.xyz > 0.5); bool b = all(_c0.xy);");
    has(
        &w,
        &[
            "any((U._c0.xyz > vec3<f32>(0.5)))",
            "all(vec2<bool>(U._c0.xy))",
        ],
    );
}

#[test]
fn discard_is_discard() {
    let w = emit_ok("", "if (_c0.x > 0.5) discard;");
    has(&w, &["if (U._c0.x > 0.5) {\n        discard;\n    }"]);
}

#[test]
fn integer_literals_in_float_context_get_point_zero() {
    let w = emit_ok("", "float a = 2; float b = a * 3; int i = 4;");
    has(
        &w,
        &["var h_a: f32 = 2.0;", "(h_a * 3.0)", "var h_i: i32 = 4i;"],
    );
}

#[test]
fn scalar_types_are_f32_i32_bool() {
    let w = emit_ok("", "float a = 1.5; int i = 2; bool b = true;");
    has(
        &w,
        &[
            "var h_a: f32 = 1.5;",
            "var h_i: i32 = 2i;",
            "var h_b: bool = true;",
        ],
    );
}

#[test]
fn swizzles_pass_through_and_scalar_swizzles_splat() {
    let w = emit_ok(
        "",
        "float4 v = _c0.wzyx; float2 w = v.xx; float s = 1; float3 t = s.xxx;",
    );
    has(
        &w,
        &[
            "= U._c0.wzyx;",
            "var h_w: vec2<f32> = h_v.xx;",
            "var h_t: vec3<f32> = vec3<f32>(h_s);",
        ],
    );
}

#[test]
fn out_and_inout_params_are_function_pointers() {
    let w = emit_ok(
        "void f(inout float x, out float y) { x += 1; y = 2; }",
        "float a = 1, b; f(a, b);",
    );
    has(
        &w,
        &[
            "fn h_f(h_x: ptr<function, f32>, h_y: ptr<function, f32>) {",
            "(*h_x) += 1.0;",
            "(*h_y) = 2.0;",
            "var md_t0: f32 = h_a;\n    var md_t1: f32;\n    h_f(&md_t0, &md_t1);\n    \
             h_a = md_t0;\n    h_b = md_t1;",
        ],
    );
}

#[test]
fn static_globals_are_private_vars() {
    let w = emit_ok(
        "static float k = 2; static float3 m = _c0.xyz;",
        "_return_value = float4(m, k);",
    );
    has(
        &w,
        &[
            "var<private> h_k: f32 = 2.0;",
            "var<private> h_m: vec3<f32>;",
            "fn fs_main(in: VsOut) -> @location(0) vec4<f32> {\n    h_m = U._c0.xyz;",
        ],
    );
}

#[test]
fn static_consts_are_consts() {
    let w = emit_ok(
        "static const float K = 0.5; static const float2 V = float2(K, 2);",
        "_return_value = K * V.xyxy;",
    );
    has(
        &w,
        &[
            "const h_K: f32 = 0.5;",
            "const h_V: vec2<f32> = vec2<f32>(h_K, 2.0);",
        ],
    );
}

#[test]
fn hlsl_identifiers_get_h_prefix_avoiding_wgsl_keywords() {
    let w = emit_ok("", "float var = 1; float let = 2; float ptr = var + let;");
    has(
        &w,
        &[
            "var h_var: f32 = 1.0;",
            "var h_let: f32 = 2.0;",
            "var h_ptr: f32 = (h_var + h_let);",
        ],
    );
}

#[test]
fn tex2dgrad_is_texture_sample_grad() {
    let w = emit_ok(
        "",
        "float4 a = tex2Dgrad(sampler_main, _uv.xy, ddx(_uv.xy), ddy(_uv.xy));",
    );
    has(
        &w,
        &["textureSampleGrad(tex_0, samp_0, h__uv.xy, dpdx(h__uv.xy), dpdy(h__uv.xy))"],
    );
}

#[test]
fn tex2dproj_divides_by_w() {
    let w = emit_ok("", "float4 c = _c0; float4 a = tex2Dproj(sampler_main, c);");
    has(&w, &["textureSample(tex_0, samp_0, (h_c.xy / h_c.w))"]);
}

#[test]
fn tex3dlod_is_texture_sample_level_on_3d() {
    let w = emit_ok(
        "",
        "float4 c = _c0; float4 a = tex3Dlod(sampler_noisevol_lq, c);",
    );
    has(
        &w,
        &[
            "var tex_0: texture_3d<f32>;",
            "textureSampleLevel(tex_0, samp_0, h_c.xyz, h_c.w)",
        ],
    );
}

#[test]
fn modf_returns_fract_and_writes_whole() {
    let w = emit_ok("", "float ip; float f = modf(_c0.x, ip);");
    has(
        &w,
        &["let md_t0 = modf(U._c0.x);\n    h_ip = md_t0.whole;\n    \
             var h_f: f32 = md_t0.fract;"],
    );
}

#[test]
fn frexp_returns_mantissa_and_writes_float_exponent() {
    let w = emit_ok(
        "",
        "float e; float m = frexp(_c0.x, e); float3 e3; float3 m3 = frexp(_c0.xyz, e3);",
    );
    has(
        &w,
        &[
            "let md_t0 = frexp(U._c0.x);\n    h_e = f32(md_t0.exp);\n    \
             var h_m: f32 = md_t0.fract;",
            "h_e3 = vec3<f32>(md_t1.exp);",
        ],
    );
}

#[test]
fn ldexp_multiplies_by_exp2() {
    let w = emit_ok("", "float a = ldexp(_c0.x, _c0.y);");
    has(&w, &["(U._c0.x * exp2(U._c0.y))"]);
}

#[test]
fn texcube_is_a_translate_error() {
    // resolve_bindings already rejects a referenced cube sampler; emit
    // rejects texCUBE itself whatever the bindings say.
    let src = format!(
        "{}\nsamplerCUBE sampler_cube;\nvoid PS(out float4 r : COLOR0) {{ r = texCUBE(sampler_cube, _c0.xyz); }}",
        common::asset("include.fx")
    );
    let t = md_hlsl::check(&md_hlsl::parse(&md_hlsl::preprocess_tokens(&src).unwrap()).unwrap())
        .unwrap();
    let e =
        md_hlsl::emit(&t, md_hlsl::ShaderKind::Warp, &md_hlsl::Bindings::default()).unwrap_err();
    assert!(e.message.contains("texCUBE"), "{e}");
    assert!(e.line > 0);
    assert!(try_emit(
        "samplerCUBE sampler_cube;",
        "_return_value = texCUBE(sampler_cube, 0);"
    )
    .is_err());
}

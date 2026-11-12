//! Task 4.4: WGSL lowerings the type checker leaves to the emitter (ledger
//! notes of Task 4.3) and the WGSL specifics (loops, hoisting, contract).
//! Run with `cargo test -p md-hlsl --test emit_lowering`.

mod common;

use common::{emit_ok, has};

#[test]
fn compound_assign_evaluates_the_place_once() {
    let w = emit_ok("", "int a[3] = {1, 2, 3}; int i = 0; a[i++] *= 1.5;");
    has(
        &w,
        &["let md_t0 = h_i;\n    h_i = (h_i + 1i);\n    \
           h_a[md_t0] = i32((f32(h_a[md_t0]) * 1.5));"],
    );
    assert_eq!(w.matches("(h_i + 1i)").count(), 1);
}

#[test]
fn compound_assign_uses_wgsl_op_or_converts_through_op_ty() {
    let w = emit_ok("", "float x = 1; x += 2; float y = 0; y += _c0.xyz;");
    has(
        &w,
        &["h_x += 2.0;", "h_y = (vec3<f32>(h_y) + U._c0.xyz).x;"],
    );
}

#[test]
fn written_uniforms_get_a_private_copy_initialised_in_fs_main() {
    let w = emit_ok(
        "float4 bump() { _qb.y += 1; return _qb; }",
        "_qa.x = 1; _return_value = _qa + bump();",
    );
    has(
        &w,
        &[
            "var<private> h__qa: vec4<f32>;",
            "var<private> h__qb: vec4<f32>;",
            "h__qa = U._qa;\n    h__qb = U._qb;",
            "h__qa.x = 1.0;",
            "h__qb.y += 1.0;",
        ],
    );
}

#[test]
fn scalar_dot_is_a_product() {
    let w = emit_ok("", "float d = dot(_c0.x, _c1.x);");
    has(&w, &["var h_d: f32 = (U._c0.x * U._c1.x);"]);
}

#[test]
fn matrix_constructor_rows_pass_and_straddling_args_flatten() {
    let w = emit_ok(
        "",
        "float2x3 r = float2x3(_c0.xyz, _c1.xyz); float2x2 s = float2x2(_c0.xyz, _c1.x);",
    );
    has(
        &w,
        &[
            "mat2x3<f32>(U._c0.xyz, U._c1.xyz)",
            "mat2x2<f32>(U._c0.xyz.x, U._c0.xyz.y, U._c0.xyz.z, U._c1.x)",
        ],
    );
}

#[test]
fn matrix_star_is_componentwise() {
    let w = emit_ok(
        "",
        "float2x2 a = float2x2(1, 2, 3, 4); float2x2 b = a * a; b *= a;",
    );
    has(
        &w,
        &[
            "var h_b: mat2x2<f32> = mat2x2<f32>((h_a[0] * h_a[0]), (h_a[1] * h_a[1]));",
            "h_b = mat2x2<f32>((h_b[0] * h_a[0]), (h_b[1] * h_a[1]));",
        ],
    );
}

#[test]
fn matrix_splat_repeats_columns() {
    let w = emit_ok("", "float3x3 m = 0.5; float2x2 n = _c0.x;");
    has(
        &w,
        &[
            "mat3x3<f32>(vec3<f32>(0.5), vec3<f32>(0.5), vec3<f32>(0.5))",
            "mat2x2<f32>(vec2<f32>(U._c0.x), vec2<f32>(U._c0.x))",
        ],
    );
}

#[test]
fn matrix_cast_keeps_the_top_left_block() {
    let w = emit_ok(
        "",
        "float3x3 m = (float3x3)rot_s1; float2x2 n = (float2x2)rot_s1;",
    );
    has(
        &w,
        &[
            "mat3x3<f32>(U.rot_s1[0], U.rot_s1[1], U.rot_s1[2])",
            "mat2x2<f32>(U.rot_s1[0].xy, U.rot_s1[1].xy)",
        ],
    );
}

#[test]
fn matrix_rows_and_elements_index_wgsl_columns() {
    let w = emit_ok(
        "",
        "float3 r = rot_s1[1]; float e = rot_s1._m21; float2 f = rot_s1._12_21;",
    );
    has(
        &w,
        &[
            "var h_r: vec3<f32> = U.rot_s1[1i];",
            "var h_e: f32 = U.rot_s1[2][1];",
            "vec2<f32>(U.rot_s1[0][1], U.rot_s1[1][0])",
        ],
    );
}

#[test]
fn ternary_is_select_and_vector_conditions_select_componentwise() {
    let w = emit_ok(
        "",
        "float s = _c0.x > 0 ? 1 : 2; float3 r = _c0.xyz > 0.5 ? _c1.xyz : _c2.xyz;",
    );
    has(
        &w,
        &[
            "f32(select(2i, 1i, (U._c0.x > 0.0)))",
            "select(U._c2.xyz, U._c1.xyz, (U._c0.xyz > vec3<f32>(0.5)))",
        ],
    );
}

#[test]
fn ternary_with_side_effects_becomes_an_if() {
    let w = emit_ok("", "int i = 0; int t = _c0.x > 0 ? i++ : 0;");
    has(
        &w,
        &["var md_t0: i32;\n    if (U._c0.x > 0.0) {\n        let md_t1 = h_i;\n        \
           h_i = (h_i + 1i);\n        md_t0 = md_t1;\n    } else {\n        md_t0 = 0i;\n    }\n    \
           var h_t: i32 = md_t0;"],
    );
}

#[test]
fn swizzle_writes_assign_components() {
    let w = emit_ok("", "float4 v = 0; v.xz = _c0.xy; v.y = 1; v.zx = v.xz;");
    has(
        &w,
        &[
            "let md_t0 = U._c0.xy;\n    h_v.x = md_t0.x;\n    h_v.z = md_t0.y;",
            "h_v.y = 1.0;",
            "let md_t1 = h_v.xz;\n    h_v.z = md_t1.x;\n    h_v.x = md_t1.y;",
        ],
    );
}

#[test]
fn written_in_params_are_copied_to_locals() {
    let w = emit_ok(
        "float f(float x) { x *= 2; return x; }",
        "_return_value = f(_c0.x);",
    );
    has(
        &w,
        &["fn h_f(md_in_h_x: f32) -> f32 {\n    var h_x: f32 = md_in_h_x;\n    h_x *= 2.0;"],
    );
}

#[test]
fn overloads_use_unique_names() {
    let w = emit_ok(
        "float f(float x) { return x; } float f(float2 x) { return x.y; }",
        "_return_value = f(_c0.x) + f(_c0.xy);",
    );
    has(
        &w,
        &[
            "fn h_f(h_x: f32) -> f32 {",
            "fn h_f__ovl1(h_x: vec2<f32>) -> f32 {",
            // user calls may write globals: the earlier operand is captured
            "let md_t0 = h_f(U._c0.x);",
            "(md_t0 + h_f__ovl1(U._c0.xy))",
        ],
    );
}

#[test]
fn for_loop_with_declaration_is_a_loop_with_continuing() {
    let w = emit_ok("", "float s = 0; for (int i = 0; i < 4; i++) { s += i; }");
    has(
        &w,
        &[
            "var h_i: i32 = 0i;\n    loop {\n        if !((h_i < 4i)) { break; }\n        \
           h_s += f32(h_i);\n        continuing {\n            h_i = (h_i + 1i);\n        }\n    }",
        ],
    );
}

#[test]
fn while_and_do_while_loops() {
    let w = emit_ok(
        "",
        "int i = 0; while (i < 3) i++; do { i++; } while (i < 9);",
    );
    has(
        &w,
        &[
            "loop {\n        if !((h_i < 3i)) { break; }\n        h_i = (h_i + 1i);\n    }",
            "loop {\n        h_i = (h_i + 1i);\n        continuing {\n            \
             break if !((h_i < 9i));\n        }\n    }",
        ],
    );
}

#[test]
fn loop_conditions_with_side_effects_rerun_each_iteration() {
    let w = emit_ok("", "int i = 0; while (i++ < 3) { }");
    has(
        &w,
        &[
            "loop {\n        let md_t0 = h_i;\n        h_i = (h_i + 1i);\n        \
           if !((md_t0 < 3i)) { break; }",
        ],
    );
}

#[test]
fn logical_operators_are_scalar_and_or_and_vector_bitwise() {
    let w = emit_ok(
        "",
        "bool a = _c0.x > 0 && _c0.y > 0; bool2 b = _c0.xy > 0 || _c1.xy > 0;",
    );
    has(
        &w,
        &[
            "((U._c0.x > 0.0) && (U._c0.y > 0.0))",
            "((U._c0.xy > vec2<f32>(0.0)) | (U._c1.xy > vec2<f32>(0.0)))",
        ],
    );
}

#[test]
fn sampler_parameters_become_texture_and_sampler_pairs() {
    let w = emit_ok(
        "float4 g(sampler2D s, float2 p) { return tex2D(s, p); }",
        "_return_value = g(sampler_blur1, _uv.xy);",
    );
    has(
        &w,
        &[
            "fn h_g(md_tex_h_s: texture_2d<f32>, md_samp_h_s: sampler, h_p: vec2<f32>) -> vec4<f32> {",
            "return textureSample(md_tex_h_s, md_samp_h_s, h_p);",
            "h_g(tex_0, samp_0, h__uv.xy)",
        ],
    );
}

#[test]
fn division_by_literal_zero_is_not_const_evaluated() {
    let w = emit_ok("", "int a = 7; int b = a / 0; float c = _c0.x % 0;");
    has(
        &w,
        &[
            "var md_t0: i32 = 0i;\n    var h_b: i32 = (h_a / md_t0);",
            "var md_t1: f32 = 0.0;",
        ],
    );
}

#[test]
fn output_contract_interface() {
    let w = emit_ok(
        "float4 texsize_main;",
        "_return_value = texsize_main + texsize_noise_lq + tex2D(sampler_noise_lq, 0) \
         + tex2D(sampler_main, 0);",
    );
    assert!(w.starts_with("diagnostic(off, derivative_uniformity);\n"));
    has(
        &w,
        &[
            "struct VsOut {\n    @builtin(position) pos: vec4<f32>,\n    \
             @location(0) diffuse: vec4<f32>,\n    @location(1) uv: vec4<f32>,\n    \
             @location(2) rad_ang: vec2<f32>,\n}",
            &md_hlsl::layout::md_uniforms_wgsl(),
            "@group(0) @binding(0) var<uniform> U: MdUniforms;",
            "struct TexSizes {\n    texsize_main: vec4<f32>,\n    texsize_noise_lq: vec4<f32>,\n}",
            "@group(0) @binding(1) var<uniform> T: TexSizes;",
            // slots in declaration order: sampler_main (include.fx) first
            "@group(1) @binding(0) var tex_0: texture_2d<f32>;\n\
             @group(1) @binding(1) var samp_0: sampler;\n\
             @group(1) @binding(2) var tex_1: texture_2d<f32>;\n\
             @group(1) @binding(3) var samp_1: sampler;",
            "(T.texsize_main + T.texsize_noise_lq)",
            "textureSample(tex_1, samp_1, vec2<f32>(0.0))",
            "@fragment\nfn fs_main(in: VsOut) -> @location(0) vec4<f32> {\n    \
             var md_out: vec4<f32>;\n    h_PS(in.diffuse, in.uv, in.rad_ang, &md_out);\n    \
             return md_out;\n}",
        ],
    );
    let w = emit_ok("", "_return_value = 1;");
    has(&w, &["struct TexSizes {\n    md_unused: vec4<f32>,\n}"]);
}

#[test]
fn non_void_functions_get_a_fallback_return() {
    let w = emit_ok(
        "float f(float x) { if (x > 0) return 1; else return 2; }",
        "_return_value = f(_c0.x);",
    );
    has(&w, &["return f32();\n}"]);
}

#[test]
fn ps_without_entry_or_with_unknown_semantic_is_an_error() {
    let inc = common::asset("include.fx");
    let typed = |src: &str| {
        md_hlsl::check(&md_hlsl::parse(&md_hlsl::preprocess_tokens(src).unwrap()).unwrap()).unwrap()
    };
    let t = typed(&format!(
        "{inc}\nvoid main(out float4 r : COLOR0) {{ r = 0; }}"
    ));
    let e = common::emit_typed(&t, md_hlsl::ShaderKind::Warp).unwrap_err();
    assert!(e.message.contains("PS"), "{e}");
    let t = typed(&format!(
        "{inc}\nvoid PS(float4 p : POSITION, out float4 r : COLOR0) {{ r = p; }}"
    ));
    let e = common::emit_typed(&t, md_hlsl::ShaderKind::Warp).unwrap_err();
    assert!(e.message.contains("POSITION"), "{e}");
}

#[test]
fn deep_expressions_do_not_overflow() {
    let chain = vec!["a"; 500].join(" + ");
    let swz = format!("a{}", ".x".repeat(500));
    let ifs = "if (a > 0) ".repeat(60);
    let blocks = format!("{}a = 2;{}", "{".repeat(60), "}".repeat(60));
    let neg = format!("{}a{}", "-(".repeat(60), ")".repeat(60));
    let body = format!(
        "float a = 1; float b = {chain}; float c = {swz}; {ifs} a = 1; {blocks} \
         float d = {neg}; _return_value = b + c + d;"
    );
    let src = format!(
        "{}\nvoid PS({}) {{ {body} }}",
        common::asset("include.fx"),
        common::WARP_PARAMS
    );
    let t = md_hlsl::check(&md_hlsl::parse(&md_hlsl::preprocess_tokens(&src).unwrap()).unwrap())
        .unwrap();
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            common::emit_typed(&t, md_hlsl::ShaderKind::Warp).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn only_reachable_structs_are_emitted() {
    // An unused struct WGSL cannot express must not fail the shader.
    let w = emit_ok(
        "struct Bad { int2x2 m; }; struct Inner { float x; }; \
         struct Outer { Inner i[2]; }; struct Unused { float y; };",
        "Outer o; o.i[1].x = 2; _return_value = o.i[1].x;",
    );
    has(&w, &["struct h_Outer {", "struct h_Inner {"]);
    assert!(!w.contains("h_Bad") && !w.contains("h_Unused"), "{w}");
}

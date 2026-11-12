//! Task 4.6: wrapper assembly and the public `translate` entry point.

mod common;

use common::asset;
use md_hlsl::{translate, PsVersion, ShaderInput, ShaderKind, TranslateError, Translated};

fn run(kind: ShaderKind, body: &str) -> Result<Translated, TranslateError> {
    let preamble = asset("include.fx");
    translate(&ShaderInput {
        kind,
        body,
        preamble: &preamble,
        ps_version: PsVersion::Ps2_0,
    })
}

#[track_caller]
fn ok(kind: ShaderKind, body: &str) -> Translated {
    run(kind, body).unwrap_or_else(|e| panic!("translate: {e}"))
}

#[test]
fn default_warp_shader_translates() {
    let t = ok(ShaderKind::Warp, &asset("warp_ps.fx"));
    common::naga_validate(&t.wgsl).unwrap();
    assert!(t.wgsl.contains("@fragment"));
    assert!(t
        .wgsl
        .contains("fn fs_main(in: VsOut) -> @location(0) vec4<f32>"));
    common::has(
        &t.wgsl,
        &[
            "var h_ret: vec3<f32> = vec3<f32>(0.0);",
            "(*h__return_value) = vec4<f32>(h_ret.xyz, h__vDiffuse.w);",
        ],
    );
    let roots: Vec<&str> = t.samplers.iter().map(|s| s.root.as_str()).collect();
    assert!(roots.contains(&"main"), "{roots:?}");
    assert_eq!(t.samplers[0].slot, 0);
}

#[test]
fn default_comp_shader_translates() {
    let t = ok(ShaderKind::Comp, &asset("comp_ps.fx"));
    common::naga_validate(&t.wgsl).unwrap();
    assert!(t.wgsl.contains("fn fs_main(in: VsOut)"));
    assert!(t.samplers.iter().any(|s| s.root == "main"));
}

#[test]
fn shader_body_missing_is_error() {
    let e = run(ShaderKind::Warp, "float4 x;\n").unwrap_err();
    assert!(e.message.contains("shader_body"), "{e}");
    let e = run(ShaderKind::Comp, "").unwrap_err();
    assert!(e.message.contains("shader_body"), "{e}");
}

#[test]
fn comp_defines_hue_shader() {
    let t = ok(ShaderKind::Comp, "shader_body { ret = hue_shader; }");
    assert!(t.wgsl.contains("h_ret = h__vDiffuse.xyz;"), "{}", t.wgsl);
    // warp has no hue_shader define
    let e = run(ShaderKind::Warp, "shader_body { ret = hue_shader; }").unwrap_err();
    assert!(e.message.contains("hue_shader"), "{e}");
}

#[test]
fn warp_uv_orig_is_zw_and_comp_uv_orig_is_xy() {
    let body = "shader_body { ret = float3(uv_orig, rad + ang); }";
    let w = ok(ShaderKind::Warp, body);
    assert!(w.wgsl.contains("h__uv.zw"), "{}", common::body_of(&w.wgsl));
    let c = ok(ShaderKind::Comp, body);
    assert!(!c.wgsl.contains("h__uv.zw"));
    assert!(c.wgsl.contains("h__uv.xy"), "{}", common::body_of(&c.wgsl));
    for t in [&w, &c] {
        assert!(t.wgsl.contains("h__rad_ang.x"));
        assert!(t.wgsl.contains("h__rad_ang.y"));
    }
}

#[test]
fn error_lines_are_relative_to_the_preset_body() {
    let e = run(
        ShaderKind::Warp,
        "float4 g;\nshader_body\n{\n    ret = 1;\n    ret = nope;\n}\n",
    )
    .unwrap_err();
    assert_eq!((e.line, e.col), (5, 11), "{e}");
    assert!(!e.message.contains("include.fx"), "{e}");
    // missing `;` is a parse error on the body's line
    let e = run(ShaderKind::Comp, "shader_body {\n ret = 1\n ret = 2;\n}").unwrap_err();
    assert_eq!(e.line, 3, "{e}");
}

#[test]
fn preamble_errors_say_so() {
    let preamble = "float4 a;\nfloat4 b = ;\n";
    let e = translate(&ShaderInput {
        kind: ShaderKind::Warp,
        body: "shader_body { ret = 0; }",
        preamble,
        ps_version: PsVersion::Ps3_0,
    })
    .unwrap_err();
    assert_eq!(e.line, 2, "{e}");
    assert!(e.message.contains("include.fx"), "{e}");
}

#[test]
fn missing_semicolon_before_closing_brace_is_tolerated() {
    // the last statement of shader_body may omit `;` (Task 4.2 ruling)
    ok(
        ShaderKind::Warp,
        "shader_body { ret = tex2D(sampler_main, uv).xyz }",
    );
}

#[test]
fn ps_version_selects_no_feature_and_ps4_is_rejected() {
    let preamble = asset("include.fx");
    let body = asset("warp_ps.fx");
    let go = |ps_version| {
        translate(&ShaderInput {
            kind: ShaderKind::Warp,
            body: &body,
            preamble: &preamble,
            ps_version,
        })
    };
    let base = go(PsVersion::Ps2_0).unwrap();
    for v in [PsVersion::None, PsVersion::Ps2_X, PsVersion::Ps3_0] {
        assert_eq!(go(v).unwrap(), base);
    }
    assert!(go(PsVersion::Ps4_0).unwrap_err().message.contains("ps_4_0"));
}

#[test]
fn highest_blur_comes_from_bound_blur_samplers() {
    assert_eq!(ok(ShaderKind::Comp, &asset("comp_ps.fx")).highest_blur, 0);
    let t = ok(
        ShaderKind::Comp,
        "shader_body { ret = GetBlur2(uv) + tex2D(sampler_blur1, uv).xyz; }",
    );
    assert_eq!(t.highest_blur, 2);
    let names: Vec<&str> = t.samplers.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"sampler_blur2"), "{names:?}");
}

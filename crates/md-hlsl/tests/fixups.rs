//! Task 4.7: one test per legacy corpus idiom; each loads a fixture from
//! `tests/fixtures/` and asserts `translate` succeeds. Idioms an earlier
//! task already handles keep their test (so a regression is caught) and get
//! no source fixup; see the "covered by" notes.

mod common;

use common::{asset, fixture};
use md_hlsl::{apply_fixups, translate, PsVersion, ShaderInput, ShaderKind, Translated};

#[track_caller]
fn ok(kind: ShaderKind, name: &str) -> Translated {
    let preamble = asset("include.fx");
    let body = fixture(name);
    let t = translate(&ShaderInput {
        kind,
        body: &body,
        preamble: &preamble,
        ps_version: PsVersion::Ps2_X,
    })
    .unwrap_or_else(|e| panic!("{name}: {e}"));
    common::naga_validate(&t.wgsl).unwrap();
    t
}

fn roots(t: &Translated) -> Vec<&str> {
    t.samplers.iter().map(|s| s.root.as_str()).collect()
}

/// Covered by Task 4.2 (tokens after the `shader_body` block are dropped).
#[test]
fn trailing_semicolon_after_body() {
    ok(ShaderKind::Warp, "trailing_semicolon_after_body.hlsl");
}

/// Covered by Task 4.3 (single-scalar constructor → `Splat`).
#[test]
fn single_arg_vector_ctor_splats() {
    let t = ok(ShaderKind::Warp, "single_arg_vector_ctor.hlsl");
    common::has(&t.wgsl, &["vec2<f32>(h__rad_ang.x)", "vec3<f32>(0.5)"]);
}

/// Covered by Tasks 4.2-4.4 (swizzle into `ret`; expression statements).
#[test]
fn xyz_swizzle_into_float3_ret() {
    ok(ShaderKind::Comp, "xyz_swizzle_to_float3_ret.hlsl");
}

/// Covered by Task 4.3 (implicit float4 → float3 truncation).
#[test]
fn float4_texture_result_truncates_into_ret() {
    let t = ok(ShaderKind::Warp, "float4_to_ret_truncation.hlsl");
    assert_eq!(roots(&t), ["main", "blur1"]);
}

/// Covered by Task 4.1 (a redefinition replaces the macro, as fxc's X1519
/// warning does).
#[test]
fn define_redefinition_takes_the_new_body() {
    let t = ok(ShaderKind::Comp, "define_redefinition.hlsl");
    assert!(t.wgsl.contains("0.75"), "{}", common::body_of(&t.wgsl));
    assert!(!t.wgsl.contains("0.25"));
}

/// Needs [`apply_fixups`]: without it the `\` joins the next line into the
/// comment (C line splicing), losing `#define BOOST`, `shader_body` and the
/// blur sample.
#[test]
fn line_comment_ending_in_backslash_does_not_continue() {
    let t = ok(ShaderKind::Warp, "line_comment_backslash.hlsl");
    assert_eq!(roots(&t), ["main", "blur1"]);
    assert!(t.wgsl.contains("2.0"), "{}", common::body_of(&t.wgsl));
}

/// Covered by Task 4.4's `h_` prefix (preflight F28: no fixup).
#[test]
fn wgsl_keyword_identifiers() {
    ok(ShaderKind::Warp, "wgsl_keyword_identifiers.hlsl");
}

/// Covered by Task 4.6's comp defines (`uv_orig` = `_uv.xy`; F28: no fixup).
#[test]
fn comp_uv_orig_aliases_uv() {
    let t = ok(ShaderKind::Comp, "comp_uv_orig.hlsl");
    assert!(t.wgsl.contains("h__uv.xy"));
}

#[test]
fn apply_fixups_strips_line_comments_like_strip_comments() {
    let src = "a; // x \\\nb; /* // not a line comment */ c;\n/* multi\n// inside */ d; // e\n";
    assert_eq!(
        apply_fixups(src),
        "a; \nb; /* // not a line comment */ c;\n/* multi\n// inside */ d; \n"
    );
    // line count (and so every error line) is preserved
    let body = fixture("line_comment_backslash.hlsl");
    assert_eq!(apply_fixups(&body).lines().count(), body.lines().count());
    assert_eq!(apply_fixups("x;\r\n// c\r\ny;"), "x;\r\n\r\ny;");
}

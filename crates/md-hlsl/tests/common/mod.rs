//! Shared helpers of the md-hlsl integration tests: the preamble, the
//! Task 4.6 wrapper (`md_hlsl::parse_wrapped`), and the emit + naga
//! validation pipeline.
#![allow(dead_code)]

use md_hlsl::{Bindings, ShaderKind, TranslateError, TypedModule};

pub const WARP_PARAMS: &str = "float4 _vDiffuse : COLOR, float4 _uv : TEXCOORD0, \
    float2 _rad_ang : TEXCOORD1, out float4 _return_value : COLOR0";

pub fn asset(name: &str) -> String {
    let p = format!("{}/../../assets/hlsl/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

pub fn fixture(name: &str) -> String {
    let p = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// `include.fx` + defines + a preset shader text, parsed and wrapped by the
/// real Task 4.6 wrapper ([`md_hlsl::parse_wrapped`]).
pub fn wrap_preset(body: &str, warp: bool) -> Result<md_hlsl::ast::Module, TranslateError> {
    let preamble = asset("include.fx");
    md_hlsl::parse_wrapped(&md_hlsl::ShaderInput {
        kind: if warp {
            ShaderKind::Warp
        } else {
            ShaderKind::Comp
        },
        body,
        preamble: &preamble,
        ps_version: md_hlsl::PsVersion::Ps2_0,
    })
}

/// check → resolve_bindings → emit.
pub fn emit_typed(t: &TypedModule, kind: ShaderKind) -> Result<String, TranslateError> {
    let b: Bindings = md_hlsl::resolve_bindings(t)?;
    md_hlsl::emit(t, kind, &b)
}

/// Validate WGSL with naga; the error is naga's rendered message.
pub fn naga_validate(wgsl: &str) -> Result<naga::Module, String> {
    md_hlsl::validate_wgsl(wgsl)?;
    naga::front::wgsl::parse_str(wgsl).map_err(|e| e.emit_to_string(wgsl))
}

/// Emit `include.fx` + `globals` + `void PS(<warp params>) { body }`.
pub fn try_emit(globals: &str, body: &str) -> Result<String, TranslateError> {
    let src = format!(
        "{}\n{globals}\nvoid PS({WARP_PARAMS}) {{\n{body}\n}}\n",
        asset("include.fx")
    );
    let t = md_hlsl::check(&md_hlsl::parse(&md_hlsl::preprocess_tokens(&src)?)?)?;
    emit_typed(&t, ShaderKind::Warp)
}

/// [`try_emit`], which must succeed and validate under naga.
pub fn emit_ok(globals: &str, body: &str) -> String {
    let w = try_emit(globals, body).unwrap_or_else(|e| panic!("emit: {e}"));
    if let Err(e) = naga_validate(&w) {
        panic!("naga rejected:\n{e}\n--- WGSL ---\n{w}");
    }
    w
}

/// Assert `wgsl` contains every snippet.
#[track_caller]
pub fn has(wgsl: &str, snippets: &[&str]) {
    for s in snippets {
        assert!(wgsl.contains(s), "missing `{s}` in:\n{}", body_of(wgsl));
    }
}

/// The emitted text after the interface blocks (for failure messages).
pub fn body_of(wgsl: &str) -> &str {
    wgsl.find("var<uniform> T").map_or(wgsl, |i| &wgsl[i..])
}

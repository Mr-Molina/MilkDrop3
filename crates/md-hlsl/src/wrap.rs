//! Input assembly and the `shader_body` wrap, mirroring
//! `plugin.cpp::LoadShaderFromMemory` (code/vis_milk2/plugin.cpp:3490-3650).
//!
//! The C++ builds one text: the preamble (`include.fx`), then the
//! type-specific `#define`s, then the preset body with `shader_body`
//! replaced by `void PS(<params>)`, `float3 ret = 0;` inserted after the
//! first `{` and `_return_value = float4(ret.xyz, _vDiffuse.w);` before the
//! last `}`. Here the first two parts are assembled as text (so the body sees
//! the defines exactly as fxc did), while the wrap itself is done on the AST
//! (ledger: Task 4.2 ruling binding Task 4.6) — the parser's
//! missing-`;`-before-`}` tolerance only applies inside `Decl::ShaderBody`.
//!
//! Differences from the C++, all on inputs it mishandled:
//! - A body without `shader_body` is a [`TranslateError`] (the C++ scanned
//!   past the end of its buffer).
//! - The inserted first/last statements are not subject to macros the body
//!   `#define`s (the C++ pasted them as text into the macro-expanded region);
//!   no known preset redefines `ret`, `_return_value` or `_vDiffuse`.

use crate::ast::{Decl, Module};
use crate::{parse, preprocess_tokens, tokenize, ShaderKind, TranslateError};

/// `szWarpDefines` (plugin.cpp:3493).
pub(crate) const WARP_DEFINES: &str = "#define rad _rad_ang.x\n\
    #define ang _rad_ang.y\n\
    #define uv _uv.xy\n\
    #define uv_orig _uv.zw\n";
/// `szCompDefines` (plugin.cpp:3497); `uv_orig` is `_uv.xy` [sic].
pub(crate) const COMP_DEFINES: &str = "#define rad _rad_ang.x\n\
    #define ang _rad_ang.y\n\
    #define uv _uv.xy\n\
    #define uv_orig _uv.xy\n\
    #define hue_shader _vDiffuse.xyz\n";
/// `szWarpParams` (plugin.cpp:3502).
const WARP_PARAMS: &str = "float4 _vDiffuse : COLOR, float4 _uv : TEXCOORD0, \
    float2 _rad_ang : TEXCOORD1, out float4 _return_value : COLOR0";
/// `szCompParams` (plugin.cpp:3503).
const COMP_PARAMS: &str = "float4 _vDiffuse : COLOR, float2 _uv : TEXCOORD0, \
    float2 _rad_ang : TEXCOORD1, out float4 _return_value : COLOR0";
/// `szFirstLine` (plugin.cpp:3504).
const FIRST_LINE: &str = "float3 ret = 0;";
/// `szLastLine` (plugin.cpp:3505).
const LAST_LINE: &str = "_return_value = float4(ret.xyz, _vDiffuse.w);";

/// The assembled source and where its parts start, for error mapping.
pub(crate) struct Assembled {
    pub src: String,
    /// Lines taken by the preamble (the defines start on the next line).
    preamble_lines: u32,
    /// Lines before the body (preamble + defines).
    body_offset: u32,
}

fn defines(kind: ShaderKind) -> &'static str {
    match kind {
        ShaderKind::Warp => WARP_DEFINES,
        ShaderKind::Comp => COMP_DEFINES,
    }
}

fn line_count(s: &str) -> u32 {
    u32::try_from(s.bytes().filter(|&b| b == b'\n').count()).unwrap_or(u32::MAX)
}

/// Preamble, then the defines, then the body. The C++ pastes the defines
/// directly after the include text (which ends with a newline); a preamble
/// without a trailing newline gets one, so the first define starts a line.
pub(crate) fn assemble(kind: ShaderKind, preamble: &str, body: &str) -> Assembled {
    let d = defines(kind);
    let mut src = String::with_capacity(preamble.len() + d.len() + body.len() + 1);
    src.push_str(preamble);
    if !preamble.is_empty() && !preamble.ends_with('\n') {
        src.push('\n');
    }
    let preamble_lines = line_count(&src);
    src.push_str(d);
    let body_offset = line_count(&src);
    src.push_str(body);
    Assembled {
        src,
        preamble_lines,
        body_offset,
    }
}

impl Assembled {
    /// Rebase an error position from the assembled source to the part it
    /// lies in: body lines become body-relative (ledger: Task 4.1 note
    /// binding Task 4.6); preamble and define lines keep their line within
    /// that part and say so in the message. Line 0 (no position) is kept.
    pub(crate) fn map_error(&self, mut e: TranslateError) -> TranslateError {
        if e.line == 0 {
            return e;
        }
        if e.line > self.body_offset {
            e.line -= self.body_offset;
        } else if e.line > self.preamble_lines {
            e.line -= self.preamble_lines;
            e.message = format!("in the shader-type #defines: {}", e.message);
        } else {
            e.message = format!("in the preamble (include.fx): {}", e.message);
        }
        e
    }
}

/// Replace the trailing `Decl::ShaderBody` of `m` by
/// `void PS(<params>) { float3 ret = 0; <body> _return_value = ...; }`.
/// The inserted parameters and statements carry the position of the
/// `shader_body` keyword, so errors they cause point there.
pub(crate) fn wrap(m: &mut Module, kind: ShaderKind) -> Result<(), TranslateError> {
    let Some(Decl::ShaderBody(sb)) = m.decls.pop() else {
        return Err(TranslateError::new(
            0,
            0,
            "the shader text has no `shader_body { ... }` block",
        ));
    };
    let params = match kind {
        ShaderKind::Warp => WARP_PARAMS,
        ShaderKind::Comp => COMP_PARAMS,
    };
    let text = format!("void PS({params}) {{ {FIRST_LINE} {LAST_LINE} }}");
    let mut toks = tokenize(&text)?;
    for t in &mut toks {
        t.line = sb.pos.line;
        t.col = sb.pos.col;
    }
    let Some(Decl::Function(mut f)) = parse(&toks)?.decls.pop() else {
        unreachable!("the wrapper text is one function");
    };
    f.pos = sb.pos;
    let last = f.body.stmts.pop().expect("wrapper has two statements");
    f.body.stmts.extend(sb.body.stmts);
    f.body.stmts.push(last);
    f.body.pos = sb.body.pos;
    m.decls.push(Decl::Function(f));
    Ok(())
}

/// Assemble, preprocess, parse and wrap; errors are NOT yet rebased.
pub(crate) fn parse_assembled(a: &Assembled, kind: ShaderKind) -> Result<Module, TranslateError> {
    let mut m = parse(&preprocess_tokens(&a.src)?)?;
    wrap(&mut m, kind)?;
    Ok(m)
}

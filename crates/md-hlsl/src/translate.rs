//! The public entry point (A.3): [`translate`] composes the wrapper
//! assembly, preprocess, lex, parse, type check, binding resolution, WGSL
//! emission and naga validation.

use crate::ast::Module;
use crate::wrap::{assemble, parse_assembled, Assembled};
use crate::{check, emit, resolve_bindings, PsVersion, SamplerBinding, ShaderKind, TranslateError};

/// One preset shader to translate (A.3).
#[derive(Clone, Copy, Debug)]
pub struct ShaderInput<'a> {
    pub kind: ShaderKind,
    /// The preset's shader text (`shader_body { ... }` plus any globals and
    /// helper functions before it), with `\n` line ends.
    pub body: &'a str,
    /// The shared preamble, normally `assets/hlsl/include.fx`.
    pub preamble: &'a str,
    /// The preset's `PSVERSION_WARP`/`PSVERSION_COMP`.
    ///
    /// The C++ only uses it to pick the fxc profile (plugin.cpp:3370-3385:
    /// `None` and `Ps2_0` -> `ps_2_0`, `Ps2_X` -> `ps_2_a` then `ps_2_b`,
    /// `Ps3_0` -> `ps_3_0`, `Ps4_0` -> `ps_4_0`). Profiles differ only in
    /// instruction/register limits, which a WGSL target does not have, so
    /// every supported version translates the same accepted language
    /// identically. `Ps4_0` is rejected: D3DX9 cannot compile `ps_4_0`, so
    /// MilkDrop never ran such a shader ("not supported by milkdrop",
    /// plugin.h:98).
    pub ps_version: PsVersion,
}

/// A translated shader (A.3).
#[derive(Clone, Debug, PartialEq)]
pub struct Translated {
    /// The validated WGSL module (output contract of the M4 intro).
    pub wgsl: String,
    /// Bound samplers in slot order.
    pub samplers: Vec<SamplerBinding>,
    /// Full `texsize_*` global names, in `TexSizes` field order.
    pub texsize_names: Vec<String>,
    /// Highest blur level (0..=3) the shader samples, derived from the
    /// bound samplers' roots `blur1..3` — md-render's `highest_used` (blur
    /// passes beyond it are skipped). Addition to A.3.
    pub highest_blur: u8,
}

fn check_version(v: PsVersion) -> Result<(), TranslateError> {
    if v == PsVersion::Ps4_0 {
        return Err(TranslateError::new(
            0,
            0,
            "pixel shader version ps_4_0 is not supported (MilkDrop could not compile it)",
        ));
    }
    Ok(())
}

fn assembled(input: &ShaderInput) -> Assembled {
    let body = crate::apply_fixups(input.body);
    assemble(input.kind, input.preamble, &body)
}

/// The wrapped AST `translate` type-checks: preamble + defines + body
/// (after [`crate::apply_fixups`]) parsed, with `shader_body` turned into `void PS(...)`. For tests and diagnostics. Errors are body-relative like
/// [`translate`]'s; positions inside the returned module refer to the
/// assembled source (preamble first).
pub fn parse_wrapped(input: &ShaderInput) -> Result<Module, TranslateError> {
    check_version(input.ps_version)?;
    let a = assembled(input);
    parse_assembled(&a, input.kind).map_err(|e| a.map_error(e))
}

/// Translate one preset shader to a validated WGSL module.
///
/// Error positions are 1-based lines/columns of `input.body`; an error in
/// the preamble or the shader-type defines keeps its line within that part
/// and its message says so; line 0 means no position. A WGSL module naga
/// rejects is a translator bug, returned as an error at line 0 carrying
/// naga's message.
pub fn translate(input: &ShaderInput) -> Result<Translated, TranslateError> {
    check_version(input.ps_version)?;
    let a = assembled(input);
    let (wgsl, b) = (|| {
        let m = parse_assembled(&a, input.kind)?;
        let t = check(&m)?;
        let b = resolve_bindings(&t)?;
        let wgsl = emit(&t, input.kind, &b)?;
        Ok((wgsl, b))
    })()
    .map_err(|e| a.map_error(e))?;
    validate_wgsl(&wgsl).map_err(|msg| {
        TranslateError::new(
            0,
            0,
            format!("internal translator error: the generated WGSL failed naga validation:\n{msg}"),
        )
    })?;
    Ok(Translated {
        wgsl,
        samplers: b.samplers,
        texsize_names: b.texsize_names,
        highest_blur: b.highest_blur,
    })
}

/// Parse and validate a WGSL module with naga; the error is naga's
/// rendered message.
pub fn validate_wgsl(wgsl: &str) -> Result<(), String> {
    let m = naga::front::wgsl::parse_str(wgsl).map_err(|e| e.emit_to_string(wgsl))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&m)
    .map_err(|e| e.emit_to_string(wgsl))?;
    Ok(())
}

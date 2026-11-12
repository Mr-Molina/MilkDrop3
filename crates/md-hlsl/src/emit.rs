//! WGSL emitter (Task 4.4): [`emit`] turns a checked [`TypedModule`] plus
//! its [`Bindings`] into one WGSL module honouring the Milestone 4 output
//! contract (`fs_main`, `VsOut`, `U: MdUniforms`, `T: TexSizes`, `tex_i` /
//! `samp_i` at `@group(1) @binding(2*i / 2*i+1)`, and a leading
//! `diagnostic(off, derivative_uniformity);`).
//!
//! Naming: every HLSL identifier (locals, parameters, statics, constants,
//! user functions — by their unique `name__ovl<k>` names —, structs and
//! struct fields) is prefixed `h_`, which also keeps WGSL reserved words
//! out of the output. Names the emitter invents start with `md_`
//! (temporaries `md_t<N>`, copies of written `in` parameters `md_in_h_x`,
//! texture/sampler parameters `md_tex_h_s` / `md_samp_h_s`); contract names
//! (`fs_main`, `VsOut`, `in`, `U`, `T`, `tex_i`, `samp_i`, the `MdUniforms`
//! and `TexSizes` fields) are not prefixed.
//!
//! Globals:
//! - a read-only global named like an `MdUniforms` field ([`crate::layout`])
//!   is spelled `U.<name>`, a read-only `texsize_*` global `T.<name>`; the
//!   HLSL declaration must have the field's type;
//! - a global flagged [`TGlobal::written`] (MilkDrop allowed writes to
//!   uniforms) becomes `var<private> h_<name>` initialised at the start of
//!   `fs_main` from `U`/`T` — user functions see the same copy;
//! - any other uniform (a body-declared `float4 foo;`, which MilkDrop never
//!   sets) is a `var<private>` initialised from its initializer, else zero.
//!   This differs from the C++: plugin.cpp never applies constant-table
//!   defaults (no `SetDefaults`), so there the value is undefined (whatever
//!   the constant register last held); the initializer is the author's
//!   evident intent;
//! - `static` globals are `var<private>` whose initializers run at the
//!   start of `fs_main`, in declaration order;
//! - `static const` globals are `const` when the initializer is a WGSL
//!   const-expression, else they are treated like `static` globals;
//! - samplers become the `tex_i`/`samp_i` pair of their [`Bindings`] slot.
//!
//! Literals: a float literal outside the `f32` range becomes `±f32::MAX`
//! (fxc produced infinity; WGSL rejects an out-of-range literal) and NaN
//! becomes `0.0`; an int literal is saturated to `0..=i32::MAX` (literals
//! are never negative: `-` is a separate operator).
//!
//! Only declarations (globals, functions, structs) reachable from `PS` are
//! emitted. `PS` itself becomes
//! the ordinary function `h_PS`, called by `fs_main`, which maps each `PS`
//! parameter by semantic: `COLOR`/`COLOR0` ← `in.diffuse`, `TEXCOORD`/
//! `TEXCOORD0` ← `in.uv`, `TEXCOORD1` ← `in.rad_ang` (truncated to the
//! parameter's width), `out` `COLOR`/`COLOR0`/`SV_Target` → the return value.
//!
//! The expression rules are documented in `emit_expr.rs`, `emit_call.rs`
//! and `emit_place.rs`; statements in `emit_stmt.rs`.

use std::collections::{HashMap, HashSet};

use crate::ast::{Lit, Pos};
use crate::bindings::{reach, Bindings, Reach};
use crate::emit_stmt::{visit_types, Fx};
use crate::layout::{self, md_uniform_kind, TEXSIZES_PLACEHOLDER};
use crate::typed::{
    Storage, TDecl, TExpr, TExprKind, TFunction, TGlobal, TStruct, TypedModule, VarOrigin,
};
use crate::types::{Scalar, Shape, Ty};
use crate::TranslateError;

pub(crate) type R<T> = Result<T, TranslateError>;

pub(crate) fn err<T>(pos: Pos, msg: impl Into<String>) -> R<T> {
    Err(TranslateError::new(pos.line, pos.col, msg))
}

/// Which preset shader is translated (A.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderKind {
    Warp,
    Comp,
}

/// How a reference to a global is spelled.
#[derive(Clone, Debug)]
pub(crate) enum GlobalRef {
    Expr(String),
    Sampler(u32),
}

/// Module-wide emission context.
pub(crate) struct Ctx<'a> {
    pub globals: HashMap<&'a str, GlobalRef>,
    pub funcs: HashMap<&'a str, &'a TFunction>,
    /// Names of `static const` globals emitted as WGSL `const`.
    pub consts: HashSet<&'a str>,
}

/// Emit the WGSL module for `m` (checked by [`crate::check`]) with the
/// sampler slots and `texsize_*` names of `bindings`
/// ([`crate::resolve_bindings`] on the same module).
///
/// Returns `Result` rather than the brief's bare `String` (F26): constructs
/// the checker accepts but WGSL cannot express are positioned errors —
/// `texCUBE`, integer/bool matrices, matrix comparisons, a missing `PS`
/// entry point or a `PS` parameter with an unknown semantic, a uniform
/// redeclared with a type different from its `MdUniforms`/`TexSizes` field,
/// and sampler-typed locals or struct fields. `kind` only labels the module:
/// the entry point is mapped by semantic.
pub fn emit(m: &TypedModule, kind: ShaderKind, bindings: &Bindings) -> R<String> {
    let reach = reach(m);
    let entry = m.decls.iter().find_map(|d| match d {
        TDecl::Function(f) if f.hlsl_name == "PS" => Some(f),
        _ => None,
    });
    let Some(entry) = entry else {
        return err(Pos::default(), "no entry point `PS`");
    };
    let mut cx = Ctx {
        globals: HashMap::new(),
        funcs: HashMap::new(),
        consts: HashSet::new(),
    };
    for d in &m.decls {
        if let TDecl::Function(f) = d {
            cx.funcs.insert(&f.name, f);
        }
    }
    let mut out = String::from("diagnostic(off, derivative_uniformity);\n\n");
    out += &format!("// md-hlsl: {kind:?} shader\n\n");
    out += &interface(bindings);

    // Globals: declarations, plus the initialisations fs_main runs first.
    let mut decls = String::new();
    let mut inits: Vec<(&TGlobal, Init)> = Vec::new();
    for d in &m.decls {
        let TDecl::Global(g) = d else { continue };
        if !reach.globals.contains(g.name.as_str()) {
            continue;
        }
        global(&mut cx, g, bindings, &mut decls, &mut inits)?;
    }

    let structs = used_structs(m, &reach);
    let mut body = String::new();
    for d in &m.decls {
        match d {
            TDecl::Struct(s) if structs.contains(s.name.as_str()) => body += &struct_decl(s)?,
            TDecl::Function(f) if reach.fns.contains(f.name.as_str()) => {
                body += &Fx::new(&cx).function(f)?;
                body.push('\n');
            }
            _ => {}
        }
    }
    // Const-expression initializers, once `cx` is complete.
    for (g, init) in &inits {
        let kw = match init {
            Init::Const => "const",
            Init::PrivateConst => "var<private>",
            _ => continue,
        };
        let v = Fx::new(&cx).expr(g.init.as_ref().unwrap())?;
        decls += &format!("{kw} h_{}: {} = {v};\n", g.name, wgsl_ty(&g.ty, g.pos)?);
    }
    out += &decls;
    out.push('\n');
    out += &body;
    out += &Fx::new(&cx).entry(entry, &inits)?;
    Ok(out)
}

/// `VsOut`, the uniform blocks and the sampler bindings.
fn interface(b: &Bindings) -> String {
    let mut s = String::from(
        "struct VsOut {\n    @builtin(position) pos: vec4<f32>,\n    \
         @location(0) diffuse: vec4<f32>,\n    @location(1) uv: vec4<f32>,\n    \
         @location(2) rad_ang: vec2<f32>,\n}\n\n",
    );
    s += &layout::md_uniforms_wgsl();
    s += "\n@group(0) @binding(0) var<uniform> U: MdUniforms;\n\nstruct TexSizes {\n";
    for n in &b.texsize_names {
        s += &format!("    {n}: vec4<f32>,\n");
    }
    if b.texsize_names.is_empty() {
        s += &format!("    {TEXSIZES_PLACEHOLDER}: vec4<f32>,\n");
    }
    s += "}\n\n@group(0) @binding(1) var<uniform> T: TexSizes;\n\n";
    for sb in &b.samplers {
        let i = sb.slot;
        let dim = if sb.dims == 3 { "3d" } else { "2d" };
        s += &format!(
            "@group(1) @binding({}) var tex_{i}: texture_{dim}<f32>;\n\
             @group(1) @binding({}) var samp_{i}: sampler;\n",
            2 * i,
            2 * i + 1
        );
    }
    s.push('\n');
    s
}

/// How a global is initialised.
pub(crate) enum Init {
    /// At the start of `fs_main`, from its initializer (zero without one).
    FromDecl,
    /// At the start of `fs_main`, from this uniform field (`U._qa`).
    Uniform(String),
    /// A module-scope WGSL `const`.
    Const,
    /// A `var<private>` with a const-expression initializer.
    PrivateConst,
}

/// Classify one reachable global: record how references are spelled,
/// declare the private copies, and push how it is initialised.
fn global<'a>(
    cx: &mut Ctx<'a>,
    g: &'a TGlobal,
    b: &Bindings,
    decls: &mut String,
    inits: &mut Vec<(&'a TGlobal, Init)>,
) -> R<()> {
    let name = g.name.as_str();
    if g.ty.is_sampler() {
        if let Some(s) = b.samplers.iter().find(|s| s.name == g.name) {
            cx.globals.insert(name, GlobalRef::Sampler(s.slot));
        }
        return Ok(());
    }
    let ty = wgsl_ty(&g.ty, g.pos)?;
    // A module-private variable: initialised in its declaration when the
    // initializer is a const-expression, else at the start of fs_main.
    let private = |cx: &mut Ctx<'a>, decls: &mut String| {
        cx.globals
            .insert(name, GlobalRef::Expr(format!("h_{name}")));
        if g.init
            .as_ref()
            .is_some_and(|e| is_const_expr(e, &cx.consts))
        {
            return Init::PrivateConst;
        }
        *decls += &format!("var<private> h_{name}: {ty};\n");
        Init::FromDecl
    };
    match g.storage {
        Storage::Uniform => {
            let field = if let Some(k) = md_uniform_kind(name) {
                Some((format!("U.{name}"), k.hlsl_ty()))
            } else if b.texsize_names.iter().any(|n| n == name) {
                Some((format!("T.{name}"), Ty::Vec(Scalar::Float, 4)))
            } else {
                None
            };
            match field {
                Some((_, want)) if want != g.ty => {
                    return err(
                        g.pos,
                        format!("`{name}` must be declared `{want}`, not `{}`", g.ty),
                    )
                }
                Some((src, _)) if g.written => {
                    cx.globals
                        .insert(name, GlobalRef::Expr(format!("h_{name}")));
                    *decls += &format!("var<private> h_{name}: {ty};\n");
                    inits.push((g, Init::Uniform(src)));
                }
                Some((src, _)) => {
                    cx.globals.insert(name, GlobalRef::Expr(src));
                }
                None => {
                    let i = private(cx, decls);
                    inits.push((g, i));
                }
            }
        }
        Storage::Private => {
            let i = private(cx, decls);
            inits.push((g, i));
        }
        Storage::Const => {
            let init = g.init.as_ref();
            if init.is_some_and(|e| is_const_expr(e, &cx.consts)) {
                cx.consts.insert(name);
                cx.globals
                    .insert(name, GlobalRef::Expr(format!("h_{name}")));
                inits.push((g, Init::Const));
            } else {
                let i = private(cx, decls);
                inits.push((g, i));
            }
        }
    }
    Ok(())
}

/// True if `e` is a WGSL const-expression once emitted: literals,
/// constants and vector/array constructors, conversions, swizzles, indexing
/// and operators over them (no calls, no matrices or structs, no division,
/// remainder or shifts, which could fail const evaluation).
fn is_const_expr(e: &TExpr, consts: &HashSet<&str>) -> bool {
    use crate::ast::{BinOp, UnOp};
    use TExprKind as K;
    let c = |x: &TExpr| is_const_expr(x, consts);
    if matches!(e.ty, Ty::Mat(..) | Ty::Struct(_)) {
        return false;
    }
    match &e.kind {
        K::Lit(_) => true,
        K::Var { name, origin } => {
            *origin == VarOrigin::Global(Storage::Const) && consts.contains(name.as_str())
        }
        K::Ctor { args } => args.iter().all(c),
        K::Cast(x) | K::Splat(x) => c(x),
        K::Swizzle { base, .. } => c(base),
        K::Index { base, index } => c(base) && c(index),
        K::Unary { op, operand } => {
            matches!(op, UnOp::Neg | UnOp::Plus | UnOp::Not | UnOp::BitNot) && c(operand)
        }
        K::Binary { op, lhs, rhs } => {
            !matches!(op, BinOp::Div | BinOp::Rem | BinOp::Shl | BinOp::Shr) && c(lhs) && c(rhs)
        }
        _ => false,
    }
}

/// Names of the structs reachable code uses (directly, in arrays, or as
/// fields of other used structs); unused structs are not emitted.
fn used_structs<'a>(m: &'a TypedModule, reach: &Reach) -> HashSet<&'a str> {
    fn add<'a>(t: &'a Ty, out: &mut Vec<&'a str>) {
        match t {
            Ty::Struct(n) => out.push(n),
            Ty::Array(e, _) => add(e, out),
            _ => {}
        }
    }
    let mut todo = Vec::new();
    for d in &m.decls {
        match d {
            TDecl::Global(g) if reach.globals.contains(g.name.as_str()) => add(&g.ty, &mut todo),
            TDecl::Function(f) if reach.fns.contains(f.name.as_str()) => {
                visit_types(f, &mut |t| add(t, &mut todo))
            }
            _ => {}
        }
    }
    let mut used = HashSet::new();
    while let Some(n) = todo.pop() {
        if !used.insert(n) {
            continue;
        }
        for d in &m.decls {
            if let TDecl::Struct(s) = d {
                if s.name == n {
                    s.fields.iter().for_each(|f| add(&f.ty, &mut todo));
                }
            }
        }
    }
    used
}

fn struct_decl(s: &TStruct) -> R<String> {
    let mut out = format!("struct h_{} {{\n", s.name);
    for f in &s.fields {
        if f.ty.is_sampler() {
            return err(f.pos, "sampler struct fields are not supported");
        }
        out += &format!("    h_{}: {},\n", f.name, wgsl_ty(&f.ty, f.pos)?);
    }
    Ok(out + "}\n\n")
}

/// WGSL spelling of an element type.
pub(crate) fn elem_name(s: Scalar) -> &'static str {
    match s {
        Scalar::Bool => "bool",
        Scalar::Int => "i32",
        Scalar::Float => "f32",
    }
}

/// WGSL spelling of a value type: `float3` → `vec3<f32>`, HLSL `floatRxC` →
/// `matRxC<f32>` (stored transposed, F24), `T[n]` → `array<T, n>`.
pub(crate) fn wgsl_ty(ty: &Ty, pos: Pos) -> R<String> {
    Ok(match ty {
        Ty::Bool | Ty::Int | Ty::Float => elem_name(ty.elem().unwrap()).to_string(),
        Ty::Vec(s, n) => format!("vec{n}<{}>", elem_name(*s)),
        Ty::Mat(Scalar::Float, r, c) => format!("mat{r}x{c}<f32>"),
        Ty::Mat(..) => return err(pos, format!("`{ty}`: WGSL has only float matrices")),
        Ty::Array(t, n) => format!("array<{}, {n}>", wgsl_ty(t, pos)?),
        Ty::Struct(n) => format!("h_{n}"),
        Ty::Void | Ty::Sampler2D | Ty::Sampler3D | Ty::SamplerCube => {
            return err(pos, format!("`{ty}` is not a WGSL value type"))
        }
    })
}

/// `vecN<E>` for a vector shape or the scalar name.
pub(crate) fn shape_ty(s: Scalar, shape: Shape) -> String {
    match shape {
        Shape::Vec(n) => format!("vec{n}<{}>", elem_name(s)),
        Shape::Mat(r, c) => format!("mat{r}x{c}<f32>"),
        Shape::Scalar => elem_name(s).to_string(),
    }
}

/// A float literal: always has a `.` or an exponent (`3` → `3.0`).
pub(crate) fn float_lit(v: f64) -> String {
    let f = v as f32;
    let f = if f.is_finite() {
        f
    } else if f.is_nan() {
        0.0
    } else {
        f32::MAX.copysign(f)
    };
    format!("{f:?}")
}

/// An `i32` literal (`5i`), saturated to the `i32` range.
pub(crate) fn int_lit(v: i64) -> String {
    format!("{}i", v.clamp(0, i32::MAX as i64))
}

/// A literal of scalar type `ty`.
pub(crate) fn lit(l: Lit, ty: &Ty) -> String {
    let as_f = match l {
        Lit::Bool(b) => b as i64 as f64,
        Lit::Int(i) => i as f64,
        Lit::Float(f) => f,
    };
    match ty {
        Ty::Bool => (as_f != 0.0).to_string(),
        Ty::Int => int_lit(as_f.trunc() as i64),
        _ => match l {
            Lit::Bool(b) => float_lit(b as i64 as f64),
            _ => float_lit(as_f),
        },
    }
}

/// The literal `1` of a scalar or vector type (for `++`/`--`).
pub(crate) fn one(ty: &Ty, pos: Pos) -> R<String> {
    let (Some(s), Some(shape)) = (ty.elem(), ty.shape()) else {
        return err(pos, format!("cannot increment `{ty}`"));
    };
    let l = if s == Scalar::Int { "1i" } else { "1.0" };
    Ok(match shape {
        Shape::Scalar => l.to_string(),
        Shape::Vec(_) => format!("{}({l})", shape_ty(s, shape)),
        Shape::Mat(..) => return err(pos, "cannot increment a matrix"),
    })
}

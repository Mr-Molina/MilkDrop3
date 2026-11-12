//! Type checker with fxc's legacy implicit conversions (Task 4.3).
//!
//! [`check`] resolves every name and type of a parsed [`Module`] and returns
//! a [`TypedModule`] in which every implicit conversion is explicit (see
//! [`crate::typed`]). Decisions (beyond the brief's pinned conversions):
//!
//! - Declaration before use, as fxc: globals, structs and functions are
//!   visible after their declaration. Recursion is an error (WGSL has none).
//! - `half`/`double` are `float`; `uint`/`dword` are `int`.
//!   `float1`/`floatRx1`/`float1xC` are rejected.
//! - Untyped `sampler` globals take their dimension from use (see
//!   [`crate::typed::TGlobal::ty`]); untyped `sampler` parameters are
//!   `sampler2D`. `texture` globals are dropped; using one is an error.
//! - Writes to non-`static` globals (uniforms) and to `const` are errors;
//!   lvalues are variables, member/swizzle selections without repeated
//!   components, and indexing of lvalues.
//! - `if`/`while`/`for`/`do` conditions must be scalar (fxc rejects vector
//!   conditions); a vector `?:` condition is a componentwise select.
//! - Implicit truncation (vector to shorter vector or scalar, matrix to a
//!   smaller matrix) warns, in assignments, initializers, returns, call
//!   arguments and between operands of a binary operator or `?:`; widening
//!   is an error.
//! - `Decl::ShaderBody` must be wrapped into `void PS(...)` (Task 4.6)
//!   before checking.

mod call;
mod convert;
mod expr;
mod samplers;
mod stmt;

use std::collections::HashMap;

use crate::ast::{self, Decl, Expr, Module, Pos, SamplerKind, TypeKind, TypeName};
use crate::typed::*;
use crate::types::{Scalar, Ty};
use crate::TranslateError;

type R<T> = Result<T, TranslateError>;

/// Length of an unsized array from its element type and initializer.
type UnsizedLen<'a> = dyn Fn(&Ty) -> R<u32> + 'a;

fn err<T>(pos: Pos, msg: impl Into<String>) -> R<T> {
    Err(TranslateError::new(pos.line, pos.col, msg))
}

/// Type-check `m`. Fails on the first error; non-fatal diagnostics are in
/// [`TypedModule::warnings`].
pub fn check(m: &Module) -> Result<TypedModule, TranslateError> {
    let mut c = Checker {
        untyped_dims: samplers::untyped_sampler_kinds(m),
        ..Checker::default()
    };
    let mut decls = Vec::new();
    for d in &m.decls {
        match d {
            Decl::Global(v) => c.global(v, &mut decls)?,
            Decl::Struct(s) => decls.push(TDecl::Struct(c.struct_decl(s)?)),
            Decl::Function(f) => decls.push(TDecl::Function(c.function(f)?)),
            Decl::ShaderBody(b) => {
                return err(
                    b.pos,
                    "shader_body must be wrapped into a function before type checking",
                )
            }
        }
    }
    Ok(TypedModule {
        decls,
        warnings: c.warnings,
    })
}

/// A global symbol.
#[derive(Clone, Debug)]
struct GlobalSym {
    /// `None` for a `texture` object.
    ty: Option<Ty>,
    storage: Storage,
    const_int: Option<i64>,
}

/// A local or parameter symbol.
#[derive(Clone, Debug)]
struct LocalSym {
    ty: Ty,
    origin: VarOrigin,
    is_const: bool,
    const_int: Option<i64>,
}

/// A user function overload.
#[derive(Clone, Debug)]
struct FnSig {
    unique: String,
    params: Vec<(Ty, ParamMode, Option<TExpr>)>,
    ret: Ty,
}

#[derive(Default)]
struct Checker {
    structs: HashMap<String, Vec<(String, Ty)>>,
    globals: HashMap<String, GlobalSym>,
    funcs: HashMap<String, Vec<FnSig>>,
    scopes: Vec<HashMap<String, LocalSym>>,
    /// Return type and HLSL name of the function being checked.
    cur_fn: Option<(Ty, String)>,
    loop_depth: u32,
    warnings: Vec<Warning>,
    untyped_dims: HashMap<String, SamplerKind>,
}

/// A resolved variable.
enum Sym {
    Local(LocalSym),
    Global(GlobalSym),
}

impl Checker {
    fn warn(&mut self, pos: Pos, msg: impl Into<String>) {
        self.warnings.push(Warning {
            pos,
            message: msg.into(),
        });
    }

    fn lookup(&self, name: &str) -> Option<Sym> {
        for s in self.scopes.iter().rev() {
            if let Some(l) = s.get(name) {
                return Some(Sym::Local(l.clone()));
            }
        }
        self.globals.get(name).cloned().map(Sym::Global)
    }

    /// Declare a local in the innermost scope; redefinition there is an error.
    fn declare_local(&mut self, pos: Pos, name: &str, sym: LocalSym) -> R<()> {
        let scope = self.scopes.last_mut().expect("no scope");
        if scope.contains_key(name) {
            return err(pos, format!("redefinition of '{name}'"));
        }
        scope.insert(name.to_string(), sym);
        Ok(())
    }

    fn check_global_name(&self, pos: Pos, name: &str) -> R<()> {
        if self.globals.contains_key(name) || self.funcs.contains_key(name) {
            return err(pos, format!("redefinition of '{name}'"));
        }
        if self.structs.contains_key(name) {
            return err(pos, format!("'{name}' is already a struct name"));
        }
        Ok(())
    }

    /// Lower a written base type. `untyped` gives the kind of an untyped
    /// `sampler` (default 2D). `texture` is an error here.
    fn base_ty(&self, t: &TypeName, untyped: SamplerKind) -> R<Ty> {
        let scalar = |s: ast::Scalar| match s {
            ast::Scalar::Bool => Scalar::Bool,
            ast::Scalar::Int | ast::Scalar::Uint => Scalar::Int,
            ast::Scalar::Half | ast::Scalar::Float | ast::Scalar::Double => Scalar::Float,
        };
        Ok(match &t.kind {
            TypeKind::Void => Ty::Void,
            TypeKind::Scalar(s) => Ty::scalar(scalar(*s)),
            TypeKind::Vector(s, n) => {
                if *n < 2 {
                    return err(t.pos, "one-component vector types are not supported");
                }
                Ty::Vec(scalar(*s), *n)
            }
            TypeKind::Matrix(s, r, c) => {
                if *r < 2 || *c < 2 {
                    return err(
                        t.pos,
                        "matrix types with one row or column are not supported",
                    );
                }
                Ty::Mat(scalar(*s), *r, *c)
            }
            TypeKind::Sampler(k) => {
                let k = if *k == SamplerKind::Any { untyped } else { *k };
                match k {
                    SamplerKind::Any | SamplerKind::S2D => Ty::Sampler2D,
                    SamplerKind::S3D => Ty::Sampler3D,
                    SamplerKind::Cube => Ty::SamplerCube,
                }
            }
            TypeKind::Texture => {
                return err(
                    t.pos,
                    "texture objects are only supported as preamble globals",
                )
            }
            TypeKind::Struct(name) => {
                if !self.structs.contains_key(name) {
                    return err(t.pos, format!("unknown type '{name}'"));
                }
                Ty::Struct(name.clone())
            }
        })
    }

    /// Wrap `base` in array dimensions (outermost first). An unsized
    /// dimension is only allowed outermost and only when `unsized_len`
    /// supplies the length from an initializer list.
    fn array_ty(
        &self,
        pos: Pos,
        base: Ty,
        dims: &[Option<Expr>],
        unsized_len: Option<&UnsizedLen>,
    ) -> R<Ty> {
        if dims.is_empty() {
            return Ok(base);
        }
        if matches!(base, Ty::Void) || base.is_sampler() {
            return err(pos, format!("arrays of {base} are not supported"));
        }
        let mut ty = base;
        for (i, d) in dims.iter().enumerate().rev() {
            let n = match d {
                Some(e) => match self.const_int(e) {
                    Some(n) if (1..=65536).contains(&n) => n as u32,
                    Some(n) => return err(e.pos(), format!("invalid array size {n}")),
                    None => return err(e.pos(), "array size must be a constant integer"),
                },
                None => match unsized_len {
                    Some(f) if i == 0 => f(&ty)?,
                    _ => return err(pos, "unsized arrays need an initializer list"),
                },
            };
            ty = Ty::Array(Box::new(ty), n);
        }
        Ok(ty)
    }

    /// Fold a constant integer expression (literals, `static const`/`const`
    /// integers with constant initializers, arithmetic, casts).
    fn const_int(&self, e: &Expr) -> Option<i64> {
        use ast::{BinOp as B, Lit, UnOp as U};
        match e {
            Expr::Lit {
                lit: Lit::Int(v), ..
            } => Some(*v),
            Expr::Lit {
                lit: Lit::Bool(b), ..
            } => Some(*b as i64),
            Expr::Ident { name, .. } => match self.lookup(name)? {
                Sym::Local(l) => l.const_int,
                Sym::Global(g) => g.const_int,
            },
            Expr::Unary { op, operand, .. } => {
                let v = self.const_int(operand)?;
                match op {
                    U::Neg => v.checked_neg(),
                    U::Plus => Some(v),
                    U::BitNot => Some(!v),
                    U::Not => Some((v == 0) as i64),
                    _ => None,
                }
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let (a, b) = (self.const_int(lhs)?, self.const_int(rhs)?);
                match op {
                    B::Add => a.checked_add(b),
                    B::Sub => a.checked_sub(b),
                    B::Mul => a.checked_mul(b),
                    B::Div => a.checked_div(b),
                    B::Rem => a.checked_rem(b),
                    B::Shl => a.checked_shl(u32::try_from(b).ok()?),
                    B::Shr => a.checked_shr(u32::try_from(b).ok()?),
                    _ => None,
                }
            }
            Expr::Cast { ty, expr, .. } if matches!(ty.kind, TypeKind::Scalar(_)) => {
                self.const_int(expr)
            }
            _ => None,
        }
    }

    fn global(&mut self, v: &ast::VarDecl, out: &mut Vec<TDecl>) -> R<()> {
        let q = v.quals;
        let storage = match (q.is_static, q.is_const) {
            (true, true) => Storage::Const,
            (true, false) => Storage::Private,
            _ => Storage::Uniform,
        };
        for d in &v.vars {
            self.check_global_name(d.pos, &d.name)?;
            if v.ty.kind == TypeKind::Texture {
                if !d.array.is_empty() || d.init.is_some() {
                    return err(d.pos, "texture arrays and initializers are not supported");
                }
                let sym = GlobalSym {
                    ty: None,
                    storage,
                    const_int: None,
                };
                self.globals.insert(d.name.clone(), sym);
                continue;
            }
            let untyped = match self.untyped_dims.get(&d.name) {
                Some(k) => *k,
                None if d.name.contains("noisevol") => SamplerKind::S3D,
                None => SamplerKind::S2D,
            };
            let base = self.base_ty(&v.ty, untyped)?;
            if base == Ty::Void {
                return err(d.pos, "variables cannot be void");
            }
            let (ty, init) = self.declarator_init(d, base)?;
            if storage == Storage::Const && init.is_none() {
                return err(
                    d.pos,
                    format!("static const '{}' needs an initializer", d.name),
                );
            }
            if ty.is_sampler() && storage != Storage::Uniform {
                return err(d.pos, "samplers cannot be static");
            }
            let const_int = match (&d.init, storage, &ty) {
                (Some(ast::Init::Expr(e)), Storage::Const, Ty::Int) => self.const_int(e),
                _ => None,
            };
            self.globals.insert(
                d.name.clone(),
                GlobalSym {
                    ty: Some(ty.clone()),
                    storage,
                    const_int,
                },
            );
            out.push(TDecl::Global(TGlobal {
                pos: d.pos,
                name: d.name.clone(),
                ty,
                storage,
                semantic: d.semantic.clone(),
                init,
            }));
        }
        Ok(())
    }

    fn struct_decl(&mut self, s: &ast::StructDecl) -> R<TStruct> {
        self.check_global_name(s.pos, &s.name)?;
        let mut fields = Vec::new();
        let mut seen: Vec<(String, Ty)> = Vec::new();
        for f in &s.fields {
            let base = self.base_ty(&f.ty, SamplerKind::S2D)?;
            if base == Ty::Void || base.is_sampler() {
                return err(f.pos, format!("struct fields cannot be {base}"));
            }
            let ty = self.array_ty(f.pos, base, &f.array, None)?;
            if seen.iter().any(|(n, _)| *n == f.name) {
                return err(f.pos, format!("duplicate field '{}'", f.name));
            }
            seen.push((f.name.clone(), ty.clone()));
            fields.push(TField {
                pos: f.pos,
                name: f.name.clone(),
                ty,
                semantic: f.semantic.clone(),
            });
        }
        self.structs.insert(s.name.clone(), seen);
        Ok(TStruct {
            pos: s.pos,
            name: s.name.clone(),
            fields,
        })
    }

    fn function(&mut self, f: &ast::FnDecl) -> R<TFunction> {
        if self.globals.contains_key(&f.name) || self.structs.contains_key(&f.name) {
            return err(f.pos, format!("redefinition of '{}'", f.name));
        }
        let ret = self.base_ty(&f.ret, SamplerKind::S2D)?;
        if ret.is_sampler() {
            return err(f.ret.pos, "functions cannot return samplers");
        }
        let mut params = Vec::new();
        let mut sig_params = Vec::new();
        let mut scope = HashMap::new();
        for p in &f.params {
            let base = self.base_ty(&p.ty, SamplerKind::S2D)?;
            if base == Ty::Void {
                return err(p.pos, "parameters cannot be void");
            }
            let ty = self.array_ty(p.pos, base, &p.array, None)?;
            let mode = match (p.quals.inout, p.quals.out) {
                (true, _) => ParamMode::InOut,
                (false, true) => ParamMode::Out,
                _ => ParamMode::In,
            };
            if ty.is_sampler() && mode != ParamMode::In {
                return err(p.pos, "sampler parameters cannot be out/inout");
            }
            let default = match &p.default {
                Some(e) => {
                    let t = self.expr(e)?;
                    Some(self.coerce(t, &ty)?)
                }
                None => None,
            };
            if default.is_none()
                && sig_params
                    .iter()
                    .any(|(_, _, d): &(_, _, Option<_>)| d.is_some())
            {
                return err(p.pos, "parameters after a default parameter need defaults");
            }
            if scope.contains_key(&p.name) {
                return err(p.pos, format!("duplicate parameter '{}'", p.name));
            }
            let sym = LocalSym {
                ty: ty.clone(),
                origin: VarOrigin::Param(mode),
                is_const: p.quals.is_const,
                const_int: None,
            };
            scope.insert(p.name.clone(), sym);
            sig_params.push((ty.clone(), mode, default));
            params.push(TParam {
                pos: p.pos,
                name: p.name.clone(),
                ty,
                mode,
                semantic: p.semantic.clone(),
            });
        }
        let overloads = self.funcs.get(&f.name).map_or(&[][..], |v| v.as_slice());
        if overloads.iter().any(|o| {
            o.params
                .iter()
                .map(|p| &p.0)
                .eq(sig_params.iter().map(|p| &p.0))
        }) {
            return err(f.pos, format!("redefinition of '{}'", f.name));
        }
        let unique = match overloads.len() {
            0 => f.name.clone(),
            k => format!("{}__ovl{k}", f.name),
        };
        self.scopes = vec![scope];
        self.cur_fn = Some((ret.clone(), f.name.clone()));
        self.loop_depth = 0;
        let body = self.block_stmts(&f.body, false);
        self.scopes.clear();
        self.cur_fn = None;
        let body = body?;
        self.funcs.entry(f.name.clone()).or_default().push(FnSig {
            unique: unique.clone(),
            params: sig_params,
            ret: ret.clone(),
        });
        Ok(TFunction {
            pos: f.pos,
            name: unique,
            hlsl_name: f.name.clone(),
            ret,
            params,
            semantic: f.semantic.clone(),
            body,
        })
    }
}

//! Function bodies and statements of the WGSL emitter (Task 4.4).
//!
//! WGSL assignments are statements, so expressions with side effects
//! (assignments, `++`/`--`, calls with `out` arguments, `sincos`/`modf`/
//! `frexp`) are *hoisted*: [`Fx::expr`] pushes the statements they need onto
//! `pre`, and the statement that owns the expression flushes them just
//! before itself. Operands evaluated before a hoisted side effect are first
//! captured in `let md_t<N>` temporaries, preserving HLSL's left-to-right
//! order. Loop conditions are evaluated inside the loop, so their hoisted
//! statements rerun every iteration:
//!
//! - `while (c) s` → `loop { if !(c) { break; } s }`;
//! - `for (i; c; st) s` → `i;` then `loop { if !(c) { break; } s
//!   continuing { st } }` (declarations in the init clause were hoisted
//!   before the loop by the checker, as fxc scopes them);
//! - `do s while (c)` → `loop { s continuing { break if !(c); } }`.
//!
//! Locals are `var` (zero-initialised when HLSL leaves them undefined).
//! `in` parameters the body writes are copied to a local `var` (WGSL
//! parameters are immutable); `out`/`inout` parameters are
//! `ptr<function, T>`; sampler parameters become a `texture_2d<f32>` /
//! `texture_3d<f32>` plus `sampler` parameter pair. A non-void function
//! whose last statement is not a `return` gets a trailing `return T();`
//! (WGSL requires a return on every path; fxc tolerated the fall-through).

use std::collections::HashSet;

use crate::ast::UnOp;
use crate::emit::{err, wgsl_ty, Ctx, Init, R};
use crate::typed::{
    Callee, ParamMode, TExpr, TExprKind, TFunction, TGlobal, TParam, TStmt, VarOrigin,
};
use crate::types::{Scalar, Ty};

/// Emitter state for one function.
pub(crate) struct Fx<'c, 'a> {
    pub cx: &'c Ctx<'a>,
    /// Finished lines (with indentation).
    pub out: Vec<String>,
    pub indent: usize,
    /// Hoisted statements of the expression being emitted (relative
    /// indentation).
    pub pre: Vec<String>,
    pub tmp: u32,
}

impl<'c, 'a> Fx<'c, 'a> {
    pub fn new(cx: &'c Ctx<'a>) -> Self {
        Fx {
            cx,
            out: Vec::new(),
            indent: 1,
            pre: Vec::new(),
            tmp: 0,
        }
    }

    /// A fresh temporary name.
    pub fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("md_t{}", self.tmp - 1)
    }

    fn line(&mut self, s: impl AsRef<str>) {
        self.out
            .push(format!("{}{}", "    ".repeat(self.indent), s.as_ref()));
    }

    fn flush(&mut self) {
        for l in std::mem::take(&mut self.pre) {
            self.line(l);
        }
    }

    /// Run `f` with an empty `pre`, returning the statements it hoisted.
    pub fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> R<T>) -> R<(Vec<String>, T)> {
        let saved = std::mem::take(&mut self.pre);
        let r = f(self);
        let inner = std::mem::replace(&mut self.pre, saved);
        Ok((inner, r?))
    }

    /// `fn h_<name>(…) -> … { … }`.
    pub fn function(mut self, f: &TFunction) -> R<String> {
        let written = written_params(self.cx, f);
        let mut params = Vec::new();
        for p in &f.params {
            params.push(self.param(p, &written)?);
            if p.mode == ParamMode::In && written.contains(p.name.as_str()) {
                let ty = wgsl_ty(&p.ty, p.pos)?;
                self.line(format!("var h_{0}: {ty} = md_in_h_{0};", p.name));
            }
        }
        let ret = match f.ret {
            Ty::Void => String::new(),
            ref t => format!(" -> {}", wgsl_ty(t, f.pos)?),
        };
        self.stmts(&f.body.stmts)?;
        if f.ret != Ty::Void && !matches!(f.body.stmts.last(), Some(TStmt::Return { .. })) {
            let t = wgsl_ty(&f.ret, f.pos)?;
            self.line(format!("return {t}();"));
        }
        Ok(format!(
            "fn h_{}({}){ret} {{\n{}\n}}\n",
            f.name,
            params.join(", "),
            self.out.join("\n")
        ))
    }

    fn param(&self, p: &TParam, written: &HashSet<&str>) -> R<String> {
        let n = &p.name;
        Ok(match (&p.ty, p.mode) {
            (Ty::Sampler2D | Ty::Sampler3D, _) => {
                let d = if p.ty == Ty::Sampler3D { "3d" } else { "2d" };
                format!("md_tex_h_{n}: texture_{d}<f32>, md_samp_h_{n}: sampler")
            }
            (Ty::SamplerCube, _) => return err(p.pos, "cube samplers are not supported"),
            (t, ParamMode::In) if written.contains(n.as_str()) => {
                format!("md_in_h_{n}: {}", wgsl_ty(t, p.pos)?)
            }
            (t, ParamMode::In) => format!("h_{n}: {}", wgsl_ty(t, p.pos)?),
            (t, _) => format!("h_{n}: ptr<function, {}>", wgsl_ty(t, p.pos)?),
        })
    }

    /// `@fragment fn fs_main`: initialise private globals, call `h_PS`.
    pub fn entry(mut self, ps: &TFunction, inits: &[(&TGlobal, Init)]) -> R<String> {
        for (g, init) in inits {
            match init {
                Init::Const | Init::PrivateConst => {}
                Init::Uniform(src) => self.line(format!("h_{} = {src};", g.name)),
                Init::FromDecl => {
                    if let Some(e) = &g.init {
                        let v = self.expr(e)?;
                        self.flush();
                        self.line(format!("h_{} = {v};", g.name));
                    }
                }
            }
        }
        let mut args = Vec::new();
        let mut ret = None;
        for p in &ps.params {
            let sem = p.semantic.as_deref().unwrap_or("").to_ascii_uppercase();
            let float4 = Ty::Vec(Scalar::Float, 4);
            if p.mode != ParamMode::In {
                if !matches!(sem.as_str(), "COLOR" | "COLOR0" | "SV_TARGET") || p.ty != float4 {
                    return err(p.pos, "PS output must be `out float4 … : COLOR0`");
                }
                self.line("var md_out: vec4<f32>;");
                args.push("&md_out".to_string());
                ret = Some("md_out".to_string());
                continue;
            }
            let (src, width) = match sem.as_str() {
                "COLOR" | "COLOR0" => ("in.diffuse", 4),
                "TEXCOORD" | "TEXCOORD0" => ("in.uv", 4),
                "TEXCOORD1" => ("in.rad_ang", 2),
                _ => return err(p.pos, format!("unsupported PS input semantic `{sem}`")),
            };
            let n = match p.ty {
                Ty::Float => 1,
                Ty::Vec(Scalar::Float, n) => n,
                _ => 0,
            };
            if n == 0 || n > width {
                return err(
                    p.pos,
                    format!("PS input `{}` must be float1..{width}", p.name),
                );
            }
            args.push(if n == width {
                src.to_string()
            } else {
                format!("{src}.{}", &"xyzw"[..n as usize])
            });
        }
        let call = format!("h_{}({})", ps.name, args.join(", "));
        match (&ps.ret, ret) {
            (Ty::Void, Some(r)) => {
                self.line(format!("{call};"));
                self.line(format!("return {r};"));
            }
            (Ty::Vec(Scalar::Float, 4), None) => self.line(format!("return {call};")),
            (Ty::Void, None) => {
                self.line(format!("{call};"));
                self.line("return vec4<f32>(0.0);");
            }
            _ => return err(ps.pos, "PS must return `float4` or write an `out float4`"),
        }
        Ok(format!(
            "@fragment\nfn fs_main(in: VsOut) -> @location(0) vec4<f32> {{\n{}\n}}\n",
            self.out.join("\n")
        ))
    }

    fn stmts(&mut self, stmts: &[TStmt]) -> R<()> {
        stmts.iter().try_for_each(|s| self.stmt(s))
    }

    /// The statements of a branch or loop body, without its braces.
    fn body(&mut self, s: &TStmt) -> R<()> {
        self.indent += 1;
        let r = match s {
            TStmt::Block(b) => self.stmts(&b.stmts),
            s => self.stmt(s),
        };
        self.indent -= 1;
        r
    }

    fn stmt(&mut self, s: &TStmt) -> R<()> {
        match s {
            TStmt::Local(l) => {
                if l.ty.is_sampler() {
                    return err(l.pos, "sampler locals are not supported");
                }
                let ty = wgsl_ty(&l.ty, l.pos)?;
                match &l.init {
                    Some(e) => {
                        let v = self.expr(e)?;
                        self.flush();
                        self.line(format!("var h_{}: {ty} = {v};", l.name));
                    }
                    None => self.line(format!("var h_{}: {ty};", l.name)),
                }
            }
            TStmt::Expr(e) => {
                self.effect(e)?;
                self.flush();
            }
            TStmt::If {
                cond, then, els, ..
            } => {
                let c = self.expr(cond)?;
                self.flush();
                self.if_chain(c, then, els.as_deref())?;
            }
            TStmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                for e in init {
                    self.effect(e)?;
                    self.flush();
                }
                self.line("loop {");
                self.indent += 1;
                if let Some(c) = cond {
                    self.break_unless(c)?;
                }
                self.indent -= 1;
                self.body(body)?;
                if !step.is_empty() {
                    self.indent += 1;
                    self.line("continuing {");
                    self.indent += 1;
                    for e in step {
                        self.effect(e)?;
                        self.flush();
                    }
                    self.indent -= 1;
                    self.line("}");
                    self.indent -= 1;
                }
                self.line("}");
            }
            TStmt::While { cond, body, .. } => {
                self.line("loop {");
                self.indent += 1;
                self.break_unless(cond)?;
                self.indent -= 1;
                self.body(body)?;
                self.line("}");
            }
            TStmt::DoWhile { body, cond, .. } => {
                self.line("loop {");
                self.body(body)?;
                self.indent += 1;
                self.line("continuing {");
                self.indent += 1;
                let c = self.expr(cond)?;
                self.flush();
                self.line(format!("break if !({c});"));
                self.indent -= 1;
                self.line("}");
                self.indent -= 1;
                self.line("}");
            }
            TStmt::Return { value, .. } => match value {
                Some(e) => {
                    let v = self.expr(e)?;
                    self.flush();
                    self.line(format!("return {v};"));
                }
                None => self.line("return;"),
            },
            TStmt::Block(b) => {
                self.line("{");
                self.indent += 1;
                self.stmts(&b.stmts)?;
                self.indent -= 1;
                self.line("}");
            }
            TStmt::Break { .. } => self.line("break;"),
            TStmt::Continue { .. } => self.line("continue;"),
            TStmt::Discard { .. } => self.line("discard;"),
        }
        Ok(())
    }

    fn break_unless(&mut self, cond: &TExpr) -> R<()> {
        let c = self.expr(cond)?;
        self.flush();
        self.line(format!("if !({c}) {{ break; }}"));
        Ok(())
    }

    /// `if c { then } else …`, chaining `else if` when the nested
    /// condition hoists nothing.
    fn if_chain(&mut self, c: String, then: &TStmt, els: Option<&TStmt>) -> R<()> {
        self.line(format!("if {c} {{"));
        self.body(then)?;
        let mut els = els;
        while let Some(e) = els {
            if let TStmt::If {
                cond,
                then,
                els: next,
                ..
            } = e
            {
                let (pre, c) = self.nested(|s| s.expr(cond))?;
                if pre.is_empty() {
                    self.line(format!("}} else if {c} {{"));
                    self.body(then)?;
                    els = next.as_deref();
                    continue;
                }
                // The condition hoists statements: nest the rest.
                self.line("} else {");
                self.indent += 1;
                for l in pre {
                    self.line(l);
                }
                self.if_chain(c, then, next.as_deref())?;
                self.indent -= 1;
                break;
            }
            self.line("} else {");
            self.body(e)?;
            break;
        }
        self.line("}");
        Ok(())
    }

    /// An expression evaluated for its side effects only.
    pub fn effect(&mut self, e: &TExpr) -> R<()> {
        match &e.kind {
            TExprKind::Assign { .. } => {
                self.assign(e, false)?;
            }
            TExprKind::Unary { op, operand } if is_incdec(*op) => {
                self.incdec(*op, operand, false)?;
            }
            TExprKind::Call { callee, args } => {
                let v = self.call(e, callee, args, true)?;
                if !v.is_empty() && has_effects(e) {
                    self.pre.push(format!("_ = {v};"));
                }
            }
            _ => {
                let v = self.expr(e)?;
                if has_effects(e) {
                    self.pre.push(format!("_ = {v};"));
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn is_incdec(op: UnOp) -> bool {
    matches!(
        op,
        UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec
    )
}

/// True if evaluating `e` may write a variable: assignments, `++`/`--`,
/// user calls (they may write globals or `out` arguments) and the `out`
/// intrinsics.
pub(crate) fn has_effects(e: &TExpr) -> bool {
    use crate::intrinsics::Intrinsic as I;
    use TExprKind as K;
    match &e.kind {
        K::Assign { .. } => true,
        K::Unary { op, operand } => is_incdec(*op) || has_effects(operand),
        K::Call { callee, args } => {
            matches!(
                callee,
                Callee::User(_) | Callee::Intrinsic(I::Sincos | I::Modf | I::Frexp)
            ) || args.iter().any(has_effects)
        }
        K::Lit(_) | K::Var { .. } => false,
        K::Index { base, index } => has_effects(base) || has_effects(index),
        K::Swizzle { base, .. } | K::MatElems { base, .. } | K::Field { base, .. } => {
            has_effects(base)
        }
        K::Ctor { args } => args.iter().any(has_effects),
        K::Binary { lhs, rhs, .. } => has_effects(lhs) || has_effects(rhs),
        K::Ternary { cond, then, els } => {
            has_effects(cond) || has_effects(then) || has_effects(els)
        }
        K::Cast(x) | K::Splat(x) => has_effects(x),
    }
}

/// The variable an lvalue expression is rooted in.
fn root_var(e: &TExpr) -> Option<(&str, VarOrigin)> {
    match &e.kind {
        TExprKind::Var { name, origin } => Some((name, *origin)),
        TExprKind::Index { base, .. }
        | TExprKind::Swizzle { base, .. }
        | TExprKind::MatElems { base, .. }
        | TExprKind::Field { base, .. } => root_var(base),
        _ => None,
    }
}

/// Names of `in` parameters of `f` that its body writes (assigned,
/// incremented, or passed as an `out`/`inout` argument).
fn written_params<'f>(cx: &Ctx, f: &'f TFunction) -> HashSet<&'f str> {
    use crate::intrinsics::Intrinsic as I;
    let mut targets: Vec<&TExpr> = Vec::new();
    for s in &f.body.stmts {
        walk_stmt(s, &mut |e| match &e.kind {
            TExprKind::Assign { lhs, .. } => targets.push(lhs),
            TExprKind::Unary { op, operand } if is_incdec(*op) => targets.push(operand),
            TExprKind::Call {
                callee: Callee::Intrinsic(I::Sincos | I::Modf | I::Frexp),
                args,
            } => targets.extend(args.iter().skip(1)),
            TExprKind::Call {
                callee: Callee::User(n),
                args,
            } => {
                if let Some(g) = cx.funcs.get(n.as_str()) {
                    for (p, a) in g.params.iter().zip(args) {
                        if p.mode != ParamMode::In {
                            targets.push(a);
                        }
                    }
                }
            }
            _ => {}
        });
    }
    targets
        .into_iter()
        .filter_map(|e| match root_var(e) {
            Some((n, VarOrigin::Param(ParamMode::In))) => Some(n),
            _ => None,
        })
        .collect()
}

/// Visit every type a function mentions: parameters, return, locals and
/// every expression's type.
pub(crate) fn visit_types<'e>(f: &'e TFunction, v: &mut impl FnMut(&'e Ty)) {
    fn locals<'e>(s: &'e TStmt, v: &mut impl FnMut(&'e Ty)) {
        match s {
            TStmt::Local(l) => v(&l.ty),
            TStmt::If { then, els, .. } => {
                locals(then, v);
                if let Some(e) = els {
                    locals(e, v);
                }
            }
            TStmt::For { body, .. } | TStmt::While { body, .. } | TStmt::DoWhile { body, .. } => {
                locals(body, v)
            }
            TStmt::Block(b) => b.stmts.iter().for_each(|s| locals(s, v)),
            _ => {}
        }
    }
    v(&f.ret);
    f.params.iter().for_each(|p| v(&p.ty));
    for s in &f.body.stmts {
        locals(s, v);
        walk_stmt(s, &mut |e| v(&e.ty));
    }
}

/// Visit every expression (pre-order) of a statement.
fn walk_stmt<'e>(s: &'e TStmt, f: &mut impl FnMut(&'e TExpr)) {
    let ex = |e: &'e TExpr, f: &mut _| walk_expr(e, f);
    match s {
        TStmt::Local(l) => {
            if let Some(e) = &l.init {
                ex(e, f)
            }
        }
        TStmt::Expr(e) => ex(e, f),
        TStmt::If {
            cond, then, els, ..
        } => {
            ex(cond, f);
            walk_stmt(then, f);
            if let Some(e) = els {
                walk_stmt(e, f);
            }
        }
        TStmt::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            init.iter().chain(cond).chain(step).for_each(|e| ex(e, f));
            walk_stmt(body, f);
        }
        TStmt::While { cond, body, .. } | TStmt::DoWhile { body, cond, .. } => {
            ex(cond, f);
            walk_stmt(body, f);
        }
        TStmt::Return { value: Some(e), .. } => ex(e, f),
        TStmt::Block(b) => b.stmts.iter().for_each(|s| walk_stmt(s, f)),
        _ => {}
    }
}

fn walk_expr<'e>(e: &'e TExpr, f: &mut impl FnMut(&'e TExpr)) {
    use TExprKind as K;
    f(e);
    match &e.kind {
        K::Lit(_) | K::Var { .. } => {}
        K::Index { base, index } => {
            walk_expr(base, f);
            walk_expr(index, f);
        }
        K::Swizzle { base, .. } | K::MatElems { base, .. } | K::Field { base, .. } => {
            walk_expr(base, f)
        }
        K::Call { args, .. } | K::Ctor { args } => args.iter().for_each(|a| walk_expr(a, f)),
        K::Unary { operand, .. } => walk_expr(operand, f),
        K::Binary { lhs, rhs, .. } | K::Assign { lhs, rhs, .. } => {
            walk_expr(lhs, f);
            walk_expr(rhs, f);
        }
        K::Ternary { cond, then, els } => {
            walk_expr(cond, f);
            walk_expr(then, f);
            walk_expr(els, f);
        }
        K::Cast(x) | K::Splat(x) => walk_expr(x, f),
    }
}

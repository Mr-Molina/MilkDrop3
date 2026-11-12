//! Expression checking. Each node kind has its own function so the
//! recursive `expr` frame stays small (the parser allows trees 512 deep).

use super::convert::{cast_elem, splat};
use super::{err, Checker, Sym, R};
use crate::ast::{BinOp, Expr, Lit, Pos, SamplerKind, TypeName, UnOp};
use crate::intrinsics::Intrinsic;
use crate::typed::{Storage, TExpr, TExprKind, VarOrigin};
use crate::types::{Scalar, Shape, Ty};

impl Checker {
    /// Dispatch through a function pointer so this frame holds a single
    /// call result: a `match` calling the helpers directly costs ~1.4 KiB
    /// of debug-build stack per nesting level.
    pub(super) fn expr(&mut self, e: &Expr) -> R<TExpr> {
        type Node = fn(&mut Checker, &Expr) -> R<TExpr>;
        macro_rules! node {
            ($pat:pat => |$c:ident| $call:expr) => {
                (|$c: &mut Checker, e: &Expr| match e {
                    $pat => $call,
                    _ => unreachable!(),
                }) as Node
            };
        }
        let f: Node = match e {
            Expr::Lit { .. } => node!(Expr::Lit { pos, lit } => |_c| Ok(lit_expr(*pos, *lit))),
            Expr::Ident { .. } => {
                node!(Expr::Ident { pos, name } => |c| c.ident(*pos, name))
            }
            Expr::Index { .. } => {
                node!(Expr::Index { pos, base, index } => |c| c.index(*pos, base, index))
            }
            Expr::Member { .. } => {
                node!(Expr::Member { pos, base, field } => |c| c.member(*pos, base, field))
            }
            Expr::Call { .. } => {
                node!(Expr::Call { pos, name, args } => |c| c.call(*pos, name, args))
            }
            Expr::Ctor { .. } => {
                node!(Expr::Ctor { pos, ty, args } => |c| c.ctor(*pos, ty, args))
            }
            Expr::Unary { .. } => {
                node!(Expr::Unary { pos, op, operand } => |c| c.unary(*pos, *op, operand))
            }
            Expr::Binary { .. } => {
                node!(Expr::Binary { pos, op, lhs, rhs } => |c| c.binary(*pos, *op, lhs, rhs))
            }
            Expr::Assign { .. } => {
                node!(Expr::Assign { pos, op, lhs, rhs } => |c| c.assign(*pos, *op, lhs, rhs))
            }
            Expr::Ternary { .. } => {
                node!(Expr::Ternary { pos, cond, then, els } => |c| c.ternary(*pos, cond, then, els))
            }
            Expr::Cast { .. } => {
                node!(Expr::Cast { pos, ty, expr } => |c| c.cast(*pos, ty, expr))
            }
            Expr::Splat { .. } => {
                node!(Expr::Splat { pos, ty, expr } => |c| c.ast_splat(*pos, ty, expr))
            }
        };
        f(self, e)
    }

    fn ident(&mut self, pos: Pos, name: &str) -> R<TExpr> {
        let (ty, origin) = match self.lookup(name) {
            Some(Sym::Local(l)) => (l.ty, l.origin),
            Some(Sym::Global(g)) => match g.ty {
                Some(ty) => (ty, VarOrigin::Global(g.storage)),
                None => return err(pos, format!("texture object '{name}' cannot be used here")),
            },
            None if self.funcs.contains_key(name) || Intrinsic::from_name(name).is_some() => {
                return err(pos, format!("function '{name}' used as a value"))
            }
            None => return err(pos, format!("unknown identifier '{name}'")),
        };
        let kind = TExprKind::Var {
            name: name.to_string(),
            origin,
        };
        Ok(TExpr::new(pos, ty, kind))
    }

    fn index(&mut self, pos: Pos, base: &Expr, index: &Expr) -> R<TExpr> {
        let b = self.expr(base)?;
        let i = self.expr(index)?;
        self.index_tail(pos, b, i, index)
    }

    fn index_tail(&mut self, pos: Pos, b: TExpr, i: TExpr, index: &Expr) -> R<TExpr> {
        if !i.ty.is_scalar() {
            return err(i.pos, format!("index must be a scalar, got '{}'", i.ty));
        }
        let i = self.coerce(i, &Ty::Int)?;
        let (ty, len) = match &b.ty {
            Ty::Vec(s, n) => (Ty::scalar(*s), *n as u32),
            Ty::Mat(s, r, c) => (Ty::Vec(*s, *c), *r as u32),
            Ty::Array(t, n) => ((**t).clone(), *n),
            t => return err(pos, format!("cannot index a value of type '{t}'")),
        };
        if let Some(k) = self.const_int(index) {
            if k < 0 || k >= len as i64 {
                return err(
                    index.pos(),
                    format!("index {k} out of range for '{}'", b.ty),
                );
            }
        }
        let kind = TExprKind::Index {
            base: Box::new(b),
            index: Box::new(i),
        };
        Ok(TExpr::new(pos, ty, kind))
    }

    fn member(&mut self, pos: Pos, base: &Expr, field: &str) -> R<TExpr> {
        let b = self.expr(base)?;
        self.member_tail(pos, b, field)
    }

    fn member_tail(&mut self, pos: Pos, b: TExpr, field: &str) -> R<TExpr> {
        match b.ty.clone() {
            Ty::Struct(name) => {
                let fields = &self.structs[&name];
                let Some((_, ty)) = fields.iter().find(|(n, _)| n == field) else {
                    return err(pos, format!("struct '{name}' has no field '{field}'"));
                };
                let kind = TExprKind::Field {
                    base: Box::new(b),
                    name: field.to_string(),
                };
                Ok(TExpr::new(pos, ty.clone(), kind))
            }
            Ty::Mat(s, r, c) => {
                let Some(elems) = parse_mat_elems(field, r, c) else {
                    return err(
                        pos,
                        format!("invalid matrix member '.{field}' of '{}'", b.ty),
                    );
                };
                let ty = Ty::vec_or_scalar(s, elems.len() as u8);
                let kind = TExprKind::MatElems {
                    base: Box::new(b),
                    elems,
                };
                Ok(TExpr::new(pos, ty, kind))
            }
            t if t.is_numeric() => {
                let n = match t {
                    Ty::Vec(_, n) => n,
                    _ => 1,
                };
                let Some(comps) = parse_swizzle(field, n) else {
                    return err(pos, format!("invalid swizzle '.{field}' of '{t}'"));
                };
                let ty = Ty::vec_or_scalar(t.elem().unwrap(), comps.len() as u8);
                let kind = TExprKind::Swizzle {
                    base: Box::new(b),
                    comps,
                };
                Ok(TExpr::new(pos, ty, kind))
            }
            t => err(pos, format!("'{t}' has no member '{field}'")),
        }
    }

    fn ctor(&mut self, pos: Pos, ty: &TypeName, args: &[Expr]) -> R<TExpr> {
        let targs = self.args(args)?;
        self.ctor_tail(pos, ty, targs)
    }

    pub(super) fn args(&mut self, args: &[Expr]) -> R<Vec<TExpr>> {
        let mut targs = Vec::with_capacity(args.len());
        for a in args {
            targs.push(self.expr(a)?);
        }
        Ok(targs)
    }

    fn ctor_tail(&mut self, pos: Pos, ty: &TypeName, mut targs: Vec<TExpr>) -> R<TExpr> {
        let target = self.base_ty(ty, SamplerKind::S2D)?;
        let (Some(shape), Some(te)) = (target.shape(), target.elem()) else {
            return err(pos, format!("cannot construct a value of type '{target}'"));
        };
        if let [a] = targs.as_slice() {
            if a.ty.is_scalar() || a.ty.shape() == Some(shape) {
                let a = targs.pop().unwrap();
                let mut out = if a.ty.is_scalar() && shape != Shape::Scalar {
                    splat(cast_elem(a, te), &target)
                } else {
                    cast_elem(a, te)
                };
                out.pos = pos;
                return Ok(out);
            }
        }
        if shape == Shape::Scalar {
            return err(
                pos,
                format!("'{target}' constructor takes one scalar argument"),
            );
        }
        let mut n = 0;
        for a in &targs {
            match a.ty.shape() {
                Some(Shape::Scalar) => n += 1,
                Some(Shape::Vec(k)) => n += k as u32,
                _ => {
                    return err(
                        a.pos,
                        format!(
                            "'{}' arguments are not supported in a '{target}' constructor",
                            a.ty
                        ),
                    )
                }
            }
        }
        if n != shape.components() {
            return err(
                pos,
                format!(
                    "'{target}' constructor needs {} components, got {n}",
                    shape.components()
                ),
            );
        }
        let args = targs.into_iter().map(|a| cast_elem(a, te)).collect();
        Ok(TExpr::new(pos, target, TExprKind::Ctor { args }))
    }

    fn unary(&mut self, pos: Pos, op: UnOp, operand: &Expr) -> R<TExpr> {
        let x = self.expr(operand)?;
        self.unary_tail(pos, op, x)
    }

    fn unary_tail(&mut self, pos: Pos, op: UnOp, x: TExpr) -> R<TExpr> {
        let Some(elem) = x.ty.elem() else {
            return err(
                pos,
                format!("unary operator cannot be applied to '{}'", x.ty),
            );
        };
        let x = match op {
            UnOp::Neg | UnOp::Plus if elem == Scalar::Bool => cast_elem(x, Scalar::Int),
            UnOp::Neg | UnOp::Plus => x,
            UnOp::Not => cast_elem(x, Scalar::Bool),
            UnOp::BitNot if elem == Scalar::Float => {
                return err(pos, "bitwise operators require integer operands")
            }
            UnOp::BitNot => cast_elem(x, Scalar::Int),
            UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec => {
                self.lvalue(&x)?;
                if elem == Scalar::Bool {
                    return err(pos, "cannot increment or decrement a bool");
                }
                x
            }
        };
        let kind = TExprKind::Unary {
            op,
            operand: Box::new(x.clone()),
        };
        Ok(TExpr::new(pos, x.ty, kind))
    }

    fn binary(&mut self, pos: Pos, op: BinOp, lhs: &Expr, rhs: &Expr) -> R<TExpr> {
        let a = self.expr(lhs)?;
        let b = self.expr(rhs)?;
        self.binary_typed(pos, op, a, b)
    }

    pub(super) fn binary_typed(&mut self, pos: Pos, op: BinOp, a: TExpr, b: TExpr) -> R<TExpr> {
        use BinOp as B;
        let (Some(ea), Some(eb)) = (a.ty.elem(), b.ty.elem()) else {
            return err(
                pos,
                format!("operator cannot be applied to '{}' and '{}'", a.ty, b.ty),
            );
        };
        let shape = self.common_shape(pos, &[&a.ty, &b.ty])?;
        let (elem, result) = match op {
            B::Mul | B::Div | B::Rem | B::Add | B::Sub => {
                let s = if ea == Scalar::Float || eb == Scalar::Float {
                    Scalar::Float
                } else {
                    Scalar::Int
                };
                (s, s)
            }
            B::Shl | B::Shr | B::BitAnd | B::BitXor | B::BitOr => {
                if ea == Scalar::Float || eb == Scalar::Float {
                    return err(pos, "bitwise operators require integer operands");
                }
                (Scalar::Int, Scalar::Int)
            }
            B::Lt | B::Gt | B::Le | B::Ge | B::Eq | B::Ne => (ea.max(eb), Scalar::Bool),
            B::And | B::Or => (Scalar::Bool, Scalar::Bool),
        };
        let a = self.coerce(a, &Ty::numeric(elem, shape))?;
        let b = self.coerce(b, &Ty::numeric(elem, shape))?;
        let kind = TExprKind::Binary {
            op,
            lhs: Box::new(a),
            rhs: Box::new(b),
        };
        Ok(TExpr::new(pos, Ty::numeric(result, shape), kind))
    }

    fn assign(&mut self, pos: Pos, op: Option<BinOp>, lhs: &Expr, rhs: &Expr) -> R<TExpr> {
        let l = self.expr(lhs)?;
        self.lvalue(&l)?;
        let r = self.expr(rhs)?;
        self.assign_tail(pos, op, l, r)
    }

    fn assign_tail(&mut self, pos: Pos, op: Option<BinOp>, l: TExpr, r: TExpr) -> R<TExpr> {
        let ty = l.ty.clone();
        let (op, rhs) = match op {
            None => {
                if ty.is_sampler() {
                    return err(pos, "samplers cannot be assigned");
                }
                (None, self.coerce(r, &ty)?)
            }
            Some(bop) => {
                let bin = self.binary_typed(pos, bop, l.clone(), r)?;
                let TExprKind::Binary {
                    lhs: bl, rhs: br, ..
                } = &bin.kind
                else {
                    unreachable!()
                };
                if **bl == l && bin.ty == ty {
                    (Some(bop), (**br).clone())
                } else {
                    (None, self.coerce(bin, &ty)?)
                }
            }
        };
        let kind = TExprKind::Assign {
            op,
            lhs: Box::new(l),
            rhs: Box::new(rhs),
        };
        Ok(TExpr::new(pos, ty, kind))
    }

    fn ternary(&mut self, pos: Pos, cond: &Expr, then: &Expr, els: &Expr) -> R<TExpr> {
        let c = self.expr(cond)?;
        let t = self.expr(then)?;
        let e = self.expr(els)?;
        self.ternary_tail(pos, c, t, e)
    }

    fn ternary_tail(&mut self, pos: Pos, c: TExpr, t: TExpr, e: TExpr) -> R<TExpr> {
        let (c, ty) = match c.ty.shape() {
            Some(Shape::Scalar) => {
                let c = cast_elem(c, Scalar::Bool);
                if !(t.ty.is_numeric() && e.ty.is_numeric()) {
                    if t.ty != e.ty {
                        return err(
                            pos,
                            format!("'?:' branches differ: '{}' and '{}'", t.ty, e.ty),
                        );
                    }
                    let ty = t.ty.clone();
                    (c, ty)
                } else {
                    let shape = self.common_shape(pos, &[&t.ty, &e.ty])?;
                    let elem = t.ty.elem().unwrap().max(e.ty.elem().unwrap());
                    (c, Ty::numeric(elem, shape))
                }
            }
            Some(Shape::Vec(_)) => {
                let shape = self.common_shape(pos, &[&c.ty, &t.ty, &e.ty])?;
                let elem = t.ty.elem().unwrap().max(e.ty.elem().unwrap());
                let c = self.coerce(c, &Ty::numeric(Scalar::Bool, shape))?;
                (c, Ty::numeric(elem, shape))
            }
            _ => return err(c.pos, format!("'?:' condition cannot be '{}'", c.ty)),
        };
        let t = self.coerce(t, &ty)?;
        let e = self.coerce(e, &ty)?;
        let kind = TExprKind::Ternary {
            cond: Box::new(c),
            then: Box::new(t),
            els: Box::new(e),
        };
        Ok(TExpr::new(pos, ty, kind))
    }

    fn cast(&mut self, pos: Pos, ty: &TypeName, expr: &Expr) -> R<TExpr> {
        let x = self.expr(expr)?;
        self.cast_tail(pos, ty, x)
    }

    fn cast_tail(&mut self, pos: Pos, ty: &TypeName, x: TExpr) -> R<TExpr> {
        let target = self.base_ty(ty, SamplerKind::S2D)?;
        let mut out = self.explicit_cast(x, &target)?;
        out.pos = pos;
        Ok(out)
    }

    fn ast_splat(&mut self, pos: Pos, ty: &TypeName, expr: &Expr) -> R<TExpr> {
        let x = self.expr(expr)?;
        self.splat_tail(pos, ty, x)
    }

    fn splat_tail(&mut self, pos: Pos, ty: &TypeName, x: TExpr) -> R<TExpr> {
        let target = self.base_ty(ty, SamplerKind::S2D)?;
        match (target.elem(), target.is_scalar(), x.ty.is_scalar()) {
            (Some(te), false, true) => {
                let mut out = splat(cast_elem(x, te), &target);
                out.pos = pos;
                Ok(out)
            }
            _ => err(pos, format!("cannot splat '{}' to '{target}'", x.ty)),
        }
    }

    /// Fails unless `e` is an assignable location.
    pub(super) fn lvalue(&self, e: &TExpr) -> R<()> {
        match &e.kind {
            TExprKind::Var { name, origin } => match origin {
                VarOrigin::Global(Storage::Uniform) => err(
                    e.pos,
                    format!(
                        "cannot assign to '{name}': globals are uniform (read-only) unless static"
                    ),
                ),
                VarOrigin::Global(Storage::Const) => {
                    err(e.pos, format!("cannot assign to const '{name}'"))
                }
                VarOrigin::Global(Storage::Private) => Ok(()),
                VarOrigin::Local | VarOrigin::Param(_) => match self.lookup(name) {
                    Some(Sym::Local(l)) if l.is_const => {
                        err(e.pos, format!("cannot assign to const '{name}'"))
                    }
                    _ => Ok(()),
                },
            },
            TExprKind::Swizzle { base, comps } => {
                if has_repeats(comps) {
                    return err(e.pos, "write mask has repeated components");
                }
                self.lvalue(base)
            }
            TExprKind::MatElems { base, elems } => {
                if has_repeats(elems) {
                    return err(e.pos, "write mask has repeated components");
                }
                self.lvalue(base)
            }
            TExprKind::Index { base, .. } | TExprKind::Field { base, .. } => self.lvalue(base),
            _ => err(e.pos, "assignment target is not an lvalue"),
        }
    }
}

fn has_repeats<T: PartialEq>(v: &[T]) -> bool {
    v.iter().enumerate().any(|(i, x)| v[..i].contains(x))
}

pub(super) fn lit_expr(pos: Pos, lit: Lit) -> TExpr {
    let ty = match lit {
        Lit::Bool(_) => Ty::Bool,
        Lit::Int(_) => Ty::Int,
        Lit::Float(_) => Ty::Float,
    };
    TExpr::new(pos, ty, TExprKind::Lit(lit))
}

/// `xyzw` or `rgba` selection (not mixed) of a value with `n` components.
fn parse_swizzle(field: &str, n: u8) -> Option<Vec<u8>> {
    if field.is_empty() || field.len() > 4 {
        return None;
    }
    for set in ["xyzw", "rgba"] {
        let comps: Option<Vec<u8>> = field
            .chars()
            .map(|ch| set.find(ch).map(|i| i as u8).filter(|&i| i < n))
            .collect();
        if comps.is_some() {
            return comps;
        }
    }
    None
}

/// `_m00_m11` (0-based) or `_11_22` (1-based) matrix element selection.
fn parse_mat_elems(field: &str, rows: u8, cols: u8) -> Option<Vec<(u8, u8)>> {
    let rest = field.strip_prefix('_')?;
    let mut out = Vec::new();
    for part in rest.split('_') {
        let (digits, base) = match part.strip_prefix('m') {
            Some(d) => (d, 0),
            None => (part, 1),
        };
        let d = digits.as_bytes();
        if d.len() != 2 || !d.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let (r, c) = (
            (d[0] - b'0').checked_sub(base)?,
            (d[1] - b'0').checked_sub(base)?,
        );
        if r >= rows || c >= cols {
            return None;
        }
        out.push((r, c));
    }
    (1..=4).contains(&out.len()).then_some(out)
}

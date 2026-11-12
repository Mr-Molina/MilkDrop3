//! Calls: user-function overload resolution and the intrinsic signatures
//! documented in [`crate::intrinsics`].

use super::convert::{cast_elem, conv_cost};
use super::{err, Checker, R};
use crate::ast::{Expr, Pos};
use crate::intrinsics::{Intrinsic, SigClass};
use crate::typed::{Callee, ParamMode, TExpr, TExprKind};
use crate::types::{Scalar, Shape, Ty};

fn call_expr(pos: Pos, ty: Ty, callee: Callee, args: Vec<TExpr>) -> TExpr {
    TExpr::new(pos, ty, TExprKind::Call { callee, args })
}

impl Checker {
    pub(super) fn call(&mut self, pos: Pos, name: &str, args: &[Expr]) -> R<TExpr> {
        let targs = self.args(args)?;
        self.call_tail(pos, name, targs)
    }

    fn call_tail(&mut self, pos: Pos, name: &str, targs: Vec<TExpr>) -> R<TExpr> {
        if self.funcs.contains_key(name) {
            return self.user_call(pos, name, targs);
        }
        if let Some(i) = Intrinsic::from_name(name) {
            return self.intrinsic(pos, i, targs);
        }
        if self.cur_fn.as_ref().is_some_and(|(_, n)| n == name) {
            return err(pos, format!("recursive call to '{name}' is not supported"));
        }
        err(pos, format!("unknown function '{name}'"))
    }

    fn user_call(&mut self, pos: Pos, name: &str, args: Vec<TExpr>) -> R<TExpr> {
        let overloads = self.funcs[name].clone();
        let mut best: Option<(u32, usize)> = None;
        let mut tie = false;
        for (k, sig) in overloads.iter().enumerate() {
            if args.len() > sig.params.len()
                || sig.params[args.len()..].iter().any(|p| p.2.is_none())
            {
                continue;
            }
            let mut cost = 0;
            let ok = args.iter().zip(&sig.params).all(|(a, (pty, mode, _))| {
                let c = if *mode == ParamMode::In {
                    conv_cost(&a.ty, pty)
                } else {
                    (a.ty == *pty).then_some(0)
                };
                c.map(|c| cost += c).is_some()
            });
            if !ok {
                continue;
            }
            match best {
                Some((b, _)) if b < cost => {}
                Some((b, _)) if b == cost => tie = true,
                _ => {
                    best = Some((cost, k));
                    tie = false;
                }
            }
        }
        let Some((_, k)) = best else {
            let tys: Vec<String> = args.iter().map(|a| a.ty.to_string()).collect();
            return err(
                pos,
                format!(
                    "no overload of '{name}' takes argument types ({})",
                    tys.join(", ")
                ),
            );
        };
        if tie {
            return err(pos, format!("ambiguous call to overloaded '{name}'"));
        }
        let sig = &overloads[k];
        let mut out = Vec::with_capacity(sig.params.len());
        let mut args = args.into_iter();
        for (pty, mode, default) in &sig.params {
            match args.next() {
                Some(a) if *mode == ParamMode::In => out.push(self.coerce(a, pty)?),
                Some(a) => {
                    self.out_arg(&a)?;
                    out.push(a);
                }
                None => out.push(default.clone().expect("checked above")),
            }
        }
        Ok(call_expr(
            pos,
            sig.ret.clone(),
            Callee::User(sig.unique.clone()),
            out,
        ))
    }

    fn out_arg(&mut self, a: &TExpr) -> R<()> {
        self.lvalue(a).map_err(|mut e| {
            e.message = format!("out argument must be an lvalue: {}", e.message);
            e
        })
    }

    fn intrinsic(&mut self, pos: Pos, i: Intrinsic, args: Vec<TExpr>) -> R<TExpr> {
        let name = i.name();
        let arity = match i.sig() {
            SigClass::Float(n) | SigClass::Num(n) | SigClass::VecOnly(n) => n as usize,
            SigClass::Sign | SigClass::Reduce | SigClass::Bool | SigClass::Transpose => 1,
            SigClass::Reduce2 | SigClass::Dot | SigClass::Cross | SigClass::Mul => 2,
            SigClass::Refract => 3,
            SigClass::OutLike(n) => 1 + n as usize,
            SigClass::Tex(_, coords) => 1 + coords.len(),
        };
        if args.len() != arity {
            return err(
                pos,
                format!("'{name}' takes {arity} argument(s), got {}", args.len()),
            );
        }
        let callee = Callee::Intrinsic(i);
        match i.sig() {
            SigClass::Float(_) => {
                let (args, t) = self.generic(pos, name, args, false, false)?;
                Ok(call_expr(pos, t, callee, args))
            }
            SigClass::Num(_) => {
                let (args, t) = self.generic(pos, name, args, true, false)?;
                Ok(call_expr(pos, t, callee, args))
            }
            SigClass::Sign => {
                let (args, t) = self.generic(pos, name, args, false, false)?;
                Ok(call_expr(pos, t.with_elem(Scalar::Int), callee, args))
            }
            SigClass::Reduce | SigClass::Reduce2 => {
                let (args, _) = self.generic(pos, name, args, false, false)?;
                Ok(call_expr(pos, Ty::Float, callee, args))
            }
            SigClass::Dot => {
                let (args, _) = self.generic(pos, name, args, false, false)?;
                Ok(call_expr(pos, Ty::Float, callee, args))
            }
            SigClass::VecOnly(_) => {
                let (args, t) = self.generic(pos, name, args, false, true)?;
                Ok(call_expr(pos, t, callee, args))
            }
            SigClass::Refract => {
                let mut args = args;
                let eta = args.pop().unwrap();
                let (mut args, t) = self.generic(pos, name, args, false, true)?;
                args.push(self.coerce(eta, &Ty::Float)?);
                Ok(call_expr(pos, t, callee, args))
            }
            SigClass::Cross => {
                let f3 = Ty::Vec(Scalar::Float, 3);
                let mut out = Vec::new();
                for a in args {
                    out.push(self.coerce(a, &f3)?);
                }
                Ok(call_expr(pos, f3, callee, out))
            }
            SigClass::Bool => {
                let a = args.into_iter().next().unwrap();
                match a.ty.shape() {
                    Some(Shape::Scalar | Shape::Vec(_)) => Ok(call_expr(
                        pos,
                        Ty::Bool,
                        callee,
                        vec![cast_elem(a, Scalar::Bool)],
                    )),
                    _ => err(a.pos, format!("'{name}' does not take '{}'", a.ty)),
                }
            }
            SigClass::OutLike(_) => {
                let mut rest = args;
                let first = rest.remove(0);
                let (mut out, t) = self.generic(pos, name, vec![first], false, false)?;
                for a in rest {
                    self.out_arg(&a)?;
                    if a.ty != t {
                        return err(
                            a.pos,
                            format!("'{name}' out argument must be '{t}', got '{}'", a.ty),
                        );
                    }
                    out.push(a);
                }
                let ret = if i == Intrinsic::Sincos { Ty::Void } else { t };
                Ok(call_expr(pos, ret, callee, out))
            }
            SigClass::Mul => self.mul(pos, args),
            SigClass::Transpose => {
                let a = args.into_iter().next().unwrap();
                let Ty::Mat(_, r, c) = a.ty else {
                    return err(a.pos, format!("'transpose' needs a matrix, got '{}'", a.ty));
                };
                let a = cast_elem(a, Scalar::Float);
                Ok(call_expr(
                    pos,
                    Ty::Mat(Scalar::Float, c, r),
                    callee,
                    vec![a],
                ))
            }
            SigClass::Tex(dims, coords) => {
                let mut it = args.into_iter();
                let s = it.next().unwrap();
                let want = match dims {
                    2 => Ty::Sampler2D,
                    3 => Ty::Sampler3D,
                    _ => Ty::SamplerCube,
                };
                if s.ty != want || !matches!(s.kind, TExprKind::Var { .. }) {
                    return err(
                        s.pos,
                        format!("'{name}' needs a {want} variable, got '{}'", s.ty),
                    );
                }
                let mut out = vec![s];
                for (a, &w) in it.zip(coords) {
                    out.push(self.coerce(a, &Ty::vec_or_scalar(Scalar::Float, w))?);
                }
                Ok(call_expr(pos, Ty::Vec(Scalar::Float, 4), callee, out))
            }
        }
    }

    /// Resolve the generic type `T` of a componentwise intrinsic and convert
    /// every argument to it. `int_ok`: keep `int` when all arguments are
    /// integer. `vec_only`: `T` must be a vector.
    fn generic(
        &mut self,
        pos: Pos,
        name: &str,
        args: Vec<TExpr>,
        int_ok: bool,
        vec_only: bool,
    ) -> R<(Vec<TExpr>, Ty)> {
        for a in &args {
            match a.ty.shape() {
                Some(Shape::Mat(..)) => {
                    return err(
                        a.pos,
                        format!("matrix arguments to '{name}' are not supported"),
                    )
                }
                None => return err(a.pos, format!("'{name}' does not take '{}'", a.ty)),
                _ => {}
            }
        }
        let tys: Vec<&Ty> = args.iter().map(|a| &a.ty).collect();
        let shape = self.common_shape(pos, &tys)?;
        if vec_only && shape == Shape::Scalar {
            return err(pos, format!("'{name}' needs vector arguments"));
        }
        let all_int = args.iter().all(|a| a.ty.elem() != Some(Scalar::Float));
        let elem = if int_ok && all_int {
            Scalar::Int
        } else {
            Scalar::Float
        };
        let t = Ty::numeric(elem, shape);
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push(self.coerce(a, &t)?);
        }
        Ok((out, t))
    }

    /// `mul` overloads (see [`crate::intrinsics`]).
    fn mul(&mut self, pos: Pos, args: Vec<TExpr>) -> R<TExpr> {
        let mut it = args.into_iter();
        let (a, b) = (it.next().unwrap(), it.next().unwrap());
        let (Some(sa), Some(sb)) = (a.ty.shape(), b.ty.shape()) else {
            return err(
                pos,
                format!("'mul' does not take '{}' and '{}'", a.ty, b.ty),
            );
        };
        let f = |s: Shape| Ty::numeric(Scalar::Float, s);
        let (ta, tb, ret) = match (sa, sb) {
            (Shape::Scalar, s) | (s, Shape::Scalar) => {
                let (x, y) = if sa == Shape::Scalar {
                    (Ty::Float, f(s))
                } else {
                    (f(s), Ty::Float)
                };
                (x, y, f(s))
            }
            (Shape::Vec(n), Shape::Vec(m)) => {
                let k = n.min(m);
                (f(Shape::Vec(k)), f(Shape::Vec(k)), Ty::Float)
            }
            (Shape::Vec(n), Shape::Mat(r, c)) => {
                let k = n.min(r);
                (f(Shape::Vec(k)), f(Shape::Mat(k, c)), f(Shape::Vec(c)))
            }
            (Shape::Mat(r, c), Shape::Vec(n)) => {
                let k = c.min(n);
                (f(Shape::Mat(r, k)), f(Shape::Vec(k)), f(Shape::Vec(r)))
            }
            (Shape::Mat(r, c), Shape::Mat(p, q)) => {
                let k = c.min(p);
                (
                    f(Shape::Mat(r, k)),
                    f(Shape::Mat(k, q)),
                    f(Shape::Mat(r, q)),
                )
            }
        };
        let a = self.coerce(a, &ta)?;
        let b = self.coerce(b, &tb)?;
        Ok(call_expr(
            pos,
            ret,
            Callee::Intrinsic(Intrinsic::Mul),
            vec![a, b],
        ))
    }
}

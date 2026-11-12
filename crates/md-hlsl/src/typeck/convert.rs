//! Implicit and explicit conversions (fxc legacy rules).

use super::{err, Checker, R};
use crate::typed::{TExpr, TExprKind};
use crate::types::{Scalar, Shape, Ty};

/// Element change only (no-op when the element already matches).
pub(super) fn cast_elem(e: TExpr, s: Scalar) -> TExpr {
    if e.ty.elem() == Some(s) || !e.ty.is_numeric() {
        return e;
    }
    let ty = e.ty.with_elem(s);
    TExpr::new(e.pos, ty, TExprKind::Cast(Box::new(e)))
}

/// Replicate a scalar (of `to`'s element) into vector/matrix `to`.
pub(super) fn splat(e: TExpr, to: &Ty) -> TExpr {
    TExpr::new(e.pos, to.clone(), TExprKind::Splat(Box::new(e)))
}

/// The first `n` components of a scalar or vector (`n == 1` → scalar).
pub(super) fn swizzle(e: TExpr, comps: Vec<u8>) -> TExpr {
    let s = e.ty.elem().expect("swizzle of non-numeric");
    let ty = Ty::vec_or_scalar(s, comps.len() as u8);
    TExpr::new(
        e.pos,
        ty,
        TExprKind::Swizzle {
            base: Box::new(e),
            comps,
        },
    )
}

/// Cost of an implicit conversion, for overload ranking: 0 exact, 1 element
/// change, 2 splat, 3 truncation; `None` when impossible.
pub(super) fn conv_cost(from: &Ty, to: &Ty) -> Option<u32> {
    if from == to {
        return Some(0);
    }
    let (fs, ts) = (from.shape()?, to.shape()?);
    let base = match (fs, ts) {
        (Shape::Scalar, Shape::Scalar) => 1,
        (Shape::Scalar, _) => 2,
        (Shape::Vec(n), Shape::Vec(m)) if m == n => 1,
        (Shape::Vec(n), Shape::Vec(m)) if m < n => 3,
        (Shape::Vec(_), Shape::Scalar) | (Shape::Mat(..), Shape::Scalar) => 3,
        (Shape::Mat(r, c), Shape::Mat(r2, c2)) if (r2, c2) == (r, c) => 1,
        (Shape::Mat(r, c), Shape::Mat(r2, c2)) if r2 <= r && c2 <= c => 3,
        _ => return None,
    };
    Some(base)
}

impl Checker {
    /// Implicit conversion of `e` to `to` (assignment, initializer, return,
    /// argument); truncation warns.
    pub(super) fn coerce(&mut self, e: TExpr, to: &Ty) -> R<TExpr> {
        self.convert(e, to, true)
    }

    /// Explicit `(T)e`: the implicit rules without warnings.
    pub(super) fn explicit_cast(&mut self, e: TExpr, to: &Ty) -> R<TExpr> {
        self.convert(e, to, false)
    }

    fn convert(&mut self, e: TExpr, to: &Ty, implicit: bool) -> R<TExpr> {
        if e.ty == *to {
            return Ok(e);
        }
        let fail = |e: &TExpr| {
            let how = if implicit {
                "implicitly convert"
            } else {
                "cast"
            };
            err(e.pos, format!("cannot {how} from '{}' to '{to}'", e.ty))
        };
        let (Some(fs), Some(ts)) = (e.ty.shape(), to.shape()) else {
            return fail(&e);
        };
        let te = to.elem().unwrap();
        let truncated = |c: &mut Checker, e: &TExpr| {
            if implicit {
                c.warn(
                    e.pos,
                    format!("implicit truncation of vector type ({} to {to})", e.ty),
                );
            }
        };
        Ok(match (fs, ts) {
            (Shape::Scalar, Shape::Scalar) => cast_elem(e, te),
            (Shape::Scalar, _) => splat(cast_elem(e, te), to),
            (Shape::Vec(n), Shape::Vec(m)) if m <= n => {
                if m < n {
                    truncated(self, &e);
                }
                cast_elem(swizzle(e, (0..m).collect()), te)
            }
            (Shape::Vec(_), Shape::Scalar) => {
                truncated(self, &e);
                cast_elem(swizzle(e, vec![0]), te)
            }
            (Shape::Mat(r, c), Shape::Mat(r2, c2)) if r2 <= r && c2 <= c => {
                if (r2, c2) == (r, c) {
                    cast_elem(e, te)
                } else {
                    truncated(self, &e);
                    TExpr::new(e.pos, to.clone(), TExprKind::Cast(Box::new(e)))
                }
            }
            (Shape::Mat(..), Shape::Scalar) => {
                truncated(self, &e);
                let s = e.ty.elem().unwrap();
                let el = TExpr::new(
                    e.pos,
                    Ty::scalar(s),
                    TExprKind::MatElems {
                        base: Box::new(e),
                        elems: vec![(0, 0)],
                    },
                );
                cast_elem(el, te)
            }
            _ => return fail(&e),
        })
    }

    /// The common shape of operands: scalars adapt to anything, vectors and
    /// matrices truncate to the smallest (the truncation warns when the
    /// operands are coerced). Vectors and matrices do not mix.
    pub(super) fn common_shape(&self, pos: crate::ast::Pos, tys: &[&Ty]) -> R<Shape> {
        let mut shape = Shape::Scalar;
        for t in tys {
            let Some(s) = t.shape() else {
                return err(pos, format!("expected a numeric operand, got '{t}'"));
            };
            shape = match (shape, s) {
                (a, Shape::Scalar) => a,
                (Shape::Scalar, b) => b,
                (Shape::Vec(a), Shape::Vec(b)) => Shape::Vec(a.min(b)),
                (Shape::Mat(r, c), Shape::Mat(r2, c2)) => Shape::Mat(r.min(r2), c.min(c2)),
                _ => {
                    let list: Vec<String> = tys.iter().map(|t| t.to_string()).collect();
                    return err(
                        pos,
                        format!(
                            "cannot combine vector and matrix operands ({})",
                            list.join(", ")
                        ),
                    );
                }
            };
        }
        Ok(shape)
    }
}

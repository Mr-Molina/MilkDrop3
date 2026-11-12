//! Assignment targets of the WGSL emitter (Task 4.4).
//!
//! A [`Place`] is an lvalue whose sub-expressions were evaluated once: a
//! non-literal, non-identifier array/vector index is captured in a
//! `let md_t<N>` first, so the place can be read and written again
//! without re-running side effects (`a[i++] *= 1.5` increments `i` once).
//!
//! WGSL can assign a whole variable, one component (`v.x = …`, `v[i] = …`,
//! `m[r][c] = …`) or use a compound operator on such a reference, but not a
//! multi-component swizzle. A write to `v.xz` therefore becomes
//! `let t = value; v.x = t.x; v.z = t.y;` (likewise for matrix element
//! masks `m._m00_m11`).
//!
//! Compound assignment `lhs op= rhs` (with the checker's `op_ty`, the type
//! HLSL computes in) uses WGSL's own `op=` when `op_ty` is the lhs type and
//! the place is a single reference; otherwise it is lowered to
//! `place = L(O(place) op rhs)` with `O`/`L` the implicit conversions to and
//! from `op_ty` (e.g. `int i; i *= 1.5` → `h_i = i32((f32(h_i) * 1.5));`).
//! Matrix `*=`, `/=`, `%=` are componentwise in HLSL and always lowered.
//! `++`/`--` write `place = (place ± 1)`; the postfix forms used as a value
//! snapshot the old value first.

use crate::ast::{BinOp, UnOp};
use crate::emit::{err, one, R};
use crate::emit_expr::{letters, op_sym};
use crate::emit_stmt::Fx;
use crate::typed::{TExpr, TExprKind, VarOrigin};
use crate::types::Ty;

/// The selected part of a place's base.
#[derive(Clone, Debug)]
pub(crate) enum Sel {
    Whole,
    /// Vector components (0-based).
    Comps(Vec<u8>),
    /// Matrix elements as HLSL (row, col) = WGSL `[row][col]`.
    Elems(Vec<(u8, u8)>),
}

#[derive(Clone, Debug)]
pub(crate) struct Place {
    pub base: String,
    pub sel: Sel,
}

impl Place {
    fn single(&self) -> bool {
        match &self.sel {
            Sel::Whole => true,
            Sel::Comps(c) => c.len() == 1,
            Sel::Elems(e) => e.len() == 1,
        }
    }
}

impl Fx<'_, '_> {
    /// Evaluate the lvalue `e` once.
    pub fn place(&mut self, e: &TExpr) -> R<Place> {
        use TExprKind as K;
        let whole = |p: Place, what: &str| match p.sel {
            Sel::Whole => Ok(p.base),
            _ => err(e.pos, format!("{what} of a swizzle cannot be assigned")),
        };
        let (base, sel) = match &e.kind {
            K::Var { name, origin } => {
                let v = self.var_ref(name, *origin, e.pos)?;
                if matches!(origin, VarOrigin::Global(_)) && !v.starts_with("h_") {
                    return err(e.pos, format!("`{name}` is read-only"));
                }
                (v, Sel::Whole)
            }
            K::Index { base, index } => {
                let b = self.place(base)?;
                let b = whole(b, "an index")?;
                let i = self.expr(index)?;
                let i = if crate::emit_expr::is_lit(index) || i.chars().all(is_ident_char) {
                    i
                } else {
                    self.snapshot(i)
                };
                (format!("{b}[{i}]"), Sel::Whole)
            }
            K::Field { base, name } => {
                let b = self.place(base)?;
                (format!("{}.h_{name}", whole(b, "a field")?), Sel::Whole)
            }
            K::Swizzle { base, comps } => {
                let p = self.place(base)?;
                if base.ty.is_scalar() {
                    return Ok(p);
                }
                let sel = match p.sel {
                    Sel::Whole => Sel::Comps(comps.clone()),
                    Sel::Comps(c0) => Sel::Comps(comps.iter().map(|&i| c0[i as usize]).collect()),
                    Sel::Elems(e0) => Sel::Elems(comps.iter().map(|&i| e0[i as usize]).collect()),
                };
                (p.base, sel)
            }
            K::MatElems { base, elems } => {
                let b = self.place(base)?;
                (whole(b, "a matrix element")?, Sel::Elems(elems.clone()))
            }
            _ => return err(e.pos, "expression is not assignable"),
        };
        Ok(Place { base, sel })
    }

    /// The current value of `p`.
    pub fn read(&self, p: &Place) -> String {
        let b = &p.base;
        match &p.sel {
            Sel::Whole => b.clone(),
            Sel::Comps(c) => format!("{b}.{}", letters(c.iter().copied())),
            Sel::Elems(e) if e.len() == 1 => format!("{b}[{}][{}]", e[0].0, e[0].1),
            Sel::Elems(e) => {
                let parts: Vec<String> = e.iter().map(|(r, c)| format!("{b}[{r}][{c}]")).collect();
                format!("vec{}<f32>({})", e.len(), parts.join(", "))
            }
        }
    }

    /// Store `v` into `p`.
    pub fn write(&mut self, p: &Place, v: String) {
        if p.single() {
            let r = self.read(p);
            self.pre.push(format!("{r} = {v};"));
            return;
        }
        let t = self.snapshot(v);
        let b = &p.base;
        let dests: Vec<String> = match &p.sel {
            Sel::Comps(c) => c.iter().map(|&i| format!("{b}.{}", letters([i]))).collect(),
            Sel::Elems(e) => e.iter().map(|(r, c)| format!("{b}[{r}][{c}]")).collect(),
            Sel::Whole => unreachable!(),
        };
        for (k, d) in dests.into_iter().enumerate() {
            self.pre.push(format!("{d} = {t}.{};", letters([k as u8])));
        }
    }

    /// An assignment; returns the assigned value when `value` is set.
    pub fn assign(&mut self, e: &TExpr, value: bool) -> R<String> {
        let TExprKind::Assign {
            op,
            op_ty,
            lhs,
            rhs,
        } = &e.kind
        else {
            unreachable!()
        };
        let p = self.place(lhs)?;
        let r = self.expr(rhs)?;
        match op {
            None => self.write(&p, r),
            Some(op) => {
                let r = self.divisor(*op, rhs, r)?;
                let lt = &lhs.ty;
                let native = p.single()
                    && op_ty == lt
                    && !(matches!(lt, Ty::Mat(..))
                        && matches!(op, BinOp::Mul | BinOp::Div | BinOp::Rem))
                    && matches!(
                        op,
                        BinOp::Add
                            | BinOp::Sub
                            | BinOp::Mul
                            | BinOp::Div
                            | BinOp::Rem
                            | BinOp::BitAnd
                            | BinOp::BitOr
                            | BinOp::BitXor
                    );
                let cur = self.read(&p);
                if native {
                    self.pre.push(format!("{cur} {}= {r};", op_sym(*op, true)));
                } else {
                    let o = self.convert(cur, lt, op_ty, e.pos)?;
                    let v = self.binop(*op, o, r, op_ty, e.pos)?;
                    let l = self.convert(v, op_ty, lt, e.pos)?;
                    self.write(&p, l);
                }
            }
        }
        Ok(if value { self.read(&p) } else { String::new() })
    }

    /// `++`/`--`; returns the expression's value when `value` is set.
    pub fn incdec(&mut self, op: UnOp, operand: &TExpr, value: bool) -> R<String> {
        let p = self.place(operand)?;
        let one = one(&operand.ty, operand.pos)?;
        let sym = if matches!(op, UnOp::PreInc | UnOp::PostInc) {
            "+"
        } else {
            "-"
        };
        let cur = self.read(&p);
        let post = matches!(op, UnOp::PostInc | UnOp::PostDec);
        let old = (post && value).then(|| self.snapshot(cur.clone()));
        self.write(&p, format!("({cur} {sym} {one})"));
        Ok(match old {
            Some(o) => o,
            None if value => self.read(&p),
            None => String::new(),
        })
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

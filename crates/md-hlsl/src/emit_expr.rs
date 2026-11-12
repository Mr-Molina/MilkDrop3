//! Expressions of the WGSL emitter (Task 4.4).
//!
//! Every emitted expression is parenthesised or a postfix-safe atom, so it
//! can be used as an operand or swizzled. Rules:
//!
//! - literals: `float` always has a `.`/exponent (`0` in float context is
//!   `0.0`; the checker's `Cast(Lit)` is folded), `int` is `5i`;
//! - swizzles pass through (`v.zx`); a swizzle of a scalar is a splat
//!   (`x.xxx` → `vec3<f32>(x)`); matrix rows `M[i]` and elements `M._m01`
//!   (`M[0][1]`) are unchanged under the transposed storage (F24);
//! - `Cast` is a WGSL conversion (`f32(i)`, `vec3<i32>(v)`), `Splat` a
//!   one-argument vector constructor (`vec3<f32>(x)`); matrix splats,
//!   matrix truncation casts (top-left block) and straddling matrix
//!   constructors are spelled column by column;
//! - `+ - * / %` map to WGSL operators; float `%` (HLSL `fmod`) is WGSL `%`,
//!   which the WGSL spec defines as `e1 - e2 * trunc(e1 / e2)`; `*`, `/`,
//!   `%` between matrices are componentwise in HLSL and are spelled per
//!   column; a right operand whose constant evaluation could fail (a
//!   constant zero divisor, a shift amount of 32 or more, ...) goes
//!   through a `var` so the operation is not const-evaluated (a WGSL
//!   shader-creation error) — see [`Fx::guard_rhs`] for the values;
//! - shifts convert the amount to `u32`; `<`, `>`, `<=`, `>=` on bools
//!   compare them as `i32` (WGSL orders no bools); `&&`/`||` stay `&&`/`||` on
//!   scalars (their operands' side effects are hoisted, so both are always
//!   evaluated, as fxc does) and become `&`/`|` on `bool` vectors;
//! - `?:` is `select(else, then, cond)` (vector conditions select
//!   componentwise); when a branch has side effects, or the type is not a
//!   scalar or vector, it becomes an `if` assigning a temporary.

use crate::ast::{BinOp, Lit, Pos, UnOp};
use crate::emit::{err, lit, shape_ty, wgsl_ty, GlobalRef, R};
use crate::emit_stmt::{has_effects, is_incdec, Fx};
use crate::typed::{ParamMode, TExpr, TExprKind, VarOrigin};
use crate::types::{Scalar, Shape, Ty};

const XYZW: [char; 4] = ['x', 'y', 'z', 'w'];

/// Component letters for 0-based indices.
pub(crate) fn letters(comps: impl IntoIterator<Item = u8>) -> String {
    comps.into_iter().map(|c| XYZW[c as usize]).collect()
}

/// A string that can be evaluated repeatedly without side effects or cost:
/// an identifier path (`h_x`, `U._c0`, `h_s.h_f`) or a dereferenced
/// pointer parameter.
pub(crate) fn is_path(s: &str) -> bool {
    let p = s
        .strip_prefix("(*")
        .and_then(|r| r.strip_suffix(')'))
        .unwrap_or(s);
    !p.is_empty()
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        && !p.starts_with(|c: char| c.is_ascii_digit() || c == '.')
}

/// A literal, possibly converted (`Cast(Lit)` / `Splat(Lit)`).
pub(crate) fn is_lit(e: &TExpr) -> bool {
    match &e.kind {
        TExprKind::Lit(_) => true,
        TExprKind::Cast(x) | TExprKind::Splat(x) => is_lit(x),
        _ => false,
    }
}

/// The value of an integer (or bool) literal, possibly converted.
fn int_lit(e: &TExpr) -> Option<i64> {
    match &e.kind {
        TExprKind::Lit(Lit::Int(v)) => Some(*v),
        TExprKind::Lit(Lit::Bool(b)) => Some(i64::from(*b)),
        TExprKind::Cast(x) | TExprKind::Splat(x) => int_lit(x),
        _ => None,
    }
}

/// A literal whose value is zero.
pub(crate) fn is_zero_lit(e: &TExpr) -> bool {
    match &e.kind {
        TExprKind::Lit(Lit::Int(0) | Lit::Bool(false)) => true,
        TExprKind::Lit(Lit::Float(f)) => *f == 0.0,
        TExprKind::Cast(x) | TExprKind::Splat(x) => is_zero_lit(x),
        _ => false,
    }
}

/// Side-effect-free and cheap to evaluate twice.
fn is_pure_cheap(e: &TExpr) -> bool {
    match &e.kind {
        TExprKind::Lit(_) | TExprKind::Var { .. } => true,
        TExprKind::Swizzle { base, .. } | TExprKind::Field { base, .. } => is_pure_cheap(base),
        TExprKind::Index { base, index } => is_pure_cheap(base) && is_lit(index),
        TExprKind::Cast(x) => is_lit(x),
        _ => false,
    }
}

impl Fx<'_, '_> {
    /// Emit `e` as a WGSL expression; side effects go to `pre`.
    ///
    /// A thin dispatcher: each kind is handled by a separate non-inlined
    /// method so deep expression trees (the parser allows height 512) use
    /// little stack per level.
    pub fn expr(&mut self, e: &TExpr) -> R<String> {
        use TExprKind as K;
        match &e.kind {
            K::Lit(l) => Ok(lit(*l, &e.ty)),
            K::Var { name, origin } => self.var_ref(name, *origin, e.pos),
            K::Index { base, index } => self.index(base, index),
            K::Swizzle { base, comps } => self.swizzle_expr(base, comps),
            K::MatElems { base, elems } => self.mat_elems(base, elems),
            K::Field { base, name } => self.field(base, name),
            K::Call { callee, args } => self.call(e, callee, args, false),
            K::Ctor { args } => self.ctor(e, args),
            K::Unary { op, operand } if is_incdec(*op) => self.incdec(*op, operand, true),
            K::Unary { op, operand } => self.unary(*op, operand),
            K::Binary { op, lhs, rhs } => self.binary(e, *op, lhs, rhs),
            K::Assign { .. } => self.assign(e, true),
            K::Ternary { cond, then, els } => self.ternary(e, cond, then, els),
            K::Cast(x) | K::Splat(x) => self.cast(e, x),
        }
    }

    #[inline(never)]
    fn index(&mut self, base: &TExpr, index: &TExpr) -> R<String> {
        let v = self.operands(&[base, index])?;
        Ok(format!("{}[{}]", v[0], v[1]))
    }

    #[inline(never)]
    fn swizzle_expr(&mut self, base: &TExpr, comps: &[u8]) -> R<String> {
        let b = self.expr(base)?;
        Ok(swizzle(b, &base.ty, comps))
    }

    #[inline(never)]
    fn field(&mut self, base: &TExpr, name: &str) -> R<String> {
        Ok(format!("{}.h_{name}", self.expr(base)?))
    }

    #[inline(never)]
    fn unary(&mut self, op: UnOp, operand: &TExpr) -> R<String> {
        let x = self.expr(operand)?;
        Ok(match op {
            // WGSL has no matrix negation.
            UnOp::Neg if matches!(operand.ty, Ty::Mat(..)) => format!("(-1.0 * {x})"),
            UnOp::Neg => format!("(-{x})"),
            UnOp::Not => format!("(!{x})"),
            UnOp::BitNot => format!("(~{x})"),
            _ => x,
        })
    }

    #[inline(never)]
    fn cast(&mut self, e: &TExpr, x: &TExpr) -> R<String> {
        if let TExprKind::Lit(l) = x.kind {
            if e.ty.is_scalar() {
                return Ok(lit(l, &e.ty));
            }
        }
        let v = self.expr(x)?;
        self.convert(v, &x.ty, &e.ty, e.pos)
    }

    /// The spelling of a variable reference.
    pub fn var_ref(&self, name: &str, origin: VarOrigin, pos: Pos) -> R<String> {
        Ok(match origin {
            VarOrigin::Local | VarOrigin::Param(ParamMode::In) => format!("h_{name}"),
            VarOrigin::Param(_) => format!("(*h_{name})"),
            VarOrigin::Global(_) => match self.cx.globals.get(name) {
                Some(GlobalRef::Expr(s)) => s.clone(),
                Some(GlobalRef::Sampler(_)) => {
                    return err(pos, format!("sampler `{name}` used as a value"))
                }
                None => return err(pos, format!("global `{name}` has no binding")),
            },
        })
    }

    /// `let md_t<N> = s;` unless `s` already is a temporary or a literal.
    pub fn snapshot(&mut self, s: String) -> String {
        let is_lit = s.starts_with(|c: char| c.is_ascii_digit()) || s == "true" || s == "false";
        if is_lit || (s.starts_with("md_t") && is_path(&s)) {
            return s;
        }
        let t = self.fresh();
        self.pre.push(format!("let {t} = {s};"));
        t
    }

    /// `s` itself if it is a path, else a snapshot (for repeated use).
    pub fn reuse(&mut self, s: String) -> String {
        if is_path(&s) {
            s
        } else {
            self.snapshot(s)
        }
    }

    /// Emit `e` for repeated use.
    pub fn cheap(&mut self, e: &TExpr) -> R<String> {
        let s = self.expr(e)?;
        Ok(if is_pure_cheap(e) { s } else { self.reuse(s) })
    }

    /// Emit operands left to right; operands before the last one with side
    /// effects are snapshotted so they observe the HLSL evaluation order.
    pub fn operands(&mut self, es: &[&TExpr]) -> R<Vec<String>> {
        let last = es.iter().rposition(|e| has_effects(e));
        let mut out = Vec::with_capacity(es.len());
        for (i, e) in es.iter().enumerate() {
            let s = self.expr(e)?;
            out.push(if last.is_some_and(|l| i < l) && !is_lit(e) {
                self.snapshot(s)
            } else {
                s
            });
        }
        Ok(out)
    }

    /// Guard the right operand `s` of `op` against WGSL constant
    /// evaluation: when naga would fold the operation (or check it in the
    /// validator) and the fold could fail, the operand is moved into a
    /// `var`, so the operation happens at run time under WGSL's runtime
    /// rules. fxc folded these with a warning; the results are:
    ///
    /// - integer `/`, `%` whose divisor is a constant other than a literal
    ///   outside {0, -1} (`5 / N` with `static const int N = 0`,
    ///   `INT_MIN / -1`): WGSL runtime `x / 0 = x`, `x % 0 = 0`,
    ///   `INT_MIN / -1 = INT_MIN`, `INT_MIN % -1 = 0`. The SM2/3 targets did
    ///   integer math in float (`5 / 0` = +inf, cast back to int), which is
    ///   not reproducible exactly here (the cast of inf is undefined);
    /// - float `/`, `%` (`fmod`) whose divisor is a literal zero or a
    ///   non-literal constant: IEEE `±inf`/NaN at run time, as the SM2/3
    ///   float hardware produced, instead of naga's "literal is infinite";
    /// - shifts whose amount is a constant other than a literal in 0..32, or
    ///   whose left operand is also constant (`3 << 31` overflows naga's
    ///   signed constant shift): WGSL runtime shifts take the amount modulo
    ///   32 (`1 << 40` = 256, `1 << -1` = `1 << 31`) and discard the bits
    ///   shifted out — what x86 `shl`/`sar` (count masked to 5 bits) gave a
    ///   C-style folder; fxc's own folded value is not verifiable here.
    ///
    /// `lhs` is `None` when the left operand is a runtime place (compound
    /// assignment).
    pub fn guard_rhs(
        &mut self,
        op: BinOp,
        lhs: Option<&TExpr>,
        rhs: &TExpr,
        s: String,
    ) -> R<String> {
        let konst = |e: &TExpr| crate::emit::naga_const(e, &self.cx.consts);
        let int = rhs.ty.elem() != Some(Scalar::Float);
        let guard = match op {
            BinOp::Div | BinOp::Rem if int => {
                konst(rhs) && !int_lit(rhs).is_some_and(|v| v != 0 && v != -1)
            }
            BinOp::Div | BinOp::Rem => is_zero_lit(rhs) || (konst(rhs) && !is_lit(rhs)),
            BinOp::Shl | BinOp::Shr => {
                konst(rhs)
                    && (lhs.is_some_and(konst)
                        || !int_lit(rhs).is_some_and(|v| (0..32).contains(&v)))
            }
            _ => false,
        };
        if !guard {
            return Ok(s);
        }
        let t = self.fresh();
        let ty = wgsl_ty(&rhs.ty, rhs.pos)?;
        self.pre.push(format!("var {t}: {ty} = {s};"));
        Ok(t)
    }

    #[inline(never)]
    fn binary(&mut self, e: &TExpr, op: BinOp, lhs: &TExpr, rhs: &TExpr) -> R<String> {
        let mut v = self.operands(&[lhs, rhs])?;
        let b = v.pop().unwrap();
        let b = self.guard_rhs(op, Some(lhs), rhs, b)?;
        let a = v.pop().unwrap();
        self.binop(op, a, b, &lhs.ty, e.pos)
    }

    /// `a op b` for operands of type `ty`.
    pub fn binop(&mut self, op: BinOp, a: String, b: String, ty: &Ty, pos: Pos) -> R<String> {
        use BinOp::*;
        let scalar = ty.is_scalar();
        let sym = op_sym(op, scalar);
        if let Ty::Mat(_, r, c) = *ty {
            if matches!(op, Mul | Div | Rem) {
                let (a, b) = (self.reuse(a), self.reuse(b));
                let cols: Vec<String> = (0..r)
                    .map(|i| format!("({a}[{i}] {sym} {b}[{i}])"))
                    .collect();
                return Ok(format!("mat{r}x{c}<f32>({})", cols.join(", ")));
            }
            if !matches!(op, Add | Sub) {
                return err(
                    pos,
                    format!("operator `{sym}` on matrices is not supported"),
                );
            }
        }
        if matches!(op, Lt | Gt | Le | Ge) && ty.elem() == Some(Scalar::Bool) {
            // WGSL orders no bools; HLSL compares them as 0/1.
            let i = match ty.shape() {
                Some(Shape::Vec(n)) => format!("vec{n}<i32>"),
                _ => "i32".to_string(),
            };
            return Ok(format!("({i}({a}) {sym} {i}({b}))"));
        }
        if matches!(op, Shl | Shr) {
            let u = match ty.shape() {
                Some(Shape::Vec(n)) => format!("vec{n}<u32>"),
                _ => "u32".to_string(),
            };
            return Ok(format!("({a} {sym} {u}({b}))"));
        }
        Ok(format!("({a} {sym} {b})"))
    }

    #[inline(never)]
    fn ternary(&mut self, e: &TExpr, cond: &TExpr, then: &TExpr, els: &TExpr) -> R<String> {
        let selectable = matches!(e.ty.shape(), Some(Shape::Scalar | Shape::Vec(_)));
        if selectable && !has_effects(then) && !has_effects(els) {
            let v = self.operands(&[cond, then, els])?;
            return Ok(format!("select({}, {}, {})", v[2], v[1], v[0]));
        }
        if !cond.ty.is_scalar() {
            // Componentwise select: fxc evaluates both branches anyway.
            let v = self.operands(&[cond, then, els])?;
            let (t, f) = (v[1].clone(), v[2].clone());
            let (t, f) = (self.snapshot(t), self.snapshot(f));
            return Ok(format!("select({f}, {t}, {})", v[0]));
        }
        let c = self.expr(cond)?;
        let t = self.fresh();
        let ty = wgsl_ty(&e.ty, e.pos)?;
        let (tp, tv) = self.nested(|s| s.expr(then))?;
        let (ep, ev) = self.nested(|s| s.expr(els))?;
        self.pre.push(format!("var {t}: {ty};"));
        self.pre.push(format!("if {c} {{"));
        self.pre.extend(tp.into_iter().map(|l| format!("    {l}")));
        self.pre.push(format!("    {t} = {tv};"));
        self.pre.push("} else {".to_string());
        self.pre.extend(ep.into_iter().map(|l| format!("    {l}")));
        self.pre.push(format!("    {t} = {ev};"));
        self.pre.push("}".to_string());
        Ok(t)
    }

    #[inline(never)]
    fn mat_elems(&mut self, base: &TExpr, elems: &[(u8, u8)]) -> R<String> {
        if let [(r, c)] = elems {
            return Ok(format!("{}[{r}][{c}]", self.expr(base)?));
        }
        let b = self.cheap(base)?;
        let parts: Vec<String> = elems
            .iter()
            .map(|(r, c)| format!("{b}[{r}][{c}]"))
            .collect();
        Ok(format!("vec{}<f32>({})", elems.len(), parts.join(", ")))
    }

    #[inline(never)]
    fn ctor(&mut self, e: &TExpr, args: &[TExpr]) -> R<String> {
        let refs: Vec<&TExpr> = args.iter().collect();
        let v = self.operands(&refs)?;
        let ty = wgsl_ty(&e.ty, e.pos)?;
        let Ty::Mat(_, _, c) = e.ty else {
            return Ok(format!("{ty}({})", v.join(", ")));
        };
        // HLSL row-major arguments are WGSL column-major arguments of the
        // transposed matrix: all scalars, or whole rows, pass through.
        let rows = args.iter().all(|a| a.ty == Ty::Vec(Scalar::Float, c));
        if rows || args.iter().all(|a| a.ty.is_scalar()) {
            return Ok(format!("{ty}({})", v.join(", ")));
        }
        let mut flat = Vec::new();
        for (a, s) in args.iter().zip(v) {
            match a.ty {
                Ty::Vec(_, n) => {
                    let s = self.reuse(s);
                    flat.extend((0..n).map(|k| format!("{s}.{}", XYZW[k as usize])));
                }
                _ => flat.push(s),
            }
        }
        Ok(format!("{ty}({})", flat.join(", ")))
    }

    /// Convert `s` of type `from` to `to`: element casts, scalar splats,
    /// vector truncation and matrix splat / truncation.
    pub fn convert(&mut self, s: String, from: &Ty, to: &Ty, pos: Pos) -> R<String> {
        if from == to {
            return Ok(s);
        }
        let (Some(fe), Some(fs), Some(te), Some(ts)) =
            (from.elem(), from.shape(), to.elem(), to.shape())
        else {
            return err(pos, format!("cannot convert `{from}` to `{to}`"));
        };
        let cast = |s: String, shape: Shape| {
            if fe == te {
                s
            } else {
                format!("{}({s})", shape_ty(te, shape))
            }
        };
        Ok(match (fs, ts) {
            (Shape::Scalar, Shape::Scalar) => cast(s, ts),
            (Shape::Vec(n), Shape::Vec(m)) if n == m => cast(s, ts),
            (Shape::Vec(n), Shape::Vec(m)) if m < n => cast(format!("{s}.{}", letters(0..m)), ts),
            (Shape::Vec(_), Shape::Scalar) => cast(format!("{s}.x"), ts),
            (Shape::Scalar, Shape::Vec(_)) => {
                format!("{}({})", shape_ty(te, ts), cast(s, Shape::Scalar))
            }
            (Shape::Scalar, Shape::Mat(r, c)) if te == Scalar::Float => {
                let x = self.reuse(cast(s, Shape::Scalar));
                let col = format!("vec{c}<f32>({x})");
                format!("mat{r}x{c}<f32>({})", vec![col; r as usize].join(", "))
            }
            (Shape::Mat(r, c), Shape::Mat(r2, c2)) if r2 <= r && c2 <= c && fe == te => {
                let x = self.reuse(s);
                let sw = if c2 < c {
                    format!(".{}", letters(0..c2))
                } else {
                    String::new()
                };
                let cols: Vec<String> = (0..r2).map(|i| format!("{x}[{i}]{sw}")).collect();
                format!("mat{r2}x{c2}<f32>({})", cols.join(", "))
            }
            _ => return err(pos, format!("cannot convert `{from}` to `{to}`")),
        })
    }
}

/// The WGSL operator for `op` (`&&`/`||` become `&`/`|` on vectors).
pub(crate) fn op_sym(op: BinOp, scalar: bool) -> &'static str {
    use BinOp::*;
    match op {
        Mul => "*",
        Div => "/",
        Rem => "%",
        Add => "+",
        Sub => "-",
        Shl => "<<",
        Shr => ">>",
        Lt => "<",
        Gt => ">",
        Le => "<=",
        Ge => ">=",
        Eq => "==",
        Ne => "!=",
        BitAnd => "&",
        BitXor => "^",
        BitOr => "|",
        And if scalar => "&&",
        Or if scalar => "||",
        And => "&",
        Or => "|",
    }
}

/// A swizzle of `b` (of type `ty`).
pub(crate) fn swizzle(b: String, ty: &Ty, comps: &[u8]) -> String {
    match ty {
        Ty::Bool | Ty::Int | Ty::Float if comps.len() == 1 => b,
        Ty::Bool | Ty::Int | Ty::Float => {
            format!(
                "vec{}<{}>({b})",
                comps.len(),
                crate::emit::elem_name(ty.elem().unwrap())
            )
        }
        _ => format!("{b}.{}", letters(comps.iter().copied())),
    }
}

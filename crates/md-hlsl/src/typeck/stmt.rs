//! Statements, local declarations and initializers (including `{…}` lists).

use std::collections::VecDeque;

use super::convert::cast_elem;
use super::{err, Checker, LocalSym, R};
use crate::ast::{Block, Declarator, Expr, ForInit, Init, Pos, SamplerKind, Stmt, VarDecl};
use crate::typed::{TBlock, TExpr, TExprKind, TLocal, TStmt, VarOrigin};
use crate::types::{Scalar, Shape, Ty};

impl Checker {
    /// Check a block's statements; `scoped` pushes a new scope (the
    /// function's top block shares the parameters' scope).
    pub(super) fn block_stmts(&mut self, b: &Block, scoped: bool) -> R<TBlock> {
        if scoped {
            self.scopes.push(Default::default());
        }
        let mut stmts = Vec::new();
        for s in &b.stmts {
            self.stmt(s, &mut stmts)?;
        }
        if scoped {
            self.scopes.pop();
        }
        Ok(TBlock { pos: b.pos, stmts })
    }

    /// A nested statement (branch or loop body) in its own scope.
    fn sub_stmt(&mut self, s: &Stmt) -> R<TStmt> {
        self.scopes.push(Default::default());
        let mut out = Vec::new();
        self.stmt(s, &mut out)?;
        self.scopes.pop();
        Ok(if out.len() == 1 {
            out.pop().unwrap()
        } else {
            TStmt::Block(TBlock {
                pos: s.pos(),
                stmts: out,
            })
        })
    }

    fn loop_body(&mut self, s: &Stmt) -> R<TStmt> {
        self.loop_depth += 1;
        let b = self.sub_stmt(s);
        self.loop_depth -= 1;
        b
    }

    /// A scalar condition, converted to `bool`.
    fn cond(&mut self, e: &Expr) -> R<TExpr> {
        let t = self.expr(e)?;
        if !t.ty.is_scalar() {
            return err(t.pos, format!("condition must be a scalar, got '{}'", t.ty));
        }
        Ok(cast_elem(t, Scalar::Bool))
    }

    fn stmt(&mut self, s: &Stmt, out: &mut Vec<TStmt>) -> R<()> {
        let t = match s {
            Stmt::VarDecl(v) => return self.local_decl(v, out, false).map(drop),
            Stmt::Expr { expr, .. } => TStmt::Expr(self.expr(expr)?),
            Stmt::If {
                pos,
                cond,
                then,
                els,
            } => TStmt::If {
                pos: *pos,
                cond: self.cond(cond)?,
                then: Box::new(self.sub_stmt(then)?),
                els: match els {
                    Some(e) => Some(Box::new(self.sub_stmt(e)?)),
                    None => None,
                },
            },
            Stmt::For {
                pos,
                init,
                cond,
                step,
                body,
            } => {
                let init = match init {
                    ForInit::None => Vec::new(),
                    ForInit::Decl(v) => self.local_decl(v, out, true)?,
                    ForInit::Exprs(es) => self.exprs(es)?,
                };
                let cond = match cond {
                    Some(c) => Some(self.cond(c)?),
                    None => None,
                };
                TStmt::For {
                    pos: *pos,
                    init,
                    cond,
                    step: self.exprs(step)?,
                    body: Box::new(self.loop_body(body)?),
                }
            }
            Stmt::While { pos, cond, body } => TStmt::While {
                pos: *pos,
                cond: self.cond(cond)?,
                body: Box::new(self.loop_body(body)?),
            },
            Stmt::DoWhile { pos, body, cond } => TStmt::DoWhile {
                pos: *pos,
                body: Box::new(self.loop_body(body)?),
                cond: self.cond(cond)?,
            },
            Stmt::Return { pos, value } => self.ret(*pos, value.as_ref())?,
            Stmt::Block(b) => TStmt::Block(self.block_stmts(b, true)?),
            Stmt::Break { pos } | Stmt::Continue { pos } if self.loop_depth == 0 => {
                return err(*pos, "break/continue outside a loop");
            }
            Stmt::Break { pos } => TStmt::Break { pos: *pos },
            Stmt::Continue { pos } => TStmt::Continue { pos: *pos },
            Stmt::Discard { pos } => TStmt::Discard { pos: *pos },
        };
        out.push(t);
        Ok(())
    }

    fn exprs(&mut self, es: &[Expr]) -> R<Vec<TExpr>> {
        es.iter().map(|e| self.expr(e)).collect()
    }

    fn ret(&mut self, pos: Pos, value: Option<&Expr>) -> R<TStmt> {
        let ret = self
            .cur_fn
            .as_ref()
            .expect("return outside a function")
            .0
            .clone();
        let value = match (value, &ret) {
            (None, Ty::Void) => None,
            (Some(_), Ty::Void) => return err(pos, "void function cannot return a value"),
            (None, t) => return err(pos, format!("function must return a '{t}'")),
            (Some(e), t) => {
                let v = self.expr(e)?;
                Some(self.coerce(v, t)?)
            }
        };
        Ok(TStmt::Return { pos, value })
    }

    /// Local declarations, appended to `out` as `Local`s. In a `for` init
    /// (`for_init`), a redeclaration of a same-typed variable of the
    /// current scope becomes an assignment, returned as an init expression.
    fn local_decl(&mut self, v: &VarDecl, out: &mut Vec<TStmt>, for_init: bool) -> R<Vec<TExpr>> {
        let mut inits = Vec::new();
        for d in &v.vars {
            let base = self.base_ty(&v.ty, SamplerKind::S2D)?;
            if base == Ty::Void || base.is_sampler() {
                return err(d.pos, format!("local variables cannot be {base}"));
            }
            let (ty, init) = self.declarator_init(d, base)?;
            let existing = self.scopes.last().unwrap().get(&d.name).cloned();
            if let (true, Some(prev)) = (for_init, existing) {
                if prev.ty != ty || prev.origin != VarOrigin::Local || prev.is_const {
                    return err(d.pos, format!("redefinition of '{}'", d.name));
                }
                self.warn(
                    d.pos,
                    format!(
                        "loop variable '{}' redeclared in the same scope; reusing it",
                        d.name
                    ),
                );
                if let Some(init) = init {
                    let var = TExpr::new(
                        d.pos,
                        ty.clone(),
                        TExprKind::Var {
                            name: d.name.clone(),
                            origin: VarOrigin::Local,
                        },
                    );
                    let kind = TExprKind::Assign {
                        op: None,
                        lhs: Box::new(var),
                        rhs: Box::new(init),
                    };
                    inits.push(TExpr::new(d.pos, ty, kind));
                }
                continue;
            }
            let const_int = match (&d.init, v.quals.is_const, &ty) {
                (Some(Init::Expr(e)), true, Ty::Int) => self.const_int(e),
                _ => None,
            };
            let sym = LocalSym {
                ty: ty.clone(),
                origin: VarOrigin::Local,
                is_const: v.quals.is_const,
                const_int,
            };
            self.declare_local(d.pos, &d.name, sym)?;
            out.push(TStmt::Local(TLocal {
                pos: d.pos,
                name: d.name.clone(),
                ty,
                is_const: v.quals.is_const,
                init,
            }));
        }
        Ok(inits)
    }

    /// The declarator's full type (array dimensions, unsized length from an
    /// initializer list) and its converted initializer.
    pub(super) fn declarator_init(&mut self, d: &Declarator, base: Ty) -> R<(Ty, Option<TExpr>)> {
        match &d.init {
            None => Ok((self.array_ty(d.pos, base, &d.array, None)?, None)),
            Some(Init::Expr(e)) => {
                let ty = self.array_ty(d.pos, base, &d.array, None)?;
                let v = self.expr(e)?;
                Ok((ty.clone(), Some(self.coerce(v, &ty)?)))
            }
            Some(list @ Init::List { pos, .. }) => {
                let mut leaves = VecDeque::new();
                self.flatten_init(list, &mut leaves)?;
                let total: Option<u32> = leaves
                    .iter()
                    .map(|l| l.ty.shape().map(Shape::components))
                    .sum();
                let unsized_len = |elem: &Ty| -> R<u32> {
                    let per = elem.shape().map(Shape::components);
                    match (total, per) {
                        (Some(t), Some(p)) if t > 0 && t % p == 0 => Ok(t / p),
                        _ => err(
                            *pos,
                            "cannot size the unsized array from this initializer list",
                        ),
                    }
                };
                let ty = self.array_ty(d.pos, base, &d.array, Some(&unsized_len))?;
                let v = self.build_init(*pos, &ty, &mut leaves)?;
                if let Some(extra) = leaves.front() {
                    return err(extra.pos, format!("too many initializers for '{ty}'"));
                }
                Ok((ty, Some(v)))
            }
        }
    }

    fn flatten_init(&mut self, init: &Init, out: &mut VecDeque<TExpr>) -> R<()> {
        match init {
            Init::Expr(e) => out.push_back(self.expr(e)?),
            Init::List { items, .. } => {
                for i in items {
                    self.flatten_init(i, out)?;
                }
            }
        }
        Ok(())
    }

    /// Build a value of `ty` from the flattened initializer components.
    fn build_init(&mut self, pos: Pos, ty: &Ty, leaves: &mut VecDeque<TExpr>) -> R<TExpr> {
        let Some(front) = leaves.front() else {
            return err(pos, format!("not enough initializers for '{ty}'"));
        };
        if front.ty == *ty {
            return Ok(leaves.pop_front().unwrap());
        }
        let ctor = |args| TExpr::new(pos, ty.clone(), TExprKind::Ctor { args });
        match ty {
            Ty::Array(t, n) => {
                let mut args = Vec::new();
                for _ in 0..*n {
                    args.push(self.build_init(pos, t, leaves)?);
                }
                Ok(ctor(args))
            }
            Ty::Struct(name) => {
                let fields = self.structs[name].clone();
                let mut args = Vec::new();
                for (_, ft) in &fields {
                    args.push(self.build_init(pos, ft, leaves)?);
                }
                Ok(ctor(args))
            }
            t if t.is_numeric() => {
                let (shape, te) = (t.shape().unwrap(), t.elem().unwrap());
                let need = shape.components();
                let mut got = 0;
                let mut args = Vec::new();
                while got < need {
                    let Some(l) = leaves.pop_front() else {
                        return err(pos, format!("not enough initializers for '{ty}'"));
                    };
                    let k = match l.ty.shape() {
                        Some(Shape::Scalar) => 1,
                        Some(Shape::Vec(k)) => k as u32,
                        Some(s) if args.is_empty() && s == shape => need,
                        _ => {
                            return err(
                                l.pos,
                                format!("cannot use '{}' in an initializer list for '{ty}'", l.ty),
                            )
                        }
                    };
                    if got + k > need {
                        return err(
                            l.pos,
                            format!("initializer '{}' straddles an element of '{ty}'", l.ty),
                        );
                    }
                    got += k;
                    args.push(cast_elem(l, te));
                }
                if args.len() == 1 && args[0].ty.shape() == Some(shape) {
                    return Ok(args.pop().unwrap());
                }
                Ok(ctor(args))
            }
            t => err(pos, format!("cannot initialize '{t}' with a list")),
        }
    }
}

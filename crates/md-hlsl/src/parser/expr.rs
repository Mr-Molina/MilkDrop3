//! Expressions: C/HLSL precedence by precedence climbing.
//!
//! Lowest to highest: assignment (right-assoc), `?:` (right-assoc), `||`,
//! `&&`, `|`, `^`, `&`, `== !=`, `< > <= >=`, `<< >>`, `+ -`, `* / %`,
//! prefix unary and casts, postfix (`[]`, `.`, `++`, `--`). There is no
//! general comma operator (see `expr_list` for `for` clauses).
//!
//! Internally every parse returns the tree height alongside the node so
//! iteratively built chains are bounded by `MAX_EXPR_HEIGHT`.

use super::{builtin_type, Parser, MAX_EXPR_HEIGHT, R};
use crate::ast::*;
use crate::TokKind;

/// An expression and its tree height. Boxed so the `Result`s moved through
/// each recursive frame stay small (debug frames hold several copies).
type H = (Box<Expr>, u32);

fn binop(op: &str) -> Option<(BinOp, u8)> {
    Some(match op {
        "||" => (BinOp::Or, 1),
        "&&" => (BinOp::And, 2),
        "|" => (BinOp::BitOr, 3),
        "^" => (BinOp::BitXor, 4),
        "&" => (BinOp::BitAnd, 5),
        "==" => (BinOp::Eq, 6),
        "!=" => (BinOp::Ne, 6),
        "<" => (BinOp::Lt, 7),
        ">" => (BinOp::Gt, 7),
        "<=" => (BinOp::Le, 7),
        ">=" => (BinOp::Ge, 7),
        "<<" => (BinOp::Shl, 8),
        ">>" => (BinOp::Shr, 8),
        "+" => (BinOp::Add, 9),
        "-" => (BinOp::Sub, 9),
        "*" => (BinOp::Mul, 10),
        "/" => (BinOp::Div, 10),
        "%" => (BinOp::Rem, 10),
        _ => return None,
    })
}

fn assignop(op: &str) -> Option<AssignOp> {
    Some(match op {
        "=" => None,
        "+=" => Some(BinOp::Add),
        "-=" => Some(BinOp::Sub),
        "*=" => Some(BinOp::Mul),
        "/=" => Some(BinOp::Div),
        "%=" => Some(BinOp::Rem),
        "&=" => Some(BinOp::BitAnd),
        "|=" => Some(BinOp::BitOr),
        "^=" => Some(BinOp::BitXor),
        "<<=" => Some(BinOp::Shl),
        ">>=" => Some(BinOp::Shr),
        _ => return None,
    })
}

impl Parser<'_> {
    /// One full expression (assignment level, no comma operator).
    pub(super) fn expr(&mut self) -> R<Expr> {
        Ok(*self.assign()?.0)
    }

    /// Build a node of height `h`, rejecting over-tall trees.
    fn node(&self, e: Expr, h: u32) -> R<H> {
        if h > MAX_EXPR_HEIGHT {
            return self.err(format!(
                "expression nesting too deep (max height {MAX_EXPR_HEIGHT})"
            ));
        }
        Ok((Box::new(e), h))
    }

    fn cur_op(&self) -> Option<&'static str> {
        match self.peek()?.kind {
            TokKind::Op(o) => Some(o),
            _ => None,
        }
    }

    // Stack discipline: untrusted input nests up to MAX_DEPTH levels, and
    // debug frames reserve room for every local of a function. So each
    // function on the recursive descent path (assign → cond → binary → unary
    // → postfix → primary → assign) only calls down and hands the result to
    // a `*_tail` function that builds nodes after the call returns.

    fn assign(&mut self) -> R<H> {
        self.enter()?;
        let r = self.cond().and_then(|lhs| self.assign_tail(lhs));
        self.leave();
        r
    }

    fn assign_tail(&mut self, lhs: H) -> R<H> {
        let Some(op) = self.cur_op().and_then(assignop) else {
            return Ok(lhs);
        };
        let pos = self.pos();
        self.i += 1;
        let rhs = self.assign()?;
        let h = lhs.1.max(rhs.1) + 1;
        let e = Expr::Assign {
            pos,
            op,
            lhs: lhs.0,
            rhs: rhs.0,
        };
        self.node(e, h)
    }

    fn cond(&mut self) -> R<H> {
        let c = self.binary(1)?;
        if self.at_op("?") {
            self.cond_tail(c)
        } else {
            Ok(c)
        }
    }

    fn cond_tail(&mut self, c: H) -> R<H> {
        let pos = self.pos();
        self.i += 1;
        let t = self.assign()?;
        self.expect_op(":")?;
        self.enter()?;
        let f = self.cond();
        self.leave();
        let f = f?;
        let h = c.1.max(t.1).max(f.1) + 1;
        let e = Expr::Ternary {
            pos,
            cond: c.0,
            then: t.0,
            els: f.0,
        };
        self.node(e, h)
    }

    fn binary(&mut self, min_prec: u8) -> R<H> {
        let lhs = self.unary()?;
        self.binary_tail(lhs, min_prec)
    }

    fn binary_tail(&mut self, mut lhs: H, min_prec: u8) -> R<H> {
        while let Some((op, prec)) = self.cur_op().and_then(binop) {
            if prec < min_prec {
                break;
            }
            let pos = self.pos();
            self.i += 1;
            let rhs = self.binary(prec + 1)?;
            let h = lhs.1.max(rhs.1) + 1;
            let e = Expr::Binary {
                pos,
                op,
                lhs: lhs.0,
                rhs: rhs.0,
            };
            lhs = self.node(e, h)?;
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> R<H> {
        match self.cur_op() {
            Some("-" | "+" | "!" | "~" | "++" | "--") => self.prefixed(),
            Some("(") if self.cast_ahead().is_some() => self.prefixed(),
            _ => self.postfix(),
        }
    }

    /// A prefix operator or cast followed by its operand.
    fn prefixed(&mut self) -> R<H> {
        let pos = self.pos();
        let op = match self.cur_op() {
            Some("-") => UnOp::Neg,
            Some("+") => UnOp::Plus,
            Some("!") => UnOp::Not,
            Some("~") => UnOp::BitNot,
            Some("++") => UnOp::PreInc,
            Some("--") => UnOp::PreDec,
            _ => {
                let Some(ty) = self.cast_ahead() else {
                    return self.err(format!("expected an expression, found {}", self.describe()));
                };
                self.i += 3;
                let (inner, h) = self.nested_unary()?;
                let e = Expr::Cast {
                    pos,
                    ty,
                    expr: inner,
                };
                return self.node(e, h + 1);
            }
        };
        self.i += 1;
        let (operand, h) = self.nested_unary()?;
        let e = Expr::Unary { pos, op, operand };
        self.node(e, h + 1)
    }

    fn nested_unary(&mut self) -> R<H> {
        self.enter()?;
        let r = self.unary();
        self.leave();
        r
    }

    /// The type of a `( type )` cast at the cursor, without consuming it. A
    /// parenthesized constructor such as `(float2(a, b))` is not a cast.
    fn cast_ahead(&self) -> Option<TypeName> {
        if !self.at_op("(") || !self.peek_at(2).is_some_and(|t| t.is_op(")")) {
            return None;
        }
        let t = self.peek_at(1)?;
        let kind = self.type_of(t.ident()?)?;
        let pos = Pos {
            line: t.line,
            col: t.col,
        };
        Some(TypeName { pos, kind })
    }

    fn postfix(&mut self) -> R<H> {
        let p = self.primary()?;
        self.postfix_tail(p)
    }

    fn postfix_tail(&mut self, (mut e, mut h): H) -> R<H> {
        loop {
            let pos = self.pos();
            let next = match self.cur_op() {
                Some("[") => {
                    self.i += 1;
                    let (index, ih) = self.assign()?;
                    self.expect_op("]")?;
                    h = h.max(ih);
                    Expr::Index {
                        pos,
                        base: e,
                        index,
                    }
                }
                Some(".") => {
                    self.i += 1;
                    let Some(field) = self.peek().and_then(|t| t.ident()) else {
                        return self
                            .err(format!("expected a member name, found {}", self.describe()));
                    };
                    self.i += 1;
                    Expr::Member {
                        pos,
                        base: e,
                        field: field.to_string(),
                    }
                }
                Some(o @ ("++" | "--")) => {
                    self.i += 1;
                    let op = if o == "++" {
                        UnOp::PostInc
                    } else {
                        UnOp::PostDec
                    };
                    Expr::Unary {
                        pos,
                        op,
                        operand: e,
                    }
                }
                _ => return Ok((e, h)),
            };
            (e, h) = self.node(next, h + 1)?;
        }
    }

    /// `( args )` after a callee or constructor type.
    fn args(&mut self) -> R<(Vec<Expr>, u32)> {
        self.expect_op("(")?;
        let mut args = Vec::new();
        let mut h = 0;
        if self.eat_op(")") {
            return Ok((args, h));
        }
        loop {
            let (a, ah) = self.assign()?;
            args.push(*a);
            h = h.max(ah);
            if self.eat_op(")") {
                return Ok((args, h));
            }
            self.expect_op(",")?;
        }
    }

    fn primary(&mut self) -> R<H> {
        let pos = self.pos();
        let Some(tok) = self.peek() else {
            return self.err("expected an expression, found end of input");
        };
        let lit = match &tok.kind {
            TokKind::Int(v) => Some(Lit::Int(*v)),
            TokKind::Float(v) => Some(Lit::Float(*v)),
            TokKind::Ident(s) if &**s == "true" => Some(Lit::Bool(true)),
            TokKind::Ident(s) if &**s == "false" => Some(Lit::Bool(false)),
            _ => None,
        };
        if let Some(lit) = lit {
            self.i += 1;
            return Ok((Box::new(Expr::Lit { pos, lit }), 1));
        }
        if self.at_op("(") {
            self.i += 1;
            let inner = self.assign()?;
            self.expect_op(")")?;
            return Ok(inner);
        }
        let Some(name) = tok.ident() else {
            return self.err(format!("expected an expression, found {}", self.describe()));
        };
        let call = self.peek_at(1).is_some_and(|t| t.is_op("("));
        if let Some(kind) = builtin_type(name) {
            if !call {
                return self.err(format!("unexpected type name '{name}'"));
            }
            self.i += 1;
            let (args, h) = self.args()?;
            let ty = TypeName { pos, kind };
            return self.node(Expr::Ctor { pos, ty, args }, h + 1);
        }
        if self.is_reserved(name) {
            return self.err(format!("expected an expression, found '{name}'"));
        }
        self.i += 1;
        let name = name.to_string();
        if call {
            let (args, h) = self.args()?;
            return self.node(Expr::Call { pos, name, args }, h + 1);
        }
        Ok((Box::new(Expr::Ident { pos, name }), 1))
    }
}

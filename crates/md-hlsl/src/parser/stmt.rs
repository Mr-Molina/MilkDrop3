//! Statements and blocks.

use super::{Parser, R};
use crate::ast::*;

impl Parser<'_> {
    /// `{ stmts }`. With `shader_body`, the last statement directly in the
    /// block may omit its `;` (fxc tolerated it); nowhere else.
    pub(super) fn block(&mut self, shader_body: bool) -> R<Block> {
        let pos = self.pos();
        self.expect_op("{")?;
        let mut stmts = Vec::new();
        while !self.eat_op("}") {
            if self.peek().is_none() {
                return self.err("expected '}', found end of input");
            }
            stmts.push(self.stmt(shader_body)?);
        }
        Ok(Block { pos, stmts })
    }

    /// One statement. `semi_optional`: a missing `;` right before `}` is
    /// accepted (only for statements directly in a shader_body block).
    fn stmt(&mut self, semi_optional: bool) -> R<Stmt> {
        self.enter()?;
        let r = self.stmt_inner(semi_optional);
        self.leave();
        r
    }

    /// The terminating `;` of a simple statement.
    fn end(&mut self, semi_optional: bool) -> R<()> {
        if self.eat_op(";") || (semi_optional && self.at_op("}")) {
            Ok(())
        } else {
            self.err(format!("expected ';', found {}", self.describe()))
        }
    }

    /// Dispatch on the first token. Kept free of node temporaries (each kind
    /// is its own function) because nested statements recurse through it.
    fn stmt_inner(&mut self, semi: bool) -> R<Stmt> {
        // Attributes: `[unroll]`, `[loop]`, `[unroll(4)]`, ... (dropped).
        while self.at_op("[") {
            self.skip_balanced("[", "]")?;
        }
        let pos = self.pos();
        if self.at_op("{") {
            return self.block(false).map(Stmt::Block);
        }
        if self.eat_op(";") {
            return Ok(Stmt::Block(Block {
                pos,
                stmts: Vec::new(),
            }));
        }
        match self.peek().and_then(|t| t.ident()) {
            Some("if") => self.if_stmt(pos),
            Some("for") => self.for_stmt(pos),
            Some("while") => self.while_stmt(pos),
            Some("do") => self.do_stmt(pos, semi),
            Some("return") => self.return_stmt(pos, semi),
            Some("break" | "continue" | "discard") => self.jump_stmt(pos, semi),
            _ if self.at_decl_start() => self.decl_stmt(semi),
            _ => self.expr_stmt(pos, semi),
        }
    }

    fn if_stmt(&mut self, pos: Pos) -> R<Stmt> {
        self.i += 1;
        let cond = self.paren_cond()?;
        let then = Box::new(self.stmt(false)?);
        let els = if self.at_ident("else") {
            self.i += 1;
            Some(Box::new(self.stmt(false)?))
        } else {
            None
        };
        Ok(Stmt::If {
            pos,
            cond,
            then,
            els,
        })
    }

    fn while_stmt(&mut self, pos: Pos) -> R<Stmt> {
        self.i += 1;
        let cond = self.paren_cond()?;
        let body = Box::new(self.stmt(false)?);
        Ok(Stmt::While { pos, cond, body })
    }

    fn do_stmt(&mut self, pos: Pos, semi: bool) -> R<Stmt> {
        self.i += 1;
        let body = Box::new(self.stmt(false)?);
        if !self.at_ident("while") {
            return self.err(format!("expected 'while', found {}", self.describe()));
        }
        self.i += 1;
        let cond = self.paren_cond()?;
        self.end(semi)?;
        Ok(Stmt::DoWhile { pos, body, cond })
    }

    fn return_stmt(&mut self, pos: Pos, semi: bool) -> R<Stmt> {
        self.i += 1;
        let value = if self.at_op(";") || (semi && self.at_op("}")) {
            None
        } else {
            Some(self.expr()?)
        };
        self.end(semi)?;
        Ok(Stmt::Return { pos, value })
    }

    /// `break;`, `continue;`, `discard;`.
    fn jump_stmt(&mut self, pos: Pos, semi: bool) -> R<Stmt> {
        let kw = self.peek().and_then(|t| t.ident());
        let s = match kw {
            Some("break") => Stmt::Break { pos },
            Some("continue") => Stmt::Continue { pos },
            _ => Stmt::Discard { pos },
        };
        self.i += 1;
        self.end(semi)?;
        Ok(s)
    }

    fn decl_stmt(&mut self, semi: bool) -> R<Stmt> {
        let d = self.var_decl()?;
        self.end(semi)?;
        Ok(Stmt::VarDecl(d))
    }

    fn expr_stmt(&mut self, pos: Pos, semi: bool) -> R<Stmt> {
        let expr = self.expr()?;
        self.end(semi)?;
        Ok(Stmt::Expr { pos, expr })
    }

    /// `( expr )` of `if` / `while`.
    fn paren_cond(&mut self) -> R<Expr> {
        self.expect_op("(")?;
        let e = self.expr()?;
        self.expect_op(")")?;
        Ok(e)
    }

    /// Comma-separated expressions (only in `for` clauses).
    fn expr_list(&mut self) -> R<Vec<Expr>> {
        let mut v = vec![self.expr()?];
        while self.eat_op(",") {
            v.push(self.expr()?);
        }
        Ok(v)
    }

    fn for_stmt(&mut self, pos: Pos) -> R<Stmt> {
        self.i += 1;
        self.expect_op("(")?;
        let init = if self.at_op(";") {
            ForInit::None
        } else if self.at_decl_start() {
            ForInit::Decl(self.var_decl()?)
        } else {
            ForInit::Exprs(self.expr_list()?)
        };
        self.expect_op(";")?;
        let cond = if self.at_op(";") {
            None
        } else {
            Some(self.expr()?)
        };
        self.expect_op(";")?;
        let step = if self.at_op(")") {
            Vec::new()
        } else {
            self.expr_list()?
        };
        self.expect_op(")")?;
        let body = Box::new(self.stmt(false)?);
        Ok(Stmt::For {
            pos,
            init,
            cond,
            step,
            body,
        })
    }
}

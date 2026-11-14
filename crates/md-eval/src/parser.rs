//! Parser: tokens -> [`Expr`].
//!
//! Two stages, mirroring ns-eel2 (`nseel-compiler.c`, `nseel-caltab.c`):
//!
//! 1. [`crate::preproc`] rewrites every operator the bison grammar does not
//!    know (`= op= == != < > <= >= && || ! ? % ^ [ ]`) into a call, and `;`
//!    inside parentheses into a sequence token. That rewrite decides how
//!    those operators bind (see its docs and the parser tests).
//! 2. The result is split into statements at the remaining `;` (empty
//!    statements are skipped) and each statement is parsed with the grammar
//!    of `nseel-caltab.c`: a Pratt parser over `|` < `&` < `+ -` <
//!    `* / seq` (all left-associative) < unary `- +` < primary, where a
//!    primary is a number, a variable, `( expr )`, or a built-in call with
//!    exactly its arity. Operator-like calls are then lowered to the
//!    canonical nodes listed in [`crate::ast`].

use crate::ast::{AssignOp, BinOp, Expr, UnOp};
use crate::error::CompileError;
use crate::preproc::{preprocess, Spanned, TOO_DEEP};
use crate::token::Tok;

/// Bound on parenthesis / unary nesting in one statement (bison's own stack
/// is 5000 states; this keeps the recursive descent off the guard page).
pub(crate) const MAX_NESTING: usize = 400;

/// Parse a token stream (as produced by [`crate::Lexer::tokenize`]; the
/// trailing `Eof` is optional). The value of the program is the value of
/// its last statement; an empty program is `Seq([])`.
pub fn parse(tokens: &[(Tok, usize)]) -> Result<Expr, CompileError> {
    let (body, end) = match tokens.split_last() {
        Some(((Tok::Eof, off), rest)) => (rest, *off),
        _ => (tokens, tokens.last().map_or(0, |t| t.1 + 1)),
    };
    let toks = preprocess(body)?;
    let mut stmts = Vec::new();
    let mut start = 0;
    while start < toks.len() {
        let stop = toks[start..]
            .iter()
            .position(|(t, _)| *t == Tok::Semi)
            .map_or(toks.len(), |k| start + k);
        if stop > start {
            let mut p = Parser {
                toks: &toks[start..stop],
                pos: 0,
                end: toks.get(stop).map_or(end, |t| t.1),
                nesting: 0,
            };
            let e = p.expr(0)?;
            if p.pos < p.toks.len() {
                return Err(p.err_here());
            }
            stmts.push(e);
        }
        start = stop + 1;
    }
    Ok(match stmts.len() {
        1 => stmts.pop().unwrap_or(Expr::Seq(Vec::new())),
        _ => Expr::Seq(stmts),
    })
}

fn unexpected(t: &Tok, off: usize) -> CompileError {
    let what = match t {
        Tok::Num(v) => format!("number {v}"),
        Tok::Ident(n) => format!("'{n}'"),
        Tok::Op(o) => format!("'{o}'"),
        Tok::LParen => "'('".into(),
        Tok::RParen => "')'".into(),
        Tok::LBracket => "'['".into(),
        Tok::RBracket => "']'".into(),
        Tok::Comma => "','".into(),
        Tok::Semi => "';'".into(),
        Tok::Eof => "end of input".into(),
    };
    CompileError::new(off, format!("syntax error: unexpected {what}"))
}

/// Resolve the aliases `nseel_lookup` applies (nseel-eval.c), then look up
/// the arity in ns-eel2's `fnTable1`. Names are already lowercase.
fn builtin(name: &str) -> Option<(&'static str, usize)> {
    let name = match name {
        "if" => "_if",
        "bnot" => "_not",
        "assign" => "_set",
        "equal" => "_equal",
        "below" => "_below",
        "above" => "_above",
        "megabuf" => "_mem",
        "gmegabuf" => "_gmem",
        "int" => "floor",
        n => n,
    };
    const TABLE: &[(&str, usize)] = &[
        ("_if", 3),
        ("_and", 2),
        ("_or", 2),
        ("loop", 2),
        ("while", 1),
        ("_not", 1),
        ("_equal", 2),
        ("_noteq", 2),
        ("_below", 2),
        ("_above", 2),
        ("_beleq", 2),
        ("_aboeq", 2),
        ("_set", 2),
        ("_mod", 2),
        ("_mulop", 2),
        ("_divop", 2),
        ("_orop", 2),
        ("_andop", 2),
        ("_addop", 2),
        ("_subop", 2),
        ("_modop", 2),
        ("sin", 1),
        ("cos", 1),
        ("tan", 1),
        ("asin", 1),
        ("acos", 1),
        ("atan", 1),
        ("atan2", 2),
        ("sqr", 1),
        ("sqrt", 1),
        ("pow", 2),
        ("_powop", 2),
        ("exp", 1),
        ("log", 1),
        ("log10", 1),
        ("abs", 1),
        ("min", 2),
        ("max", 2),
        ("sign", 1),
        ("rand", 1),
        ("floor", 1),
        ("ceil", 1),
        ("invsqrt", 1),
        ("sigmoid", 2),
        ("band", 2),
        ("bor", 2),
        ("exec2", 2),
        ("exec3", 3),
        ("_mem", 1),
        ("_gmem", 1),
        ("freembuf", 1),
        ("memcpy", 3),
        ("memset", 3),
    ];
    TABLE.iter().copied().find(|(n, _)| *n == name)
}

/// Lower a built-in call to its canonical node.
fn lower(name: &'static str, mut args: Vec<Expr>, off: usize) -> Result<Expr, CompileError> {
    let bin = |op: BinOp, mut a: Vec<Expr>| -> Expr {
        let r = a.pop().unwrap_or(Expr::Num(0.0));
        let l = a.pop().unwrap_or(Expr::Num(0.0));
        Expr::Binary(op, Box::new(l), Box::new(r))
    };
    let assign = |op: AssignOp, mut a: Vec<Expr>| -> Result<Expr, CompileError> {
        let r = a.pop().unwrap_or(Expr::Num(0.0));
        let l = a.pop().unwrap_or(Expr::Num(0.0));
        let ok = match &l {
            Expr::Var(_) => true,
            Expr::Call { name, .. } => name == "_mem" || name == "_gmem",
            _ => false,
        };
        if !ok {
            return Err(CompileError::new(
                off,
                "can only assign to a variable or megabuf/gmegabuf element",
            ));
        }
        Ok(Expr::Assign(op, Box::new(l), Box::new(r)))
    };
    Ok(match name {
        "_if" => {
            let b = args.pop().unwrap_or(Expr::Num(0.0));
            let a = args.pop().unwrap_or(Expr::Num(0.0));
            let c = args.pop().unwrap_or(Expr::Num(0.0));
            Expr::Ternary(Box::new(c), Box::new(a), Box::new(b))
        }
        "_not" => Expr::Unary(UnOp::Not, Box::new(args.pop().unwrap_or(Expr::Num(0.0)))),
        "_equal" => bin(BinOp::Eq, args),
        "_noteq" => bin(BinOp::Ne, args),
        "_below" => bin(BinOp::Lt, args),
        "_above" => bin(BinOp::Gt, args),
        "_beleq" => bin(BinOp::Le, args),
        "_aboeq" => bin(BinOp::Ge, args),
        "_and" => bin(BinOp::And, args),
        "_or" => bin(BinOp::Or, args),
        "_mod" => bin(BinOp::Mod, args),
        "_set" => assign(AssignOp::Set, args)?,
        "_addop" => assign(AssignOp::Add, args)?,
        "_subop" => assign(AssignOp::Sub, args)?,
        "_mulop" => assign(AssignOp::Mul, args)?,
        "_divop" => assign(AssignOp::Div, args)?,
        "_modop" => assign(AssignOp::Mod, args)?,
        "_andop" => assign(AssignOp::BitAnd, args)?,
        "_orop" => assign(AssignOp::BitOr, args)?,
        "_powop" => assign(AssignOp::Pow, args)?,
        _ => Expr::Call {
            name: name.to_string(),
            args,
        },
    })
}

/// Binary operators of the bison grammar: (precedence, operator).
fn infix(t: &Tok) -> Option<(u8, Option<BinOp>)> {
    match t {
        Tok::Op("|") => Some((1, Some(BinOp::BitOr))),
        Tok::Op("&") => Some((2, Some(BinOp::BitAnd))),
        Tok::Op("+") => Some((3, Some(BinOp::Add))),
        Tok::Op("-") => Some((3, Some(BinOp::Sub))),
        Tok::Op("*") => Some((4, Some(BinOp::Mul))),
        Tok::Op("/") => Some((4, Some(BinOp::Div))),
        // After preprocessing `%` is always the sequence token (exec2).
        Tok::Op("%") => Some((4, None)),
        _ => None,
    }
}

struct Parser<'a> {
    toks: &'a [Spanned],
    pos: usize,
    /// Offset reported for "unexpected end".
    end: usize,
    nesting: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.0)
    }

    fn here(&self) -> usize {
        self.toks.get(self.pos).map_or(self.end, |t| t.1)
    }

    fn err_here(&self) -> CompileError {
        match self.toks.get(self.pos) {
            Some((t, off)) => unexpected(t, *off),
            None => CompileError::new(self.end, "syntax error: unexpected end of input"),
        }
    }

    fn enter(&mut self) -> Result<(), CompileError> {
        self.nesting += 1;
        if self.nesting > MAX_NESTING {
            return Err(CompileError::new(
                self.here(),
                "expression nested too deeply",
            ));
        }
        Ok(())
    }

    /// Precedence climbing over the left-associative binary operators.
    fn expr(&mut self, min_prec: u8) -> Result<Expr, CompileError> {
        let mut lhs = self.unary()?;
        while let Some((prec, op)) = self.peek().and_then(infix) {
            if prec < min_prec {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(prec + 1)?;
            lhs = match op {
                Some(op) => Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                None => match lhs {
                    Expr::Seq(mut items) => {
                        items.push(rhs);
                        Expr::Seq(items)
                    }
                    other => Expr::Seq(vec![other, rhs]),
                },
            };
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, CompileError> {
        self.enter()?;
        let e = match self.peek() {
            Some(Tok::Op("-")) => {
                self.pos += 1;
                Expr::Unary(UnOp::Neg, Box::new(self.unary()?))
            }
            Some(Tok::Op("+")) => {
                // FN_UPLUS is the identity
                self.pos += 1;
                self.unary()?
            }
            _ => self.primary()?,
        };
        self.nesting -= 1;
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, CompileError> {
        let Some((tok, off)) = self.toks.get(self.pos) else {
            return Err(self.err_here());
        };
        let off = *off;
        match tok {
            Tok::Num(v) => {
                self.pos += 1;
                Ok(Expr::Num(*v))
            }
            Tok::LParen => {
                self.pos += 1;
                let e = self.expr(0)?;
                self.expect_rparen()?;
                Ok(e)
            }
            Tok::Ident(name) if name == TOO_DEEP => {
                Err(CompileError::new(off, "expression nested too deeply"))
            }
            Tok::Ident(name) => {
                self.pos += 1;
                let is_call = self.peek() == Some(&Tok::LParen);
                match (builtin(name), is_call) {
                    (Some((canon, arity)), true) => self.call(canon, arity, name, off),
                    (Some(_), false) => Err(CompileError::new(
                        off,
                        format!("function '{name}' must be called with arguments"),
                    )),
                    (None, true) => {
                        Err(CompileError::new(off, format!("unknown function '{name}'")))
                    }
                    (None, false) => Ok(Expr::Var(name.clone())),
                }
            }
            _ => Err(self.err_here()),
        }
    }

    fn call(
        &mut self,
        canon: &'static str,
        arity: usize,
        written: &str,
        off: usize,
    ) -> Result<Expr, CompileError> {
        self.pos += 1; // '('
        let mut args = Vec::with_capacity(arity);
        loop {
            args.push(self.expr(0)?);
            match self.peek() {
                Some(Tok::Comma) => self.pos += 1,
                _ => break,
            }
        }
        self.expect_rparen()?;
        if args.len() != arity {
            return Err(CompileError::new(
                off,
                format!(
                    "function '{written}' takes {arity} argument{}, got {}",
                    if arity == 1 { "" } else { "s" },
                    args.len()
                ),
            ));
        }
        lower(canon, args, off)
    }

    fn expect_rparen(&mut self) -> Result<(), CompileError> {
        if self.peek() == Some(&Tok::RParen) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err_here())
        }
    }
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;

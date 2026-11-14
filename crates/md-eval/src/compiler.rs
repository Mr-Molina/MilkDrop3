//! AST -> bytecode.
//!
//! Lazy forms compile to jumps so untaken code never runs, as ns-eel2's
//! `nseel_asm_if`, `nseel_asm_band` and `nseel_asm_bor` call their branch
//! bodies only when taken:
//!
//! * `c ? a : b` -> `[c] JumpIfFalse(E) [a] Jump(X) E: [b] X:`
//! * `a && b`    -> `[a] And(X) [b] Bool X:` (likewise `||` with `Or`)
//! * `loop(n, body)` -> `[n] Loop{..} [body]`, `while(body)` -> `While{..} [body]`
//!
//! Evaluation order matches the generated x86: arguments left to right,
//! and for an assignment the index (if any) first, then the right side.

use crate::ast::{AssignOp, BinOp, Expr, UnOp};
use crate::bytecode::{Fn1, Fn2, Fn3, Op, Program};
use crate::error::CompileError;
use crate::lexer::Lexer;
use crate::parser::parse;
use crate::vm::Vm;

/// Compile a parsed expression. Variable names are resolved (and created)
/// in `vm`, so the program must run on that `vm`.
pub fn compile_ast(vm: &mut Vm, ast: &Expr) -> Result<Program, CompileError> {
    let mut c = Compiler {
        vm,
        ops: Vec::new(),
    };
    c.expr(ast)?;
    Ok(Program { ops: c.ops })
}

impl Program {
    /// Lex, parse and compile `src` against `vm`.
    pub fn compile(vm: &mut Vm, src: &str) -> Result<Program, CompileError> {
        let tokens = Lexer::tokenize(src)?;
        let ast = parse(&tokens)?;
        compile_ast(vm, &ast)
    }
}

struct Compiler<'a> {
    vm: &'a mut Vm,
    ops: Vec<Op>,
}

fn err(message: String) -> CompileError {
    // The AST carries no offsets; the parser has already rejected anything
    // malformed, so reaching this means a hand-built tree.
    CompileError::new(0, message)
}

/// `Some(true)` for `gmegabuf`, `Some(false)` for `megabuf`.
fn mem_kind(name: &str) -> Option<bool> {
    match name {
        "_mem" => Some(false),
        "_gmem" => Some(true),
        _ => None,
    }
}

impl Compiler<'_> {
    fn here(&self) -> u32 {
        self.ops.len() as u32
    }

    fn emit(&mut self, op: Op) -> usize {
        self.ops.push(op);
        self.ops.len() - 1
    }

    /// Point the jump at `at` to the current end.
    fn patch(&mut self, at: usize) {
        let to = self.here();
        match &mut self.ops[at] {
            Op::Jump(t) | Op::JumpIfFalse(t) | Op::And(t) | Op::Or(t) => *t = to,
            Op::Loop { body_end, .. } | Op::While { body_end, .. } => *body_end = to,
            _ => {}
        }
    }

    fn expr(&mut self, e: &Expr) -> Result<(), CompileError> {
        match e {
            Expr::Num(v) => {
                self.emit(Op::PushConst(*v));
            }
            Expr::Var(name) => {
                let v = self.vm.var(name);
                self.emit(Op::Load(v));
            }
            Expr::Unary(op, a) => {
                self.expr(a)?;
                self.emit(match op {
                    UnOp::Neg => Op::Neg,
                    UnOp::Not => Op::Not,
                });
            }
            Expr::Binary(op, a, b) => match binary(*op) {
                o @ (Op::And(_) | Op::Or(_)) => self.logic(a, b, o)?,
                o => {
                    self.expr(a)?;
                    self.expr(b)?;
                    self.emit(o);
                }
            },
            Expr::Assign(op, target, value) => self.assign(*op, target, value)?,
            Expr::Ternary(c, a, b) => {
                self.expr(c)?;
                let to_else = self.emit(Op::JumpIfFalse(0));
                self.expr(a)?;
                let to_end = self.emit(Op::Jump(0));
                self.patch(to_else);
                self.expr(b)?;
                self.patch(to_end);
            }
            Expr::Seq(items) => self.seq(items)?,
            Expr::Call { name, args } => self.call(name, args)?,
        }
        Ok(())
    }

    fn seq(&mut self, items: &[Expr]) -> Result<(), CompileError> {
        let Some((last, init)) = items.split_last() else {
            self.emit(Op::PushConst(0.0));
            return Ok(());
        };
        for item in init {
            self.expr(item)?;
            self.emit(Op::Pop);
        }
        self.expr(last)
    }

    fn logic(&mut self, a: &Expr, b: &Expr, op: Op) -> Result<(), CompileError> {
        self.expr(a)?;
        let at = self.emit(op);
        self.expr(b)?;
        self.emit(Op::Bool);
        self.patch(at);
        Ok(())
    }

    fn assign(&mut self, op: AssignOp, target: &Expr, value: &Expr) -> Result<(), CompileError> {
        match target {
            Expr::Var(name) => {
                let v = self.vm.var(name);
                self.expr(value)?;
                self.emit(match op {
                    AssignOp::Set => Op::Store(v),
                    op => Op::Update(v, op),
                });
            }
            Expr::Call { name, args } if args.len() == 1 => {
                let Some(global) = mem_kind(name) else {
                    return Err(err(format!("cannot assign to '{name}(..)'")));
                };
                self.expr(&args[0])?;
                self.expr(value)?;
                self.emit(match op {
                    AssignOp::Set => Op::StoreMem(global),
                    op => Op::UpdateMem(global, op),
                });
            }
            _ => {
                return Err(err(
                    "can only assign to a variable or megabuf/gmegabuf element".into(),
                ))
            }
        }
        Ok(())
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> Result<(), CompileError> {
        match (name, args) {
            ("loop", [n, body]) => {
                self.expr(n)?;
                let at = self.emit(Op::Loop {
                    body_start: self.here() + 1,
                    body_end: 0,
                });
                self.expr(body)?;
                self.patch(at);
            }
            ("while", [body]) => {
                let at = self.emit(Op::While {
                    body_start: self.here() + 1,
                    body_end: 0,
                });
                self.expr(body)?;
                self.patch(at);
            }
            (_, [i]) if mem_kind(name).is_some() => {
                self.expr(i)?;
                self.emit(Op::LoadMem(name == "_gmem"));
            }
            _ => {
                let op = match args.len() {
                    1 => Fn1::from_name(name).map(Op::Call1),
                    2 => Fn2::from_name(name).map(Op::Call2),
                    3 => Fn3::from_name(name).map(Op::Call3),
                    _ => None,
                };
                let Some(op) = op else {
                    return Err(err(format!(
                        "unknown function '{name}' with {} argument(s)",
                        args.len()
                    )));
                };
                for a in args {
                    self.expr(a)?;
                }
                self.emit(op);
            }
        }
        Ok(())
    }
}

fn binary(op: BinOp) -> Op {
    match op {
        BinOp::Add => Op::Add,
        BinOp::Sub => Op::Sub,
        BinOp::Mul => Op::Mul,
        BinOp::Div => Op::Div,
        BinOp::Mod => Op::Mod,
        BinOp::BitAnd => Op::BitAnd,
        BinOp::BitOr => Op::BitOr,
        BinOp::Eq => Op::Eq,
        BinOp::Ne => Op::Ne,
        BinOp::Lt => Op::Lt,
        BinOp::Gt => Op::Gt,
        BinOp::Le => Op::Le,
        BinOp::Ge => Op::Ge,
        // short-circuit: compiled with jumps by `logic`
        BinOp::And => Op::And(0),
        BinOp::Or => Op::Or(0),
    }
}

#[cfg(test)]
#[path = "compiler_tests.rs"]
mod tests;

//! Bytecode for the stack VM: [`Op`], the built-in function enums and
//! [`Program`].
//!
//! Every expression leaves exactly one value on the stack. Jump targets and
//! loop bodies are absolute indices into [`Program::ops`].

use crate::ast::AssignOp;
use crate::vm::VarRef;

/// One-argument built-ins (`fnTable1` in `nseel-compiler.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fn1 {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sqrt,
    Sqr,
    Abs,
    Exp,
    Log,
    Log10,
    Floor,
    Ceil,
    Sign,
    Invsqrt,
    Rand,
    Freembuf,
}

/// Two-argument built-ins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fn2 {
    Atan2,
    Pow,
    Min,
    Max,
    Sigmoid,
    /// Non-short-circuit logical and (`eel1band`); `&&` is [`Op::And`].
    Band,
    /// Non-short-circuit logical or (`eel1bor`); `||` is [`Op::Or`].
    Bor,
    Exec2,
}

/// Three-argument built-ins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fn3 {
    Exec3,
    Memcpy,
    Memset,
}

impl Fn1 {
    /// Canonical (lowercase, alias-resolved) name to function.
    pub fn from_name(name: &str) -> Option<Fn1> {
        Some(match name {
            "sin" => Fn1::Sin,
            "cos" => Fn1::Cos,
            "tan" => Fn1::Tan,
            "asin" => Fn1::Asin,
            "acos" => Fn1::Acos,
            "atan" => Fn1::Atan,
            "sqrt" => Fn1::Sqrt,
            "sqr" => Fn1::Sqr,
            "abs" => Fn1::Abs,
            "exp" => Fn1::Exp,
            "log" => Fn1::Log,
            "log10" => Fn1::Log10,
            "floor" => Fn1::Floor,
            "ceil" => Fn1::Ceil,
            "sign" => Fn1::Sign,
            "invsqrt" => Fn1::Invsqrt,
            "rand" => Fn1::Rand,
            "freembuf" => Fn1::Freembuf,
            _ => return None,
        })
    }
}

impl Fn2 {
    pub fn from_name(name: &str) -> Option<Fn2> {
        Some(match name {
            "atan2" => Fn2::Atan2,
            "pow" => Fn2::Pow,
            "min" => Fn2::Min,
            "max" => Fn2::Max,
            "sigmoid" => Fn2::Sigmoid,
            "band" => Fn2::Band,
            "bor" => Fn2::Bor,
            "exec2" => Fn2::Exec2,
            _ => return None,
        })
    }
}

impl Fn3 {
    pub fn from_name(name: &str) -> Option<Fn3> {
        Some(match name {
            "exec3" => Fn3::Exec3,
            "memcpy" => Fn3::Memcpy,
            "memset" => Fn3::Memset,
            _ => return None,
        })
    }
}

/// One VM instruction. Stack effects are written `before -> after`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// `-> c`
    PushConst(f64),
    /// `-> var`
    Load(VarRef),
    /// `_set`: `v -> v`; stores `v`, or 0 when `v` is denormal, ±Inf or NaN.
    Store(VarRef),
    /// `+=` and friends: `v -> var op v`, stored unfiltered.
    Update(VarRef, AssignOp),
    /// `i -> mem[i]`; `true` selects `gmegabuf`.
    LoadMem(bool),
    /// `_set` on memory: `i v -> v`, filtered like [`Op::Store`].
    StoreMem(bool),
    /// Compound assignment on memory: `i v -> mem[i] op v`, unfiltered.
    UpdateMem(bool, AssignOp),
    /// `x ->`
    Pop,
    /// `a b -> a op b`
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    /// `x -> !x` (1 when `x` is false, else 0)
    Not,
    /// `x -> -x`
    Neg,
    /// `x -> truthy(x) as 0/1`
    Bool,
    /// `&&` left side: `a -> 0` and jump when `a` is false, else `a ->`.
    And(u32),
    /// `||` left side: `a -> 1` and jump when `a` is true, else `a ->`.
    Or(u32),
    /// `x -> f(x)`
    Call1(Fn1),
    /// `a b -> f(a, b)`
    Call2(Fn2),
    /// `a b c -> f(a, b, c)`
    Call3(Fn3),
    Jump(u32),
    /// `c ->`; jumps when `c` is false.
    JumpIfFalse(u32),
    /// `n -> v`: runs `ops[body_start..body_end]` `n` times (truncated,
    /// capped at [`crate::vm::LOOP_CAP`]), then continues at `body_end`.
    /// `v` is the last body value, or `n` when the body never ran.
    Loop {
        body_start: u32,
        body_end: u32,
    },
    /// `-> v`: runs the body until its value is false, at most
    /// [`crate::vm::LOOP_CAP`] times; `v` is the last body value.
    While {
        body_start: u32,
        body_end: u32,
    },
}

/// A compiled expression program, bound to the `Vm` it was compiled with.
#[derive(Debug, Clone, Default)]
pub struct Program {
    /// Only the compiler builds these, so jump targets and loop bodies are
    /// always in range.
    pub(crate) ops: Vec<Op>,
}

impl Program {
    /// The compiled instructions (for inspection and tests).
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }
}

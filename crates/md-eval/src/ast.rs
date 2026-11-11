//! Abstract syntax tree produced by the parser.
//!
//! Canonical forms (ns-eel2 lowers operators to calls in its preprocessor;
//! here every operator-like call gets one node shape, whatever the spelling):
//!
//! | source                                   | node                              |
//! |------------------------------------------|-----------------------------------|
//! | `a ? b : c`, `if(a,b,c)`, `_if(a,b,c)`   | `Ternary`                         |
//! | `x = v`, `assign(x,v)`, `_set(x,v)`      | `Assign(Set, ..)`                 |
//! | `x += v` .. `x ^= v`, `_addop(x,v)` ..   | `Assign(Add..Pow, ..)`            |
//! | `a == b`, `equal(a,b)`, `_equal(a,b)`    | `Binary(Eq, ..)` (likewise `!= < > <= >=`) |
//! | `a && b`, `_and(a,b)` / `a \|\| b`, `_or` | `Binary(And/Or, ..)`              |
//! | `a % b`, `_mod(a,b)`                     | `Binary(Mod, ..)`                 |
//! | `!a`, `bnot(a)`, `_not(a)`               | `Unary(Not, ..)`                  |
//! | `a ^ b`, `pow(a,b)`                      | `Call { name: "pow" }`            |
//! | `x[i]`, `megabuf(i)`                     | `Call { name: "_mem" }`           |
//! | `gmem[i]`, `gmegabuf(i)`                 | `Call { name: "_gmem" }`          |
//! | `int(x)`                                 | `Call { name: "floor" }`          |
//! | `(a; b)` inside parentheses              | `Seq`                             |
//!
//! Unary `+` is dropped (`FN_UPLUS` is the identity). The `Display` impl
//! prints an S-expression, used by tests.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
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
    /// Short-circuit `&&` (`_and`).
    And,
    /// Short-circuit `||` (`_or`).
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    Pow,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    /// Variable, name lowercased.
    Var(String),
    /// Built-in function call; `name` is the canonical (lowercase) name.
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// The target is a `Var` or a `Call` of `_mem` / `_gmem`.
    Assign(AssignOp, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    /// Evaluate in order; the value is the last one's (0 when empty).
    Seq(Vec<Expr>),
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

impl AssignOp {
    pub fn symbol(self) -> &'static str {
        match self {
            AssignOp::Set => "=",
            AssignOp::Add => "+=",
            AssignOp::Sub => "-=",
            AssignOp::Mul => "*=",
            AssignOp::Div => "/=",
            AssignOp::Mod => "%=",
            AssignOp::BitAnd => "&=",
            AssignOp::BitOr => "|=",
            AssignOp::Pow => "^=",
        }
    }
}

fn fmt_num(f: &mut fmt::Formatter<'_>, v: f64) -> fmt::Result {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        write!(f, "{}", v as i64)
    } else {
        write!(f, "{v}")
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Num(v) => fmt_num(f, *v),
            Expr::Var(n) => write!(f, "{n}"),
            Expr::Call { name, args } => {
                write!(f, "({name}")?;
                for a in args {
                    write!(f, " {a}")?;
                }
                write!(f, ")")
            }
            Expr::Unary(UnOp::Neg, a) => write!(f, "(neg {a})"),
            Expr::Unary(UnOp::Not, a) => write!(f, "(! {a})"),
            Expr::Binary(op, a, b) => write!(f, "({} {a} {b})", op.symbol()),
            Expr::Assign(op, a, b) => write!(f, "({} {a} {b})", op.symbol()),
            Expr::Ternary(c, a, b) => write!(f, "(? {c} {a} {b})"),
            Expr::Seq(items) => {
                write!(f, "(seq")?;
                for a in items {
                    write!(f, " {a}")?;
                }
                write!(f, ")")
            }
        }
    }
}

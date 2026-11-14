//! Variable table and memory owned by one expression VM, and the stack
//! interpreter that runs a [`Program`] on it.

use crate::ast::AssignOp;
use crate::bytecode::{Op, Program};
use crate::mem::{eel_int, GlobalMem, MegaBuf};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// `NSEEL_MAX_VARIABLE_NAMELEN` (ns-eel.h): names compare on this many
/// characters, so longer names are truncated to it.
pub const MAX_VARIABLE_NAMELEN: usize = 16;

/// Registers are numbered from here, like `NSEEL_GLOBALVAR_BASE` (1 << 24).
const REG_BASE: u32 = 1 << 24;

/// Handle to a variable in a `Vm`'s table, or to a shared register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarRef(u32);

impl VarRef {
    /// Raw handle value (registers start at `1 << 24`).
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Variables, `megabuf`, and a handle to the shared registers and `gmegabuf`.
pub struct Vm {
    names: HashMap<String, u32>,
    values: Vec<f64>,
    pub megabuf: MegaBuf,
    pub global: Rc<RefCell<GlobalMem>>,
    /// Value stack reused by every [`Program::run`] (no per-run allocation).
    stack: Vec<f64>,
}

/// Register number when `name` (already lowercase) is exactly `reg` + 2 digits.
fn reg_index(name: &str) -> Option<u32> {
    let d = name.strip_prefix("reg")?.as_bytes();
    (d.len() == 2 && d.iter().all(u8::is_ascii_digit))
        .then(|| u32::from(d[0] - b'0') * 10 + u32::from(d[1] - b'0'))
}

impl Vm {
    pub fn new(global: Rc<RefCell<GlobalMem>>) -> Vm {
        Vm {
            names: HashMap::new(),
            values: Vec::new(),
            megabuf: MegaBuf::new(),
            global,
            stack: Vec::new(),
        }
    }

    /// Look up or create (at 0.0) a variable. Case-insensitive; only the
    /// first 16 characters matter. `regNN` (exactly two digits) is a shared
    /// register.
    pub fn var(&mut self, name: &str) -> VarRef {
        let key: String = name
            .chars()
            .map(|c| c.to_ascii_lowercase())
            .take(MAX_VARIABLE_NAMELEN)
            .collect();
        if let Some(n) = reg_index(&key) {
            return VarRef(REG_BASE + n);
        }
        if let Some(&i) = self.names.get(&key) {
            return VarRef(i);
        }
        let i = self.values.len() as u32;
        self.values.push(0.0);
        self.names.insert(key, i);
        VarRef(i)
    }

    pub fn get(&self, v: VarRef) -> f64 {
        if v.0 >= REG_BASE {
            self.global.borrow().regs[(v.0 - REG_BASE) as usize]
        } else {
            self.values[v.0 as usize]
        }
    }

    pub fn set(&mut self, v: VarRef, x: f64) {
        if v.0 >= REG_BASE {
            self.global.borrow_mut().regs[(v.0 - REG_BASE) as usize] = x;
        } else {
            self.values[v.0 as usize] = x;
        }
    }

    /// Zero every local variable and free `megabuf`. The shared registers
    /// and `gmegabuf` belong to `GlobalMem` and are left alone.
    pub fn reset_vars(&mut self) {
        self.values.fill(0.0);
        self.megabuf.clear();
    }
}

/// `NSEEL_LOOPFUNC_SUPPORT_MAXLEN` (ns-eel.h): most iterations one `loop`
/// or `while` runs.
pub const LOOP_CAP: u32 = 1_048_576;

/// `NSEEL_CLOSEFACTOR` (`g_closefact`).
pub const CLOSEFACTOR: f64 = 0.00001;

/// Truth test of `_if`, `_and`, `_or`, `!` and `while`: `fabs` then `fcomp
/// g_closefact`, false when below (C0 set) or unordered. So exactly
/// 0.00001 is true and NaN is false.
#[inline]
pub(crate) fn truthy(x: f64) -> bool {
    x.abs() >= CLOSEFACTOR
}

/// The C helpers `eel1band`/`eel1bor`/`eel1sigmoid` test
/// `fabs(x) > g_closefact` instead (exactly 0.00001 is false).
#[inline]
pub(crate) fn truthy_strict(x: f64) -> bool {
    x.abs() > CLOSEFACTOR
}

/// `fld a; fcomp b` sets C0: `a < b`, or unordered (NaN).
#[inline]
fn below(a: f64, b: f64) -> bool {
    a < b || a.is_nan() || b.is_nan()
}

/// `!=` of `nseel_asm_notequal`: `|b - a| >= g_closefact`, false when the
/// difference is NaN (so NaN, and Inf vs Inf, compare equal).
#[inline]
fn differ(a: f64, b: f64) -> bool {
    (b - a).abs() >= CLOSEFACTOR
}

#[inline]
fn bool_f(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}

/// `nseel_asm_assign`: a value whose exponent bits are all 0 (±0,
/// denormals) or all 1 (±Inf, NaN) is stored as +0.
#[inline]
fn assign_filter(x: f64) -> f64 {
    let e = x.to_bits() & 0x7ff0_0000_0000_0000;
    if e == 0 || e == 0x7ff0_0000_0000_0000 {
        0.0
    } else {
        x
    }
}

/// `nseel_asm_mod`: both operands go through `fabs` and a 32-bit chop
/// `fistp` (out of range or NaN gives 0x80000000), then an unsigned
/// `divl`; a zero divisor gives 0. The result is never negative.
#[inline]
fn eel_mod(a: f64, b: f64) -> f64 {
    let d = eel_int(b.abs()) as u32;
    if d == 0 {
        0.0
    } else {
        f64::from((eel_int(a.abs()) as u32) % d)
    }
}

/// 64-bit chop `fistp` used by `&` and `|`: out of range or NaN gives
/// 0x8000000000000000.
#[inline]
fn eel_int64(x: f64) -> i64 {
    let t = x.trunc();
    if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&t) {
        t as i64
    } else {
        i64::MIN
    }
}

/// `target op= value` as the `_addop`.. `_powop` stubs compute it.
#[inline]
fn compound(op: AssignOp, target: f64, value: f64) -> f64 {
    match op {
        AssignOp::Set => value,
        AssignOp::Add => target + value,
        AssignOp::Sub => target - value,
        AssignOp::Mul => target * value,
        AssignOp::Div => target / value,
        AssignOp::Mod => eel_mod(target, value),
        AssignOp::BitAnd => (eel_int64(target) & eel_int64(value)) as f64,
        AssignOp::BitOr => (eel_int64(target) | eel_int64(value)) as f64,
        AssignOp::Pow => target.powf(value),
    }
}

impl Vm {
    fn load_mem(&self, global: bool, i: f64) -> f64 {
        if global {
            self.global.borrow().gmegabuf.get(i)
        } else {
            self.megabuf.get(i)
        }
    }

    fn store_mem(&mut self, global: bool, i: f64, v: f64) {
        if global {
            self.global.borrow_mut().gmegabuf.set(i, v);
        } else {
            self.megabuf.set(i, v);
        }
    }
}

impl Program {
    /// Run the program on `vm` (the `Vm` it was compiled with) and return
    /// the value of its last statement.
    pub fn run(&self, vm: &mut Vm) -> f64 {
        let mut stack = std::mem::take(&mut vm.stack);
        stack.clear();
        self.exec(0, self.ops.len(), vm, &mut stack);
        let v = stack.pop().unwrap_or(0.0);
        vm.stack = stack;
        v
    }

    /// Run `ops[start..end]`, which leaves exactly one value on `st`.
    fn exec(&self, start: usize, end: usize, vm: &mut Vm, st: &mut Vec<f64>) {
        let mut pc = start;
        while pc < end {
            let op = self.ops[pc];
            pc += 1;
            match op {
                Op::PushConst(c) => st.push(c),
                Op::Load(v) => st.push(vm.get(v)),
                Op::Store(v) => {
                    let x = top(st);
                    vm.set(v, assign_filter(x));
                }
                Op::Update(v, op) => {
                    let x = compound(op, vm.get(v), pop(st));
                    vm.set(v, x);
                    st.push(x);
                }
                Op::LoadMem(g) => {
                    let i = pop(st);
                    st.push(vm.load_mem(g, i));
                }
                Op::StoreMem(g) => {
                    let x = pop(st);
                    let i = pop(st);
                    vm.store_mem(g, i, assign_filter(x));
                    st.push(x);
                }
                Op::UpdateMem(g, op) => {
                    let x = pop(st);
                    let i = pop(st);
                    let r = compound(op, vm.load_mem(g, i), x);
                    vm.store_mem(g, i, r);
                    st.push(r);
                }
                Op::Pop => {
                    st.pop();
                }
                Op::Add => bin(st, |a, b| a + b),
                Op::Sub => bin(st, |a, b| a - b),
                Op::Mul => bin(st, |a, b| a * b),
                Op::Div => bin(st, |a, b| a / b),
                Op::Mod => bin(st, eel_mod),
                Op::BitAnd => bin(st, |a, b| (eel_int64(a) & eel_int64(b)) as f64),
                Op::BitOr => bin(st, |a, b| (eel_int64(a) | eel_int64(b)) as f64),
                // x87 flags: `fcomp` sets C0 for "below" and for unordered,
                // so NaN makes == < > true and != <= >= false.
                Op::Eq => bin(st, |a, b| bool_f(!differ(a, b))),
                Op::Ne => bin(st, |a, b| bool_f(differ(a, b))),
                Op::Lt => bin(st, |a, b| bool_f(below(a, b))),
                Op::Gt => bin(st, |a, b| bool_f(below(b, a))),
                Op::Le => bin(st, |a, b| bool_f(b >= a)),
                Op::Ge => bin(st, |a, b| bool_f(a >= b)),
                Op::Not => un(st, |x| bool_f(!truthy(x))),
                Op::Neg => un(st, |x| -x),
                Op::Bool => un(st, |x| bool_f(truthy(x))),
                Op::And(t) => {
                    if truthy(top(st)) {
                        st.pop();
                    } else {
                        set_top(st, 0.0);
                        pc = t as usize;
                    }
                }
                Op::Or(t) => {
                    if truthy(top(st)) {
                        set_top(st, 1.0);
                        pc = t as usize;
                    } else {
                        st.pop();
                    }
                }
                Op::Call1(f) => {
                    let x = pop(st);
                    st.push(f.eval(x, vm));
                }
                Op::Call2(f) => bin(st, |a, b| f.eval(a, b)),
                Op::Call3(f) => {
                    let c = pop(st);
                    let b = pop(st);
                    let a = pop(st);
                    st.push(f.eval(a, b, c, vm));
                }
                Op::Jump(t) => pc = t as usize,
                Op::JumpIfFalse(t) => {
                    if !truthy(pop(st)) {
                        pc = t as usize;
                    }
                }
                Op::Loop {
                    body_start,
                    body_end,
                } => {
                    let (s, e) = (body_start as usize, body_end as usize);
                    // nseel_asm_repeat: 32-bit chop fistp, skip below 1, cap
                    let n = eel_int(top(st));
                    if n >= 1 {
                        st.pop();
                        let n = (n as u32).min(LOOP_CAP);
                        for k in 0..n {
                            if k > 0 {
                                st.pop();
                            }
                            self.exec(s, e, vm, st);
                        }
                    }
                    pc = e;
                }
                Op::While {
                    body_start,
                    body_end,
                } => {
                    let (s, e) = (body_start as usize, body_end as usize);
                    for k in 0..LOOP_CAP {
                        if k > 0 {
                            st.pop();
                        }
                        self.exec(s, e, vm, st);
                        if !truthy(top(st)) {
                            break;
                        }
                    }
                    pc = e;
                }
            }
        }
    }
}

// Stack helpers. The compiler guarantees operands are present; an empty
// stack (a malformed hand-built program) reads as 0 rather than panicking.
#[inline]
fn pop(st: &mut Vec<f64>) -> f64 {
    st.pop().unwrap_or(0.0)
}

#[inline]
fn top(st: &[f64]) -> f64 {
    st.last().copied().unwrap_or(0.0)
}

#[inline]
fn set_top(st: &mut Vec<f64>, x: f64) {
    match st.last_mut() {
        Some(t) => *t = x,
        None => st.push(x),
    }
}

#[inline]
fn un(st: &mut Vec<f64>, f: impl FnOnce(f64) -> f64) {
    let x = pop(st);
    st.push(f(x));
}

#[inline]
fn bin(st: &mut Vec<f64>, f: impl FnOnce(f64, f64) -> f64) {
    let b = pop(st);
    let a = pop(st);
    st.push(f(a, b));
}

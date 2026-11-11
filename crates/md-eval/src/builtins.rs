//! Evaluation of the built-in functions ([`Fn1`], [`Fn2`], [`Fn3`]).
//!
//! Semantics follow `code/ns-eel2` (x86, FPU in chop mode). Task 1.5 owns
//! the detailed numeric tests; entries marked `TODO(1.5)` are placeholders.

use crate::bytecode::{Fn1, Fn2, Fn3};
use crate::vm::{truthy_strict, Vm};

/// `nseel_asm_invsqrt`: the 0x5f3759df estimate refined by one Newton step,
/// `y * (1.5 - 0.5 * x * y * y)`, where `y` is computed from `x` rounded
/// to `f32` with the FPU's chop rounding.
fn invsqrt(x: f64) -> f64 {
    // `as` rounds to nearest; fsts under chop rounds toward zero
    let mut f = x as f32;
    if f.is_infinite() && x.is_finite() {
        f = f32::MAX.copysign(f);
    } else if f64::from(f).abs() > x.abs() {
        f = f32::from_bits(f.to_bits() - 1);
    }
    let i = 0x5f37_59df_i32.wrapping_sub((f.to_bits() as i32) >> 1);
    let y = f64::from(f32::from_bits(i as u32));
    ((-0.5 * x) * y * y + 1.5) * y
}

/// `nseel_asm_sign`: ±0 are returned as they are, anything else (NaN
/// included) maps to ±1 by its sign bit.
fn sign(x: f64) -> f64 {
    if x == 0.0 {
        x
    } else if x.is_sign_negative() {
        -1.0
    } else {
        1.0
    }
}

impl Fn1 {
    pub(crate) fn eval(self, x: f64, _vm: &mut Vm) -> f64 {
        match self {
            // TODO(1.5): x87 fsin/fcos/fptan return |x| >= 2^63 unchanged
            Fn1::Sin => x.sin(),
            Fn1::Cos => x.cos(),
            Fn1::Tan => x.tan(),
            Fn1::Asin => x.asin(),
            Fn1::Acos => x.acos(),
            Fn1::Atan => x.atan(),
            // nseel_asm_sqrt: fabs before fsqrt
            Fn1::Sqrt => x.abs().sqrt(),
            Fn1::Sqr => x * x,
            Fn1::Abs => x.abs(),
            Fn1::Exp => x.exp(),
            Fn1::Log => x.ln(),
            Fn1::Log10 => x.log10(),
            Fn1::Floor => x.floor(),
            Fn1::Ceil => x.ceil(),
            Fn1::Sign => sign(x),
            Fn1::Invsqrt => invsqrt(x),
            // TODO(1.5): MT19937 in GlobalMem, `genrand_int32() / 0xFFFFFFFF
            // * max(floor(x), 1)`. 0 is inside that range.
            Fn1::Rand => 0.0,
            // Ledger ruling (Task 1.3): MilkDrop never frees on request, so
            // freembuf is a no-op returning its argument.
            Fn1::Freembuf => x,
        }
    }
}

impl Fn2 {
    pub(crate) fn eval(self, a: f64, b: f64) -> f64 {
        match self {
            Fn2::Atan2 => a.atan2(b),
            Fn2::Pow => a.powf(b),
            // nseel_asm_min/max: fcomp sets C0 on `a < b` or unordered
            Fn2::Min => {
                if a >= b {
                    b
                } else {
                    a
                }
            }
            Fn2::Max => {
                if a >= b {
                    a
                } else {
                    b
                }
            }
            Fn2::Sigmoid => {
                let t = 1.0 + (-a * b).exp();
                if truthy_strict(t) {
                    1.0 / t
                } else {
                    0.0
                }
            }
            Fn2::Band => f64::from(u8::from(truthy_strict(a) && truthy_strict(b))),
            Fn2::Bor => f64::from(u8::from(truthy_strict(a) || truthy_strict(b))),
            Fn2::Exec2 => b,
        }
    }
}

impl Fn3 {
    pub(crate) fn eval(self, a: f64, b: f64, c: f64, vm: &mut Vm) -> f64 {
        match self {
            Fn3::Exec3 => c,
            // __NSEEL_RAM_MemCpy / MemSet return the destination argument
            Fn3::Memcpy => {
                vm.megabuf.copy(a, b, c);
                a
            }
            Fn3::Memset => {
                vm.megabuf.fill(a, b, c);
                a
            }
        }
    }
}

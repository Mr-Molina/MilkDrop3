//! Built-in function tests, pinned against `code/ns-eel2` (`nseel-cfunc.c`,
//! `nseel-compiler.c` helpers, `asm-nseel-x86-msvc.c` stubs).

use crate::bytecode::Program;
use crate::mem::GlobalMem;
use crate::mt19937::{Mt19937, NSEEL_SEED};
use crate::vm::Vm;
use std::cell::RefCell;
use std::rc::Rc;

fn vm() -> Vm {
    Vm::new(Rc::new(RefCell::new(GlobalMem::new())))
}

fn run_on(vm: &mut Vm, src: &str) -> f64 {
    match Program::compile(vm, src) {
        Ok(p) => p.run(vm),
        Err(e) => panic!("{src:?} failed to compile: {e}"),
    }
}

fn ev(src: &str) -> f64 {
    run_on(&mut vm(), src)
}

#[test]
fn mt19937_known_answer_seed_5489() {
    let mut g = Mt19937::seeded(5489);
    assert_eq!(g.next_u32(), 3_499_211_612);
    assert_eq!(g.next_u32(), 581_869_302);
}

#[test]
fn mt19937_first_output_for_seed_0x4141f00d() {
    // Cross-checked with an independent Python implementation of the
    // published algorithm.
    let mut g = Mt19937::seeded(NSEEL_SEED);
    assert_eq!(NSEEL_SEED, 0x4141_f00d);
    let first: Vec<u32> = (0..4).map(|_| g.next_u32()).collect();
    assert_eq!(
        first,
        [978_073_627, 1_463_485_156, 3_838_205_119, 4_078_471_044]
    );
    // Past the first twist (624 words) as well.
    let mut g = Mt19937::default();
    let v1000 = (0..1000).map(|_| g.next_u32()).last().unwrap();
    assert_eq!(v1000, 1_961_510_904);
}

#[test]
fn rand_stream_is_pinned_for_a_fresh_generator() {
    // genrand_int32() * (1.0 / 4294967295.0) * x
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "rand(1)"), 0.227_725_512_168_306_28);
    assert_eq!(run_on(&mut vm, "rand(1)"), 0.340_744_190_928_699_5);
    assert_eq!(run_on(&mut vm, "rand(10)"), 8.936_517_685_404_17);
    assert_eq!(run_on(&mut vm, "rand(10)"), 9.495_930_385_192_84);
}

#[test]
fn rand_range_is_closed_and_floors_and_clamps() {
    // Extremes of the generator output: 0 and 0xFFFFFFFF map to 0 and x.
    let inv = 1.0 / 4_294_967_295.0;
    assert_eq!(f64::from(u32::MAX) * inv, 1.0);
    // x floors, then clamps to at least 1
    let mut v = vm();
    for _ in 0..200 {
        let r = run_on(&mut v, "rand(0.2)");
        assert!((0.0..=1.0).contains(&r));
        let r = run_on(&mut v, "rand(-50)");
        assert!((0.0..=1.0).contains(&r));
        let r = run_on(&mut v, "rand(3.9)");
        assert!((0.0..=3.0).contains(&r));
    }
    // rand(1.9) uses x = 1, same stream position as rand(1)
    assert_eq!(ev("rand(1.9)"), ev("rand(1)"));
    assert_eq!(ev("rand(0)"), ev("rand(1)"));
    assert_eq!(ev("rand(-3)"), ev("rand(1)"));
    assert_eq!(ev("rand(2.5)"), ev("rand(2)"));
}

#[test]
fn rand_stream_is_shared_between_vms() {
    let g = Rc::new(RefCell::new(GlobalMem::new()));
    let mut a = Vm::new(g.clone());
    let mut b = Vm::new(g);
    assert_eq!(run_on(&mut a, "rand(1)"), 0.227_725_512_168_306_28);
    assert_eq!(run_on(&mut b, "rand(1)"), 0.340_744_190_928_699_5);
}

#[test]
fn sigmoid_matches_formula() {
    let expect = |x: f64, c: f64| 1.0 / (1.0 + (-x * c).exp());
    assert_eq!(ev("sigmoid(1, 2)"), expect(1.0, 2.0));
    assert_eq!(ev("sigmoid(-3, 0.5)"), expect(-3.0, 0.5));
    assert_eq!(ev("sigmoid(0, 7)"), 0.5);
    // t = 1 + exp(..) is always >= 1 for finite input; huge negative x*c
    // overflows exp to inf, giving 1/inf = 0
    assert_eq!(ev("sigmoid(-1000, 1000)"), 0.0);
}

#[test]
fn band_bor_use_strict_closefactor() {
    assert_eq!(ev("band(1, 1)"), 1.0);
    assert_eq!(ev("band(1, 0)"), 0.0);
    assert_eq!(ev("band(0.00001, 1)"), 0.0);
    assert_eq!(ev("band(0.00002, 1)"), 1.0);
    assert_eq!(ev("band(-2, -3)"), 1.0);
    assert_eq!(ev("bor(0, 0)"), 0.0);
    assert_eq!(ev("bor(0.00001, 0)"), 0.0);
    assert_eq!(ev("bor(0, -0.5)"), 1.0);
}

#[test]
fn sign_of_zero_is_zero() {
    assert_eq!(ev("sign(0)"), 0.0);
    assert_eq!(ev("sign(5)"), 1.0);
    assert_eq!(ev("sign(-5)"), -1.0);
    assert_eq!(ev("sign(1e-300)"), 1.0);
    // -0.0 is returned as is (bits tested without the sign bit)
    let mut v = vm();
    let r = run_on(&mut v, "sign(0 * -1)");
    assert_eq!(r, 0.0);
    assert!(r.is_sign_negative());
    // NaN has non-zero magnitude bits: sign by its sign bit (x87 NaN from
    // 0/0 style invalid ops is the negative "real indefinite")
    let n = run_on(&mut v, "sign(sqrt(-1) * 0 + 0 / 0)");
    assert!(n == 1.0 || n == -1.0);
}

#[test]
fn invsqrt_is_the_fast_estimate_with_one_newton_step() {
    // Values computed by an independent Python model of nseel_asm_invsqrt.
    assert_eq!(ev("invsqrt(4)"), 0.499_153_574_792_391_03);
    assert_eq!(ev("invsqrt(1)"), 0.998_307_149_584_782_1);
    assert_eq!(ev("invsqrt(2)"), 0.706_930_038_698_333_4);
    assert_eq!(ev("invsqrt(0.25)"), 1.996_614_299_169_564_1);
    assert_eq!(ev("invsqrt(100)"), 0.099_844_882_593_201_4);
    // close to, but not exactly, 1/sqrt
    assert_ne!(ev("invsqrt(4)"), 0.5);
}

#[test]
fn sqr_abs_sqrt() {
    assert_eq!(ev("sqr(-3)"), 9.0);
    assert_eq!(ev("abs(-2.5)"), 2.5);
    assert_eq!(ev("abs(-0)"), 0.0);
    assert_eq!(ev("sqrt(9)"), 3.0);
    // nseel_asm_sqrt takes fabs first
    assert_eq!(ev("sqrt(-9)"), 3.0);
}

#[test]
fn floor_ceil_are_real_floor_and_ceil() {
    assert_eq!(ev("floor(-1.5)"), -2.0);
    assert_eq!(ev("floor(1.9)"), 1.0);
    assert_eq!(ev("ceil(-1.5)"), -1.0);
    assert_eq!(ev("ceil(1.1)"), 2.0);
    // no int conversion: large values are not clamped
    assert_eq!(ev("floor(1e15 + 0.5)"), 1e15);
    assert_eq!(ev("floor(3e9)"), 3e9);
    assert_eq!(ev("ceil(-3e9 - 0.5)"), -3e9);
}

#[test]
fn min_max_follow_x87_compare_with_nan() {
    assert_eq!(ev("min(1, 2)"), 1.0);
    assert_eq!(ev("min(2, 1)"), 1.0);
    assert_eq!(ev("max(1, 2)"), 2.0);
    assert_eq!(ev("max(2, 1)"), 2.0);
    // Unordered: C0 is set, so min picks the first, max picks the second.
    assert!(ev("min(0/0, 1)").is_nan());
    assert_eq!(ev("min(1, 0/0)"), 1.0);
    assert_eq!(ev("max(0/0, 1)"), 1.0);
    assert!(ev("max(1, 0/0)").is_nan());
}

#[test]
fn exp_log_edge_cases() {
    assert_eq!(ev("exp(0)"), 1.0);
    assert_eq!(ev("log(1)"), 0.0);
    assert_eq!(ev("log(0)"), f64::NEG_INFINITY);
    assert!(ev("log(-1)").is_nan());
    assert_eq!(ev("log10(0)"), f64::NEG_INFINITY);
    assert!(ev("log10(-1)").is_nan());
    assert_eq!(ev("log10(1000)"), 3.0);
    assert!((ev("log(exp(2))") - 2.0).abs() < 1e-15);
    assert_eq!(ev("exp(1000)"), f64::INFINITY);
    assert_eq!(ev("exp(-1000)"), 0.0);
}

#[test]
fn atan2_argument_order_is_y_then_x() {
    assert_eq!(ev("atan2(1, 0)"), std::f64::consts::FRAC_PI_2);
    assert_eq!(ev("atan2(0, -1)"), std::f64::consts::PI);
    assert_eq!(ev("atan2(0, 1)"), 0.0);
    assert_eq!(ev("atan2(-1, 0)"), -std::f64::consts::FRAC_PI_2);
}

#[test]
fn pow_edge_cases() {
    assert_eq!(ev("pow(2, 10)"), 1024.0);
    assert!(ev("pow(-8, 1/3)").is_nan());
    assert_eq!(ev("pow(-2, 3)"), -8.0);
    assert_eq!(ev("pow(0, 0)"), 1.0);
    assert_eq!(ev("pow(0, -1)"), f64::INFINITY);
    assert_eq!(ev("pow(2, -1)"), 0.5);
    assert_eq!(ev("2 ^ 3"), 8.0);
}

#[test]
fn trig_basic_and_inverse() {
    assert_eq!(ev("sin(0)"), 0.0);
    assert_eq!(ev("cos(0)"), 1.0);
    assert_eq!(ev("tan(0)"), 0.0);
    assert_eq!(ev("asin(1)"), std::f64::consts::FRAC_PI_2);
    assert_eq!(ev("acos(1)"), 0.0);
    assert_eq!(ev("atan(1)"), std::f64::consts::FRAC_PI_4);
    assert!(ev("asin(2)").is_nan());
}

#[test]
fn sin_cos_tan_huge_arguments_follow_x87() {
    // fsin/fcos leave |x| >= 2^63 unchanged (C2 set); fptan then leaves the
    // stack empty and the store yields NaN.
    assert_eq!(ev("sin(1e19)"), 1e19);
    assert_eq!(ev("cos(-1e19)"), -1e19);
    assert_eq!(ev("sin(9223372036854775808)"), 9_223_372_036_854_775_808.0);
    assert!(ev("tan(1e19)").is_nan());
    // just below 2^63 is reduced normally
    assert!(ev("sin(9223372036854774784)").abs() <= 1.0);
    // infinity is an invalid operation
    assert!(ev("sin(1/0)").is_nan());
    assert!(ev("cos(1/0)").is_nan());
}

#[test]
fn exec2_exec3_return_last() {
    assert_eq!(ev("exec2(1, 2)"), 2.0);
    assert_eq!(ev("exec3(1, 2, 3)"), 3.0);
    let mut v = vm();
    assert_eq!(run_on(&mut v, "exec3(a = 1, b = 2, a + b)"), 3.0);
}

#[test]
fn memcpy_memset_return_dst_and_freembuf_is_noop() {
    let mut v = vm();
    assert_eq!(run_on(&mut v, "memset(10, 7, 5)"), 10.0);
    assert_eq!(run_on(&mut v, "megabuf(12)"), 7.0);
    assert_eq!(run_on(&mut v, "memcpy(100, 10, 5)"), 100.0);
    assert_eq!(run_on(&mut v, "megabuf(104)"), 7.0);
    // Ledger ruling (Task 1.3): freembuf(n) returns n and frees nothing.
    assert_eq!(run_on(&mut v, "freembuf(0)"), 0.0);
    assert_eq!(run_on(&mut v, "freembuf(70000)"), 70000.0);
    assert_eq!(run_on(&mut v, "megabuf(12)"), 7.0);
}

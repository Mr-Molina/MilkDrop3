//! Compiler + stack VM tests. Expected values follow the x86 code ns-eel2
//! generates (`code/ns-eel2/asm-nseel-x86-gcc.c`, `nseel-compiler.c`), run
//! with the FPU in chop mode as `GLUE_CALL_CODE` sets it.

use super::compile_ast;
use crate::ast::Expr;
use crate::bytecode::Program;
use crate::mem::GlobalMem;
use crate::vm::Vm;
use std::cell::RefCell;
use std::rc::Rc;

fn vm() -> Vm {
    Vm::new(Rc::new(RefCell::new(GlobalMem::new())))
}

/// Compile and run `src` on `vm`, returning the program's value.
fn run_on(vm: &mut Vm, src: &str) -> f64 {
    match Program::compile(vm, src) {
        Ok(p) => p.run(vm),
        Err(e) => panic!("{src:?} failed to compile: {e}"),
    }
}

fn ev(src: &str) -> f64 {
    run_on(&mut vm(), src)
}

fn get(vm: &mut Vm, name: &str) -> f64 {
    let v = vm.var(name);
    vm.get(v)
}

#[test]
fn if_does_not_evaluate_untaken_branch() {
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "if(0, a=1, b=1)"), 1.0);
    assert_eq!(get(&mut vm, "a"), 0.0);
    assert_eq!(get(&mut vm, "b"), 1.0);
    assert_eq!(run_on(&mut vm, "c = 1 ? (d = 2) : (e = 3); c"), 2.0);
    assert_eq!(get(&mut vm, "e"), 0.0);
    // `a ? b` is `a ? b : 0`
    assert_eq!(ev("0 ? 5"), 0.0);
}

#[test]
fn truthiness_is_abs_not_below_closefactor() {
    // nseel_asm_if: fabs; fcomp g_closefact; C0 (below or unordered) => false
    assert_eq!(ev("if(0.000009, 1, 2)"), 2.0);
    assert_eq!(ev("if(-0.000009, 1, 2)"), 2.0);
    assert_eq!(ev("if(-0.00002, 1, 2)"), 1.0);
    // exactly NSEEL_CLOSEFACTOR is not below it: true
    assert_eq!(ev("if(0.00001, 1, 2)"), 1.0);
    // NaN compares unordered: false
    assert_eq!(ev("if(0/0, 1, 2)"), 2.0);
}

#[test]
fn not_yields_exact_zero_or_one() {
    assert_eq!(ev("!0"), 1.0);
    assert_eq!(ev("!0.000009"), 1.0);
    assert_eq!(ev("!0.5"), 0.0);
    assert_eq!(ev("!(-3)"), 0.0);
    assert_eq!(ev("!(0/0)"), 1.0);
    assert_eq!(ev("bnot(7)"), 0.0);
}

#[test]
fn and_or_short_circuit_and_yield_zero_or_one() {
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "0 && (a = 1)"), 0.0);
    assert_eq!(get(&mut vm, "a"), 0.0);
    assert_eq!(run_on(&mut vm, "1 || (b = 1)"), 1.0);
    assert_eq!(get(&mut vm, "b"), 0.0);
    assert_eq!(run_on(&mut vm, "2 && (c = 3)"), 1.0);
    assert_eq!(get(&mut vm, "c"), 3.0);
    assert_eq!(run_on(&mut vm, "0 || (d = 5)"), 1.0);
    assert_eq!(get(&mut vm, "d"), 5.0);
    assert_eq!(ev("0.5 && 0.000001"), 0.0);
    assert_eq!(ev("0 || 0"), 0.0);
    assert_eq!(ev("-4 || 0"), 1.0);
}

#[test]
fn band_bor_evaluate_both_operands() {
    // eel1band/eel1bor are plain 2-arg C functions: no short circuit.
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "band(0, a = 1)"), 0.0);
    assert_eq!(get(&mut vm, "a"), 1.0);
    assert_eq!(run_on(&mut vm, "bor(1, b = 1)"), 1.0);
    assert_eq!(get(&mut vm, "b"), 1.0);
    assert_eq!(ev("band(2, 3)"), 1.0);
    assert_eq!(ev("bor(0, 0)"), 0.0);
}

#[test]
fn comparisons_yield_exact_zero_or_one() {
    assert_eq!(ev("1 < 2"), 1.0);
    assert_eq!(ev("2 < 1"), 0.0);
    assert_eq!(ev("2 > 1"), 1.0);
    assert_eq!(ev("1 <= 1"), 1.0);
    assert_eq!(ev("2 <= 1"), 0.0);
    assert_eq!(ev("1 >= 1"), 1.0);
    assert_eq!(ev("0 >= 1"), 0.0);
    assert_eq!(ev("3 != 3"), 0.0);
    assert_eq!(ev("3 != 4"), 1.0);
    assert_eq!(ev("equal(3, 3)"), 1.0);
    assert_eq!(ev("below(1, 2)"), 1.0);
    assert_eq!(ev("above(1, 2)"), 0.0);
}

#[test]
fn equality_uses_closefactor() {
    assert_eq!(ev("1 == 1.000009"), 1.0);
    assert_eq!(ev("1 == 1.00002"), 0.0);
    assert_eq!(ev("1 != 1.000009"), 0.0);
    // `<` is exact, unlike `==`
    assert_eq!(ev("1 < 1.000009"), 1.0);
}

#[test]
fn nan_comparisons_follow_x87_unordered_flags() {
    // fcomp sets C0 on unordered; each op tests C0 one way or the other.
    assert_eq!(ev("(0/0) == 5"), 1.0);
    assert_eq!(ev("(0/0) != 5"), 0.0);
    assert_eq!(ev("(0/0) < 5"), 1.0);
    assert_eq!(ev("(0/0) > 5"), 1.0);
    assert_eq!(ev("(0/0) <= 5"), 0.0);
    assert_eq!(ev("(0/0) >= 5"), 0.0);
    // Inf - Inf is NaN, so infinities compare equal through `==`
    assert_eq!(ev("(1/0) == (1/0)"), 1.0);
    assert_eq!(ev("(1/0) == 5"), 0.0);
}

#[test]
fn mod_by_zero_is_zero() {
    assert_eq!(ev("7 % 0"), 0.0);
    assert_eq!(ev("7 % 0.9"), 0.0);
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "x = 7; x %= 0; x"), 0.0);
}

#[test]
fn mod_uses_absolute_truncated_operands() {
    // nseel_asm_mod: fabs both, fistp (chop) to 32 bits, unsigned divl.
    assert_eq!(ev("7 % 3"), 1.0);
    assert_eq!(ev("7.9 % 3.9"), 1.0);
    assert_eq!(ev("(-7) % 3"), 1.0);
    // `%` binds only the adjacent primary: this is -(7 % 3)
    assert_eq!(ev("-7 % 3"), -1.0);
    assert_eq!(ev("7 % -3"), 1.0);
    // out of i32 range (and NaN) is x87's integer indefinite, 0x80000000
    assert_eq!(ev("(0/0) % 3"), (2_147_483_648u32 % 3) as f64);
    assert_eq!(ev("5 % (0/0)"), 5.0);
    assert_eq!(ev("1e10 % 1000"), (2_147_483_648u32 % 1000) as f64);
}

#[test]
fn bitops_truncate_operands() {
    assert_eq!(ev("7.9 & 3.2"), 3.0);
    assert_eq!(ev("4.9 | 3.9"), 7.0);
    assert_eq!(ev("-1.5 & 3"), 3.0);
    // nseel_asm_and/or use 64-bit fistp, so values past i32 survive
    assert_eq!(ev("4294967296 | 1"), 4_294_967_297.0);
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "x = 12.7; x &= 10; x"), 8.0);
    assert_eq!(run_on(&mut vm, "x |= 1.9; x"), 9.0);
}

#[test]
fn division_by_zero_is_ieee_not_panic() {
    assert_eq!(ev("1/0"), f64::INFINITY);
    assert_eq!(ev("-1/0"), f64::NEG_INFINITY);
    assert!(ev("0/0").is_nan());
}

#[test]
fn assignment_stores_zero_for_denormal_inf_nan() {
    // nseel_asm_assign: exponent bits all 0 or all 1 => store 0
    let mut vm = vm();
    for src in ["1/0", "-1/0", "0/0", "1e-300 * 1e-10", "-(0)"] {
        let p = format!("x = 5; x = {src}; x");
        let v = run_on(&mut vm, &p);
        assert_eq!(v.to_bits(), 0.0f64.to_bits(), "{p}");
    }
    // the value of the assignment itself is the unfiltered right side
    assert_eq!(run_on(&mut vm, "x = 1/0"), f64::INFINITY);
    assert_eq!(get(&mut vm, "x"), 0.0);
    // megabuf stores go through the same _set
    assert_eq!(run_on(&mut vm, "megabuf(5) = 1/0; megabuf(5)"), 0.0);
    assert_eq!(run_on(&mut vm, "gmegabuf(5) = 0/0; gmegabuf(5)"), 0.0);
    // ordinary values store unchanged
    assert_eq!(run_on(&mut vm, "x = 1e-300; x"), 1e-300);
}

#[test]
fn compound_assignment_does_not_filter() {
    // _addop.._powop store with plain fstp: no exponent check.
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "x = 1; x /= 0; x"), f64::INFINITY);
    assert_eq!(
        run_on(&mut vm, "megabuf(1) = 1; megabuf(1) /= 0; megabuf(1)"),
        f64::INFINITY
    );
}

#[test]
fn compound_assignments_compute_target_op_value() {
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "x = 10; x += 2"), 12.0);
    assert_eq!(run_on(&mut vm, "x -= 5; x"), 7.0);
    assert_eq!(run_on(&mut vm, "x *= 3"), 21.0);
    assert_eq!(run_on(&mut vm, "x /= 2"), 10.5);
    assert_eq!(run_on(&mut vm, "x %= 4"), 2.0);
    assert_eq!(run_on(&mut vm, "x ^= 3"), 8.0);
    assert_eq!(
        run_on(&mut vm, "megabuf(3) = 2; megabuf(3) += 5; megabuf(3) *= 2"),
        14.0
    );
    assert_eq!(run_on(&mut vm, "gmegabuf(3) = 2; gmegabuf(3) -= 5"), -3.0);
}

#[test]
fn megabuf_and_gmegabuf_round_trip() {
    let mut vm = vm();
    assert_eq!(
        run_on(&mut vm, "megabuf(10) = 4; megabuf(10) + megabuf(11)"),
        4.0
    );
    assert_eq!(run_on(&mut vm, "x[2] = 9; megabuf(x + 2)"), 9.0);
    assert_eq!(run_on(&mut vm, "gmem[7] = 3; gmegabuf(7)"), 3.0);
    assert_eq!(vm.megabuf.get(10.0), 4.0);
    assert_eq!(vm.global.borrow().gmegabuf.get(7.0), 3.0);
    // out of range: reads 0, write ignored, value is the right side
    assert_eq!(run_on(&mut vm, "megabuf(-5) = 2"), 2.0);
    assert_eq!(run_on(&mut vm, "megabuf(-5)"), 0.0);
}

#[test]
fn gmem_and_regs_read_while_written_do_not_panic() {
    let global = Rc::new(RefCell::new(GlobalMem::new()));
    let mut a = Vm::new(global.clone());
    let mut b = Vm::new(global.clone());
    global.borrow_mut().regs[0] = 2.0;
    let src = "gmegabuf(1) = gmegabuf(1) + reg00; reg00 = gmegabuf(1); \
               reg01 += reg00 + gmegabuf(1); gmegabuf(2) += gmegabuf(1) + reg01";
    let pa = Program::compile(&mut a, src).unwrap();
    let pb = Program::compile(&mut b, src).unwrap();
    pa.run(&mut a);
    assert_eq!(global.borrow().gmegabuf.get(1.0), 2.0);
    assert_eq!(global.borrow().regs[0], 2.0);
    pb.run(&mut b);
    assert_eq!(global.borrow().gmegabuf.get(1.0), 4.0);
    assert_eq!(global.borrow().regs[0], 4.0);
    assert_eq!(global.borrow().regs[1], 4.0 + 8.0);
}

#[test]
fn loop_count_is_floored_and_capped() {
    let mut vm = vm();
    run_on(&mut vm, "i = 0; loop(2.9, i += 1)");
    assert_eq!(get(&mut vm, "i"), 2.0);
    run_on(&mut vm, "i = 0; loop(0.99, i += 1)");
    assert_eq!(get(&mut vm, "i"), 0.0);
    run_on(&mut vm, "i = 0; loop(-3, i += 1)");
    assert_eq!(get(&mut vm, "i"), 0.0);
    // fistp of a count past i32 range gives 0x80000000 (< 1): no iterations
    run_on(&mut vm, "i = 0; loop(3e9, i += 1)");
    assert_eq!(get(&mut vm, "i"), 0.0);
    run_on(&mut vm, "i = 0; loop(2000000, i += 1)");
    assert_eq!(get(&mut vm, "i"), 1_048_576.0);
}

#[test]
fn loop_value_is_last_body_value_or_the_count() {
    // nseel_asm_repeat leaves eax at the body's result, or at the count
    // when the body never runs.
    assert_eq!(ev("i = 0; loop(3, i += 1)"), 3.0);
    assert_eq!(ev("loop(0.5, 7)"), 0.5);
    assert_eq!(ev("loop(-2, 7)"), -2.0);
}

#[test]
fn loop_body_count_is_evaluated_once() {
    let mut vm = vm();
    run_on(&mut vm, "n = 3; i = 0; loop(n, n += 1; i += 1)");
    assert_eq!(get(&mut vm, "i"), 3.0);
    assert_eq!(get(&mut vm, "n"), 6.0);
}

#[test]
fn while_repeats_while_body_is_true() {
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "i = 0; while(i += 1; i < 5)"), 0.0);
    assert_eq!(get(&mut vm, "i"), 5.0);
    // body runs at least once
    run_on(&mut vm, "j = 0; while(j += 1; 0)");
    assert_eq!(get(&mut vm, "j"), 1.0);
}

#[test]
fn nested_loops() {
    let mut vm = vm();
    run_on(
        &mut vm,
        "s = 0; loop(3, j = 0; loop(4, s += 1; j += 1); while(j -= 1))",
    );
    assert_eq!(get(&mut vm, "s"), 12.0);
    assert_eq!(get(&mut vm, "j"), 0.0);
}

#[test]
fn sequences_evaluate_in_order_and_return_last() {
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "a = 1; b = a + 1; b * 10"), 20.0);
    assert_eq!(run_on(&mut vm, "exec2(c = 2, c + 1)"), 3.0);
    assert_eq!(run_on(&mut vm, "exec3(c = 4, c = c * 2, c + 1)"), 9.0);
    assert_eq!(run_on(&mut vm, "(c = 1; c += 1; c * 7)"), 14.0);
    assert_eq!(ev(""), 0.0);
    assert_eq!(ev("  ;  ; "), 0.0);
}

#[test]
fn arithmetic_and_precedence() {
    assert_eq!(ev("1 + 2 * 3"), 7.0);
    assert_eq!(ev("10 - 4 - 3"), 3.0);
    assert_eq!(ev("2 ^ 3 ^ 2"), 64.0);
    assert_eq!(ev("-2 ^ 2"), -4.0);
    assert_eq!(ev("-(3)"), -3.0);
    assert_eq!(ev("$x1F + $pi * 0"), 31.0);
}

#[test]
fn builtins_dispatch() {
    assert_eq!(ev("sqr(3)"), 9.0);
    assert_eq!(ev("sqrt(-16)"), 4.0); // nseel_asm_sqrt takes fabs first
    assert_eq!(ev("abs(-2)"), 2.0);
    assert_eq!(ev("min(3, 2)"), 2.0);
    assert_eq!(ev("max(3, 2)"), 3.0);
    assert_eq!(ev("floor(-1.5)"), -2.0);
    assert_eq!(ev("int(2.7)"), 2.0);
    assert_eq!(ev("ceil(1.2)"), 2.0);
    assert_eq!(ev("sign(-3)"), -1.0);
    assert_eq!(ev("sign(0)"), 0.0);
    assert_eq!(ev("pow(2, 10)"), 1024.0);
    assert_eq!(ev("atan2(1, 1)"), std::f64::consts::FRAC_PI_4);
    assert_eq!(ev("sin(0) + cos(0)"), 1.0);
    assert!((ev("invsqrt(4)") - 0.5).abs() < 0.002);
    assert_eq!(ev("sigmoid(0, 1)"), 0.5);
    assert_eq!(ev("freembuf(5)"), 5.0);
}

#[test]
fn memset_and_memcpy_return_dest() {
    let mut vm = vm();
    assert_eq!(run_on(&mut vm, "memset(10, 3, 4)"), 10.0);
    assert_eq!(run_on(&mut vm, "memcpy(20, 10, 4)"), 20.0);
    assert_eq!(run_on(&mut vm, "megabuf(23) + megabuf(24)"), 3.0);
}

#[test]
fn variables_persist_across_runs_and_programs() {
    let mut vm = vm();
    let p = Program::compile(&mut vm, "t = t + 1").unwrap();
    p.run(&mut vm);
    p.run(&mut vm);
    assert_eq!(p.run(&mut vm), 3.0);
    let q = Program::compile(&mut vm, "T * 2").unwrap();
    assert_eq!(q.run(&mut vm), 6.0);
}

#[test]
fn compile_reports_syntax_errors() {
    let mut vm = vm();
    let e = Program::compile(&mut vm, "x = (1 + ").unwrap_err();
    assert!(e.message.contains("syntax"), "{e}");
}

#[test]
fn compile_ast_rejects_unknown_function() {
    let mut vm = vm();
    let ast = Expr::Call {
        name: "nosuch".into(),
        args: vec![Expr::Num(1.0)],
    };
    assert!(compile_ast(&mut vm, &ast).is_err());
}

#[test]
fn lazy_forms_compile_to_jumps() {
    use crate::bytecode::Op;
    let mut vm = vm();
    let p = Program::compile(&mut vm, "if(c, 1, 2)").unwrap();
    let c = vm.var("c");
    assert_eq!(
        p.ops(),
        &[
            Op::Load(c),
            Op::JumpIfFalse(4),
            Op::PushConst(1.0),
            Op::Jump(5),
            Op::PushConst(2.0),
        ]
    );
    let p = Program::compile(&mut vm, "c && 3").unwrap();
    assert_eq!(
        p.ops(),
        &[Op::Load(c), Op::And(4), Op::PushConst(3.0), Op::Bool]
    );
}

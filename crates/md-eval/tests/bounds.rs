//! Loop caps and non-panicking arithmetic (ns-eel2 `nseel_asm_repeat`,
//! `nseel_asm_repeatwhile`; x87 division semantics).

use std::cell::RefCell;
use std::rc::Rc;

use md_eval::vm::LOOP_CAP;
use md_eval::{GlobalMem, Program, Vm};

fn vm() -> Vm {
    Vm::new(Rc::new(RefCell::new(GlobalMem::new())))
}

fn run_on(vm: &mut Vm, src: &str) -> f64 {
    let p = Program::compile(vm, src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
    p.run(vm)
}

fn ev(src: &str) -> f64 {
    run_on(&mut vm(), src)
}

fn var(vm: &mut Vm, name: &str) -> f64 {
    let v = vm.var(name);
    vm.get(v)
}

#[test]
fn loop_cap_matches_ns_eel_header() {
    assert_eq!(LOOP_CAP, 1_048_576);
}

#[test]
fn while_is_capped_at_1_048_576_iterations() {
    // repeatwhile: ecx = cap; run body; leave when falsy; decl ecx / jnz.
    // The body therefore runs exactly LOOP_CAP times when always true.
    let mut vm = vm();
    run_on(&mut vm, "i=0; while(i=i+1)");
    assert_eq!(var(&mut vm, "i"), f64::from(LOOP_CAP));
}

#[test]
fn while_stops_when_body_goes_false() {
    let mut vm = vm();
    // body value is the last statement; falsy after 5 passes
    let v = run_on(&mut vm, "i=0; while(i=i+1; i<5)");
    assert_eq!(var(&mut vm, "i"), 5.0);
    assert_eq!(v, 0.0);
}

#[test]
fn nested_loop_cap_applies_per_loop() {
    let mut vm = vm();
    run_on(&mut vm, "n=0; loop(2, i=0; while(i=i+1); n += i)");
    assert_eq!(var(&mut vm, "n"), 2.0 * f64::from(LOOP_CAP));
}

#[test]
fn loop_count_out_of_range_runs_zero_times_and_large_count_is_capped() {
    let mut vm = vm();
    run_on(&mut vm, "i=0; loop(1e12, i += 1)");
    assert_eq!(var(&mut vm, "i"), 0.0, "out-of-range count runs zero times");
    run_on(&mut vm, "i=0; loop(2000000, i += 1)");
    assert_eq!(var(&mut vm, "i"), f64::from(LOOP_CAP));
}

#[test]
fn loop_negative_count_runs_zero_times() {
    let mut vm = vm();
    run_on(&mut vm, "i=0; loop(-5, i += 1)");
    assert_eq!(var(&mut vm, "i"), 0.0);
    run_on(&mut vm, "i=0; loop(0/0, i += 1)");
    assert_eq!(var(&mut vm, "i"), 0.0);
    run_on(&mut vm, "i=0; loop(0, i += 1)");
    assert_eq!(var(&mut vm, "i"), 0.0);
}

#[test]
fn division_by_zero_yields_inf_not_panic() {
    assert_eq!(ev("1/0"), f64::INFINITY);
    assert_eq!(ev("-1/0"), f64::NEG_INFINITY);
    assert!(ev("0/0").is_nan());
    assert_eq!(ev("x=0; 5/x"), f64::INFINITY);
    // compound division by zero stores raw (only plain `=` filters)
    assert_eq!(ev("a=2; a/=0; a"), f64::INFINITY);
    // `%` by zero: nseel_asm_mod skips the `div` when the chopped |divisor|
    // is 0 and returns edx = 0 (Task 1.4 `%` ruling). A divisor that chops
    // to 0 counts as zero too; compound `%=` takes the same path.
    assert_eq!(ev("5 % 0").to_bits(), 0.0f64.to_bits());
    assert_eq!(ev("5 % 0.5").to_bits(), 0.0f64.to_bits());
    assert_eq!(ev("(-7) % (-0.9)").to_bits(), 0.0f64.to_bits());
    assert_eq!(ev("a=5; a %= 0; a").to_bits(), 0.0f64.to_bits());
}

#[test]
fn nan_is_falsy() {
    assert_eq!(ev("(0/0) ? 1 : 2"), 2.0);
    assert_eq!(ev("if(0/0, 1, 2)"), 2.0);
    assert_eq!(ev("(0/0) && 1"), 0.0);
    assert_eq!(ev("!(0/0)"), 1.0);
    // a NaN body ends a while loop
    assert_eq!(ev("i=0; while(i=i+1; 0/0); i"), 1.0);
}

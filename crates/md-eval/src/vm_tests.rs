use crate::mem::{GlobalMem, MegaBuf};
use crate::vm::Vm;
use std::cell::RefCell;
use std::rc::Rc;

fn vm() -> Vm {
    Vm::new(Rc::new(RefCell::new(GlobalMem::new())))
}

#[test]
fn var_lookup_is_case_insensitive() {
    let mut vm = vm();
    let a = vm.var("Zoom");
    let b = vm.var("zOOM");
    assert_eq!(a, b);
    vm.set(a, 2.5);
    assert_eq!(vm.get(b), 2.5);
    assert_ne!(vm.var("zoom2"), a);
}

#[test]
fn new_var_starts_at_zero() {
    let mut vm = vm();
    let v = vm.var("fresh");
    assert_eq!(vm.get(v), 0.0);
}

#[test]
fn var_names_compare_on_first_16_characters() {
    // NSEEL_MAX_VARIABLE_NAMELEN = 16 (ns-eel.h); register_var uses strnicmp(.., 16).
    let mut vm = vm();
    let a = vm.var("abcdefghijklmnop"); // exactly 16
    let b = vm.var("ABCDEFGHIJKLMNOPQRST"); // same first 16
    assert_eq!(a, b);
    let c = vm.var("abcdefghijklmno"); // 15 chars: different
    assert_ne!(a, c);
    vm.set(a, 7.0);
    assert_eq!(vm.get(b), 7.0);
    assert_eq!(vm.get(c), 0.0);
}

#[test]
fn reg_vars_are_shared_between_vms() {
    let g = Rc::new(RefCell::new(GlobalMem::new()));
    let mut a = Vm::new(g.clone());
    let mut b = Vm::new(g.clone());
    let ra = a.var("reg07");
    a.set(ra, 3.5);
    let rb = b.var("REG07");
    assert_eq!(b.get(rb), 3.5);
    assert_eq!(g.borrow().regs[7], 3.5);
    b.set(rb, -1.0);
    assert_eq!(a.get(ra), -1.0);
    // Only exactly reg + two digits is a register.
    let plain = a.var("reg7");
    assert_eq!(a.get(plain), 0.0);
    a.set(plain, 9.0);
    assert_eq!(g.borrow().regs[7], -1.0);
    let r99 = a.var("reg99");
    a.set(r99, 1.0);
    assert_eq!(g.borrow().regs[99], 1.0);
}

#[test]
fn megabuf_out_of_range_reads_zero_and_ignores_writes() {
    let mut m = MegaBuf::new();
    m.set(-2.0, 5.0);
    m.set(8_388_608.0, 5.0);
    m.set(1e300, 5.0);
    m.set(f64::NAN, 5.0);
    assert_eq!(m.allocated_blocks(), 0);
    assert_eq!(m.get(-2.0), 0.0);
    assert_eq!(m.get(8_388_608.0), 0.0);
    assert_eq!(m.get(f64::NAN), 0.0);
    m.set(8_388_607.0, 4.0);
    assert_eq!(m.get(8_388_607.0), 4.0);
    assert_eq!(m.get(8_388_608.0), 0.0);
}

#[test]
fn megabuf_reads_do_not_allocate_and_writes_allocate_one_block() {
    let mut m = MegaBuf::new();
    assert_eq!(m.get(100_000.0), 0.0);
    assert_eq!(m.allocated_blocks(), 0);
    m.set(100_000.0, 1.0);
    assert_eq!(m.allocated_blocks(), 1);
    assert_eq!(m.get(65_536.0), 0.0);
}

#[test]
fn megabuf_index_truncates_after_adding_1e5() {
    // _asm_megabuf: fld i; fadd g_closefact(0.00001); fistp under chop rounding.
    let mut m = MegaBuf::new();
    m.set(3.0, 33.0);
    m.set(2.0, 22.0);
    assert_eq!(m.get(2.999995), 33.0); // 3.000005 -> 3
    assert_eq!(m.get(2.99998), 22.0); // 2.99999 -> 2
    assert_eq!(m.get(3.9), 33.0); // truncation, not rounding
                                  // Truncation is toward zero, so (-1, 0) after the epsilon is slot 0.
    m.set(0.0, 5.0);
    assert_eq!(m.get(-0.5), 5.0);
    assert_eq!(m.get(-1.0), 5.0); // -0.99999 -> 0
    assert_eq!(m.get(-1.5), 0.0); // -1.49999 -> -1: out of range
    m.set(-0.25, 6.0);
    assert_eq!(m.get(0.0), 6.0);
}

#[test]
fn gmegabuf_is_flat_1m_and_wraps_indices() {
    // __NSEEL_RAMAllocGMEM with no gram blocks: static buffer, w & (2^20 - 1).
    let mut g = GlobalMem::new();
    assert_eq!(g.gmegabuf.get(5.0), 0.0);
    g.gmegabuf.set(5.0, 9.0);
    assert_eq!(g.gmegabuf.get(5.0), 9.0);
    assert_eq!(g.gmegabuf.get(5.0 + 1_048_576.0), 9.0);
    g.gmegabuf.set(-3.0, 4.0); // -2.99999 truncates to -2 -> slot 1048574
    assert_eq!(g.gmegabuf.get(1_048_574.0), 4.0);
    g.gmegabuf.set(-1.5, 2.0); // -1.49999 -> -1 -> slot 1048575
    assert_eq!(g.gmegabuf.get(1_048_575.0), 2.0);
    g.gmegabuf.set(f64::NAN, 3.0); // x87 indefinite 0x80000000 -> slot 0
    assert_eq!(g.gmegabuf.get(0.0), 3.0);
}

#[test]
fn gmegabuf_is_shared_between_vms() {
    let g = Rc::new(RefCell::new(GlobalMem::new()));
    let (a, b) = (Vm::new(g.clone()), Vm::new(g.clone()));
    a.global.borrow_mut().gmegabuf.set(10.0, 1.25);
    assert_eq!(b.global.borrow().gmegabuf.get(10.0), 1.25);
}

#[test]
fn reset_vars_zeroes_and_frees() {
    let g = Rc::new(RefCell::new(GlobalMem::new()));
    let mut vm = Vm::new(g.clone());
    let v = vm.var("x");
    let r = vm.var("reg00");
    vm.set(v, 4.0);
    vm.set(r, 8.0);
    vm.megabuf.set(10.0, 1.0);
    vm.reset_vars();
    assert_eq!(vm.get(v), 0.0);
    assert_eq!(vm.megabuf.get(10.0), 0.0);
    assert_eq!(vm.megabuf.allocated_blocks(), 0);
    assert_eq!(vm.get(r), 8.0); // shared register is not VM state
    assert_eq!(vm.var("x"), v);
}

#[test]
fn free_drops_blocks_starting_at_or_above_index() {
    // freembuf(n): blocks x with x*65536 >= n go; a block straddling n stays.
    let mut m = MegaBuf::new();
    for b in 0..4 {
        m.set(f64::from(b) * 65_536.0 + 1.0, 1.0);
    }
    m.free(65_537.0); // block 1 starts below 65537: kept; blocks 2,3 freed
    assert_eq!(m.allocated_blocks(), 2);
    assert_eq!(m.get(65_537.0), 1.0);
    assert_eq!(m.get(2.0 * 65_536.0 + 1.0), 0.0);
    m.free(65_536.0); // exact block start: freed
    assert_eq!(m.get(65_537.0), 0.0);
    assert_eq!(m.allocated_blocks(), 1);
    m.free(8_388_608.0); // at the bound: no-op
    assert_eq!(m.allocated_blocks(), 1);
    m.free(-5.0); // negative clamps to 0: everything
    assert_eq!(m.allocated_blocks(), 0);
}

#[test]
fn copy_trims_negative_ranges_and_clamps_at_bound() {
    let mut m = MegaBuf::new();
    for i in 0..4 {
        m.set(f64::from(i), f64::from(i) + 1.0);
    }
    // src -2.5 + 0.0001 truncates to -2: first two elements dropped, dst
    // shifts by 2. (A plain -2.0 would truncate to -1 under chop rounding.)
    m.copy(100.0, -2.5, 6.0);
    assert_eq!(m.get(100.0), 0.0);
    assert_eq!(m.get(101.0), 0.0);
    assert_eq!(m.get(102.0), 1.0);
    assert_eq!(m.get(105.0), 4.0);
    // dst below zero trims the front of both.
    m.copy(-1.5, 0.0, 3.0);
    assert_eq!(m.get(0.0), 2.0);
    assert_eq!(m.get(1.0), 3.0);
    // Copy that runs past the end writes only what fits.
    m.set(8_388_607.0, 7.0);
    m.copy(8_388_600.0, 0.0, 100.0);
    assert_eq!(m.get(8_388_600.0), 2.0);
    assert_eq!(m.get(8_388_607.0), 0.0);
    // Destination wholly out of range: nothing happens.
    m.copy(8_388_608.0, 0.0, 4.0);
    assert_eq!(m.get(8_388_608.0), 0.0);
}

#[test]
fn copy_overlapping_within_block_has_memmove_semantics() {
    let mut m = MegaBuf::new();
    for i in 0..5 {
        m.set(f64::from(i), f64::from(i) + 1.0);
    }
    m.copy(2.0, 0.0, 5.0); // forward overlap
    let got: Vec<f64> = (0..7).map(|i| m.get(f64::from(i))).collect();
    assert_eq!(got, [1.0, 2.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    m.copy(0.0, 1.0, 6.0); // backward overlap
    let got: Vec<f64> = (0..7).map(|i| m.get(f64::from(i))).collect();
    assert_eq!(got, [2.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0]);
}

#[test]
fn copy_crosses_block_boundaries_and_uses_epsilon() {
    let mut m = MegaBuf::new();
    m.set(65_535.0, 1.0);
    m.set(65_536.0, 2.0);
    m.copy(200_000.0, 65_535.0, 2.0);
    assert_eq!(m.get(200_000.0), 1.0);
    assert_eq!(m.get(200_001.0), 2.0);
    // Unallocated source zeroes an allocated destination, allocates nothing.
    let before = m.allocated_blocks();
    m.copy(200_000.0, 1_000_000.0, 2.0);
    assert_eq!(m.get(200_000.0), 0.0);
    assert_eq!(m.allocated_blocks(), before);
    // len 1.99995 + 0.0001 truncates to 2 (bulk epsilon is 0.0001).
    m.set(0.0, 5.0);
    m.set(1.0, 6.0);
    m.copy(10.0, 0.0, 1.99995);
    assert_eq!(m.get(11.0), 6.0);
}

#[test]
fn fill_trims_clamps_and_spans_blocks() {
    let mut m = MegaBuf::new();
    m.fill(65_530.0, 3.0, 12.0); // spans blocks 0 and 1
    assert_eq!(m.get(65_529.0), 0.0);
    assert_eq!(m.get(65_530.0), 3.0);
    assert_eq!(m.get(65_541.0), 3.0);
    assert_eq!(m.get(65_542.0), 0.0);
    m.fill(-2.5, 8.0, 4.0); // start -2 trims len to 2: slots 0,1
    assert_eq!(m.get(0.0), 8.0);
    assert_eq!(m.get(1.0), 8.0);
    assert_eq!(m.get(2.0), 0.0);
    m.fill(8_388_600.0, 9.0, 1000.0); // clamped to the bound
    assert_eq!(m.get(8_388_607.0), 9.0);
    m.fill(8_388_608.0, 1.0, 5.0); // wholly outside
    m.fill(10.0, 1.0, 0.0); // zero length
    m.fill(10.0, 1.0, -3.0); // negative length
    assert_eq!(m.get(10.0), 0.0);
    m.fill(20.0, 4.0, 2.99995); // +0.0001 -> 3
    assert_eq!(m.get(22.0), 4.0);
    assert_eq!(m.get(23.0), 0.0);
}

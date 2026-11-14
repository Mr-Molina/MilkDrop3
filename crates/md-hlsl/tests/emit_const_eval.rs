//! Folded into Task 4.9 (ledger, Tasks 4.6/4.7 deferred): operations whose
//! operands WGSL would const-evaluate into a shader-creation error — shifts
//! by 32 or more (or a negative amount), and integer `/`/`%` by a constant
//! zero (or `INT_MIN / -1`) — must still emit a module naga accepts. fxc
//! folded these with a warning; the emitter moves the right operand into a
//! `var`, so WGSL's runtime rules give the value (see `emit_expr.rs`).
//! Run with `cargo test -p md-hlsl --test emit_const_eval`.

mod common;

use common::{emit_ok, has};

#[test]
fn constant_shift_of_32_or_more_validates() {
    // `1 << 40` overflowed naga's constant evaluation; at run time WGSL
    // takes the amount modulo 32, so the value is `1 << 8` = 256.
    let w = emit_ok("", "int a = 1 << 40; _return_value = a;");
    has(&w, &["var md_t0: i32 = 40i;", "(1i << u32(md_t0))"]);
    emit_ok("", "int a = 1 >> 40; _return_value = a;");
    emit_ok("", "int a = 3 << 31; _return_value = a;");
    emit_ok("", "int a = 1 << -1; _return_value = a;");
    emit_ok("", "int2 v = int2(1, 2) << 40; _return_value = v.xyxy;");
    emit_ok("static const int N = 1 << 40;", "_return_value = N;");
    emit_ok(
        "static const int S = 33;",
        "int a = 7 >> S; _return_value = a;",
    );
}

#[test]
fn compound_shift_by_constant_32_or_more_validates() {
    // A runtime left operand with a constant amount >= 32 is rejected by
    // naga's validator, not its constant evaluator.
    emit_ok(
        "",
        "int a = (int)_c0.x; a <<= 40; a >>= 33; _return_value = a;",
    );
    emit_ok(
        "",
        "int a = (int)_c0.x; int b = a << 32; _return_value = b;",
    );
}

#[test]
fn in_range_literal_shift_stays_inline() {
    let w = emit_ok("", "int a = (int)_c0.x; int b = a << 3; _return_value = b;");
    has(&w, &["(h_a << u32(3i))"]);
}

#[test]
fn integer_division_by_a_constant_zero_validates() {
    // `static const int N = 0` is a WGSL `const`; `5 / N` failed naga's
    // constant evaluation and `x / N` its validator. At run time WGSL gives
    // `x / 0 = x` and `x % 0 = 0`.
    let w = emit_ok(
        "static const int N = 0;",
        "int a = 5 / N; _return_value = a;",
    );
    has(&w, &["var md_t0: i32 = h_N;", "(5i / md_t0)"]);
    emit_ok(
        "static const int N = 0;",
        "int a = 5 % N; _return_value = a;",
    );
    emit_ok(
        "static const int N = 0;",
        "int x = (int)_c0.x; int a = x / N + x % N; _return_value = a;",
    );
    emit_ok(
        "static const int N = 0;",
        "int a = 5; a /= N; a %= N; a /= 0; _return_value = a;",
    );
    emit_ok(
        "static const int N = 0;",
        "int2 v = int2(4, 5) / N; _return_value = v.xyxy;",
    );
}

#[test]
fn integer_min_divided_by_minus_one_validates() {
    emit_ok("", "int a = (-2147483647 - 1) / -1; _return_value = a;");
    emit_ok("", "int a = (-2147483647 - 1) % -1; _return_value = a;");
}

#[test]
fn float_division_by_a_constant_zero_validates() {
    // A float `const` zero divisor made naga fold `1.0 / 0.0` to an
    // infinite constant (an error); at run time it is IEEE inf/NaN, as the
    // SM2/3 float hardware produced.
    emit_ok(
        "static const float Z = 0;",
        "float f = 1 / Z; _return_value = f;",
    );
    emit_ok("static const float Z = 0;", "_return_value = fmod(5, Z);");
    emit_ok(
        "static const float Z = 0;",
        "float f = 2; f /= Z; _return_value = f;",
    );
}

#[test]
fn ordinary_literal_divisors_stay_inline() {
    let w = emit_ok(
        "",
        "int i = (int)_c0.x; int a = i / 2 + i % 3; float f = _c0.y / 2; _return_value = a + f;",
    );
    has(&w, &["(h_i / 2i)", "(h_i % 3i)", "(U._c0.y / 2.0)"]);
}

/// Found by Task 4.10's typed soup: an intrinsic call whose arguments are
/// all constants is either left as an AbstractFloat call naga cannot
/// concretize (`mix(2.0, 1.0, 0.5)`: "Function 'h_PS' is invalid") or
/// const-evaluated into an error (`exp2(1e30)` overflows, `pow(-1, 0.5)`
/// is out of range). The arguments go through `var`s, so the call is
/// evaluated at run time (IEEE inf/NaN, as the SM2/3 hardware gave).
#[test]
fn all_constant_intrinsic_calls_validate() {
    let w = emit_ok("", "_return_value = lerp(2, 1, 0.5);");
    has(&w, &["var md_t0: f32 = 2.0;", "mix(md_t0, md_t1, md_t2)"]);
    emit_ok("", "int i = 1; i *= lerp(2, 1, 0.5); _return_value = i;");
    emit_ok("", "_return_value = ldexp(1, 1e30);");
    emit_ok("", "_return_value = exp2(1e30);");
    emit_ok("", "_return_value = pow(-1, 0.5);");
    emit_ok(
        "",
        "_return_value = saturate(lerp(40, 1e30, 0.5)) + frac(lerp(0.5, 2, 0.5));",
    );
    emit_ok("", "_return_value = lerp(lerp(2, 40, 0.5), sign(0), 0.5);");
    emit_ok(
        "static const float Z = 0;",
        "_return_value = lerp(Z, step(40, 0), 0.5) + !lerp(0.5, 0, 0.5);",
    );
    emit_ok(
        "",
        "float3x3 m3 = 2; m3 %= lerp(1, 1e30, 0.5); _return_value = m3[0].xyzz;",
    );
}

#[test]
fn intrinsic_calls_with_a_runtime_argument_stay_inline() {
    let w = emit_ok(
        "",
        "_return_value = lerp(_uv.x, 1, 0.5) + pow(_rad_ang.x, 2);",
    );
    has(&w, &["mix(h__uv.x, 1.0, 0.5)", "pow(h__rad_ang.x, 2.0)"]);
}

/// Found by Task 4.10's typed soup: `(2 ? 0.5 : 40)` emitted
/// `select(40.0, 0.5, true)`, an AbstractFloat `select` naga leaves
/// unconcretized; all-constant operands now go through `var`s too.
#[test]
fn all_constant_ternary_validates() {
    let w = emit_ok("", "_return_value = floor(lerp((2 ? 0.5 : 40), 0.5, 0.5));");
    has(&w, &["select(md_t2, md_t1, md_t0)"]);
    emit_ok("", "_return_value = (true ? float2(1, 2) : 3).xyxy;");
    let w = emit_ok("", "_return_value = _c0.x > 0 ? 0.5 : 40;");
    has(&w, &["select(40.0, 0.5, (U._c0.x > 0.0))"]);
}

//! Type checker tests (Task 4.3). Run with `cargo test -p md-hlsl --test typeck`.

use md_hlsl::ast::{BinOp, Lit};
use md_hlsl::typed::*;
use md_hlsl::types::{Scalar, Ty};
use md_hlsl::{check, parse, preprocess_tokens, TranslateError};

const F3: Ty = Ty::Vec(Scalar::Float, 3);
const F4: Ty = Ty::Vec(Scalar::Float, 4);
const F2: Ty = Ty::Vec(Scalar::Float, 2);

fn try_check(src: &str) -> Result<TypedModule, TranslateError> {
    check(&parse(&preprocess_tokens(src)?)?)
}

fn check_ok(src: &str) -> TypedModule {
    try_check(src).unwrap_or_else(|e| panic!("{e}\n--- source:\n{src}"))
}

fn check_err(src: &str) -> TranslateError {
    match try_check(src) {
        Ok(m) => panic!("expected an error, got {m:?}\n--- source:\n{src}"),
        Err(e) => e,
    }
}

fn func<'a>(m: &'a TypedModule, name: &str) -> &'a TFunction {
    m.decls
        .iter()
        .find_map(|d| match d {
            TDecl::Function(f) if f.name == name => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no function {name}"))
}

/// Statements of `void f() { <body> }` with the given globals before it.
fn body_with(globals: &str, body: &str) -> (Vec<TStmt>, Vec<Warning>) {
    let m = check_ok(&format!("{globals}\nvoid f() {{ {body} }}"));
    (func(&m, "f").body.stmts.clone(), m.warnings)
}

fn body(body: &str) -> Vec<TStmt> {
    body_with("", body).0
}

fn local(s: &TStmt) -> &TLocal {
    match s {
        TStmt::Local(l) => l,
        s => panic!("not a local: {s:?}"),
    }
}

fn expr_stmt(s: &TStmt) -> &TExpr {
    match s {
        TStmt::Expr(e) => e,
        s => panic!("not an expression statement: {s:?}"),
    }
}

/// The value of the last `x = <e>;` statement in the body.
fn last_expr(stmts: &[TStmt]) -> &TExpr {
    expr_stmt(stmts.last().unwrap())
}

fn is_lit_cast(e: &TExpr, to: Ty, lit: Lit) -> bool {
    e.ty == to
        && matches!(&e.kind, TExprKind::Cast(x) if matches!(x.kind, TExprKind::Lit(l) if l == lit))
}

#[test]
fn scalar_splats_into_vector_assignment() {
    let stmts = body("float3 ret = 0; ret *= 0.5; ret = 1;");
    // float3 ret = 0;  ->  Splat(Cast<float>(0))
    let init = local(&stmts[0]).init.as_ref().unwrap();
    assert_eq!(init.ty, F3);
    match &init.kind {
        TExprKind::Splat(x) => assert!(is_lit_cast(x, Ty::Float, Lit::Int(0)), "{x:?}"),
        k => panic!("{k:?}"),
    }
    // ret *= 0.5;  ->  Assign(Mul, ret, Splat(0.5))
    match &expr_stmt(&stmts[1]).kind {
        TExprKind::Assign {
            op,
            op_ty,
            lhs,
            rhs,
        } => {
            assert_eq!(*op, Some(BinOp::Mul));
            assert_eq!(*op_ty, F3);
            assert_eq!(lhs.ty, F3);
            assert_eq!(rhs.ty, F3);
            match &rhs.kind {
                TExprKind::Splat(x) => {
                    assert_eq!(x.kind, TExprKind::Lit(Lit::Float(0.5)));
                    assert_eq!(x.ty, Ty::Float);
                }
                k => panic!("{k:?}"),
            }
        }
        k => panic!("{k:?}"),
    }
    // Splat in arithmetic: ret = ret * 2 + 1  — operands of the vector ops are float3.
    let stmts = body("float3 ret = 0; ret = ret * 2 + 1;");
    let TExprKind::Assign { rhs, .. } = &last_expr(&stmts).kind else {
        panic!()
    };
    let TExprKind::Binary {
        op: BinOp::Add,
        lhs,
        rhs: one,
    } = &rhs.kind
    else {
        panic!("{rhs:?}")
    };
    assert_eq!((lhs.ty.clone(), one.ty.clone()), (F3, F3));
    assert!(matches!(one.kind, TExprKind::Splat(_)));
}

#[test]
fn vector_truncation_warns_not_errors() {
    let (stmts, warnings) = body_with(
        "float g(float2 v) { return v.x; }",
        "float3 a = float4(1, 2, 3, 4); float b = g(a);",
    );
    let init = local(&stmts[0]).init.as_ref().unwrap();
    assert_eq!(init.ty, F3);
    match &init.kind {
        TExprKind::Swizzle { base, comps } => {
            assert_eq!(comps, &[0, 1, 2]);
            assert_eq!(base.ty, F4);
        }
        k => panic!("{k:?}"),
    }
    // call argument float3 -> float2 truncates too
    let call = local(&stmts[1]).init.as_ref().unwrap();
    let TExprKind::Call { args, .. } = &call.kind else {
        panic!("{call:?}")
    };
    assert_eq!(args[0].ty, F2);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].message.contains("truncation"), "{warnings:?}");
    assert_eq!(warnings[0].pos.line, 2);
    // widening is an error
    let e = check_err("void f() { float2 a = 1; float3 b = a; }");
    assert!(
        e.message.contains("float2") && e.message.contains("float3"),
        "{e}"
    );
}

#[test]
fn mul_vec_mat_result_type() {
    let globals = "float4x3 rot_s1;";
    let (stmts, w) = body_with(
        globals,
        "float3 p = mul(float4(1, 2, 3, 1), rot_s1); \
         float4 q = mul(rot_s1, float3(1, 2, 3)); \
         float3x3 r = mul((float3x3)rot_s1, (float3x3)rot_s1); \
         float2 s = mul(2, float2(1, 1));",
    );
    assert!(w.is_empty(), "{w:?}");
    let p = local(&stmts[0]).init.as_ref().unwrap();
    assert_eq!(p.ty, F3);
    assert!(matches!(
        &p.kind,
        TExprKind::Call {
            callee: Callee::Intrinsic(md_hlsl::Intrinsic::Mul),
            ..
        }
    ));
    assert_eq!(local(&stmts[1]).init.as_ref().unwrap().ty, F4);
    assert_eq!(
        local(&stmts[2]).init.as_ref().unwrap().ty,
        Ty::Mat(Scalar::Float, 3, 3)
    );
    let s = local(&stmts[3]).init.as_ref().unwrap();
    assert_eq!(s.ty, F2);
    let TExprKind::Call { args, .. } = &s.kind else {
        panic!()
    };
    assert_eq!(args[0].ty, Ty::Float, "mul(scalar, x) keeps the scalar");
    // inner-dimension mismatch truncates with a warning (fxc legacy)
    let (stmts, w) = body_with(globals, "float3 p = mul(float3(1, 2, 3), rot_s1);");
    assert_eq!(local(&stmts[0]).init.as_ref().unwrap().ty, F3);
    assert_eq!(w.len(), 1, "{w:?}");
}

#[test]
fn tex2d_returns_float4() {
    let globals = "sampler2D sampler_main; sampler3D sampler_noisevol_hq; sampler sampler_rand00;";
    let (stmts, _) = body_with(
        globals,
        "float4 a = tex2D(sampler_main, float2(0.5, 0.5)); \
         float3 b = tex2D(sampler_main, a.xy).xyz; \
         float4 c = tex3D(sampler_noisevol_hq, a.xyz); \
         float4 d = tex2Dlod(sampler_main, a); \
         float4 e = tex2D(sampler_rand00, a.xy);",
    );
    for s in &stmts {
        let init = local(s).init.as_ref().unwrap();
        if let TExprKind::Call { callee, args } = &init.kind {
            assert_eq!(init.ty, F4, "{callee:?}");
            assert!(args[0].ty.is_sampler());
        }
    }
    let e = check_err(&format!(
        "{globals} void f() {{ float4 a = tex2D(sampler_noisevol_hq, 0.5); }}"
    ));
    assert!(e.message.contains("sampler2D"), "{e}");
}

#[test]
fn unknown_identifier_errors_with_position() {
    let e = check_err("void f()\n{\n  float3 a =   nosuch * 2;\n}");
    assert_eq!((e.line, e.col), (3, 16), "{e}");
    assert!(e.message.contains("nosuch"), "{e}");
    let e = check_err("void f() {\n nofunc(1); }");
    assert_eq!((e.line, e.col), (2, 2), "{e}");
    assert!(e.message.contains("nofunc"), "{e}");
}

#[test]
fn swizzle_write_mask_type() {
    let stmts = body("float4 v = 0; v.xz = 1; v.wy = v.xx; v.b = 2;");
    let TExprKind::Assign { lhs, rhs, .. } = &expr_stmt(&stmts[1]).kind else {
        panic!()
    };
    assert_eq!(lhs.ty, F2);
    assert!(matches!(&lhs.kind, TExprKind::Swizzle { comps, .. } if comps == &[0, 2]));
    assert_eq!(rhs.ty, F2);
    assert!(matches!(rhs.kind, TExprKind::Splat(_)));
    let TExprKind::Assign { lhs, .. } = &expr_stmt(&stmts[2]).kind else {
        panic!()
    };
    assert!(matches!(&lhs.kind, TExprKind::Swizzle { comps, .. } if comps == &[3, 1]));
    let TExprKind::Assign { lhs, .. } = &expr_stmt(&stmts[3]).kind else {
        panic!()
    };
    assert_eq!(lhs.ty, Ty::Float);
    // repeated components are not a write mask
    let e = check_err("void f() { float4 v = 0;\n v.xx = 1; }");
    assert_eq!(e.line, 2, "{e}");
    assert!(e.message.contains("repeated"), "{e}");
}

#[test]
fn non_lvalue_assignment_targets_are_errors() {
    for (src, needle) in [
        (
            "void f() { float a = 0, b = 0, c = 0, d = 0, e = 0;\n a = b ? c : d = e; }",
            "lvalue",
        ),
        ("void f() { float a = 0; (a + 1) = 2; }", "lvalue"),
        ("const float4 g = 1; void f() { g = 1; }", "const"),
        ("sampler2D s; sampler2D t; void f() { s = t; }", "sampler"),
        ("static const float k = 1; void f() { k = 2; }", "const"),
        ("void f() { const float k = 1; k += 2; }", "const"),
        ("void f() { 3++; }", "lvalue"),
    ] {
        let e = check_err(src);
        assert!(e.message.contains(needle), "{src}: {e}");
    }
    // valid targets: locals, statics, params, swizzles, members, indexing
    check_ok(
        "struct S { float3 a; }; static float4 s; \
         void f(float x, out float4 o) { float a[3]; S st; x = 1; o = 0; s.y = 2; \
         a[1] = 3; st.a.zx = 1; o.w++; float4x4 m = 0; m[1] = 1; m._m00_m11 = 2; }",
    );
}

#[test]
fn literal_and_mixed_arithmetic_types() {
    let stmts = body(
        "int i = 3; float f = i / 2; float g = i * 0.5; bool b = true; \
         int j = b + b; int k = 2.7; float m = 7 % 3.0; bool2 c = float2(1, 2) < 2; \
         float4 v = 0; float3 t = v.x > 0.5 ? v.xyz : 0; float2 sel = float2(1, 0) > 0 ? v.xy : v.zw;",
    );
    // int / int stays int, then converts to float
    let f = local(&stmts[1]).init.as_ref().unwrap();
    assert_eq!(f.ty, Ty::Float);
    let TExprKind::Cast(x) = &f.kind else {
        panic!("{f:?}")
    };
    assert_eq!(x.ty, Ty::Int);
    assert_eq!(local(&stmts[2]).init.as_ref().unwrap().ty, Ty::Float);
    let j = local(&stmts[4]).init.as_ref().unwrap();
    assert_eq!(j.ty, Ty::Int);
    let k = local(&stmts[5]).init.as_ref().unwrap();
    assert!(is_lit_cast(k, Ty::Int, Lit::Float(2.7)), "{k:?}");
    let m = local(&stmts[6]).init.as_ref().unwrap();
    assert!(
        matches!(&m.kind, TExprKind::Binary { op: BinOp::Rem, lhs, .. } if lhs.ty == Ty::Float)
    );
    assert_eq!(
        local(&stmts[7]).init.as_ref().unwrap().ty,
        Ty::Vec(Scalar::Bool, 2)
    );
    let t = local(&stmts[9]).init.as_ref().unwrap();
    let TExprKind::Ternary { cond, els, .. } = &t.kind else {
        panic!()
    };
    assert_eq!((cond.ty.clone(), els.ty.clone()), (Ty::Bool, F3));
    let sel = local(&stmts[10]).init.as_ref().unwrap();
    let TExprKind::Ternary { cond, .. } = &sel.kind else {
        panic!()
    };
    assert_eq!(cond.ty, Ty::Vec(Scalar::Bool, 2));
    // vector if-condition is an error (fxc: must be scalar)
    let e = check_err("void f() { float2 v = 1;\n if (v) v = 0; }");
    assert!(e.message.contains("scalar"), "{e}");
}

#[test]
fn constructors_casts_and_initializer_lists() {
    let stmts = body(
        "float2 v2 = 1; float4 a = float4(v2, 0, 1); float3 s = float3(2); float x = float(3); \
         float2x2 m = float2x2(1, 2, 3, 4); float3x3 n = float3x3(a.xyz, a.xyz, a.xyz); \
         float4 l = {1, 2, v2}; float arr[] = {1, 2, 3}; half h = 1; double d = h; \
         float2 c = (float2)a; int3 iv = (int3)a.xyz; float4x4 z = 0;",
    );
    let a = local(&stmts[1]).init.as_ref().unwrap();
    let TExprKind::Ctor { args } = &a.kind else {
        panic!("{a:?}")
    };
    assert_eq!(args.len(), 3);
    assert_eq!(args[1].ty, Ty::Float);
    assert!(matches!(
        local(&stmts[2]).init.as_ref().unwrap().kind,
        TExprKind::Splat(_)
    ));
    assert!(matches!(
        local(&stmts[3]).init.as_ref().unwrap().kind,
        TExprKind::Cast(_)
    ));
    assert_eq!(local(&stmts[4]).ty, Ty::Mat(Scalar::Float, 2, 2));
    let l = local(&stmts[6]).init.as_ref().unwrap();
    assert!(matches!(&l.kind, TExprKind::Ctor { args } if args.len() == 3) && l.ty == F4);
    assert_eq!(local(&stmts[7]).ty, Ty::Array(Box::new(Ty::Float), 3));
    assert_eq!(local(&stmts[8]).ty, Ty::Float);
    assert_eq!(local(&stmts[9]).ty, Ty::Float);
    let c = local(&stmts[10]).init.as_ref().unwrap();
    assert!(matches!(&c.kind, TExprKind::Swizzle { comps, .. } if comps == &[0, 1]));
    assert_eq!(local(&stmts[11]).ty, Ty::Vec(Scalar::Int, 3));
    let z = local(&stmts[12]).init.as_ref().unwrap();
    assert!(matches!(z.kind, TExprKind::Splat(_)) && z.ty == Ty::Mat(Scalar::Float, 4, 4));
    for (src, needle) in [
        ("void f() { float4 a = float4(1, 2, 3); }", "components"),
        ("void f() { float2 a = {1, 2, 3}; }", "initializer"),
        ("void f() { float a[]; }", "unsized"),
        ("texture T; void f() { float4 a = T; }", "texture"),
    ] {
        let e = check_err(src);
        assert!(e.message.contains(needle), "{src}: {e}");
    }
}

#[test]
fn intrinsic_overloads() {
    let stmts = body(
        "float3 a = 1; float3 b = lerp(a, a * 2, 0.5); float c = dot(a, 1); float l = length(a); \
         int s = sign(-2.0); float3 p = pow(a, 2); float4 v = 0; float3 sat = saturate(v); \
         bool any3 = any(a); float sn, cs; sincos(1.0, sn, cs); float3 mx = max(a, 0); \
         float d1 = dot(2.0, 3); float3 nrm = normalize(a); int imx = max(1, 2); \
         float2 sat2 = saturate(v.xy);",
    );
    let init_ty = |i: usize| local(&stmts[i]).init.as_ref().unwrap().ty.clone();
    let call_args = |i: usize| match &local(&stmts[i]).init.as_ref().unwrap().kind {
        TExprKind::Call { args, .. } => args.iter().map(|a| a.ty.clone()).collect::<Vec<_>>(),
        k => panic!("{k:?}"),
    };
    // saturate(float4) -> float4, then truncated to float3 by the initializer
    let sat = local(&stmts[7]).init.as_ref().unwrap();
    assert!(matches!(&sat.kind, TExprKind::Swizzle { base, .. } if base.ty == F4));
    assert_eq!(init_ty(8), Ty::Bool);
    assert_eq!(call_args(8), vec![Ty::Vec(Scalar::Bool, 3)]);
    let TStmt::Expr(sc) = &stmts[11] else {
        panic!("{:?}", stmts[11])
    };
    assert_eq!(sc.ty, Ty::Void);
    assert_eq!(call_args(12), vec![F3, F3]);
    assert_eq!(init_ty(13), Ty::Float);
    assert_eq!(call_args(13), vec![Ty::Float, Ty::Float], "scalar dot");
    assert_eq!(init_ty(14), F3);
    assert_eq!(init_ty(15), Ty::Int);
    assert_eq!(init_ty(16), F2);
    let b = local(&stmts[1]).init.as_ref().unwrap();
    let TExprKind::Call { args, .. } = &b.kind else {
        panic!()
    };
    assert!(args.iter().all(|x| x.ty == F3));
    assert_eq!(local(&stmts[2]).init.as_ref().unwrap().ty, Ty::Float);
    assert_eq!(local(&stmts[4]).init.as_ref().unwrap().ty, Ty::Int);
    assert_eq!(local(&stmts[5]).init.as_ref().unwrap().ty, F3);
    for (src, needle) in [
        ("void f() { float a = sincos(1.0, 2.0, 3.0); }", "lvalue"),
        (
            "void f() { float3x3 m = 0; float3x3 n = abs(m); }",
            "matrix",
        ),
        ("void f() { float a = lerp(1, 2); }", "argument"),
    ] {
        let e = check_err(src);
        assert!(e.message.contains(needle), "{src}: {e}");
    }
}

#[test]
fn user_functions_overloads_defaults_and_out_params() {
    let m = check_ok(
        "float g(float x) { return x; } float g(float3 x) { return x.x; } \
         float h(float a, float b = 2) { return a + b; } \
         void o(out float r, inout float3 q) { r = 1; q *= 2; } \
         void f() { float a = g(1.0); float b = g(float3(1, 2, 3)); float c = h(1); \
         float r; float3 q = 0; o(r, q); }",
    );
    let f = func(&m, "f");
    let b = local(&f.body.stmts[1]).init.as_ref().unwrap();
    assert!(
        matches!(&b.kind, TExprKind::Call { callee: Callee::User(n), .. } if n == "g__ovl1"),
        "{b:?}"
    );
    let c = local(&f.body.stmts[2]).init.as_ref().unwrap();
    assert!(matches!(&c.kind, TExprKind::Call { args, .. } if args.len() == 2));
    assert_eq!(func(&m, "g__ovl1").hlsl_name, "g");
    let e = check_err("void o(out float r) { r = 1; } void f() { o(2.0); }");
    assert!(e.message.contains("lvalue"), "{e}");
    let e = check_err("float f(float x) { return f(x); }");
    assert!(e.message.contains("recurs"), "{e}");
}

#[test]
fn for_loop_variables_leak_like_fxc() {
    let (stmts, w) = body_with(
        "",
        "float s = 0; for (int i = 0; i < 4; i++) s += i; for (int i = 0; i < 2; i++) s += i; s += i;",
    );
    assert!(matches!(&stmts[1], TStmt::Local(l) if l.name == "i"));
    assert!(matches!(&stmts[2], TStmt::For { init, .. } if init.is_empty()));
    assert!(matches!(&stmts[3], TStmt::For { init, .. } if init.len() == 1));
    assert_eq!(w.len(), 1, "{w:?}");
}

#[test]
fn untyped_sampler_resolves_by_usage() {
    let m = check_ok(
        "sampler sampler_a; sampler sampler_b; sampler sampler_noisevol_x; \
         void f() { float4 a = tex3D(sampler_a, float3(0, 0, 0)); float4 b = tex2D(sampler_b, a.xy); }",
    );
    let tys: Vec<Ty> = m
        .decls
        .iter()
        .filter_map(|d| match d {
            TDecl::Global(g) => Some(g.ty.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(tys, vec![Ty::Sampler3D, Ty::Sampler2D, Ty::Sampler3D]);
}

#[test]
fn shader_body_must_be_wrapped_first() {
    let e = check_err("shader_body { ret = 0; }");
    assert!(e.message.contains("shader_body"), "{e}");
}

/// Idioms common in MilkDrop 2 preset shaders, wrapped like Task 4.6 does.
#[test]
fn preset_idioms_check() {
    let inc = std::fs::read_to_string(format!(
        "{}/../../assets/hlsl/include.fx",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let src = format!(
        "{inc}\n#define uv _uv.xy\n#define rad _rad_ang.x\n#define ang _rad_ang.y\n\
         sampler sampler_pw_noise_lq; sampler sampler_noisevol_lq_x; float4 texsize_x;\n\
         float3 helper(float3 c, float k) {{ return c * k; }}\n\
         void PS(float4 _vDiffuse : COLOR, float4 _uv : TEXCOORD0, float2 _rad_ang : TEXCOORD1, \
                 out float4 _return_value : COLOR0) {{\n\
           float3 ret = 0;\n\
           float2 uv2 = uv - 0.5; float d = length(uv2); float a = atan2(uv2.y, uv2.x);\n\
           uv2 = mul(uv2, float2x2(cos(a), -sin(a), sin(a), cos(a)));\n\
           float3 p = mul(float3(uv2, 1), (float3x3)rot_s1);\n\
           ret = tex2D(sampler_main, uv);\n\
           float lumv = tex2D(sampler_main, uv);\n\
           ret += GetBlur1(uv) * 0.3 - GetBlur2(uv + 0.01) * q1;\n\
           ret = lerp(ret, 1 - ret, q2);\n\
           ret += (ret > 0.5) * 0.1;\n\
           ret = ret < 0.5 ? 0 : ret;\n\
           int k = (int)(q1 * 4);\n\
           if (q1 > 0.5 && k != 2) ret.x = ret.y; else ret.yz *= 2;\n\
           float4 n = tex2D(sampler_pw_noise_lq, uv * texsize.xy * texsize_noise_lq.zw);\n\
           float4 v = tex3D(sampler_noisevol_lq_x, float3(uv, time * 0.1));\n\
           ret = saturate(helper(ret, 1.5) + n.xyz * 0.02 + v.x * rad + lum(ret) * ang);\n\
           ret *= 1 + 0.1 * sin(time + d);\n\
           uv = frac(uv * 2);\n\
           ret = pow(abs(ret), 1.2) + (k % 2 == 0) * float3(0.1, 0, 0);\n\
           _return_value = float4(ret.xyz, _vDiffuse.w);\n\
         }}"
    );
    let m = check_ok(&src);
    assert!(m.warnings.len() >= 2, "{:?}", m.warnings);
    let ps = func(&m, "PS");
    assert!(matches!(ps.body.stmts.last(), Some(TStmt::Expr(_))));
}

/// Checking recurses over the parser's maximum tree height without
/// overflowing a 1 MiB stack (debug build).
#[test]
fn deep_expressions_do_not_overflow() {
    let chain = vec!["a"; 500].join(" + ");
    let swz = format!("v{}", ".x".repeat(500));
    let ifs = "if (a > 0) ".repeat(60);
    let blocks = format!("{}a = 2;{}", "{".repeat(60), "}".repeat(60));
    let neg = format!("{}a{}", "-(".repeat(60), ")".repeat(60));
    let src = format!(
        "void f() {{ float a = 1; float v = 0; float b = {chain}; float c = {swz}; \
         {ifs} a = 1; {blocks} float d = {neg}; }}"
    );
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            check_ok(&src);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn ast_splat_node_is_accepted() {
    use md_hlsl::ast::{Decl, Expr, Init, Stmt, TypeKind, TypeName};
    let mut m = parse(&preprocess_tokens("void f() { float3 a = 2; }").unwrap()).unwrap();
    let Decl::Function(f) = &mut m.decls[0] else {
        panic!()
    };
    let Stmt::VarDecl(v) = &mut f.body.stmts[0] else {
        panic!()
    };
    let Some(Init::Expr(e)) = v.vars[0].init.take() else {
        panic!()
    };
    let pos = e.pos();
    let ty = TypeName {
        pos,
        kind: TypeKind::Vector(md_hlsl::ast::Scalar::Float, 3),
    };
    v.vars[0].init = Some(Init::Expr(Expr::Splat {
        pos,
        ty,
        expr: Box::new(e),
    }));
    let t = check(&m).unwrap();
    let TDecl::Function(f) = &t.decls[0] else {
        panic!()
    };
    let init = local(&f.body.stmts[0]).init.as_ref().unwrap();
    assert!(matches!(&init.kind, TExprKind::Splat(x) if x.ty == Ty::Float) && init.ty == F3);
}

#[test]
fn statement_and_operator_errors() {
    for (src, needle) in [
        ("void f() { break; }", "loop"),
        ("float f() { return; }", "return"),
        ("void f() { return 1; }", "void"),
        ("void f() { float a = 1.5 & 2; }", "integer"),
        ("void f() { float a = 1; float a = 2; }", "redefinition"),
        ("float4 g; float3 g;", "redefinition"),
        ("void f() { float4 v = 0; float a = v.q; }", "swizzle"),
        ("void f() { float4 v = 0; float a = v[4]; }", "range"),
        ("void f() { float4x1 v = 0; }", "one row or column"),
        (
            "void f() { float3 v = 0; float4x4 m = 0; v = v + m; }",
            "matrix",
        ),
    ] {
        let e = check_err(src);
        assert!(e.message.contains(needle), "{src}: {e}");
    }
    // int division/modulo stay int; float % is fmod
    let stmts = body("int a = 7 / 2; int b = 7 % 2; int c = 1 << 2;");
    for s in &stmts {
        assert_eq!(local(s).init.as_ref().unwrap().ty, Ty::Int);
    }
}

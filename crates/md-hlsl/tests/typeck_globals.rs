//! Type checker tests for review fix round 1 (Task 4.3): benign global
//! redeclaration, writable uniform globals, single-evaluation compound
//! assignment. Run with `cargo test -p md-hlsl --test typeck_globals`.

use md_hlsl::ast::BinOp;
use md_hlsl::typed::*;
use md_hlsl::types::{Scalar, Ty};
use md_hlsl::{check, parse, preprocess_tokens, TranslateError};

const F3: Ty = Ty::Vec(Scalar::Float, 3);

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

/// Statements of `void f() { <body> }`.
fn body(body: &str) -> Vec<TStmt> {
    let m = check_ok(&format!("void f() {{ {body} }}"));
    match m.decls.into_iter().next() {
        Some(TDecl::Function(f)) => f.body.stmts,
        d => panic!("{d:?}"),
    }
}

fn expr_stmt(s: &TStmt) -> &TExpr {
    match s {
        TStmt::Expr(e) => e,
        s => panic!("not an expression statement: {s:?}"),
    }
}

#[test]
fn benign_global_redeclaration_merges() {
    // The authoring guide's idiom: redeclare include.fx's texsize_noise_lq.
    let inc = std::fs::read_to_string(format!(
        "{}/../../assets/hlsl/include.fx",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let m = check_ok(&format!(
        "{inc}\nfloat4 texsize_noise_lq;\nsampler sampler_noise_lq;\n\
         float2 f() {{ return texsize_noise_lq.zw + tex2D(sampler_noise_lq, 0.5).xy; }}"
    ));
    let n = m
        .decls
        .iter()
        .filter(|d| matches!(d, TDecl::Global(g) if g.name == "texsize_noise_lq"))
        .count();
    assert_eq!(n, 1);
    check_ok("float4 g : C0; float4 g; float4 g : C0;");
    for src in [
        "float4 g; float3 g;",
        "float4 g; float4 g = 1;",
        "float4 g; static float4 g;",
        "float4 g : C0; float4 g : C1;",
        "static float4 g; static float4 g;",
        "float4 g; void g() {}",
    ] {
        let e = check_err(src);
        assert!(e.message.contains("redefinition"), "{src}: {e}");
    }
}

#[test]
fn writes_to_uniform_globals_are_flagged() {
    let m = check_ok(
        "float4 _qa; float4 _qb; float4 _qc = 1; static float4 s; \
         void h(out float4 o) { o = 1; } \
         void f() { _qa.x = 1; _qc += 2; h(s); h(_qb); s = _qa; }",
    );
    let flags: Vec<(String, bool)> = m
        .decls
        .iter()
        .filter_map(|d| match d {
            TDecl::Global(g) => Some((g.name.clone(), g.written)),
            _ => None,
        })
        .collect();
    let expect = [("_qa", true), ("_qb", true), ("_qc", true), ("s", false)];
    assert_eq!(
        flags,
        expect
            .iter()
            .map(|(n, w)| (n.to_string(), *w))
            .collect::<Vec<_>>()
    );
    let m = check_ok("float4 _qa; float4 f() { return _qa; }");
    assert!(m
        .decls
        .iter()
        .all(|d| !matches!(d, TDecl::Global(g) if g.written)));
}

#[test]
fn compound_assignment_evaluates_lhs_once() {
    let stmts = body(
        "int a[4] = {1, 2, 3, 4}; int i = 0; a[i++] *= 1.5; float x = 1; x += float3(1, 2, 3);",
    );
    let e = expr_stmt(&stmts[2]);
    assert_eq!(format!("{e:?}").matches("PostInc").count(), 1, "{e:?}");
    let TExprKind::Assign {
        op,
        op_ty,
        lhs,
        rhs,
    } = &e.kind
    else {
        panic!("{e:?}")
    };
    assert_eq!(
        (*op, op_ty.clone(), lhs.ty.clone()),
        (Some(BinOp::Mul), Ty::Float, Ty::Int)
    );
    assert_eq!(rhs.ty, Ty::Float);
    assert_eq!(e.ty, Ty::Int);
    let e = expr_stmt(&stmts[4]);
    let TExprKind::Assign { op_ty, rhs, .. } = &e.kind else {
        panic!("{e:?}")
    };
    assert_eq!(
        (op_ty.clone(), rhs.ty.clone(), e.ty.clone()),
        (F3, F3, Ty::Float)
    );
}

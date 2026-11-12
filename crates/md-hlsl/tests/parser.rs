//! Parser tests (Task 4.2). Run with `cargo test -p md-hlsl --test parser`.

use md_hlsl::ast::*;
use md_hlsl::{parse, preprocess_tokens, TranslateError};

fn include_fx() -> String {
    let p = format!(
        "{}/../../assets/hlsl/include.fx",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn try_parse(src: &str) -> Result<Module, TranslateError> {
    parse(&preprocess_tokens(src)?)
}

fn parse_ok(src: &str) -> Module {
    try_parse(src).unwrap_or_else(|e| panic!("{e}\n--- source:\n{src}"))
}

/// Body statements of `void f() { <body> }`.
fn body(stmts: &str) -> Vec<Stmt> {
    let m = parse_ok(&format!("void f() {{ {stmts} }}"));
    match m.decls.into_iter().next().unwrap() {
        Decl::Function(f) => f.body.stmts,
        d => panic!("{d:?}"),
    }
}

/// The expression of the single expression statement `<e>;`.
fn expr(e: &str) -> Expr {
    match body(&format!("{e};")).remove(0) {
        Stmt::Expr { expr, .. } => expr,
        s => panic!("{s:?}"),
    }
}

/// Compact S-expression rendering for precedence assertions.
fn sx(e: &Expr) -> String {
    match e {
        Expr::Lit { lit, .. } => match lit {
            Lit::Bool(b) => b.to_string(),
            Lit::Int(i) => i.to_string(),
            Lit::Float(f) => format!("{f:?}"),
        },
        Expr::Ident { name, .. } => name.clone(),
        Expr::Index { base, index, .. } => format!("(idx {} {})", sx(base), sx(index)),
        Expr::Member { base, field, .. } => format!("(. {} {field})", sx(base)),
        Expr::Call { name, args, .. } => {
            let a: Vec<_> = args.iter().map(sx).collect();
            format!("(call {name} {})", a.join(" "))
        }
        Expr::Ctor { ty, args, .. } => {
            let a: Vec<_> = args.iter().map(sx).collect();
            format!("(ctor {:?} {})", ty.kind, a.join(" "))
        }
        Expr::Unary { op, operand, .. } => format!("({op:?} {})", sx(operand)),
        Expr::Binary { op, lhs, rhs, .. } => format!("({op:?} {} {})", sx(lhs), sx(rhs)),
        Expr::Assign { op, lhs, rhs, .. } => format!("(= {op:?} {} {})", sx(lhs), sx(rhs)),
        Expr::Ternary {
            cond, then, els, ..
        } => format!("(? {} {} {})", sx(cond), sx(then), sx(els)),
        Expr::Cast { ty, expr, .. } => format!("(cast {:?} {})", ty.kind, sx(expr)),
    }
}

#[test]
fn parses_include_fx_without_error() {
    let m = parse_ok(&include_fx());
    let globals: Vec<&VarDecl> = m
        .decls
        .iter()
        .map(|d| match d {
            Decl::Global(v) => v,
            d => panic!("{d:?}"),
        })
        .collect();
    let names: Vec<&str> = globals
        .iter()
        .flat_map(|v| v.vars.iter().map(|d| d.name.as_str()))
        .collect();
    assert!(names.contains(&"_c4") && names.contains(&"rot_rand4"));
    assert!(names.contains(&"sampler_blur3"));
    let rot = globals.iter().find(|v| v.vars[0].name == "rot_s1").unwrap();
    assert_eq!(rot.ty.kind, TypeKind::Matrix(Scalar::Float, 4, 3));
    // `= sampler_state { ... }` is parsed and discarded.
    let main = globals
        .iter()
        .find(|v| v.vars[0].name == "sampler_main")
        .unwrap();
    assert_eq!(main.ty.kind, TypeKind::Sampler(SamplerKind::S2D));
    assert!(main.vars[0].init.is_none());
    let c1 = globals.iter().find(|v| v.vars[0].name == "_c1").unwrap();
    assert_eq!(c1.vars.len(), 4);
}

#[test]
fn parses_swizzle_chain() {
    let e = expr("a.xyz.zyx.x");
    assert_eq!(sx(&e), "(. (. (. a xyz) zyx) x)");
    assert_eq!(e.pos(), Pos { line: 1, col: 21 });
    assert_eq!(
        sx(&expr("tex2D(s, uv).xyz * 2")),
        "(Mul (. (call tex2D s uv) xyz) 2)"
    );
    assert_eq!(sx(&expr("m[1][2].x")), "(. (idx (idx m 1) 2) x)");
}

#[test]
fn parses_matrix_type_float4x3() {
    let m = parse_ok("float4x3 m; half2x2 h; float1x4 r; int3 v;");
    let kinds: Vec<TypeKind> = m
        .decls
        .iter()
        .map(|d| match d {
            Decl::Global(v) => v.ty.kind.clone(),
            d => panic!("{d:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            TypeKind::Matrix(Scalar::Float, 4, 3),
            TypeKind::Matrix(Scalar::Half, 2, 2),
            TypeKind::Matrix(Scalar::Float, 1, 4),
            TypeKind::Vector(Scalar::Int, 3),
        ]
    );
    assert!(try_parse("float5 x;").is_err());
    assert!(try_parse("float4x5 x;").is_err());
}

#[test]
fn parses_for_with_decl() {
    let s = body("for (int i = 0; i < 4; i++) { x += i; }");
    match &s[0] {
        Stmt::For {
            init: ForInit::Decl(d),
            cond: Some(c),
            step,
            body,
            ..
        } => {
            assert_eq!(d.ty.kind, TypeKind::Scalar(Scalar::Int));
            assert_eq!(d.vars[0].name, "i");
            assert_eq!(sx(c), "(Lt i 4)");
            assert_eq!(step.len(), 1);
            assert_eq!(sx(&step[0]), "(PostInc i)");
            assert!(matches!(**body, Stmt::Block(_)));
        }
        s => panic!("{s:?}"),
    }
    // comma operator in for clauses; empty clauses
    match &body("for (i = 0, j = 1; ; i++, j--) ;")[0] {
        Stmt::For {
            init: ForInit::Exprs(e),
            cond: None,
            step,
            ..
        } => assert_eq!((e.len(), step.len()), (2, 2)),
        s => panic!("{s:?}"),
    }
    // ...but not as a general expression operator
    assert!(try_parse("void f() { a = 1, b = 2; }").is_err());
}

#[test]
fn parses_ternary_nested() {
    assert_eq!(sx(&expr("a ? b : c ? d : e")), "(? a b (? c d e))");
    assert_eq!(sx(&expr("a ? b ? c : d : e")), "(? a (? b c d) e)");
    assert_eq!(
        sx(&expr("x = a < b ? 1 : 2")),
        "(= None x (? (Lt a b) 1 2))"
    );
}

#[test]
fn tolerates_missing_semicolon_before_closing_brace() {
    let m = parse_ok("float g;\nshader_body\n{\n  float3 a = 1;\n  ret = a\n}");
    match m.decls.last().unwrap() {
        Decl::ShaderBody(b) => {
            assert_eq!(b.body.stmts.len(), 2);
            assert_eq!(b.pos, Pos { line: 2, col: 1 });
        }
        d => panic!("{d:?}"),
    }
    // also for a final declaration or return
    parse_ok("shader_body { float3 a = 1 }");
    // only at the end of a shader_body
    let e = try_parse("void f() { ret = a }").unwrap_err();
    assert!(e.message.contains("';'"), "{e}");
    assert!(try_parse("shader_body { ret = a  ret = b; }").is_err());
    assert!(try_parse("shader_body { if (a) ret = a }").is_err());
}

#[test]
fn precedence_follows_c() {
    assert_eq!(sx(&expr("a + b * c - d")), "(Sub (Add a (Mul b c)) d)");
    assert_eq!(sx(&expr("a || b && c | d ^ e & f")), {
        "(Or a (And b (BitOr c (BitXor d (BitAnd e f)))))"
    });
    assert_eq!(sx(&expr("a == b < c << d")), "(Eq a (Lt b (Shl c d)))");
    assert_eq!(sx(&expr("a = b += c")), "(= None a (= Some(Add) b c))");
    assert_eq!(sx(&expr("-a.x++")), "(Neg (PostInc (. a x)))");
    assert_eq!(sx(&expr("!--a")), "(Not (PreDec a))");
    assert_eq!(
        sx(&expr("a %= b >>= 2")),
        "(= Some(Rem) a (= Some(Shr) b 2))"
    );
}

#[test]
fn parses_casts_and_constructors() {
    assert_eq!(
        sx(&expr("(float3)x.y + 1")),
        "(Add (cast Vector(Float, 3) (. x y)) 1)"
    );
    assert_eq!(
        sx(&expr("float3(1, 2.5, true).x")),
        "(. (ctor Vector(Float, 3) 1 2.5 true) x)"
    );
    assert_eq!(sx(&expr("(float)(a)")), "(cast Scalar(Float) a)");
    // a parenthesized constructor is not a cast
    assert_eq!(sx(&expr("(float2(a, b))")), "(ctor Vector(Float, 2) a b)");
    assert_eq!(sx(&expr("(a)*b")), "(Mul a b)");
    // struct casts need the struct to be declared
    let m = parse_ok("struct S { float a; };\nvoid f() { S s = (S)0; }");
    assert!(matches!(m.decls[0], Decl::Struct(_)));
}

#[test]
fn parses_globals_functions_and_structs() {
    let src = "static const float k = 2.0;\nuniform float4 arr[3];\n\
               sampler sampler_foo : register(s0);\nfloat4 texsize_foo;\n\
               struct V { float4 p : POSITION; float2 uv[2]; };\n\
               float3 helper(in float3 c, out float d, inout float2 e[2], uniform float s = 1) : COLOR\n\
               { d = 1; return c * k; };\n\
               float a <string n = 1;> = 3, b[] = { 1, { 2, 3 } };";
    let m = parse_ok(src);
    assert_eq!(m.decls.len(), 7);
    match &m.decls[0] {
        Decl::Global(v) => assert!(v.quals.is_static && v.quals.is_const),
        d => panic!("{d:?}"),
    }
    match &m.decls[1] {
        Decl::Global(v) => {
            assert!(v.quals.uniform);
            assert_eq!(v.vars[0].array.len(), 1);
        }
        d => panic!("{d:?}"),
    }
    match &m.decls[2] {
        Decl::Global(v) => assert_eq!(v.ty.kind, TypeKind::Sampler(SamplerKind::Any)),
        d => panic!("{d:?}"),
    }
    match &m.decls[4] {
        Decl::Struct(s) => {
            assert_eq!(s.name, "V");
            assert_eq!(s.fields[0].semantic.as_deref(), Some("POSITION"));
        }
        d => panic!("{d:?}"),
    }
    match &m.decls[5] {
        Decl::Function(f) => {
            assert_eq!(f.name, "helper");
            assert_eq!(f.params.len(), 4);
            assert!(f.params[1].quals.out && f.params[2].quals.inout);
            assert!(f.params[3].default.is_some());
            assert_eq!(f.semantic.as_deref(), Some("COLOR"));
            assert_eq!(f.pos, Pos { line: 6, col: 1 });
        }
        d => panic!("{d:?}"),
    }
    match &m.decls[6] {
        Decl::Global(v) => {
            assert_eq!(v.vars.len(), 2);
            assert!(matches!(v.vars[1].init, Some(Init::List { .. })));
            assert_eq!(v.vars[1].array, vec![None]);
        }
        d => panic!("{d:?}"),
    }
}

#[test]
fn parses_statements() {
    let s = body(
        "if (a) b = 1; else if (c) { b = 2; } else b = 3;\n\
         while (i < 3) i++;\n do { i--; } while (i > 0);\n\
         [unroll] for (;;) { break; continue; }\n discard; return;\n\
         float3 x = 0, y; ;",
    );
    assert_eq!(s.len(), 8);
    assert!(matches!(&s[0], Stmt::If { els: Some(e), .. } if matches!(**e, Stmt::If { .. })));
    assert!(matches!(s[1], Stmt::While { .. }));
    assert!(matches!(s[2], Stmt::DoWhile { .. }));
    assert!(matches!(s[3], Stmt::For { .. }));
    assert!(matches!(s[4], Stmt::Discard { .. }));
    assert!(matches!(s[5], Stmt::Return { value: None, .. }));
    assert!(matches!(&s[6], Stmt::VarDecl(d) if d.vars.len() == 2));
    assert!(matches!(&s[7], Stmt::Block(b) if b.stmts.is_empty()));
    assert_eq!(s[1].pos(), Pos { line: 2, col: 1 });
}

#[test]
fn shader_body_is_last_and_unique() {
    // Like the C++ split, tokens after the closing brace are dropped unless
    // they contain a `}` (then the C++ would have wrapped the wrong brace).
    let m = parse_ok("shader_body { ret = 1; } float junk");
    assert_eq!(m.decls.len(), 1);
    assert!(try_parse("shader_body { ret = 1; } void g() { }").is_err());
    assert!(try_parse("shader_body { } shader_body { }").is_err());
    assert!(try_parse("shader_body ret = 1;").is_err());
}

#[test]
fn errors_report_position() {
    let e = try_parse("float4 a;\nvoid f() {\n  a = (1 + ;\n}").unwrap_err();
    assert_eq!((e.line, e.col), (3, 12), "{e}");
    let e = try_parse("float4 a").unwrap_err();
    assert_eq!(e.line, 1, "{e}");
    assert!(try_parse("void f() { float if = 1; }").is_err());
    assert!(try_parse("void f() { a[1 = 2; }").is_err());
    assert!(try_parse("void f( { }").is_err());
}

/// Input is untrusted: nesting beyond the parser's limits is a
/// `TranslateError`, and everything up to the limits fits in 1 MiB of stack
/// even in a debug build (half the default test-thread stack).
#[test]
fn deep_nesting_is_an_error_not_a_stack_overflow() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(deep_nesting_cases)
        .unwrap()
        .join()
        .unwrap();
}

fn deep_nesting_cases() {
    let n = 20_000;
    // The most frame-hungry expression shape: every precedence level open
    // inside each parenthesis.
    let worst = "1 || 1 && 1 | 1 ^ 1 & 1 == 1 < 1 << 1 + 1 * -(";
    let cases = [
        format!("void f() {{ x = {}1{}; }}", "(".repeat(n), ")".repeat(n)),
        format!("void f() {{ x = {}1; }}", "-".repeat(n)),
        format!("void f() {{ x = {}1; }}", "!~".repeat(n)),
        format!("void f() {{ {}x = 1;{} }}", "{".repeat(n), "}".repeat(n)),
        format!("void f() {{ {} x = 1; }}", "if (a) ".repeat(n)),
        format!(
            "void f() {{ {} x = 1; }}",
            "for (;;) while (1) do ".repeat(n)
        ),
        format!("void f() {{ x = 1{}; }}", " + 1".repeat(n)),
        format!("void f() {{ x {}; }}", "= x ".repeat(n)),
        format!("void f() {{ x = a{}; }}", ".x".repeat(n)),
        format!("void f() {{ x = a{}; }}", "[0]".repeat(n)),
        format!("void f() {{ x = 1{}; }}", " ? 1 : 1".repeat(n)),
        format!(
            "void f() {{ x = g({}1{}); }}",
            "g(".repeat(n),
            ")".repeat(n)
        ),
        format!("void f() {{ x = {}1{}; }}", worst.repeat(n), ")".repeat(n)),
        format!(
            "void f() {{ {} x = {}1; }}",
            "{ if (a) ".repeat(64),
            "(".repeat(n)
        ),
        format!("float a = {}1{};", "{".repeat(n), "}".repeat(n)),
    ];
    for src in &cases {
        let e = try_parse(src).unwrap_err();
        assert!(e.message.contains("too deep"), "{e}");
    }
    // Nesting just under the limits parses.
    parse_ok(&format!("void f() {{ x = 1{}; }}", " + 1".repeat(500)));
    let src = format!(
        "void f() {{ x = {}1{}; }}",
        worst.repeat(40),
        ")".repeat(40)
    );
    parse_ok(&src);
    let src = format!(
        "void f() {{ {} x = {}1{}; {} }}",
        "{ if (a) ".repeat(30),
        "(".repeat(60),
        ")".repeat(60),
        "}".repeat(30)
    );
    parse_ok(&src);
}

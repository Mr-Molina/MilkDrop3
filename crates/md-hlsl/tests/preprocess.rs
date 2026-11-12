use md_hlsl::{preprocess, preprocess_tokens, TokKind::*};

fn texts(s: &str) -> Vec<String> {
    preprocess_tokens(s)
        .unwrap()
        .into_iter()
        .map(|t| match t.kind {
            Ident(s) => s.to_string(),
            Int(i) => i.to_string(),
            Float(f) => format!("{f:?}"),
            Op(o) => o.to_string(),
        })
        .collect()
}

#[test]
fn expands_object_and_function_macros() {
    let src = "#define N 3\n#define ADD(a,b) ((a)+(b))\nx = ADD(N, ADD(1,2)) * N;";
    assert_eq!(
        texts(src).join(" "),
        "x = ( ( 3 ) + ( ( ( 1 ) + ( 2 ) ) ) ) * 3 ;"
    );
    // nested parens in args, commas inside parens, multi-line body
    let src = "#define F(a,b) \\\n  foo(a, \\\n      b)\nF(g(1,2), (3,4))";
    assert_eq!(texts(src).join(" "), "foo ( g ( 1 , 2 ) , ( 3 , 4 ) )");
    // undef, redefinition, function-like name without parens stays an ident
    let src = "#define A 1\n#undef A\nA\n#define G(x) x\nG";
    assert_eq!(texts(src).join(" "), "A G");
    // `#define N (x)` is object-like (space before paren)
    assert_eq!(texts("#define N (x)\nN").join(" "), "( x )");
    assert_eq!(texts("#pragma foo\n#\nq").join(" "), "q");
}

#[test]
fn recursion_is_guarded() {
    assert_eq!(texts("#define A A+1\nA").join(" "), "A + 1");
    assert_eq!(texts("#define A B\n#define B A\nA").join(" "), "A");
    assert_eq!(texts("#define f(x) f(x)+1\nf(2)").join(" "), "f ( 2 ) + 1");
}

#[test]
fn macro_from_include_fx_time_expands_to_c2_x() {
    let inc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/hlsl/include.fx"
    ))
    .unwrap();
    let toks = preprocess_tokens(&format!("{inc}\nret = time + GetBlur1(uv) + lum(ret);")).unwrap();
    let s: Vec<String> = toks.iter().map(|t| format!("{:?}", t.kind)).collect();
    let tail = s.join(" ");
    assert!(
        tail.contains("Ident(\"ret\") Op(\"=\") Ident(\"_c2\") Op(\".\") Ident(\"x\") Op(\"+\")"),
        "{tail}"
    );
    assert!(
        tail.contains("Ident(\"sampler_blur1\") Op(\",\") Ident(\"uv\")"),
        "{tail}"
    );
    // expansion tokens report the invocation site
    let last_line = inc.matches('\n').count() as u32 + 2;
    let t = toks
        .iter()
        .rev()
        .find(|t| t.ident() == Some("_c2"))
        .unwrap();
    assert_eq!((t.line, t.col), (last_line, 7));
}

#[test]
fn preprocess_string_convenience() {
    assert_eq!(preprocess("#define X 1\na X;").unwrap(), "a 1 ;");
}

#[test]
fn unsupported_directives_and_bad_macros_error() {
    for (src, line) in [
        ("\n#include \"a.fx\"", 2),
        ("#if 1\n#endif", 1),
        ("a\nb\n#ifdef X", 3),
        ("#define", 1),
        ("#define 3 x", 1),
    ] {
        let e = preprocess_tokens(src).unwrap_err();
        assert_eq!(e.line, line, "{src}: {e}");
    }
    let e = preprocess_tokens("#define F(a) a\n  F(1,2)").unwrap_err();
    assert_eq!((e.line, e.col), (2, 3));
    let e = preprocess_tokens("#define F(a) a\nF(1").unwrap_err();
    assert_eq!((e.line, e.col), (2, 1));
}

#[test]
fn hide_sets_survive_argument_pre_expansion() {
    assert_eq!(
        texts("#define A A+1\n#define F(x) x\nF(A)").join(" "),
        "A + 1"
    );
    assert_eq!(
        texts("#define A A+1\n#define F(x) x\nF(F(A))").join(" "),
        "A + 1"
    );
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let n = 20000;
    let src = format!("#define F(x) x\n{}1{}", "F(".repeat(n), ")".repeat(n));
    let t = std::time::Instant::now();
    let e = preprocess_tokens(&src).unwrap_err();
    assert!(
        e.message.contains("too deeply") || e.message.contains("too large"),
        "{e}"
    );
    assert!(t.elapsed().as_secs() < 5);
}

#[test]
fn exponential_expansion_is_an_error() {
    let mut src = String::from("#define A0 x\n");
    for i in 1..=30 {
        src.push_str(&format!("#define A{i} A{} A{}\n", i - 1, i - 1));
    }
    src.push_str("A30\n");
    let t = std::time::Instant::now();
    let e = preprocess_tokens(&src).unwrap_err();
    assert!(e.message.contains("too large"), "{e}");
    assert!(t.elapsed().as_secs() < 10);
}

#[test]
fn expansion_budget_is_shared_across_segments() {
    // Each segment expands to 2^18 tokens (under any per-segment cap) but the
    // directives between them used to reset the budget.
    let mut src = String::from("#define D0 x\n");
    for i in 1..=18 {
        src.push_str(&format!("#define D{i} D{} D{}\n", i - 1, i - 1));
    }
    for k in 0..40 {
        src.push_str(&format!("D18\n#define Z{k} 0\n"));
    }
    let t = std::time::Instant::now();
    let e = preprocess_tokens(&src).unwrap_err();
    assert!(e.message.contains("too large"), "{e}");
    assert!(t.elapsed().as_secs() < 10);
}

#[test]
fn budget_is_ample_for_include_fx_and_corpus_sized_input() {
    let inc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/hlsl/include.fx"
    ))
    .unwrap();
    // include.fx repeated 20 times (with redefinitions) still fits.
    preprocess_tokens(&inc.repeat(20)).unwrap();
}

#[test]
fn argument_substitution_is_charged_before_allocation() {
    // D17 expands to 2^17 tokens; F uses its argument 300 times (~39M tokens
    // if substituted). The budget must trip before that is allocated.
    let mut src = String::from("#define D0 x\n");
    for i in 1..=17 {
        src.push_str(&format!("#define D{i} D{} D{}\n", i - 1, i - 1));
    }
    let uses = vec!["x"; 300].join(" ");
    src.push_str(&format!("#define F(x) {uses}\nF(D17)\n"));
    let t = std::time::Instant::now();
    let e = preprocess_tokens(&src).unwrap_err();
    assert!(e.message.contains("too large"), "{e}");
    assert!(t.elapsed().as_secs() < 10);
}

#[test]
fn overlong_identifiers_and_literals_are_errors() {
    // Token text is capped so that per-token memory is bounded regardless of
    // how often expansion copies a token.
    let ok = "a".repeat(256);
    assert_eq!(texts(&format!("{ok} b")), vec![ok.clone(), "b".to_string()]);
    let long = "a".repeat(257);
    for src in [
        long.clone(),
        format!("#define D0 {long}\nD0\n"),
        format!("#define {long} 1\n"),
        format!("#define F({long}) 1\n"),
        format!("#undef {long}\n"),
        format!("1.{}", "0".repeat(300)),
        "1".repeat(300),
    ] {
        let e = preprocess_tokens(&src).unwrap_err();
        assert!(e.message.contains("too long"), "{e}");
    }
    let e = md_hlsl::tokenize(&long).unwrap_err();
    assert!(e.message.contains("too long"), "{e}");
}

#[test]
fn long_identifier_doubling_is_rejected_cheaply() {
    // Review probe: a 20 KB source whose 20,000-char identifier, doubled 17
    // times, used to return Ok after peaking at ~2.5 GB RSS.
    let mut src = format!("#define D0 {}\n", "a".repeat(20_000));
    for i in 1..=17 {
        src.push_str(&format!("#define D{i} D{} D{}\n", i - 1, i - 1));
    }
    src.push_str("D17\n");
    let e = preprocess_tokens(&src).unwrap_err();
    assert!(e.message.contains("too long"), "{e}");
}

#[test]
fn object_macro_chain_hits_the_depth_cap() {
    // A300 -> A299 -> ... -> A0 -> x: each step is one token, so only the
    // nesting cap (MAX_DEPTH = 128) can stop it.
    let mut src = String::from("#define A0 x\n");
    for i in 1..=300 {
        src.push_str(&format!("#define A{i} A{}\n", i - 1));
    }
    src.push_str("A300\n");
    let e = preprocess_tokens(&src).unwrap_err();
    assert!(e.message.contains("nested too deeply"), "{e}");
    // Just under the cap still expands.
    let mut ok = String::from("#define A0 x\n");
    for i in 1..=100 {
        ok.push_str(&format!("#define A{i} A{}\n", i - 1));
    }
    ok.push_str("A100\n");
    assert_eq!(texts(&ok), vec!["x"]);
}

#[test]
fn parameter_lookup_does_not_scale_with_parameter_count() {
    // Review probe: 20,000 params x 20,000-token body x 20 calls was 8e9
    // linear parameter scans (3.2 s release). With a map it is linear.
    // (5 calls since MAX_TOKENS became 500,000: still 2e9 linear scans.)
    let n = 20_000;
    let params: Vec<String> = (0..n).map(|i| format!("p{i}")).collect();
    let body = vec!["q"; n].join(" ");
    let args = vec!["1"; n].join(",");
    let calls = vec![format!("F({args})"); 5].join(" ");
    let src = format!("#define F({}) {body}\n{calls}\n", params.join(","));
    let toks = preprocess_tokens(&src).unwrap();
    assert_eq!(toks.len(), 5 * n);
    // Duplicate-free substitution still picks the right argument.
    assert_eq!(
        texts("#define G(a, b, c) c b a\nG(1, 2, 3)"),
        vec!["3", "2", "1"]
    );
}

//! Task 4.10 (preflight F3: proptest instead of a cargo-fuzz target):
//! `translate_never_panics` — arbitrary input to [`md_hlsl::translate`], for
//! both shader kinds, returns `Ok` or `Err` without panicking and within a
//! time bound. Generators: arbitrary strings (raw, and ASCII inside
//! `shader_body { }`), token soup over an HLSL-ish alphabet, typed
//! expression/statement soup (reaches the checker, emitter and naga), and
//! random token mutations of the corpus shader bodies; plus fixed
//! pathological inputs (deep nesting, long chains, macro bombs).
//!
//! Only panics and time are asserted: an `Err`, including naga rejecting
//! the generated WGSL ("internal translator error"), passes. Such
//! rejections found while writing this were fixed in the emitter (see
//! `tests/emit_const_eval.rs`); a constant float overflow like
//! `1e30 * 1e30` remains one (`tests/translate.rs`).
//!
//! Case counts are modest so a debug `cargo test` stays fast; set
//! `MD_PROPTEST_CASES=<n>` to run `n` cases per generator, e.g.
//! `MD_PROPTEST_CASES=20000 cargo test --release -p md-hlsl --test never_panics`.

mod common;

use md_hlsl::{translate, PsVersion, ShaderInput, ShaderKind};
use proptest::prelude::*;
use proptest::sample::Index;
use proptest::test_runner::FileFailurePersistence;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// Per-call bound: the brief's 200 ms for optimised builds; debug builds
/// (unoptimised naga, parallel test threads) get 2 s.
const BOUND: Duration = if cfg!(debug_assertions) {
    Duration::from_millis(2000)
} else {
    Duration::from_millis(200)
};

fn cases(default: u32) -> u32 {
    std::env::var("MD_PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// [`cases`] cases; failing inputs are saved next to this file
/// (`never_panics.proptest-regressions`, replayed first on the next run).
/// `WithSource` is what proptest's default falls back to for files under
/// `tests/`, minus its warning.
fn config() -> ProptestConfig {
    ProptestConfig {
        cases: cases(96),
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource(
            "proptest-regressions",
        ))),
        ..ProptestConfig::default()
    }
}

fn preamble() -> Arc<str> {
    static P: OnceLock<Arc<str>> = OnceLock::new();
    P.get_or_init(|| common::asset("include.fx").into()).clone()
}

/// Translate `body` as warp and as comp on a worker thread; fail on a
/// panic, or when a call takes longer than [`BOUND`] (a hang is reported
/// instead of blocking the test run; the worker is left behind).
fn never_panics(body: &str) -> Result<(), TestCaseError> {
    let body: Arc<str> = body.into();
    let pre = preamble();
    let (tx, rx) = mpsc::channel();
    let worker_body = Arc::clone(&body);
    std::thread::spawn(move || {
        for (kind, version) in [
            (ShaderKind::Warp, PsVersion::Ps2_0),
            (ShaderKind::Comp, PsVersion::Ps3_0),
        ] {
            let t = Instant::now();
            let _ = translate(&ShaderInput {
                kind,
                body: &worker_body,
                preamble: &pre,
                ps_version: version,
            });
            if tx.send((kind, t.elapsed())).is_err() {
                return;
            }
        }
    });
    for _ in 0..2 {
        match rx.recv_timeout(BOUND * 2) {
            Ok((kind, took)) if took > BOUND => {
                return Err(TestCaseError::fail(format!(
                    "{kind:?} took {took:?} (> {BOUND:?}) on {body:?}"
                )))
            }
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(TestCaseError::fail(format!("hang on {body:?}")))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(TestCaseError::fail(format!("panic on {body:?}")))
            }
        }
    }
    Ok(())
}

/// Tokens for soups and mutation insertions.
#[rustfmt::skip]
const ALPHABET: &[&str] = &[
    // structure and keywords
    "shader_body", "{", "}", "(", ")", "[", "]", ";", ",", ".", ":", "?", "if", "else", "for",
    "while", "do", "return", "break", "continue", "discard", "static", "const", "uniform",
    "struct", "in", "out", "inout", "true", "false", "typedef", "register", "\n",
    // types
    "float", "float1", "float2", "float3", "float4", "float2x2", "float3x3", "float4x3",
    "float4x4", "int", "int2", "int3", "bool", "bool2", "half", "half3", "double", "void",
    "sampler", "sampler2D", "sampler3D", "samplerCUBE", "texture",
    // intrinsics and include.fx helpers
    "tex2D", "tex3D", "tex2Dlod", "tex2Dbias", "tex2Dproj", "tex2Dgrad", "texCUBE", "lerp",
    "saturate", "mul", "dot", "cross", "normalize", "length", "pow", "sin", "cos", "fmod",
    "frac", "sincos", "modf", "frexp", "ldexp", "round", "abs", "min", "max", "clamp", "any",
    "all", "step", "smoothstep", "transpose", "sign", "GetPixel", "GetBlur1", "GetBlur3", "lum",
    // samplers, texsizes, uniforms and locals
    "sampler_main", "sampler_fc_main", "sampler_pw_noise_lq", "sampler_blur1",
    "sampler_noisevol_hq", "sampler_rand00", "sampler_rand07_smalltiled", "sampler_foo",
    "texsize_main", "texsize_noise_lq", "texsize_foo", "uv", "uv_orig", "ret", "rad", "ang",
    "q1", "q32", "_qa", "_c0", "_c13", "time", "hue_shader", "rand_frame", "rot_s1", "M_PI",
    "x", "y", "a", "N",
    // operators
    "+", "-", "*", "/", "%", "<<", ">>", "&", "|", "^", "~", "!", "&&", "||", "==", "!=", "<",
    ">", "<=", ">=", "=", "+=", "-=", "*=", "/=", "%=", "<<=", ">>=", "&=", "++", "--", ".x",
    ".xyz", ".rgba", ".wzyx", "._m01", ".xx",
    // numbers
    "0", "1", "2", "0.5", ".5", "3.0f", "1.h", "1e30", "1e-30", "40", "2147483647",
    "99999999999", "0x10", "010", "-1",
];

/// Preprocessor and comment tokens: they usually end translation at once,
/// so they are drawn rarely.
#[rustfmt::skip]
const RARE: &[&str] = &[
    "#define", "#define N 0\n", "#define F(x) (x*x)\n", "F(", "#undef", "#ifdef", "#if",
    "#else", "#endif", "#include", "#", "##", "1.#INF", "//", "/*", "*/", "\\", "\\\n", "/*/", "*//",
];

fn alphabet_token() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        100 => prop::sample::select(ALPHABET),
        1 => prop::sample::select(RARE),
    ]
}

fn soup(max: usize) -> impl Strategy<Value = String> {
    (
        prop::collection::vec(alphabet_token(), 0..max),
        any::<bool>(),
    )
        .prop_map(|(toks, wrap)| {
            let s = toks.join(" ");
            if wrap {
                format!("shader_body {{ {s} }}")
            } else {
                s
            }
        })
}

/// Well-formed-looking expressions over mixed types, so cases reach the
/// type checker and the emitter (implicit conversions, swizzles, matrix
/// and integer operators, constant folding hazards).
fn expr() -> impl Strategy<Value = String> {
    #[rustfmt::skip]
    const LEAVES: &[&str] = &[
        "uv", "uv.x", "uv.yx", "ret", "ret.zyx", "rad", "q1", "_c0", "_qa.xy", "time", "1", "0",
        "0.5", "2", "-1", "40", "1e30", "N", "Z", "i", "b", "m", "m3", "v4", "int2(1, 2)",
        "float3(1, 2, 3)", "tex2D(sampler_main, uv)", "tex3D(sampler_noisevol_hq, ret)",
        "texsize_main", "GetBlur1(uv)", "rand_frame", "rot_s1", "uv_orig",
    ];
    #[rustfmt::skip]
    const BINOPS: &[&str] = &[
        "+", "-", "*", "/", "%", "<<", ">>", "&", "|", "^", "&&", "||", "==", "<", ">=",
    ];
    #[rustfmt::skip]
    const CALL1: &[&str] = &[
        "sin", "frac", "saturate", "normalize", "length", "abs", "round", "sign", "transpose",
        "any", "floor", "(float)", "(int)", "(float3)", "(float2x2)", "-", "!", "~",
    ];
    #[rustfmt::skip]
    const CALL2: &[&str] = &[
        "pow", "dot", "mul", "min", "max", "fmod", "step", "cross", "lerp", "atan2", "float4",
        "float2", "int2", "tex2D", "tex2Dlod", "distance", "ldexp",
    ];
    #[rustfmt::skip]
    const SWIZZLES: &[&str] = &[".x", ".xy", ".xyz", ".wzyx", ".rrr", "._m00_m11", "[0]", "[1].y"];
    let leaf = prop::sample::select(LEAVES).prop_map(str::to_string);
    leaf.prop_recursive(5, 48, 3, |inner| {
        prop_oneof![
            (inner.clone(), prop::sample::select(BINOPS), inner.clone())
                .prop_map(|(a, o, b)| format!("({a} {o} {b})")),
            (prop::sample::select(CALL1), inner.clone()).prop_map(|(f, a)| format!("{f}({a})")),
            (prop::sample::select(CALL2), inner.clone(), inner.clone())
                .prop_map(|(f, a, b)| format!("{f}({a}, {b})")),
            (inner.clone(), prop::sample::select(SWIZZLES)).prop_map(|(a, s)| format!("({a}){s}")),
            (inner.clone(), inner.clone(), inner.clone())
                .prop_map(|(c, a, b)| format!("({c} ? {a} : {b})")),
            (inner.clone(), inner).prop_map(|(a, b)| format!("lerp({a}, {b}, 0.5)")),
        ]
    })
}

fn stmt() -> impl Strategy<Value = String> {
    #[rustfmt::skip]
    const TARGETS: &[&str] = &[
        "ret", "ret.x", "ret.zy", "i", "b", "m", "m3", "v4", "v4.w", "m[1]", "q1", "uv",
    ];
    #[rustfmt::skip]
    const ASSIGN: &[&str] = &["=", "+=", "*=", "/=", "%=", "<<=", "&="];
    prop_oneof![
        4 => (prop::sample::select(TARGETS), prop::sample::select(ASSIGN), expr())
            .prop_map(|(t, o, e)| format!("{t} {o} {e};")),
        1 => (expr(), expr(), expr())
            .prop_map(|(c, a, b)| format!("if ({c}) ret = {a}; else ret += {b};")),
        1 => (expr(), expr())
            .prop_map(|(c, a)| format!("for (int k = 0; k < 3; k++) {{ if ({c}) break; v4 += {a}; }}")),
        1 => expr().prop_map(|e| format!("float3 t = {e}; ret = t;")),
    ]
}

/// A preset body: hazard-prone globals, typed locals, random statements.
fn typed_body() -> impl Strategy<Value = String> {
    prop::collection::vec(stmt(), 1..5).prop_map(|ss| {
        format!(
            "static const int N = 0;\nstatic const float Z = 0;\nfloat4 G = 1;\nfloat4 texsize_main;\n\
             shader_body {{\n int i = 1; bool b = true; float2x2 m = 1; float3x3 m3 = 2;\n \
             float4 v4 = _c0;\n {}\n}}",
            ss.join("\n ")
        )
    })
}

/// Split a shader body into tokens (identifiers, numbers, single
/// punctuation characters) and the whitespace between them, so mutations
/// work on token boundaries and joining the pieces restores the text.
fn split_tokens(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' || c == '.' {
            0
        } else if c.is_whitespace() {
            1
        } else {
            2
        }
    };
    for c in s.chars() {
        let same = cur.chars().last().is_some_and(|p| {
            let k = class(p);
            k != 2 && k == class(c)
        });
        if !same && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn corpus_bodies() -> &'static [Vec<String>] {
    static B: OnceLock<Vec<Vec<String>>> = OnceLock::new();
    B.get_or_init(|| {
        let (shaders, _, _) = common::corpus::corpus_shaders();
        shaders.iter().map(|s| split_tokens(&s.body)).collect()
    })
}

#[derive(Clone, Debug)]
enum Mutation {
    Delete(Index),
    Insert(Index, &'static str),
    Swap(Index, Index),
    Duplicate(Index, Index),
}

fn mutation() -> impl Strategy<Value = Mutation> {
    prop_oneof![
        any::<Index>().prop_map(Mutation::Delete),
        (any::<Index>(), alphabet_token()).prop_map(|(i, t)| Mutation::Insert(i, t)),
        (any::<Index>(), any::<Index>()).prop_map(|(i, j)| Mutation::Swap(i, j)),
        (any::<Index>(), any::<Index>()).prop_map(|(i, j)| Mutation::Duplicate(i, j)),
    ]
}

fn mutate(mut toks: Vec<String>, muts: &[Mutation]) -> String {
    for m in muts {
        let n = toks.len();
        match m {
            _ if n == 0 => toks.push("{".into()),
            Mutation::Delete(i) => {
                toks.remove(i.index(n));
            }
            Mutation::Insert(i, t) => toks.insert(i.index(n + 1), format!(" {t} ")),
            Mutation::Swap(i, j) => toks.swap(i.index(n), j.index(n)),
            Mutation::Duplicate(i, j) => {
                let (a, b) = (i.index(n), j.index(n));
                let (a, b) = (a.min(b), a.max(b).min(a.min(b) + 32));
                let dup: Vec<String> = toks[a..=b].to_vec();
                toks.splice(b + 1..b + 1, dup);
            }
        }
    }
    toks.concat()
}

proptest! {
    #![proptest_config(config())]

    /// Arbitrary (mostly non-HLSL) strings: lexer and preprocessor errors.
    #[test]
    fn translate_never_panics_on_arbitrary_strings(s in any::<String>()) {
        never_panics(&s)?;
    }

    /// Arbitrary strings inside `shader_body { }`, biased to ASCII.
    #[test]
    fn translate_never_panics_on_wrapped_strings(s in "[ -~\n]{0,200}") {
        never_panics(&format!("shader_body {{ {s} }}"))?;
    }

    /// Token soup from the HLSL-ish alphabet: parser and checker errors.
    #[test]
    fn translate_never_panics_on_token_soup(s in soup(120)) {
        never_panics(&s)?;
    }

    /// Typed expression and statement soup: reaches the type checker and
    /// the emitter (and naga) far more often than token soup.
    #[test]
    fn translate_never_panics_on_typed_soup(s in typed_body()) {
        never_panics(&s)?;
    }

    /// Corpus shader bodies with random token deletions, insertions, swaps
    /// and duplications: inputs that get deep into the checker and emitter.
    #[test]
    fn translate_never_panics_on_corpus_mutations(
        which in any::<Index>(),
        muts in prop::collection::vec(mutation(), 1..4),
    ) {
        let bodies = corpus_bodies();
        let toks = bodies[which.index(bodies.len())].clone();
        never_panics(&mutate(toks, &muts))?;
    }
}

/// Fixed pathological inputs: nesting and chains far beyond the parser's
/// limits, long inputs and macro bombs must error out quickly.
#[test]
fn translate_never_panics_on_pathological_inputs() {
    let sb = |s: &str| format!("shader_body {{ {s} }}");
    let deep_parens = format!("ret = {}1{};", "(".repeat(100_000), ")".repeat(100_000));
    let chain = format!("ret = 1{};", " + 1".repeat(100_000));
    let unary = format!("ret = {}1;", "-".repeat(100_000));
    let else_ifs = format!("if (x) ret = 1;{}", " else if (x) ret = 1;".repeat(20_000));
    let blocks = format!("{}{}", "{".repeat(50_000), "}".repeat(50_000));
    let assigns = format!("float a; {} 1;", "a = ".repeat(50_000));
    let ternary = format!("ret = {} 1;", "x ? 1 : ".repeat(50_000));
    let swizzles = format!("ret = uv{};", ".xy".repeat(50_000));
    let calls = format!("ret = {}1{};", "sin(".repeat(50_000), ")".repeat(50_000));
    let mut bomb = String::from("#define A0 x\n");
    for i in 1..40 {
        bomb += &format!("#define A{i} A{p} A{p}\n", p = i - 1);
    }
    bomb += "shader_body { ret = A39; }";
    let self_ref = "#define A A A\n#define B(x) B(x) x\nshader_body { ret = A + B(1); }";
    let long_ident = sb(&format!("float {} = 1;", "a".repeat(1 << 20)));
    let inputs = [
        sb(&deep_parens),
        sb(&chain),
        sb(&unary),
        sb(&else_ifs),
        sb(&blocks),
        sb(&assigns),
        sb(&ternary),
        sb(&swizzles),
        sb(&calls),
        bomb,
        self_ref.to_string(),
        long_ident,
        "/*".repeat(100_000),
        "shader_body {".repeat(10_000),
        "\0\u{feff}\u{10ffff}".repeat(1000),
    ];
    for s in &inputs {
        let head: String = s.chars().take(40).collect();
        never_panics(s).unwrap_or_else(|e| panic!("{head}…: {e}"));
    }
}

#[test]
fn mutation_tokens_round_trip() {
    let body = "shader_body { ret = tex2D(sampler_main, uv).xyz * 0.5; }";
    assert_eq!(split_tokens(body).concat(), body);
    assert!(corpus_bodies().len() >= 66);
}

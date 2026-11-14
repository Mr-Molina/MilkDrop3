//! Property tests (replace the plan's cargo-fuzz target; libfuzzer-sys
//! compiles C++). No wall-clock bounds: random programs only use literal
//! `loop` counts of at most 16, and the tests assert completion only.

use std::cell::RefCell;
use std::rc::Rc;

use md_eval::{parse, AssignOp, BinOp, Expr, GlobalMem, Lexer, Program, UnOp, Vm};
use proptest::prelude::*;

fn vm() -> Vm {
    Vm::new(Rc::new(RefCell::new(GlobalMem::new())))
}

// ---------------------------------------------------------------------
// AST generator and fully parenthesising printer
// ---------------------------------------------------------------------

const VARS: [&str; 6] = ["a", "b", "c", "x", "y", "zoom"];
const CALL1: [&str; 16] = [
    "sin", "cos", "tan", "asin", "acos", "atan", "sqrt", "sqr", "abs", "exp", "log", "log10",
    "floor", "ceil", "sign", "invsqrt",
];
const CALL2: [&str; 8] = [
    "atan2", "pow", "min", "max", "sigmoid", "band", "bor", "exec2",
];
const BINOPS: [BinOp; 15] = [
    BinOp::Add,
    BinOp::Sub,
    BinOp::Mul,
    BinOp::Div,
    BinOp::Mod,
    BinOp::BitAnd,
    BinOp::BitOr,
    BinOp::Eq,
    BinOp::Ne,
    BinOp::Lt,
    BinOp::Gt,
    BinOp::Le,
    BinOp::Ge,
    BinOp::And,
    BinOp::Or,
];
const ASSIGNOPS: [AssignOp; 9] = [
    AssignOp::Set,
    AssignOp::Add,
    AssignOp::Sub,
    AssignOp::Mul,
    AssignOp::Div,
    AssignOp::Mod,
    AssignOp::BitAnd,
    AssignOp::BitOr,
    AssignOp::Pow,
];

fn var_name() -> impl Strategy<Value = String> {
    prop::sample::select(&VARS[..]).prop_map(str::to_string)
}

fn leaf() -> impl Strategy<Value = Expr> {
    prop_oneof![
        // exact binary fractions in [0, 1000): print and re-parse losslessly
        (0u32..8000).prop_map(|n| Expr::Num(f64::from(n) / 8.0)),
        var_name().prop_map(Expr::Var),
    ]
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        name: name.to_string(),
        args,
    }
}

fn expr() -> impl Strategy<Value = Expr> {
    leaf().prop_recursive(4, 40, 3, |inner| {
        let boxed = || inner.clone().prop_map(Box::new);
        prop_oneof![
            (prop::sample::select(&[UnOp::Neg, UnOp::Not][..]), boxed())
                .prop_map(|(op, e)| Expr::Unary(op, e)),
            (prop::sample::select(&BINOPS[..]), boxed(), boxed())
                .prop_map(|(op, a, b)| Expr::Binary(op, a, b)),
            (prop::sample::select(&ASSIGNOPS[..]), var_name(), boxed())
                .prop_map(|(op, v, e)| Expr::Assign(op, Box::new(Expr::Var(v)), e)),
            // megabuf / gmegabuf store and load
            (
                prop::sample::select(&ASSIGNOPS[..]),
                any::<bool>(),
                inner.clone(),
                boxed()
            )
                .prop_map(|(op, g, i, e)| {
                    let name = if g { "_gmem" } else { "_mem" };
                    Expr::Assign(op, Box::new(call(name, vec![i])), e)
                }),
            (any::<bool>(), inner.clone())
                .prop_map(|(g, i)| call(if g { "_gmem" } else { "_mem" }, vec![i])),
            (boxed(), boxed(), boxed()).prop_map(|(c, a, b)| Expr::Ternary(c, a, b)),
            (prop::sample::select(&CALL1[..]), inner.clone()).prop_map(|(n, a)| call(n, vec![a])),
            (
                prop::sample::select(&CALL2[..]),
                inner.clone(),
                inner.clone()
            )
                .prop_map(|(n, a, b)| call(n, vec![a, b])),
            prop::collection::vec(inner.clone(), 2..4).prop_map(Expr::Seq),
            // only literal counts <= 16; nesting depth is bounded by the
            // recursion depth, so at most 16^4 body runs
            (0u32..=16, inner.clone())
                .prop_map(|(n, body)| call("loop", vec![Expr::Num(f64::from(n)), body])),
        ]
    })
}

/// Print with every operand parenthesised, so precedence never matters.
fn print(e: &Expr, out: &mut String) {
    match e {
        Expr::Num(v) => out.push_str(&format!("{v}")),
        Expr::Var(n) => out.push_str(n),
        Expr::Call { name, args } => {
            // the parser lowers `megabuf(i)` to `_mem`, so print the source name
            let src = match name.as_str() {
                "_mem" => "megabuf",
                "_gmem" => "gmegabuf",
                n => n,
            };
            out.push_str(src);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                print(a, out);
            }
            out.push(')');
        }
        Expr::Unary(op, a) => {
            out.push_str(match op {
                UnOp::Neg => "-(",
                UnOp::Not => "!(",
            });
            print(a, out);
            out.push(')');
        }
        Expr::Binary(op, a, b) => {
            out.push_str("((");
            print(a, out);
            out.push_str(&format!(") {} (", op.symbol()));
            print(b, out);
            out.push_str("))");
        }
        Expr::Assign(op, t, v) => {
            out.push('(');
            print(t, out);
            out.push_str(&format!(" {} (", op.symbol()));
            print(v, out);
            out.push_str("))");
        }
        Expr::Ternary(c, a, b) => {
            out.push_str("((");
            print(c, out);
            out.push_str(") ? (");
            print(a, out);
            out.push_str(") : (");
            print(b, out);
            out.push_str("))");
        }
        Expr::Seq(items) => {
            out.push('(');
            for (i, a) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str("; ");
                }
                print(a, out);
            }
            out.push(')');
        }
    }
}

fn source(e: &Expr) -> String {
    let mut s = String::new();
    print(e, &mut s);
    s
}

fn parse_src(src: &str) -> Result<Expr, md_eval::CompileError> {
    parse(&Lexer::tokenize(src)?)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Random ASTs printed back to source compile and run to completion.
    #[test]
    fn printed_asts_compile_and_run(ast in expr()) {
        let src = source(&ast);
        let mut vm = vm();
        let prog = Program::compile(&mut vm, &src)
            .map_err(|e| TestCaseError::fail(format!("{src}: {e}")))?;
        // run twice: state left behind by the first run must not break the second
        let _ = prog.run(&mut vm);
        let _ = prog.run(&mut vm);
    }

    /// `print(parse(s))` re-parses to the same AST.
    #[test]
    fn print_parse_round_trip(ast in expr()) {
        let first = parse_src(&source(&ast))
            .map_err(|e| TestCaseError::fail(format!("{}: {e}", source(&ast))))?;
        let src2 = source(&first);
        let second = parse_src(&src2)
            .map_err(|e| TestCaseError::fail(format!("{src2}: {e}")))?;
        prop_assert_eq!(first, second, "source: {}", src2);
    }
}

// ---------------------------------------------------------------------
// Arbitrary input never panics
// ---------------------------------------------------------------------

/// Token snippets for soups. `loop` and `while` are left out on purpose:
/// nested capped loops are 1,048,576^depth iterations, which would make the
/// tests a timing test rather than a panic test.
const SOUP: [&str; 56] = [
    "0", "1", "2", "0.5", ".5", "16", "1e3", "$x1F", "$PI", "$'a'", "a", "b", "x", "reg00",
    "reg99", "megabuf", "gmegabuf", "memcpy", "memset", "exec2", "exec3", "sin", "sqrt", "pow",
    "min", "if", "rand", "int", "abs", "sigmoid", "+", "-", "*", "/", "%", "^", "&", "|", "!", "<",
    ">", "<=", ">=", "==", "!=", "&&", "||", "=", "+=", "?", ":", ";", ",", "(", ")", "[",
];

fn run_if_compiles(src: &str) {
    let mut vm = vm();
    if let Ok(prog) = Program::compile(&mut vm, src) {
        let _ = prog.run(&mut vm);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn compile_and_run_never_panics_bytes(bytes in prop::collection::vec(any::<u8>(), 0..96)) {
        run_if_compiles(&String::from_utf8_lossy(&bytes));
    }

    #[test]
    fn compile_and_run_never_panics_ascii(
        chars in prop::collection::vec(prop::sample::select(
            &b"0123456789abcxyz_ \t\n+-*/%^&|!<>=?:;,()[]{}$.'\"\\@#~`e"[..]), 0..96)
    ) {
        let src: String = chars.into_iter().map(char::from).collect();
        run_if_compiles(&src);
    }

    #[test]
    fn compile_and_run_never_panics_token_soup(
        toks in prop::collection::vec(prop::sample::select(&SOUP[..]), 0..40)
    ) {
        run_if_compiles(&toks.join(" "));
    }
}

// ---------------------------------------------------------------------
// `while` / variable-count `loop`: compile only (running them could take
// 1,048,576^depth iterations)
// ---------------------------------------------------------------------

/// Random ASTs with `while(body)` and `loop(count, body)` nodes whose count
/// is an arbitrary expression (variables, assignments, calls).
fn loopy_expr() -> impl Strategy<Value = Expr> {
    expr().prop_recursive(3, 24, 2, |inner| {
        prop_oneof![
            inner.clone().prop_map(|b| call("while", vec![b])),
            (inner.clone(), inner.clone()).prop_map(|(n, b)| call("loop", vec![n, b])),
            (inner.clone(), inner.clone()).prop_map(|(a, b)| Expr::Seq(vec![a, b])),
        ]
    })
}

const LOOP_SOUP: [&str; 16] = [
    "while(", "loop(", "while", "loop", "(", ")", ")", ",", ";", "a", "x=", "1", "n", "+=",
    "megabuf(", "-",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Well-formed `while`/`loop` programs with arbitrary arguments compile.
    #[test]
    fn printed_loop_asts_compile(ast in loopy_expr()) {
        let src = source(&ast);
        Program::compile(&mut vm(), &src)
            .map_err(|e| TestCaseError::fail(format!("{src}: {e}")))?;
    }

    /// Malformed `while(`/`loop(` soups never panic the compiler.
    #[test]
    fn loop_soup_compile_never_panics(
        toks in prop::collection::vec(
            prop_oneof![
                prop::sample::select(&LOOP_SOUP[..]),
                prop::sample::select(&SOUP[..]),
            ],
            0..48,
        )
    ) {
        let _ = Program::compile(&mut vm(), &toks.join(" "));
    }
}

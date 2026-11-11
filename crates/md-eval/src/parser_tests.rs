//! Parser tests. Expected trees were cross-checked against a char-level
//! model of ns-eel2's `preprocessCode` + the bison tables in `nseel-caltab.c`.

use super::parse;
use crate::ast::{AssignOp, Expr};
use crate::error::CompileError;
use crate::lexer::Lexer;

fn try_p(src: &str) -> Result<Expr, CompileError> {
    parse(&Lexer::tokenize(src)?)
}

/// Parse and print as an S-expression.
fn p(src: &str) -> String {
    match try_p(src) {
        Ok(e) => e.to_string(),
        Err(e) => panic!("{src:?} failed to parse: {e}"),
    }
}

fn err(src: &str) -> CompileError {
    match try_p(src) {
        Ok(e) => panic!("{src:?} unexpectedly parsed as {e}"),
        Err(e) => e,
    }
}

// ---- brief tests -----------------------------------------------------------

/// The brief says `2^3^2` is `2^(3^2)`. ns-eel2 disagrees: the preprocessor
/// rewrites `a^b` to `pow(a,b)` taking only the primary to its left, so the
/// second `^` wraps the first call: `pow(pow(2,3),2)` (= 64).
#[test]
fn power_is_left_associative() {
    assert_eq!(p("2^3^2"), "(pow (pow 2 3) 2)");
}

#[test]
fn assignment_is_right_associative() {
    assert_eq!(p("a = b = 3"), "(= a (= b 3))");
    assert_eq!(p("a += b -= 3"), "(+= a (-= b 3))");
}

#[test]
fn ternary_binds_looser_than_or() {
    assert_eq!(p("a || b ? c : d"), "(? (|| a b) c d)");
    assert_eq!(p("x = a || b ? c : d"), "(= x (? (|| a b) c d))");
}

#[test]
fn megabuf_call_is_valid_lvalue() {
    assert_eq!(p("megabuf(3) = 1"), "(= (_mem 3) 1)");
    assert_eq!(p("gmegabuf(3) += 1"), "(+= (_gmem 3) 1)");
    let e = try_p("megabuf(3) = 1").unwrap();
    assert!(matches!(e, Expr::Assign(AssignOp::Set, _, _)));
}

#[test]
fn unknown_function_is_error_with_offset() {
    let e = err("x = 1 + foo(2)");
    assert_eq!(e.offset, 8);
    assert!(e.message.contains("foo"), "{}", e.message);
}

#[test]
fn empty_statements_are_allowed() {
    assert_eq!(p("a=1;;b=2;"), "(seq (= a 1) (= b 2))");
    assert_eq!(p(";;"), "(seq)");
    assert_eq!(p(""), "(seq)");
    assert_eq!(p("a=1;"), "(= a 1)");
}

// ---- precedence as ns-eel2 really parses it ------------------------------

/// `|` < `&` < `+ -` < `* / ;-in-parens` < unary, all left-associative
/// (bison tables in nseel-caltab.c).
#[test]
fn yacc_arithmetic_precedence() {
    assert_eq!(p("a|b&c"), "(| a (& b c))");
    assert_eq!(p("a&b|c"), "(| (& a b) c)");
    // `&`/`|` sit below `+` (the brief has them above), and above every
    // comparison (see `comparisons`)
    assert_eq!(p("a+b&c"), "(& (+ a b) c)");
    assert_eq!(p("a*b+c"), "(+ (* a b) c)");
    assert_eq!(p("a-b-c"), "(- (- a b) c)");
    assert_eq!(p("a/b/c"), "(/ (/ a b) c)");
    assert_eq!(p("-a*b"), "(* (neg a) b)");
    assert_eq!(p("--a"), "(neg (neg a))");
    assert_eq!(p("a-+b"), "(- a b)");
}

/// `%` is rewritten to `_mod(l,r)` over the adjacent primaries, so it binds
/// tighter than `*`: `2*5%3` is `2*(5%3)`.
#[test]
fn mod_binds_tighter_than_mul() {
    assert_eq!(p("a*b%c"), "(* a (% b c))");
    assert_eq!(p("a%b*c"), "(* (% a b) c)");
    assert_eq!(p("x%2+1"), "(+ (% x 2) 1)");
}

/// `^` takes the primary on its left, so unary minus applies to the power.
#[test]
fn unary_minus_applies_to_power() {
    assert_eq!(p("-2^2"), "(neg (pow 2 2))");
    assert_eq!(p("2^-x*3"), "(* (pow 2 (neg x)) 3)");
    assert_eq!(p("a*b^c"), "(* a (pow b c))");
    assert_eq!(p("sin(x)^2"), "(pow (sin x) 2)");
}

/// `&&` and `||` share one level and associate left.
#[test]
fn and_or_share_precedence() {
    assert_eq!(p("a||b&&c"), "(&& (|| a b) c)");
    assert_eq!(p("a&&b||c"), "(|| (&& a b) c)");
}

/// Comparisons take everything back to `( , ; ? :` on the left and
/// everything up to `, ) ; ? : || &&` on the right: they sit below
/// arithmetic and bitwise ops, and chain to the right.
#[test]
fn comparisons() {
    assert_eq!(p("a+1 == b*2"), "(== (+ a 1) (* b 2))");
    assert_eq!(p("a & b == c"), "(== (& a b) c)");
    assert_eq!(p("a < b == c"), "(< a (== b c))");
    assert_eq!(p("a < b || c >= d"), "(|| (< a b) (>= c d))");
    assert_eq!(p("a != b"), "(!= a b)");
    assert_eq!(p("a <= b"), "(<= a b)");
    assert_eq!(p("a > b"), "(> a b)");
}

#[test]
fn not_takes_one_operand() {
    assert_eq!(p("!a+b"), "(+ (! a) b)");
    assert_eq!(p("!(a<b)"), "(! (< a b))");
    assert_eq!(p("!!a"), "(! (! a))");
}

/// `=` takes only the primary on its left: `a + b = 3` is `a + (b = 3)`.
#[test]
fn assignment_left_operand_is_a_primary() {
    assert_eq!(p("a + b = 3"), "(+ a (= b 3))");
    assert_eq!(p("-x = 3"), "(neg (= x 3))");
    assert_eq!(p("x = a ? b : c"), "(= x (? a b c))");
    assert_eq!(p("x = a || b"), "(= x (|| a b))");
}

#[test]
fn compound_assignment_ops_including_pow() {
    for (src, op) in [
        ("x += 1", "+="),
        ("x -= 1", "-="),
        ("x *= 1", "*="),
        ("x /= 1", "/="),
        ("x %= 1", "%="),
        ("x |= 1", "|="),
        ("x &= 1", "&="),
        ("x ^= 1", "^="),
    ] {
        assert_eq!(p(src), format!("({op} x 1)"));
    }
}

#[test]
fn non_lvalue_assignment_is_an_error() {
    // ns-eel2 compiles these by writing to a scratch slot; nothing useful.
    let e = err("(a+b) = 3");
    assert!(e.message.contains("assign"), "{}", e.message);
    err("5 = 3");
    err("sin(x) = 3");
}

#[test]
fn ternary_forms() {
    assert_eq!(p("a ? b : c"), "(? a b c)");
    assert_eq!(p("a ? b"), "(? a b 0)");
    assert_eq!(p("a ? b ? c : d : e"), "(? a (? b c d) e)");
    assert_eq!(p("a ? b : c ? d : e"), "(? a b (? c d e))");
    assert_eq!(p("x + y > 1 ? 1 : 0"), "(? (> (+ x y) 1) 1 0)");
    assert_eq!(p("if(a, b, c)"), "(? a b c)");
    assert_eq!(p("min(a ? b : c, d)"), "(min (? a b c) d)");
}

/// F14: `a[b]` is `megabuf(a+b)`; `gmem[b]` is `gmegabuf(b)`; `]` optional.
#[test]
fn brackets_index_megabuf() {
    assert_eq!(p("x[y]"), "(_mem (+ x y))");
    assert_eq!(p("x[y] = 3"), "(= (_mem (+ x y)) 3)");
    assert_eq!(p("x[]"), "(_mem x)");
    assert_eq!(p("x[0]"), "(_mem x)");
    assert_eq!(p("x[1][2]"), "(_mem (+ (_mem (+ x 1)) 2))");
    assert_eq!(p("gmem[3] = 1"), "(= (_gmem 3) 1)");
    assert_eq!(p("GMEM[i+1]"), "(_gmem (+ i 1))");
    assert_eq!(p("buf[i+1]*2"), "(* (_mem (+ buf (+ i 1))) 2)");
    assert_eq!(p("2^x[1]"), "(_mem (+ (pow 2 x) 1))");
}

/// Inside parentheses ns-eel2 turns `;` into its `%` token (exec2, at the
/// `*` level) when the next non-blank char is alnum, `(`, `_`, `!` or `$`,
/// and drops it otherwise.
#[test]
fn semicolon_inside_parens() {
    assert_eq!(p("if(a, b; c, d)"), "(? a (seq b c) d)");
    assert_eq!(p("loop(3, x+=1; y+=2)"), "(loop 3 (seq (+= x 1) (+= y 2)))");
    assert_eq!(p("(a; b; c)"), "(seq a b c)");
    // at the `*` level, not lowest:
    assert_eq!(p("(a+1; b)"), "(+ a (seq 1 b))");
    // dropped when not followed by an operand start:
    assert_eq!(p("(a; -b)"), "(- a b)");
    assert_eq!(p("exec2(a, b;)"), "(exec2 a b)");
    assert_eq!(p("(a;;b)"), "(seq a b)");
    assert_eq!(p("(x = 1; !y)"), "(seq (= x 1) (! y))");
    assert_eq!(
        p("while(i += 1; i < 10)"),
        "(while (seq (+= i 1) (< i 10)))"
    );
}

#[test]
fn aliases_and_case_insensitive_names() {
    assert_eq!(p("IF(a,b,c)"), "(? a b c)");
    assert_eq!(p("bnot(a)"), "(! a)");
    assert_eq!(p("assign(x, 2)"), "(= x 2)");
    assert_eq!(p("equal(a,b)"), "(== a b)");
    assert_eq!(p("below(a,b)"), "(< a b)");
    assert_eq!(p("above(a,b)"), "(> a b)");
    assert_eq!(p("MegaBuf(1)"), "(_mem 1)");
    assert_eq!(p("gmegabuf(1)"), "(_gmem 1)");
    assert_eq!(p("int(2.5)"), "(floor 2.5)");
    assert_eq!(p("Sin(x)"), "(sin x)");
    assert_eq!(p("pow(a,b)"), "(pow a b)");
    assert_eq!(p("_and(a,b) + _or(a,b)"), "(+ (&& a b) (|| a b))");
    assert_eq!(p("_mod(a,b)"), "(% a b)");
}

#[test]
fn every_builtin_parses_with_its_arity() {
    let ok = [
        "abs(1)",
        "sqr(1)",
        "sqrt(1)",
        "exp(1)",
        "log(1)",
        "log10(1)",
        "sin(1)",
        "cos(1)",
        "tan(1)",
        "asin(1)",
        "acos(1)",
        "atan(1)",
        "atan2(1,2)",
        "min(1,2)",
        "max(1,2)",
        "sign(1)",
        "floor(1)",
        "ceil(1)",
        "invsqrt(1)",
        "sigmoid(1,2)",
        "band(1,2)",
        "bor(1,2)",
        "rand(1)",
        "exec2(1,2)",
        "exec3(1,2,3)",
        "loop(1,2)",
        "while(1)",
        "memcpy(1,2,3)",
        "memset(1,2,3)",
        "freembuf(1)",
    ];
    for src in ok {
        try_p(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    }
}

#[test]
fn wrong_arity_is_error_at_name() {
    let e = err("y = sin(1, 2)");
    assert_eq!(e.offset, 4);
    let e = err("if(a, b)");
    assert_eq!(e.offset, 0);
}

#[test]
fn function_name_without_call_is_error() {
    let e = err("x = sin + 1");
    assert_eq!(e.offset, 4);
}

#[test]
fn syntax_errors() {
    err("a b");
    err("(a");
    err("a)");
    err("a +");
    err("x = ");
    err("a : b");
    err("= 3");
    err("f()");
    err("sin()");
    err("a ]");
}

#[test]
fn deep_nesting_is_an_error_not_a_stack_overflow() {
    let deep = format!("{}1{}", "(".repeat(5000), ")".repeat(5000));
    err(&deep);
    let chain = "x=".repeat(5000) + "1";
    err(&chain);
    let nots = "!".repeat(5000) + "x";
    err(&nots);
    let negs = "-".repeat(5000) + "x";
    err(&negs);
    // nesting just under the bounds is fine (and fits a 2 MiB test thread)
    let ok = format!("{}1{}", "(".repeat(350), ")".repeat(350));
    assert_eq!(p(&ok), "1");
    try_p(&("x=".repeat(190) + "1")).unwrap();
    try_p(&("-".repeat(350) + "x")).unwrap();
}

#[test]
fn syntax_zoo_lines() {
    assert_eq!(p("v = 2^3^0.5"), "(= v (pow (pow 2 3) 0.5))");
    assert_eq!(
        p("z = (mid == treb) || (bass >= 1 && !(treb < 0.5))"),
        "(= z (|| (== mid treb) (&& (>= bass 1) (! (< treb 0.5)))))"
    );
    assert_eq!(
        p("q3 = exec3(assign(tmp, 1), tmp += 1, tmp)"),
        "(= q3 (exec3 (= tmp 1) (+= tmp 1) tmp))"
    );
    assert_eq!(p("mask = $x1F"), "(= mask 31)");
}

#[test]
fn trailing_eof_token_is_optional() {
    let mut toks = Lexer::tokenize("a=1").unwrap();
    toks.pop();
    assert_eq!(parse(&toks).unwrap().to_string(), "(= a 1)");
}

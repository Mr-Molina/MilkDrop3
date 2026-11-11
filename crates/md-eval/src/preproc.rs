//! Token-level port of ns-eel2's text preprocessor (`preprocessCode`,
//! `nseel-compiler.c` lines 1011-1428).
//!
//! ns-eel2's bison grammar only knows `+ - * / % & |`, unary `+ -`, calls
//! and parentheses. Every other operator is rewritten *textually* into a
//! call before parsing, and that rewrite, not a precedence table, decides
//! how the operator binds:
//!
//! * the left operand is found by scanning back over the already-rewritten
//!   output, skipping balanced `( )` groups, either over adjacent primaries
//!   only (`=`, `op=`, `%`, `^`, `[`) or back to the nearest
//!   `: ( , ; ? %` (comparisons, `||`, `&&`, `?`);
//! * the right operand is found by scanning forward over the raw input with
//!   a per-operator stop set (see [`Right`]) and is rewritten recursively.
//!
//! Also: `;` inside parentheses becomes the grammar's `%` token (exec2: the
//! `%` emitted here always means "sequence", since a source `%` becomes
//! `_mod(..)`) when the next token can start an operand, and is dropped
//! otherwise. `x[i]` becomes `_mem((x)+(i))`, `gmem[i]` becomes `_gmem(i)`.
//!
//! Synthetic tokens carry the byte offset of the operator they replace.

use crate::error::CompileError;
use crate::token::Tok;

pub(crate) type Spanned = (Tok, usize);

/// Recursion bound for nested right operands (`a=b=c=...`, `!!!...x`).
const MAX_DEPTH: usize = 200;

/// How far back the left operand reaches (`lscan` in the C).
#[derive(Clone, Copy)]
enum Left {
    /// No left operand (`!`).
    None,
    /// Adjacent primaries and `( )` groups only (`lscan == 0`).
    Primary,
    /// Back to the nearest `: ( , ; ? %` (`lscan == 1`).
    Wide,
}

/// Where the right operand stops (`rscan` in the C). Every kind stops at a
/// depth-0 `;` or `,` and at an unbalanced `)`.
#[derive(Clone, Copy, PartialEq)]
enum Right {
    /// `rscan == 0`: one operand: `:`, or any operator once a primary or
    /// `(` has been seen (leading unary operators are skipped).
    Primary,
    /// `rscan == 2`: `:`, `?`, `||`, `&&`.
    Compare,
    /// `rscan == 3`: a `:` with no matching `?`.
    Assign,
    /// `rscan == 4`: the second unmatched `:`.
    Ternary,
    /// `rscan == 5`: an unbalanced `]`.
    Index,
}

#[derive(Clone, Copy)]
enum Rewrite {
    Call(&'static str),
    Index,
    Not,
    Ternary,
}

/// The `preprocSymbols` table, minus its order-dependence (the lexer has
/// already split multi-character operators the same way).
fn rule(t: &Tok) -> Option<(Left, Right, Rewrite)> {
    use Left::{Primary as LP, Wide as LW};
    use Rewrite::Call;
    use Right::{Assign as RA, Compare as RC, Primary as RP};
    let r = match t {
        Tok::LBracket => (LP, Right::Index, Rewrite::Index),
        Tok::Op(op) => match *op {
            "+=" => (LP, RA, Call("_addop")),
            "-=" => (LP, RA, Call("_subop")),
            "%=" => (LP, RA, Call("_modop")),
            "|=" => (LP, RA, Call("_orop")),
            "&=" => (LP, RA, Call("_andop")),
            "/=" => (LP, RA, Call("_divop")),
            "*=" => (LP, RA, Call("_mulop")),
            "^=" => (LP, RA, Call("_powop")),
            "==" => (LW, RC, Call("_equal")),
            "<=" => (LW, RC, Call("_beleq")),
            ">=" => (LW, RC, Call("_aboeq")),
            "<" => (LW, RC, Call("_below")),
            ">" => (LW, RC, Call("_above")),
            "!=" => (LW, RC, Call("_noteq")),
            "||" => (LW, RC, Call("_or")),
            "&&" => (LW, RC, Call("_and")),
            "=" => (LP, RA, Call("_set")),
            "%" => (LP, RP, Call("_mod")),
            "^" => (LP, RP, Call("pow")),
            "!" => (Left::None, RP, Rewrite::Not),
            "?" => (LW, Right::Ternary, Rewrite::Ternary),
            _ => return None,
        },
        _ => return None,
    };
    Some(r)
}

fn is_primary(t: &Tok) -> bool {
    matches!(t, Tok::Num(_) | Tok::Ident(_))
}

/// Rewrite a token stream (without the trailing `Eof`).
pub(crate) fn preprocess(src: &[Spanned]) -> Result<Vec<Spanned>, CompileError> {
    run(src, 0)
}

fn run(src: &[Spanned], depth: usize) -> Result<Vec<Spanned>, CompileError> {
    if depth > MAX_DEPTH {
        let off = src.first().map_or(0, |t| t.1);
        return Err(CompileError::new(off, "expression nested too deeply"));
    }
    let mut buf: Vec<Spanned> = Vec::with_capacity(src.len() + 8);
    let mut parens = 0usize;
    let mut i = 0;
    while i < src.len() {
        let (tok, off) = &src[i];
        let off = *off;
        i += 1;
        match tok {
            Tok::LParen => {
                parens += 1;
                buf.push((Tok::LParen, off));
            }
            Tok::RParen => {
                parens = parens.saturating_sub(1);
                buf.push((Tok::RParen, off));
            }
            Tok::Semi if parens > 0 => {
                // C: '%' if the next non-blank char is alnum ( _ ! $, else
                // dropped. A number written `.5` would be dropped in C (a
                // syntax error there); it is accepted here.
                let starts_operand = src.get(i).is_some_and(|(t, _)| {
                    is_primary(t) || matches!(t, Tok::LParen | Tok::Op("!" | "!="))
                });
                if starts_operand {
                    buf.push((Tok::Op("%"), off));
                }
            }
            _ => match rule(tok) {
                None => buf.push((tok.clone(), off)),
                Some((left, right, rewrite)) => {
                    let lhs = match left {
                        Left::None => Vec::new(),
                        Left::Primary | Left::Wide => {
                            let start = left_start(&buf, left);
                            buf.split_off(start)
                        }
                    };
                    let end = right_end(src, i, right);
                    let rhs = run(&src[i..end], depth + 1)?;
                    i = end;
                    match rewrite {
                        Rewrite::Call(name) => call(&mut buf, name, off, &[&lhs, &rhs]),
                        Rewrite::Not => call(&mut buf, "_not", off, &[&rhs]),
                        Rewrite::Ternary => ternary(&mut buf, off, &lhs, &rhs),
                        Rewrite::Index => {
                            index(&mut buf, off, &lhs, &rhs);
                            if matches!(src.get(i), Some((Tok::RBracket, _))) {
                                i += 1;
                            }
                        }
                    }
                }
            },
        }
    }
    Ok(buf)
}

/// Start of the left operand at the end of `buf`.
fn left_start(buf: &[Spanned], kind: Left) -> usize {
    let mut depth = 0i32;
    let mut j = buf.len();
    while j > 0 {
        let t = &buf[j - 1].0;
        match t {
            Tok::RParen => depth += 1,
            Tok::LParen => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            _ if depth == 0 => {
                let stop = match kind {
                    Left::Primary => !is_primary(t),
                    _ => matches!(t, Tok::Op(":" | "?" | "%") | Tok::Comma | Tok::Semi),
                };
                if stop {
                    break;
                }
            }
            _ => {}
        }
        j -= 1;
    }
    j
}

/// End (exclusive) of the right operand starting at `src[start]`.
fn right_end(src: &[Spanned], start: usize, kind: Right) -> usize {
    let (mut depth, mut q, mut brackets) = (0i32, 0i32, 0i32);
    let mut had = false;
    let mut j = start;
    while j < src.len() {
        let t = &src[j].0;
        match t {
            Tok::LParen => {
                had = true;
                depth += 1;
            }
            Tok::RParen => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
            }
            _ if depth == 0 => {
                if matches!(t, Tok::Semi | Tok::Comma) {
                    break;
                }
                let stop = match kind {
                    Right::Primary => {
                        let p = is_primary(t);
                        let stop = matches!(t, Tok::Op(":")) || (!p && had);
                        had |= p;
                        stop
                    }
                    Right::Compare => matches!(t, Tok::Op(":" | "?" | "||" | "&&")),
                    Right::Assign | Right::Ternary => {
                        match t {
                            Tok::Op(":") => q -= 1,
                            Tok::Op("?") => q += 1,
                            _ => {}
                        }
                        q < if kind == Right::Assign { 0 } else { -1 }
                    }
                    Right::Index => {
                        match t {
                            Tok::LBracket => brackets += 1,
                            Tok::RBracket => brackets -= 1,
                            _ => {}
                        }
                        brackets < 0
                    }
                };
                if stop {
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    j
}

/// Emit `name(arg, arg, ...)`.
fn call(buf: &mut Vec<Spanned>, name: &str, off: usize, args: &[&[Spanned]]) {
    buf.push((Tok::Ident(name.to_string()), off));
    buf.push((Tok::LParen, off));
    for (k, a) in args.iter().enumerate() {
        if k > 0 {
            buf.push((Tok::Comma, off));
        }
        buf.extend_from_slice(a);
    }
    buf.push((Tok::RParen, off));
}

/// `l ? a : b` -> `_if(l, a, b)`; a missing branch is `0`.
fn ternary(buf: &mut Vec<Spanned>, off: usize, cond: &[Spanned], rhs: &[Spanned]) {
    let mut depth = 0i32;
    let mut colon = rhs.len();
    for (k, (t, _)) in rhs.iter().enumerate() {
        match t {
            Tok::LParen => depth += 1,
            Tok::RParen => depth -= 1,
            Tok::Op(":") if depth == 0 => {
                colon = k;
                break;
            }
            _ => {}
        }
    }
    let zero = [(Tok::Num(0.0), off)];
    let then = &rhs[..colon];
    let other = rhs.get(colon + 1..).unwrap_or(&[]);
    let then = if then.is_empty() { &zero[..] } else { then };
    let other = if other.is_empty() { &zero[..] } else { other };
    call(buf, "_if", off, &[cond, then, other]);
}

/// `x[i]` -> `_mem((x)+(i))` (or `_mem(x)` when `i` is empty or `0`);
/// `gmem[i]` -> `_gmem(i)`.
fn index(buf: &mut Vec<Spanned>, off: usize, base: &[Spanned], idx: &[Spanned]) {
    // C: the left text starts with "gmem" followed by end or whitespace.
    let is_gmem = match base {
        [(Tok::Ident(g), _)] => g == "gmem",
        [(Tok::Ident(g), g_off), (next, n_off), ..] => {
            g == "gmem" && (!matches!(next, Tok::LParen) || *n_off > g_off + 4)
        }
        _ => false,
    };
    if is_gmem {
        let zero = [(Tok::Num(0.0), off)];
        call(
            buf,
            "_gmem",
            off,
            &[if idx.is_empty() { &zero } else { idx }],
        );
        return;
    }
    if idx.is_empty() || matches!(idx, [(Tok::Num(z), _)] if *z == 0.0) {
        call(buf, "_mem", off, &[base]);
        return;
    }
    let mut sum = Vec::with_capacity(base.len() + idx.len() + 5);
    sum.push((Tok::LParen, off));
    sum.extend_from_slice(base);
    sum.push((Tok::RParen, off));
    sum.push((Tok::Op("+"), off));
    sum.push((Tok::LParen, off));
    sum.extend_from_slice(idx);
    sum.push((Tok::RParen, off));
    call(buf, "_mem", off, &[&sum]);
}

//! The preprocessor: `#define` (object- and function-like), `#undef`,
//! `#pragma` (ignored), comments and line continuation.
//!
//! Design: the primary entry point is [`preprocess_tokens`], which returns
//! tokens whose `line`/`col` refer to the ORIGINAL source; tokens that come
//! from a macro expansion report the position of the outermost invocation.
//! [`preprocess`] is a thin convenience that renders those tokens back to
//! text; the rendering is lossy (spacing is normalised and positions are not
//! preserved), so the translator pipeline uses the token form.
//!
//! Choices, mirroring what fxc accepts that MilkDrop shaders use (the corpus
//! and `include.fx` use only `#define`):
//! - `#include`, `#if`/`#ifdef`/`#ifndef`/`#elif`/`#else`/`#endif`, `#error`,
//!   `#line` and anything else: a `TranslateError` at the directive.
//! - `#pragma ...` and a bare `#`: ignored.
//! - `#`/`##` (stringising, token pasting) and variadic macros: unsupported
//!   (`#` is a lexer error); nothing in `include.fx` or the corpus uses them.
//! - Macro recursion is guarded: a macro is never re-expanded inside its own
//!   expansion (C semantics), so `#define A A+1` yields `A + 1`.
//! - A function-like macro name not followed by `(` is left alone. Arguments
//!   may span lines and contain nested parentheses.

use std::collections::HashMap;

use crate::lexer::{clean, lex, Ch};
use crate::token::{TokKind, Token};
use crate::TranslateError;

struct Macro {
    /// `None` for object-like macros.
    params: Option<Vec<String>>,
    body: Vec<Token>,
}

type Macros = HashMap<String, Macro>;

/// Preprocess `src` and return tokens carrying original source positions.
pub fn preprocess_tokens(src: &str) -> Result<Vec<Token>, TranslateError> {
    let chars = clean(src)?;
    let mut macros = Macros::new();
    let mut out = Vec::new();
    let mut segment: Vec<Token> = Vec::new();
    for line in chars.split(|c| c.c == '\n') {
        let first = line.iter().position(|c| !c.c.is_whitespace());
        match first {
            Some(f) if line[f].c == '#' => {
                // A directive ends the current text segment: expand it with
                // the macro table as it stood.
                flush(&mut segment, &macros, &mut out)?;
                directive(&line[f..], &mut macros)?;
            }
            _ => segment.extend(lex(line)?),
        }
    }
    flush(&mut segment, &macros, &mut out)?;
    Ok(out)
}

/// Convenience wrapper: preprocess and render back to text (tokens joined by
/// single spaces, a newline wherever the source line number advances).
/// Positions are NOT recoverable from the result; prefer [`preprocess_tokens`].
pub fn preprocess(src: &str) -> Result<String, TranslateError> {
    let toks = preprocess_tokens(src)?;
    let mut s = String::new();
    let mut line = 0;
    for t in &toks {
        if !s.is_empty() {
            s.push(if t.line != line { '\n' } else { ' ' });
        }
        line = t.line;
        match &t.kind {
            TokKind::Ident(i) => s.push_str(i),
            TokKind::Int(v) => s.push_str(&v.to_string()),
            TokKind::Float(v) => s.push_str(&format!("{v:?}")),
            TokKind::Op(o) => s.push_str(o),
        }
    }
    Ok(s)
}

fn flush(
    segment: &mut Vec<Token>,
    macros: &Macros,
    out: &mut Vec<Token>,
) -> Result<(), TranslateError> {
    if segment.is_empty() {
        return Ok(());
    }
    let toks = std::mem::take(segment);
    out.extend(expand(&toks, macros, &mut Vec::new())?);
    Ok(())
}

fn ident_at(line: &[Ch], mut i: usize) -> Option<(String, usize)> {
    let start = i;
    while i < line.len() && (line[i].c.is_ascii_alphanumeric() || line[i].c == '_') {
        i += 1;
    }
    if i == start || line[start].c.is_ascii_digit() {
        return None;
    }
    Some((line[start..i].iter().map(|c| c.c).collect(), i))
}

fn skip_ws(line: &[Ch], mut i: usize) -> usize {
    while i < line.len() && line[i].c.is_whitespace() {
        i += 1;
    }
    i
}

/// `line` starts at the `#`.
fn directive(line: &[Ch], macros: &mut Macros) -> Result<(), TranslateError> {
    let hash = line[0];
    let i = skip_ws(line, 1);
    if i >= line.len() {
        return Ok(()); // null directive
    }
    let Some((name, after)) = ident_at(line, i) else {
        return Err(TranslateError::new(
            line[i].line,
            line[i].col,
            "invalid preprocessor directive",
        ));
    };
    match name.as_str() {
        "pragma" => Ok(()),
        "define" => define(line, after, hash, macros),
        "undef" => {
            let j = skip_ws(line, after);
            let Some((n, _)) = ident_at(line, j) else {
                return Err(TranslateError::new(
                    hash.line,
                    hash.col,
                    "#undef expects a macro name",
                ));
            };
            macros.remove(&n);
            Ok(())
        }
        other => Err(TranslateError::new(
            hash.line,
            hash.col,
            format!("unsupported preprocessor directive #{other}"),
        )),
    }
}

fn define(line: &[Ch], after: usize, hash: Ch, macros: &mut Macros) -> Result<(), TranslateError> {
    let bad = |m: &str| TranslateError::new(hash.line, hash.col, m.to_string());
    let j = skip_ws(line, after);
    let Some((name, mut k)) = ident_at(line, j) else {
        return Err(bad("#define expects a macro name"));
    };
    let mut params = None;
    if k < line.len() && line[k].c == '(' {
        k += 1;
        let mut ps = Vec::new();
        loop {
            k = skip_ws(line, k);
            if k < line.len() && line[k].c == ')' && ps.is_empty() {
                k += 1;
                break;
            }
            let Some((p, nk)) = ident_at(line, k) else {
                return Err(bad("malformed #define parameter list"));
            };
            ps.push(p);
            k = skip_ws(line, nk);
            match line.get(k).map(|c| c.c) {
                Some(',') => k += 1,
                Some(')') => {
                    k += 1;
                    break;
                }
                _ => return Err(bad("malformed #define parameter list")),
            }
        }
        params = Some(ps);
    }
    let body = lex(&line[k..])?;
    macros.insert(name, Macro { params, body });
    Ok(())
}

fn expand(
    toks: &[Token],
    macros: &Macros,
    active: &mut Vec<String>,
) -> Result<Vec<Token>, TranslateError> {
    let mut out = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        let mac = t
            .ident()
            .filter(|n| !active.iter().any(|a| a == n))
            .and_then(|n| macros.get(n).map(|m| (n, m)));
        let Some((name, m)) = mac else {
            out.push(t.clone());
            i += 1;
            continue;
        };
        let mut replacement = Vec::new();
        match &m.params {
            None => {
                replacement.extend(m.body.iter().cloned());
                i += 1;
            }
            Some(params) => {
                if !toks.get(i + 1).is_some_and(|n| n.is_op("(")) {
                    out.push(t.clone());
                    i += 1;
                    continue;
                }
                let (args, next) = collect_args(toks, i + 2, t)?;
                let n_args = if params.is_empty() && args.len() == 1 && args[0].is_empty() {
                    0
                } else {
                    args.len()
                };
                if n_args != params.len() {
                    return Err(TranslateError::new(
                        t.line,
                        t.col,
                        format!(
                            "macro {name} expects {} argument(s), got {n_args}",
                            params.len()
                        ),
                    ));
                }
                let expanded: Vec<Vec<Token>> = args
                    .iter()
                    .map(|a| expand(a, macros, active))
                    .collect::<Result<_, _>>()?;
                for b in &m.body {
                    match b.ident().and_then(|id| params.iter().position(|p| p == id)) {
                        Some(p) => replacement.extend(expanded[p].iter().cloned()),
                        None => replacement.push(b.clone()),
                    }
                }
                i = next;
            }
        }
        active.push(name.to_string());
        let mut result = expand(&replacement, macros, active)?;
        active.pop();
        for r in &mut result {
            r.line = t.line;
            r.col = t.col;
        }
        out.extend(result);
    }
    Ok(out)
}

/// Collect comma-separated arguments from `start` (just after the `(`) up to
/// the matching `)`. Returns the args and the index after the `)`.
fn collect_args(
    toks: &[Token],
    start: usize,
    name: &Token,
) -> Result<(Vec<Vec<Token>>, usize), TranslateError> {
    let mut args: Vec<Vec<Token>> = vec![Vec::new()];
    let mut depth = 1;
    let mut j = start;
    while j < toks.len() {
        let t = &toks[j];
        if t.is_op("(") {
            depth += 1;
        } else if t.is_op(")") {
            depth -= 1;
            if depth == 0 {
                return Ok((args, j + 1));
            }
        } else if t.is_op(",") && depth == 1 {
            args.push(Vec::new());
            j += 1;
            continue;
        }
        args.last_mut().expect("non-empty").push(t.clone());
        j += 1;
    }
    Err(TranslateError::new(
        name.line,
        name.col,
        "unterminated macro invocation",
    ))
}

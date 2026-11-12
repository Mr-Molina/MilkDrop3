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
use crate::token::{TokKind, Token, MAX_TOKEN_LEN};
use crate::TranslateError;

struct Macro {
    /// `None` for object-like macros. Maps each parameter name to its index
    /// (the first occurrence wins), so substitution is O(1) per body token.
    params: Option<Params>,
    body: Vec<Token>,
}

struct Params {
    count: usize,
    index: HashMap<String, usize>,
}

type Macros = HashMap<String, Macro>;

/// Preprocess `src` and return tokens carrying original source positions.
pub fn preprocess_tokens(src: &str) -> Result<Vec<Token>, TranslateError> {
    let chars = clean(src)?;
    let mut macros = Macros::new();
    let mut limits = Limits { produced: 0 };
    let mut out = Vec::new();
    let mut segment: Vec<Token> = Vec::new();
    for line in chars.split(|c| c.c == '\n') {
        let first = line.iter().position(|c| !c.c.is_whitespace());
        match first {
            Some(f) if line[f].c == '#' => {
                // A directive ends the current text segment: expand it with
                // the macro table as it stood.
                flush(&mut segment, &macros, &mut limits, &mut out)?;
                directive(&line[f..], &mut macros)?;
            }
            _ => segment.extend(lex(line)?),
        }
    }
    flush(&mut segment, &macros, &mut limits, &mut out)?;
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
    limits: &mut Limits,
    out: &mut Vec<Token>,
) -> Result<(), TranslateError> {
    if segment.is_empty() {
        return Ok(());
    }
    let toks = std::mem::take(segment);
    out.extend(expand(&toks, macros, &mut Vec::new(), limits)?);
    Ok(())
}

/// The identifier starting at `i` and the index after it; `Ok(None)` if there
/// is none. Directive names and parameters obey the lexer's length cap too.
fn ident_at(line: &[Ch], mut i: usize) -> Result<Option<(String, usize)>, TranslateError> {
    let start = i;
    while i < line.len() && (line[i].c.is_ascii_alphanumeric() || line[i].c == '_') {
        i += 1;
    }
    if i == start || line[start].c.is_ascii_digit() {
        return Ok(None);
    }
    if i - start > MAX_TOKEN_LEN {
        return Err(TranslateError::new(
            line[start].line,
            line[start].col,
            format!("identifier too long (max {MAX_TOKEN_LEN} characters)"),
        ));
    }
    Ok(Some((line[start..i].iter().map(|c| c.c).collect(), i)))
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
    let Some((name, after)) = ident_at(line, i)? else {
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
            let Some((n, _)) = ident_at(line, j)? else {
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
    let Some((name, mut k)) = ident_at(line, j)? else {
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
            let Some((p, nk)) = ident_at(line, k)? else {
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
        let count = ps.len();
        let mut index = HashMap::with_capacity(count);
        for (n, p) in ps.into_iter().enumerate() {
            index.entry(p).or_insert(n);
        }
        params = Some(Params { count, index });
    }
    let body = lex(&line[k..])?;
    macros.insert(name, Macro { params, body });
    Ok(())
}

/// Maximum macro-expansion nesting (replacement rescans plus argument
/// pre-expansion). Far above `include.fx` (depth 2) and any real preset.
const MAX_DEPTH: usize = 128;
/// Maximum tokens examined by expansion in one `preprocess_tokens` call: one
/// budget for the whole input, counting every token at every nesting level
/// (so pass-through tokens and argument copies count too), charged BEFORE the
/// copy is allocated. `include.fx` is about 1-2 thousand tokens and corpus
/// shaders a few thousand more; this stops exponential ("billion laughs")
/// expansion.
///
/// Resource guarantee, for ANY input:
/// - Memory: every token held by expansion was charged, and a token is a
///   fixed-size `PTok` (40 bytes) whose identifier text is a shared `Arc<str>`
///   of at most [`MAX_TOKEN_LEN`] chars created only by the lexer, so a copy
///   never copies text. Worst case is about `MAX_TOKENS * size_of::<PTok>()`
///   (~80 MB, plus `Vec` growth slack) on top of the lexed source.
/// - Time: per charged token the work is O(MAX_TOKEN_LEN) (hash lookups of a
///   bounded identifier, O(1) parameter lookup via `Params::index`) plus
///   O(MAX_DEPTH) for the active-macro check, so expansion is linear in
///   `MAX_TOKENS` plus the input size.
const MAX_TOKENS: usize = 2_000_000;

/// A token plus the C "painted blue" mark: an identifier met while its own
/// macro was being expanded is never expanded again, on any later rescan.
#[derive(Clone)]
struct PTok {
    tok: Token,
    blue: bool,
}

struct Limits {
    produced: usize,
}

impl Limits {
    /// Charge `n` tokens (examined or about to be allocated) against the
    /// whole-input budget; fails BEFORE the caller allocates them.
    fn charge(&mut self, n: usize, at: &Token) -> Result<(), TranslateError> {
        self.produced = self.produced.saturating_add(n);
        if self.produced > MAX_TOKENS {
            return Err(TranslateError::new(
                at.line,
                at.col,
                "macro expansion too large",
            ));
        }
        Ok(())
    }
}

fn expand(
    toks: &[Token],
    macros: &Macros,
    active: &mut Vec<String>,
    limits: &mut Limits,
) -> Result<Vec<Token>, TranslateError> {
    let input: Vec<PTok> = toks
        .iter()
        .map(|t| PTok {
            tok: t.clone(),
            blue: false,
        })
        .collect();
    let out = expand_p(&input, macros, active, 0, limits)?;
    Ok(out.into_iter().map(|p| p.tok).collect())
}

fn expand_p(
    toks: &[PTok],
    macros: &Macros,
    active: &mut Vec<String>,
    depth: usize,
    limits: &mut Limits,
) -> Result<Vec<PTok>, TranslateError> {
    let mut out = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        let pt = &toks[i];
        let t = &pt.tok;
        if depth > MAX_DEPTH {
            return Err(TranslateError::new(
                t.line,
                t.col,
                "macro expansion nested too deeply",
            ));
        }
        limits.charge(1, t)?;
        let Some((name, m)) = t.ident().and_then(|n| macros.get(n).map(|m| (n, m))) else {
            out.push(pt.clone());
            i += 1;
            continue;
        };
        if pt.blue || active.iter().any(|a| a == name) {
            out.push(PTok {
                tok: t.clone(),
                blue: true,
            });
            i += 1;
            continue;
        }
        let mut replacement: Vec<PTok> = Vec::new();
        match &m.params {
            None => {
                limits.charge(m.body.len(), t)?;
                replacement.extend(m.body.iter().map(|b| PTok {
                    tok: b.clone(),
                    blue: false,
                }));
                i += 1;
            }
            Some(params) => {
                if !toks.get(i + 1).is_some_and(|n| n.tok.is_op("(")) {
                    out.push(pt.clone());
                    i += 1;
                    continue;
                }
                let (args, next) = collect_args(toks, i + 2, t)?;
                limits.charge(args.iter().map(Vec::len).sum(), t)?;
                let n_args = if params.count == 0 && args.len() == 1 && args[0].is_empty() {
                    0
                } else {
                    args.len()
                };
                if n_args != params.count {
                    return Err(TranslateError::new(
                        t.line,
                        t.col,
                        format!(
                            "macro {name} expects {} argument(s), got {n_args}",
                            params.count
                        ),
                    ));
                }
                let expanded: Vec<Vec<PTok>> = args
                    .iter()
                    .map(|a| expand_p(a, macros, active, depth + 1, limits))
                    .collect::<Result<_, _>>()?;
                for b in &m.body {
                    match b.ident().and_then(|id| params.index.get(id).copied()) {
                        Some(p) => {
                            limits.charge(expanded[p].len(), t)?;
                            replacement.extend(expanded[p].iter().cloned());
                        }
                        None => replacement.push(PTok {
                            tok: b.clone(),
                            blue: false,
                        }),
                    }
                }
                i = next;
            }
        }
        active.push(name.to_string());
        let mut result = expand_p(&replacement, macros, active, depth + 1, limits)?;
        active.pop();
        for r in &mut result {
            r.tok.line = t.line;
            r.tok.col = t.col;
        }
        out.extend(result);
    }
    Ok(out)
}

/// Collect comma-separated arguments from `start` (just after the `(`) up to
/// the matching `)`. Returns the args and the index after the `)`.
fn collect_args(
    toks: &[PTok],
    start: usize,
    name: &Token,
) -> Result<(Vec<Vec<PTok>>, usize), TranslateError> {
    let mut args: Vec<Vec<PTok>> = vec![Vec::new()];
    let mut depth = 1;
    let mut j = start;
    while j < toks.len() {
        let t = &toks[j].tok;
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
        args.last_mut().expect("non-empty").push(toks[j].clone());
        j += 1;
    }
    Err(TranslateError::new(
        name.line,
        name.col,
        "unterminated macro invocation",
    ))
}

#[cfg(test)]
mod tests {
    use super::PTok;

    #[test]
    fn ptok_is_small_and_fixed_size() {
        // The memory bound documented on MAX_TOKENS relies on this.
        assert!(std::mem::size_of::<PTok>() <= 40);
    }
}

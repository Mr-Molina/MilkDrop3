//! Dimension of untyped `sampler` globals, from how the module samples them.

use std::collections::HashMap;

use crate::ast::{Decl, Expr, ForInit, Init, Module, SamplerKind, Stmt, TypeKind};

/// For every untyped `sampler` global sampled by `tex3D`/`tex3Dlod`
/// (→ `S3D`) or `texCUBE` (→ `Cube`) anywhere in the module. Unlisted
/// untyped samplers default elsewhere. Conflicting uses keep the first and
/// fail later in the type check.
pub(super) fn untyped_sampler_kinds(m: &Module) -> HashMap<String, SamplerKind> {
    let untyped: Vec<&str> = m
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Global(v) if v.ty.kind == TypeKind::Sampler(SamplerKind::Any) => Some(v),
            _ => None,
        })
        .flat_map(|v| v.vars.iter().map(|d| d.name.as_str()))
        .collect();
    let mut out = HashMap::new();
    if untyped.is_empty() {
        return out;
    }
    let mut visit = |e: &Expr| {
        if let Expr::Call { name, args, .. } = e {
            let kind = match name.as_str() {
                "tex3D" | "tex3Dlod" => SamplerKind::S3D,
                "texCUBE" => SamplerKind::Cube,
                _ => return,
            };
            if let Some(Expr::Ident { name, .. }) = args.first() {
                if untyped.contains(&name.as_str()) {
                    out.entry(name.clone()).or_insert(kind);
                }
            }
        }
    };
    for d in &m.decls {
        match d {
            Decl::Function(f) => f.body.stmts.iter().for_each(|s| stmt(s, &mut visit)),
            Decl::ShaderBody(b) => b.body.stmts.iter().for_each(|s| stmt(s, &mut visit)),
            _ => {}
        }
    }
    out
}

fn stmt(s: &Stmt, f: &mut impl FnMut(&Expr)) {
    match s {
        Stmt::VarDecl(v) => v.vars.iter().flat_map(|d| &d.init).for_each(|i| init(i, f)),
        Stmt::Expr { expr: e, .. } | Stmt::Return { value: Some(e), .. } => expr(e, f),
        Stmt::If {
            cond, then, els, ..
        } => {
            expr(cond, f);
            stmt(then, f);
            if let Some(e) = els {
                stmt(e, f);
            }
        }
        Stmt::For {
            init: fi,
            cond,
            step,
            body,
            ..
        } => {
            match fi {
                ForInit::Decl(v) => v.vars.iter().flat_map(|d| &d.init).for_each(|i| init(i, f)),
                ForInit::Exprs(es) => es.iter().for_each(|e| expr(e, f)),
                ForInit::None => {}
            }
            cond.iter().chain(step).for_each(|e| expr(e, f));
            stmt(body, f);
        }
        Stmt::While { cond, body, .. } | Stmt::DoWhile { body, cond, .. } => {
            expr(cond, f);
            stmt(body, f);
        }
        Stmt::Block(b) => b.stmts.iter().for_each(|s| stmt(s, f)),
        Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Discard { .. } => {}
    }
}

fn init(i: &Init, f: &mut impl FnMut(&Expr)) {
    match i {
        Init::Expr(e) => expr(e, f),
        Init::List { items, .. } => items.iter().for_each(|i| init(i, f)),
    }
}

fn expr(e: &Expr, f: &mut impl FnMut(&Expr)) {
    f(e);
    match e {
        Expr::Lit { .. } | Expr::Ident { .. } => {}
        Expr::Index { base, index, .. } => {
            expr(base, f);
            expr(index, f);
        }
        Expr::Member { base, .. } => expr(base, f),
        Expr::Call { args, .. } | Expr::Ctor { args, .. } => args.iter().for_each(|a| expr(a, f)),
        Expr::Unary { operand, .. } => expr(operand, f),
        Expr::Binary { lhs, rhs, .. } | Expr::Assign { lhs, rhs, .. } => {
            expr(lhs, f);
            expr(rhs, f);
        }
        Expr::Ternary {
            cond, then, els, ..
        } => {
            expr(cond, f);
            expr(then, f);
            expr(els, f);
        }
        Expr::Cast { expr: x, .. } | Expr::Splat { expr: x, .. } => expr(x, f),
    }
}

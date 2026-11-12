//! Sampler and `texsize_*` binding resolution (Task 4.5).
//!
//! Mirrors `plugin.cpp::CShaderParams::CacheParams` (the "pass 1" sampler loop
//! and the `texsize_` handling of "pass 2"):
//!
//! - the sampler's root name is the global's name without a leading
//!   `sampler_` (case-sensitive; a name without it is used whole);
//! - if the root is longer than 3 characters and its third character is `_`,
//!   the first two characters are ALWAYS peeled off as a prefix (even when
//!   they are not a known one: `sampler_no_such_tex` has root `such_tex`);
//!   the prefix is compared case-insensitively: `FW`/`WF` = bilinear + wrap,
//!   `FC`/`CF` = bilinear + clamp, `PW`/`WP` = point + wrap, `PC`/`CP` =
//!   point + clamp. Without a recognised prefix the default is bilinear +
//!   wrap. This is also how the preamble's `sampler_fc_main`, `sampler_pw_main`
//!   ... are decoded: root `main` with their filter/wrap;
//! - roots `blur1`, `blur2`, `blur3` (case-sensitive) without a recognised
//!   prefix default to bilinear + clamp;
//! - `main` is the previous frame; `randNN` / `randNN_<prefix>` (two digits,
//!   NN in 0..=15) request a random texture, see [`parse_rand_root`]. The
//!   root keeps its full text (`rand01_smalltiled`) so the renderer can pick
//!   by prefix; every other root names a texture file.
//!
//! A sampler is *bound* only when the shader entry point references it,
//! directly or through user functions it calls (entry point = the function
//! `PS`; if the module has none, every function counts). References from
//! initializers of referenced globals count too. Slots are assigned in
//! declaration order of the bound samplers; more than 16 is an error.
//!
//! `texsize_names` holds the full global names (`texsize_main`,
//! `texsize_rand01`, ...) of referenced globals whose name starts with
//! `texsize_`, in order of first reference (callee bodies are visited at the
//! first call site, expressions in evaluation order). The renderer maps a
//! name to a texture with [`texsize_root`], like the C++ does by root name.

use std::collections::HashSet;

use crate::ast::Pos;
use crate::error::TranslateError;
use crate::typed::{Callee, TBlock, TDecl, TExpr, TExprKind, TFunction, TStmt, TypedModule};
use crate::types::Ty;

/// Number of sampler slots (`@group(1) @binding(2*i)` / `(2*i+1)`).
pub const MAX_SAMPLERS: usize = 16;

/// One bound sampler (A.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplerBinding {
    /// The HLSL global name, e.g. `sampler_fc_main`.
    pub name: String,
    /// Texture name without `sampler_` and without the filter/wrap prefix:
    /// `main`, `blur1`, `noise_lq`, `rand01_smalltiled`, `mytex`.
    pub root: String,
    /// Slot `i`: the module declares `tex_i` / `samp_i`.
    pub slot: u32,
    /// Wrap (repeat) address mode; `false` = clamp.
    pub wrap: bool,
    /// Bilinear filter; `false` = point.
    pub bilinear: bool,
    /// 2 (`texture_2d`) or 3 (`texture_3d`).
    pub dims: u8,
}

/// The result of [`resolve_bindings`].
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Bindings {
    /// Bound samplers in slot order (`samplers[i].slot == i`).
    pub samplers: Vec<SamplerBinding>,
    /// Referenced `texsize_*` global names, in first-reference order.
    pub texsize_names: Vec<String>,
    /// Highest blur level (0..=3) among the bound samplers' roots
    /// `blur1..3` (0 = none), the renderer's `highest_used`.
    pub highest_blur: u8,
}

/// Decode a sampler global name into `(root, wrap, bilinear)`.
pub fn decode_sampler_name(name: &str) -> (String, bool, bool) {
    let rest = name.strip_prefix("sampler_").unwrap_or(name);
    let chars: Vec<char> = rest.chars().collect();
    let (mut bilinear, mut wrap, mut specified) = (true, true, false);
    let mut root = rest.to_string();
    if chars.len() > 3 && chars[2] == '_' {
        let p: String = chars[..2].iter().map(|c| c.to_ascii_uppercase()).collect();
        match p.as_str() {
            "FW" | "WF" => (specified, bilinear, wrap) = (true, true, true),
            "FC" | "CF" => (specified, bilinear, wrap) = (true, true, false),
            "PW" | "WP" => (specified, bilinear, wrap) = (true, false, true),
            "PC" | "CP" => (specified, bilinear, wrap) = (true, false, false),
            _ => {}
        }
        root = chars[3..].iter().collect();
    }
    if !specified && matches!(root.as_str(), "blur1" | "blur2" | "blur3") {
        (wrap, bilinear) = (false, true);
    }
    (root, wrap, bilinear)
}

/// Random-texture roots: `randNN` or `randNN_<prefix>` with two digits and
/// NN in 0..=15 yield `(NN, prefix)` (prefix empty when absent). Anything
/// else (including `rand16`) is an ordinary texture name.
pub fn parse_rand_root(root: &str) -> Option<(u8, &str)> {
    let b = root.as_bytes();
    if b.len() < 6 || !root.starts_with("rand") || !b[4].is_ascii_digit() || !b[5].is_ascii_digit()
    {
        return None;
    }
    let prefix = match b.get(6) {
        None => "",
        Some(b'_') => &root[7..],
        Some(_) => return None,
    };
    let slot = (b[4] - b'0') * 10 + (b[5] - b'0');
    (slot <= 15).then_some((slot, prefix))
}

/// The texture root a `texsize_XXX` name refers to (`texsize_main` ->
/// `main`); `None` if the name does not start with `texsize_`.
pub fn texsize_root(name: &str) -> Option<&str> {
    name.strip_prefix("texsize_")
}

/// Resolve sampler slots and `texsize_*` uniforms. See the module docs.
pub fn resolve_bindings(m: &TypedModule) -> Result<Bindings, TranslateError> {
    let mut w = Walker::new(m);
    let entry = m.decls.iter().find_map(|d| match d {
        TDecl::Function(f) if f.hlsl_name == "PS" => Some(f),
        _ => None,
    });
    match entry {
        Some(f) => w.function(f),
        None => {
            for d in &m.decls {
                if let TDecl::Function(f) = d {
                    w.function(f);
                }
            }
        }
    }

    let mut out = Bindings::default();
    for d in &m.decls {
        let TDecl::Global(g) = d else { continue };
        let dims = match g.ty {
            Ty::Sampler2D => 2,
            Ty::Sampler3D => 3,
            Ty::SamplerCube => {
                if w.used.contains(g.name.as_str()) {
                    return Err(err(
                        g.pos,
                        format!("cube sampler `{}` is not supported", g.name),
                    ));
                }
                continue;
            }
            _ => continue,
        };
        if !w.used.contains(g.name.as_str()) || out.samplers.iter().any(|s| s.name == g.name) {
            continue;
        }
        if out.samplers.len() == MAX_SAMPLERS {
            return Err(err(
                g.pos,
                format!(
                    "too many samplers: `{}` would be sampler {} (limit {MAX_SAMPLERS})",
                    g.name,
                    MAX_SAMPLERS + 1
                ),
            ));
        }
        let (root, wrap, bilinear) = decode_sampler_name(&g.name);
        match root.as_str() {
            "blur1" => out.highest_blur = out.highest_blur.max(1),
            "blur2" => out.highest_blur = out.highest_blur.max(2),
            "blur3" => out.highest_blur = out.highest_blur.max(3),
            _ => {}
        }
        out.samplers.push(SamplerBinding {
            name: g.name.clone(),
            root,
            slot: out.samplers.len() as u32,
            wrap,
            bilinear,
            dims,
        });
    }
    out.texsize_names = w.texsize;
    Ok(out)
}

fn err(pos: Pos, msg: String) -> TranslateError {
    TranslateError::new(pos.line, pos.col, msg)
}

/// Collects referenced globals by walking the entry point and its callees.
struct Walker<'a> {
    m: &'a TypedModule,
    seen_fns: HashSet<&'a str>,
    seen_inits: HashSet<&'a str>,
    /// Referenced global names.
    used: HashSet<&'a str>,
    texsize: Vec<String>,
}

impl<'a> Walker<'a> {
    fn new(m: &'a TypedModule) -> Self {
        Walker {
            m,
            seen_fns: HashSet::new(),
            seen_inits: HashSet::new(),
            used: HashSet::new(),
            texsize: Vec::new(),
        }
    }

    fn function(&mut self, f: &'a TFunction) {
        if self.seen_fns.insert(&f.name) {
            self.block(&f.body);
        }
    }

    fn block(&mut self, b: &'a TBlock) {
        for s in &b.stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &'a TStmt) {
        match s {
            TStmt::Local(l) => self.opt(l.init.as_ref()),
            TStmt::Expr(e) => self.expr(e),
            TStmt::If {
                cond, then, els, ..
            } => {
                self.expr(cond);
                self.stmt(then);
                if let Some(e) = els {
                    self.stmt(e);
                }
            }
            TStmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                init.iter().for_each(|e| self.expr(e));
                self.opt(cond.as_ref());
                step.iter().for_each(|e| self.expr(e));
                self.stmt(body);
            }
            TStmt::While { cond, body, .. } => {
                self.expr(cond);
                self.stmt(body);
            }
            TStmt::DoWhile { body, cond, .. } => {
                self.stmt(body);
                self.expr(cond);
            }
            TStmt::Return { value, .. } => self.opt(value.as_ref()),
            TStmt::Block(b) => self.block(b),
            TStmt::Break { .. } | TStmt::Continue { .. } | TStmt::Discard { .. } => {}
        }
    }

    fn opt(&mut self, e: Option<&'a TExpr>) {
        if let Some(e) = e {
            self.expr(e);
        }
    }

    fn global(&mut self, name: &'a str) {
        self.used.insert(name);
        if name.starts_with("texsize_") && !self.texsize.iter().any(|n| n == name) {
            self.texsize.push(name.to_string());
        }
        if self.seen_inits.insert(name) {
            let init = self.m.decls.iter().find_map(|d| match d {
                TDecl::Global(g) if g.name == name => g.init.as_ref(),
                _ => None,
            });
            self.opt(init);
        }
    }

    fn expr(&mut self, e: &'a TExpr) {
        use TExprKind::*;
        match &e.kind {
            Lit(_) => {}
            Var { name, origin } => {
                if matches!(origin, crate::typed::VarOrigin::Global(_)) {
                    self.global(name);
                }
            }
            Index { base, index } => {
                self.expr(base);
                self.expr(index);
            }
            Swizzle { base, .. } | MatElems { base, .. } | Field { base, .. } => self.expr(base),
            Call { callee, args } => {
                args.iter().for_each(|a| self.expr(a));
                if let Callee::User(n) = callee {
                    let f = self.m.decls.iter().find_map(|d| match d {
                        TDecl::Function(f) if f.name == *n => Some(f),
                        _ => None,
                    });
                    if let Some(f) = f {
                        self.function(f);
                    }
                }
            }
            Ctor { args } => args.iter().for_each(|a| self.expr(a)),
            Unary { operand, .. } => self.expr(operand),
            Binary { lhs, rhs, .. } | Assign { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Ternary { cond, then, els } => {
                self.expr(cond);
                self.expr(then);
                self.expr(els);
            }
            Cast(x) | Splat(x) => self.expr(x),
        }
    }
}

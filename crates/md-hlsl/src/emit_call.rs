//! Calls of the WGSL emitter (Task 4.4): user functions and intrinsics.
//!
//! User functions are called by their unique name (`h_f`, `h_f__ovl1`).
//! A sampler argument expands to its texture and sampler (`tex_i, samp_i`
//! for a bound global, the pair of a sampler parameter otherwise). `out` and
//! `inout` arguments are passed as `&md_t<N>`, a function-local copy
//! (initialised from the argument for `inout`) written back after the call —
//! HLSL's copy-in/copy-out semantics, and it lets swizzles, private globals
//! and pointer parameters be passed where WGSL wants a `ptr<function, T>`.
//!
//! Intrinsics (HLSL → WGSL):
//!
//! | HLSL | WGSL |
//! |---|---|
//! | `tex2D(s, uv)` | `textureSample(tex_i, samp_i, uv)` |
//! | `tex2Dbias(s, c)` | `textureSampleBias(tex_i, samp_i, c.xy, c.w)` |
//! | `tex2Dlod(s, c)` | `textureSampleLevel(tex_i, samp_i, c.xy, c.w)` |
//! | `tex2Dgrad(s, uv, dx, dy)` | `textureSampleGrad(tex_i, samp_i, uv, dx, dy)` |
//! | `tex2Dproj(s, c)` | `textureSample(tex_i, samp_i, (c.xy / c.w))` |
//! | `tex3D(s, c)` | `textureSample` on a `texture_3d<f32>` |
//! | `tex3Dlod(s, c)` | `textureSampleLevel(tex_i, samp_i, c.xyz, c.w)` |
//! | `texCUBE` | error (MilkDrop binds no cube maps, F26) |
//! | `mul(v, M)` / `mul(M, v)` / `mul(A, B)` | `(M * v)` / `(v * M)` / `(B * A)` (transposed storage, F24) |
//! | `mul(s, x)`, `mul(x, s)` / `mul(u, v)` | `(s * x)`, `(x * s)` / `dot(u, v)` |
//! | `dot(a, b)` on scalars | `(a * b)` |
//! | `saturate(x)` | `clamp(x, 0.0, 1.0)` (bounds splat for vectors) |
//! | `lerp` `frac` `rsqrt` `ddx` `ddy` | `mix` `fract` `inverseSqrt` `dpdx` `dpdy` |
//! | `fmod(a, b)` | `(a % b)` (WGSL float `%` is `a - b * trunc(a / b)`) |
//! | `log10(x)` | `(log(x) / log(10.0))` |
//! | `sign(x)` | `i32(sign(x))` / `vecN<i32>(sign(x))` (HLSL returns int) |
//! | `sincos(x, s, c)` | `let t = x; s = sin(t); c = cos(t);` (x captured: it may alias `s`) |
//! | `modf(x, ip)` | `let t = modf(x); ip = t.whole;` → `t.fract` |
//! | `frexp(x, e)` | `let t = frexp(x); e = f32(t.exp);` → `t.fract` |
//! | `ldexp(x, e)` | `(x * exp2(e))` (HLSL's exponent is a float) |
//! | everything else | same name (`round`, `step`, `smoothstep`, `atan2`, `any`, `all`, …) |
//!
//! A texture coordinate used twice (`c.xy`, `c.w`) is captured in a
//! temporary unless it is a plain variable. `round` keeps WGSL's
//! round-half-to-even (HLSL documents half away from zero).

use crate::emit::{err, R};
use crate::emit_stmt::{has_effects, Fx};
use crate::intrinsics::Intrinsic as I;
use crate::typed::{Callee, ParamMode, TExpr, TExprKind, VarOrigin};
use crate::types::{Shape, Ty};

impl Fx<'_, '_> {
    /// A call; `stmt` when its value is unused.
    pub fn call(&mut self, e: &TExpr, callee: &Callee, args: &[TExpr], stmt: bool) -> R<String> {
        match callee {
            Callee::User(name) => self.user_call(e, name, args, stmt),
            Callee::Intrinsic(i) => self.intrinsic(e, *i, args),
        }
    }

    /// The texture and sampler a sampler argument names.
    fn sampler_pair(&self, a: &TExpr) -> R<(String, String)> {
        match &a.kind {
            TExprKind::Var {
                name,
                origin: VarOrigin::Global(_),
            } => match self.cx.globals.get(name.as_str()) {
                Some(crate::emit::GlobalRef::Sampler(i)) => {
                    Ok((format!("tex_{i}"), format!("samp_{i}")))
                }
                _ => err(a.pos, format!("sampler `{name}` is not bound")),
            },
            TExprKind::Var {
                name,
                origin: VarOrigin::Param(_),
            } => Ok((format!("md_tex_h_{name}"), format!("md_samp_h_{name}"))),
            _ => err(a.pos, "a sampler argument must be a sampler variable"),
        }
    }

    fn user_call(&mut self, e: &TExpr, name: &str, args: &[TExpr], stmt: bool) -> R<String> {
        let Some(f) = self.cx.funcs.get(name).copied() else {
            return err(e.pos, format!("unknown function `{name}`"));
        };
        let last = args.iter().rposition(has_effects);
        let mut list = Vec::new();
        let mut copy_back = Vec::new();
        for (i, (p, a)) in f.params.iter().zip(args).enumerate() {
            if p.ty.is_sampler() {
                let (t, s) = self.sampler_pair(a)?;
                list.push(t);
                list.push(s);
                continue;
            }
            if p.mode == ParamMode::In {
                let s = self.expr(a)?;
                let keep = last.is_some_and(|l| i < l) && !crate::emit_expr::is_lit(a);
                list.push(if keep { self.snapshot(s) } else { s });
                continue;
            }
            let pl = self.place(a)?;
            let t = self.fresh();
            let ty = crate::emit::wgsl_ty(&p.ty, p.pos)?;
            if p.mode == ParamMode::InOut {
                let cur = self.read(&pl);
                self.pre.push(format!("var {t}: {ty} = {cur};"));
            } else {
                self.pre.push(format!("var {t}: {ty};"));
            }
            list.push(format!("&{t}"));
            copy_back.push((pl, t));
        }
        let call = format!("h_{name}({})", list.join(", "));
        if copy_back.is_empty() {
            if stmt {
                self.pre.push(format!("{call};"));
                return Ok(String::new());
            }
            return Ok(call);
        }
        let result = if f.ret == Ty::Void || stmt {
            self.pre.push(format!("{call};"));
            String::new()
        } else {
            let r = self.fresh();
            self.pre.push(format!("let {r} = {call};"));
            r
        };
        for (pl, t) in copy_back {
            self.write(&pl, t);
        }
        Ok(result)
    }

    fn intrinsic(&mut self, e: &TExpr, i: I, args: &[TExpr]) -> R<String> {
        if matches!(
            i,
            I::Tex2D
                | I::Tex2Dbias
                | I::Tex2Dlod
                | I::Tex2Dgrad
                | I::Tex2Dproj
                | I::Tex3D
                | I::Tex3Dlod
                | I::TexCube
        ) {
            return self.tex(e, i, args);
        }
        match i {
            I::Sincos => {
                // `x` is passed by value: capture it before the first write,
                // which may alias it (`sincos(a, a, b)`).
                let x = self.expr(&args[0])?;
                let x = if crate::emit_expr::is_lit(&args[0]) {
                    x
                } else {
                    self.snapshot(x)
                };
                let s = self.place(&args[1])?;
                self.write(&s, format!("sin({x})"));
                let c = self.place(&args[2])?;
                self.write(&c, format!("cos({x})"));
                return Ok(String::new());
            }
            I::Modf | I::Frexp => {
                let x = self.expr(&args[0])?;
                let out = self.place(&args[1])?;
                let t = self.fresh();
                let (f, part) = if i == I::Modf {
                    ("modf", "whole")
                } else {
                    ("frexp", "exp")
                };
                self.pre.push(format!("let {t} = {f}({x});"));
                let v = format!("{t}.{part}");
                let v = if i == I::Frexp {
                    let int_ty = args[1].ty.with_elem(crate::types::Scalar::Int);
                    self.convert(v, &int_ty, &args[1].ty, e.pos)?
                } else {
                    v
                };
                self.write(&out, v);
                return Ok(format!("{t}.fract"));
            }
            _ => {}
        }
        let refs: Vec<&TExpr> = args.iter().collect();
        let v = self.operands(&refs)?;
        let a = |k: usize| v[k].as_str();
        Ok(match i {
            I::Mul => match (args[0].ty.shape(), args[1].ty.shape()) {
                (Some(Shape::Scalar), _) | (_, Some(Shape::Scalar)) => {
                    format!("({} * {})", a(0), a(1))
                }
                (Some(Shape::Vec(_)), Some(Shape::Vec(_))) => format!("dot({}, {})", a(0), a(1)),
                _ => format!("({} * {})", a(1), a(0)),
            },
            I::Dot if args[0].ty.is_scalar() => format!("({} * {})", a(0), a(1)),
            I::Saturate => match e.ty.shape() {
                Some(Shape::Vec(n)) => {
                    format!("clamp({}, vec{n}<f32>(0.0), vec{n}<f32>(1.0))", a(0))
                }
                _ => format!("clamp({}, 0.0, 1.0)", a(0)),
            },
            I::Fmod => {
                let b = self.divisor(crate::ast::BinOp::Rem, &args[1], v[1].clone())?;
                format!("({} % {b})", a(0))
            }
            I::Log10 => format!("(log({}) / log(10.0))", a(0)),
            I::Ldexp => format!("({} * exp2({}))", a(0), a(1)),
            I::Sign if args[0].ty.elem() == Some(crate::types::Scalar::Float) => {
                let ty = crate::emit::wgsl_ty(&e.ty, e.pos)?;
                format!("{ty}(sign({}))", a(0))
            }
            _ => {
                let name = match i {
                    I::Lerp => "mix",
                    I::Frac => "fract",
                    I::Rsqrt => "inverseSqrt",
                    I::Ddx => "dpdx",
                    I::Ddy => "dpdy",
                    _ => i.name(),
                };
                format!("{name}({})", v.join(", "))
            }
        })
    }

    fn tex(&mut self, e: &TExpr, i: I, args: &[TExpr]) -> R<String> {
        if i == I::TexCube {
            return err(
                e.pos,
                "texCUBE is not supported (MilkDrop binds no cube maps)",
            );
        }
        let (t, s) = self.sampler_pair(&args[0])?;
        Ok(match i {
            I::Tex2D | I::Tex3D => {
                let c = self.expr(&args[1])?;
                format!("textureSample({t}, {s}, {c})")
            }
            I::Tex2Dgrad => {
                let refs: Vec<&TExpr> = args[1..].iter().collect();
                let v = self.operands(&refs)?;
                format!("textureSampleGrad({t}, {s}, {})", v.join(", "))
            }
            _ => {
                let c = self.cheap(&args[1])?;
                match i {
                    I::Tex2Dbias => format!("textureSampleBias({t}, {s}, {c}.xy, {c}.w)"),
                    I::Tex2Dlod => format!("textureSampleLevel({t}, {s}, {c}.xy, {c}.w)"),
                    I::Tex2Dproj => format!("textureSample({t}, {s}, ({c}.xy / {c}.w))"),
                    _ => format!("textureSampleLevel({t}, {s}, {c}.xyz, {c}.w)"),
                }
            }
        })
    }
}

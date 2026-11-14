//! The typed tree produced by [`crate::check`] (Task 4.3).
//!
//! It mirrors [`crate::ast`] with every expression annotated with its
//! [`Ty`], names resolved (variable origin, intrinsic or user function), and
//! every implicit conversion explicit:
//!
//! - element changes (`int`/`bool` → `float`, `float` → `int` truncating, …)
//!   are [`TExprKind::Cast`] nodes whose `ty` is the target;
//! - scalar → vector/matrix replication is [`TExprKind::Splat`] (applied
//!   after the element `Cast`, so a `Splat`'s operand already has the
//!   target's element type);
//! - implicit vector truncation is a [`TExprKind::Swizzle`] (`v.xyz`, or
//!   `v.x` to a scalar) plus a [`Warning`]; matrix truncation is a `Cast`
//!   between matrix types (plus a warning when implicit). Explicit casts
//!   lower the same way, without warnings.
//!
//! So every operand already has the type its operator, call parameter or
//! assignment target needs: `Binary` operands share one type (the node's
//! type for arithmetic; comparisons and `&&`/`||` yield `bool` of the
//! operands' shape), `Assign` rhs has the lhs type, and `Ternary` branches
//! have the node's type.

use crate::ast::{BinOp, Lit, Pos, UnOp};
use crate::intrinsics::Intrinsic;
use crate::types::Ty;

/// A non-fatal diagnostic (fxc's legacy warnings, e.g. implicit truncation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub pos: Pos,
    pub message: String,
}

/// A type-checked module.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedModule {
    /// Declarations in source order. `texture` globals (only referenced by
    /// the discarded `sampler_state` initializers) are dropped.
    pub decls: Vec<TDecl>,
    pub warnings: Vec<Warning>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TDecl {
    Struct(TStruct),
    Global(TGlobal),
    Function(TFunction),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TStruct {
    pub pos: Pos,
    pub name: String,
    pub fields: Vec<TField>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TField {
    pub pos: Pos,
    pub name: String,
    pub ty: Ty,
    pub semantic: Option<String>,
}

/// How a global is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// Plain (or `uniform`, or non-static `const`) global: a shader uniform
    /// or resource. Writable unless `const` or a sampler; see
    /// [`TGlobal::written`].
    Uniform,
    /// `static`: module-private, writable.
    Private,
    /// `static const`: a compile-time constant (has an initializer).
    Const,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TGlobal {
    pub pos: Pos,
    pub name: String,
    /// Untyped `sampler` globals are resolved to `Sampler3D` when the
    /// module samples them with `tex3D`/`tex3Dlod` (`SamplerCube` for
    /// `texCUBE`), else to `Sampler2D` (`Sampler3D` if the name contains
    /// `noisevol`).
    pub ty: Ty,
    pub storage: Storage,
    pub semantic: Option<String>,
    pub init: Option<TExpr>,
    /// A `Uniform` global the shader assigns to (MilkDrop's legacy compiler
    /// allowed writes to uniforms). WGSL uniforms are read-only, so the
    /// emitter (4.4) must declare a function-private copy, initialised at
    /// the start of the entry point from the uniform value (or from `init`
    /// when there is no uniform binding), and route every reference to it;
    /// user functions that read or write it see the same copy (e.g. a
    /// module-scope `var<private>`). Always `false` for other storages.
    pub written: bool,
}

/// Parameter passing mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamMode {
    In,
    Out,
    InOut,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TParam {
    pub pos: Pos,
    pub name: String,
    pub ty: Ty,
    pub mode: ParamMode,
    pub semantic: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TFunction {
    pub pos: Pos,
    /// Unique name: the HLSL name, or `<name>__ovl<k>` for the k-th
    /// (k ≥ 1) later overload of the same name.
    pub name: String,
    /// The name as written.
    pub hlsl_name: String,
    pub ret: Ty,
    pub params: Vec<TParam>,
    pub semantic: Option<String>,
    pub body: TBlock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TBlock {
    pub pos: Pos,
    pub stmts: Vec<TStmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TLocal {
    pub pos: Pos,
    pub name: String,
    pub ty: Ty,
    pub is_const: bool,
    pub init: Option<TExpr>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TStmt {
    /// One declared local (a `float a, b;` becomes two).
    Local(TLocal),
    Expr(TExpr),
    If {
        pos: Pos,
        /// Scalar `bool`.
        cond: TExpr,
        then: Box<TStmt>,
        els: Option<Box<TStmt>>,
    },
    /// Declarations in the `for` init clause are hoisted to `Local`
    /// statements just before the loop, in the enclosing scope (fxc's legacy
    /// scoping: the variable outlives the loop). A later `for (int i = …)`
    /// redeclaring the same name with the same type in the same scope
    /// becomes the init assignment `i = …` (with a warning, like fxc X3078).
    For {
        pos: Pos,
        init: Vec<TExpr>,
        /// Scalar `bool`.
        cond: Option<TExpr>,
        step: Vec<TExpr>,
        body: Box<TStmt>,
    },
    While {
        pos: Pos,
        cond: TExpr,
        body: Box<TStmt>,
    },
    DoWhile {
        pos: Pos,
        body: Box<TStmt>,
        cond: TExpr,
    },
    /// The value already has the function's return type.
    Return {
        pos: Pos,
        value: Option<TExpr>,
    },
    Block(TBlock),
    Break {
        pos: Pos,
    },
    Continue {
        pos: Pos,
    },
    Discard {
        pos: Pos,
    },
}

/// Where a variable reference resolves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarOrigin {
    Local,
    Param(ParamMode),
    Global(Storage),
}

/// The function a call resolves to.
#[derive(Clone, Debug, PartialEq)]
pub enum Callee {
    Intrinsic(Intrinsic),
    /// A user function by its unique [`TFunction::name`].
    User(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TExpr {
    pub pos: Pos,
    pub ty: Ty,
    pub kind: TExprKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TExprKind {
    Lit(Lit),
    Var {
        name: String,
        origin: VarOrigin,
    },
    /// Vector component (scalar), matrix row (`Vec(s, cols)`) or array
    /// element. The index is a scalar `int`.
    Index {
        base: Box<TExpr>,
        index: Box<TExpr>,
    },
    /// Component selection of a scalar or vector, 0-based (`.zx` → `[2, 0]`).
    /// One component yields a scalar.
    Swizzle {
        base: Box<TExpr>,
        comps: Vec<u8>,
    },
    /// Matrix element selection `m._m01_m10` / `m._12_21`, as 0-based
    /// (row, col) pairs.
    MatElems {
        base: Box<TExpr>,
        elems: Vec<(u8, u8)>,
    },
    Field {
        base: Box<TExpr>,
        name: String,
    },
    /// Arguments are converted to the parameter types; default arguments
    /// of user functions are filled in. For `Intrinsic` calls see
    /// [`Intrinsic`] for the per-intrinsic argument types.
    Call {
        callee: Callee,
        args: Vec<TExpr>,
    },
    /// Constructor of `ty` (vector, matrix, array or struct). Vector and
    /// matrix arguments are scalars or vectors whose element is the target's
    /// element and whose component counts sum to the target's (matrix
    /// arguments are in HLSL row-major order and may straddle rows, e.g.
    /// `float2x2(float3(…), x)`). Array arguments are the elements, struct
    /// arguments the fields, already of their types. A one-scalar vector
    /// constructor is a `Splat` instead; `float(x)` is a `Cast`.
    Ctor {
        args: Vec<TExpr>,
    },
    /// `-x`, `+x`, `~x` (int) and `++`/`--` (operand of the node's type);
    /// `!x` (operand already `bool` of the node's shape).
    Unary {
        op: UnOp,
        operand: Box<TExpr>,
    },
    /// Operands share one type. Arithmetic (`* / % + -`) has that type
    /// (`int` or `float` element: `bool` operands are promoted to `int`,
    /// mixed `int`/`float` to `float`); `%` on `float` is HLSL `fmod`
    /// (truncated, sign of the dividend), on `int` the truncated integer
    /// remainder, and `/` on `int` truncates. Bitwise ops and shifts are
    /// `int` only. Comparisons yield `bool` of the operands' shape. `&&` and
    /// `||` take `bool` operands and are componentwise on vectors; fxc does
    /// not short-circuit them, so both sides are always evaluated.
    Binary {
        op: BinOp,
        lhs: Box<TExpr>,
        rhs: Box<TExpr>,
    },
    /// The node has the lhs type. For `=` (`op: None`) the rhs has the lhs
    /// type and `op_ty` equals it.
    ///
    /// For a compound `lhs op= rhs` the lhs is evaluated ONCE (it may have
    /// side effects, e.g. `a[i++] *= 1.5`): the emitter takes its address
    /// (or a temporary) and computes `*p = L((O)*p op rhs)` where `O` is
    /// `op_ty`, the type HLSL does the arithmetic in (the `Binary` typing
    /// rules applied to the lhs and rhs types), the rhs already has type
    /// `op_ty`, `(O)` is the implicit conversion lhs type → `op_ty` (element
    /// `Cast` and/or `Splat`), and `L` the implicit conversion `op_ty` → lhs
    /// type (element `Cast` and/or truncating swizzle, already warned).
    /// When `op_ty` equals the lhs type both conversions are identities and
    /// WGSL's own `op=` can be used. Examples: `int i; i *= 1.5` has
    /// `op_ty = float`; `float x; x += v3` has `op_ty = float3`.
    Assign {
        op: Option<BinOp>,
        op_ty: Ty,
        lhs: Box<TExpr>,
        rhs: Box<TExpr>,
    },
    /// `cond` is scalar `bool` (a branch) or a `bool` vector of the node's
    /// component count (fxc legacy: a componentwise select; both branches
    /// are evaluated). Branches have the node's type.
    Ternary {
        cond: Box<TExpr>,
        then: Box<TExpr>,
        els: Box<TExpr>,
    },
    /// Conversion of the operand to `ty`: an element change of the same
    /// shape, or a matrix to a smaller matrix (keeping the top-left block,
    /// optionally changing the element). Vector truncation is a `Swizzle`
    /// and matrix-to-scalar a one-element `MatElems`, each followed by an
    /// element `Cast` when needed.
    Cast(Box<TExpr>),
    /// Replicate the scalar operand (already of `ty`'s element type) into a
    /// vector or matrix `ty`.
    Splat(Box<TExpr>),
}

impl TExpr {
    pub fn new(pos: Pos, ty: Ty, kind: TExprKind) -> TExpr {
        TExpr { pos, ty, kind }
    }
}

//! Abstract syntax tree for the HLSL subset (Task 4.2).
//!
//! Every [`Expr`], [`Stmt`] and declaration carries the 1-based [`Pos`] of the
//! token that starts it (as positions in the source handed to the
//! preprocessor), so later passes can report errors at nodes.
//!
//! Extension: the type checker (Task 4.3) adds variants such as `Expr::Splat`
//! here; every variant is struct-like with a `pos` field and [`Expr::pos`]
//! is the single place that needs a new arm.

/// A 1-based source position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

/// A parsed translation unit.
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub decls: Vec<Decl>,
}

/// A top-level declaration.
#[derive(Clone, Debug, PartialEq)]
pub enum Decl {
    /// Global variable(s): `float4 _c1, _c2;`, `static const float x = 1;`,
    /// `sampler2D sampler_main = sampler_state { ... };` (initializer discarded).
    Global(VarDecl),
    Function(FnDecl),
    Struct(StructDecl),
    /// The bare MilkDrop `shader_body { ... }` construct of a raw preset body.
    /// The wrapper (Task 4.6) turns it into `void PS(<params>)` with the
    /// first/last lines inserted, mirroring `plugin.cpp::LoadShaderFromMemory`.
    /// At most one per module, and it is the last declaration. Wrap at this
    /// AST level: the missing-`;`-before-`}` tolerance applies only inside a
    /// `ShaderBody`, so a textual `void PS(...)` rewrite would lose it.
    ShaderBody(ShaderBody),
}

/// `shader_body { ... }`.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderBody {
    pub pos: Pos,
    pub body: Block,
}

/// Scalar element types. `half` is kept distinct here; the checker treats it
/// as `float`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    Bool,
    Int,
    Uint,
    Half,
    Float,
    Double,
}

/// Sampler object types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SamplerKind {
    /// Untyped `sampler` (presets use it for `sampler sampler_foo;`).
    Any,
    S2D,
    S3D,
    Cube,
}

/// The base of a type, before any array suffix.
#[derive(Clone, Debug, PartialEq)]
pub enum TypeKind {
    Void,
    Scalar(Scalar),
    /// `floatN`, N in 2..=4.
    Vector(Scalar, u8),
    /// `floatRxC`, R and C in 1..=4: (scalar, rows, cols).
    Matrix(Scalar, u8, u8),
    Sampler(SamplerKind),
    /// `texture` (and `texture2D`/`texture3D`/`textureCUBE`): only appears
    /// in the preamble's `texture PrevFrameImage;`.
    Texture,
    /// A user-declared struct.
    Struct(String),
}

/// A written type name.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeName {
    pub pos: Pos,
    pub kind: TypeKind,
}

/// Matrix packing qualifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatrixLayout {
    RowMajor,
    ColumnMajor,
}

/// Storage / parameter qualifiers. `inline`, `extern`, `shared`, `volatile`,
/// `precise` and interpolation modifiers are accepted and dropped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Qualifiers {
    pub is_const: bool,
    pub is_static: bool,
    pub uniform: bool,
    pub is_in: bool,
    pub out: bool,
    pub inout: bool,
    pub layout: Option<MatrixLayout>,
}

/// A variable initializer: an expression or a brace list `{ a, b, { c } }`.
#[derive(Clone, Debug, PartialEq)]
pub enum Init {
    Expr(Expr),
    List { pos: Pos, items: Vec<Init> },
}

/// One declared name in a [`VarDecl`].
#[derive(Clone, Debug, PartialEq)]
pub struct Declarator {
    pub pos: Pos,
    pub name: String,
    /// Array dimensions, outermost first (`a[2][3]` → `[2, 3]`). An unsized
    /// `a[]` has `None`. Sizes are expressions; the checker folds them.
    pub array: Vec<Option<Expr>>,
    /// `: SEMANTIC` (`register(...)` / `packoffset(...)` are dropped).
    pub semantic: Option<String>,
    /// `None` also for a discarded `= sampler_state { ... }`.
    pub init: Option<Init>,
}

/// `quals type a, b[2] = ..., c;`
#[derive(Clone, Debug, PartialEq)]
pub struct VarDecl {
    pub pos: Pos,
    pub quals: Qualifiers,
    pub ty: TypeName,
    pub vars: Vec<Declarator>,
}

/// A function parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub pos: Pos,
    pub quals: Qualifiers,
    pub ty: TypeName,
    pub name: String,
    pub array: Vec<Option<Expr>>,
    pub semantic: Option<String>,
    pub default: Option<Expr>,
}

/// A function definition.
#[derive(Clone, Debug, PartialEq)]
pub struct FnDecl {
    pub pos: Pos,
    pub quals: Qualifiers,
    pub ret: TypeName,
    pub name: String,
    pub params: Vec<Param>,
    pub semantic: Option<String>,
    pub body: Block,
}

/// A struct member (one declarator).
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub pos: Pos,
    pub quals: Qualifiers,
    pub ty: TypeName,
    pub name: String,
    pub array: Vec<Option<Expr>>,
    pub semantic: Option<String>,
}

/// `struct Name { ... };`
#[derive(Clone, Debug, PartialEq)]
pub struct StructDecl {
    pub pos: Pos,
    pub name: String,
    pub fields: Vec<Field>,
}

/// `{ stmts }`.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub pos: Pos,
    pub stmts: Vec<Stmt>,
}

/// The first clause of a `for`.
#[derive(Clone, Debug, PartialEq)]
pub enum ForInit {
    None,
    Decl(VarDecl),
    /// Comma-separated expressions: `for (i = 0, j = 1; ...)`.
    Exprs(Vec<Expr>),
}

/// Statements. An empty statement `;` is an empty [`Stmt::Block`]. Loop and
/// branch attributes (`[unroll]`, `[loop]`, `[branch]`, ...) are dropped.
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    VarDecl(VarDecl),
    Expr {
        pos: Pos,
        expr: Expr,
    },
    If {
        pos: Pos,
        cond: Expr,
        then: Box<Stmt>,
        els: Option<Box<Stmt>>,
    },
    For {
        pos: Pos,
        init: ForInit,
        cond: Option<Expr>,
        /// Comma-separated step expressions.
        step: Vec<Expr>,
        body: Box<Stmt>,
    },
    While {
        pos: Pos,
        cond: Expr,
        body: Box<Stmt>,
    },
    DoWhile {
        pos: Pos,
        body: Box<Stmt>,
        cond: Expr,
    },
    Return {
        pos: Pos,
        value: Option<Expr>,
    },
    Block(Block),
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

impl Stmt {
    /// Position of the statement's first token.
    pub fn pos(&self) -> Pos {
        match self {
            Stmt::VarDecl(d) => d.pos,
            Stmt::Block(b) => b.pos,
            Stmt::Expr { pos, .. }
            | Stmt::If { pos, .. }
            | Stmt::For { pos, .. }
            | Stmt::While { pos, .. }
            | Stmt::DoWhile { pos, .. }
            | Stmt::Return { pos, .. }
            | Stmt::Break { pos }
            | Stmt::Continue { pos }
            | Stmt::Discard { pos } => *pos,
        }
    }
}

/// Literal values. Integer and float suffixes were dropped by the lexer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lit {
    Bool(bool),
    Int(i64),
    Float(f64),
}

/// Prefix and postfix unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Plus,
    Not,
    BitNot,
    PreInc,
    PreDec,
    PostInc,
    PostDec,
}

/// Binary operators (assignment is [`Expr::Assign`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Shl,
    Shr,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitXor,
    BitOr,
    And,
    Or,
}

/// Assignment operators: `=` is `None`, `+=` is `Some(BinOp::Add)`, etc.
pub type AssignOp = Option<BinOp>;

/// Expressions.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Lit {
        pos: Pos,
        lit: Lit,
    },
    Ident {
        pos: Pos,
        name: String,
    },
    /// `base[index]`.
    Index {
        pos: Pos,
        base: Box<Expr>,
        index: Box<Expr>,
    },
    /// `base.field` — struct member or swizzle (`a.xyz`, `m._m00`).
    Member {
        pos: Pos,
        base: Box<Expr>,
        field: String,
    },
    /// `name(args)` — intrinsic or user function.
    Call {
        pos: Pos,
        name: String,
        args: Vec<Expr>,
    },
    /// `float3(a, b)` — constructor of a built-in type.
    Ctor {
        pos: Pos,
        ty: TypeName,
        args: Vec<Expr>,
    },
    Unary {
        pos: Pos,
        op: UnOp,
        operand: Box<Expr>,
    },
    Binary {
        pos: Pos,
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Assign {
        pos: Pos,
        op: AssignOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Ternary {
        pos: Pos,
        cond: Box<Expr>,
        then: Box<Expr>,
        els: Box<Expr>,
    },
    /// `(type) expr`.
    Cast {
        pos: Pos,
        ty: TypeName,
        expr: Box<Expr>,
    },
}

impl Expr {
    /// Position of the node: its first token, or the operator for binary,
    /// assignment, ternary, postfix, index and member nodes.
    pub fn pos(&self) -> Pos {
        match self {
            Expr::Lit { pos, .. }
            | Expr::Ident { pos, .. }
            | Expr::Index { pos, .. }
            | Expr::Member { pos, .. }
            | Expr::Call { pos, .. }
            | Expr::Ctor { pos, .. }
            | Expr::Unary { pos, .. }
            | Expr::Binary { pos, .. }
            | Expr::Assign { pos, .. }
            | Expr::Ternary { pos, .. }
            | Expr::Cast { pos, .. } => *pos,
        }
    }
}

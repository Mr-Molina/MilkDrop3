//! Recursive-descent parser for the HLSL subset (Task 4.2).
//!
//! Input is the preprocessed token stream ([`crate::preprocess_tokens`]).
//! Keywords are plain identifiers in the token stream and are matched by
//! text here. This file holds the cursor, the nesting guards and top-level
//! declarations; statements and expressions live in `parser/stmt.rs` and
//! `parser/expr.rs`.
//!
//! The raw preset form `shader_body { ... }` parses as
//! [`Decl::ShaderBody`]. Mirroring `plugin.cpp::LoadShaderFromMemory`, which
//! replaces the first `shader_body`, wraps the next `{` and the LAST `}` of
//! the text and drops everything after that `}`: it must be followed by `{`,
//! there may be only one, and tokens after its closing brace are discarded
//! unless they contain another `}` (an error: the C++ would have wrapped the
//! wrong brace). Inside it, the last statement may omit its `;`.
//!
//! Untrusted input: recursion is bounded by [`MAX_DEPTH`] and expression
//! tree height by [`MAX_EXPR_HEIGHT`]; exceeding either is a
//! `TranslateError` ("nesting too deep"), never a stack overflow.

mod expr;
mod stmt;

use std::collections::HashSet;

use crate::ast::*;
use crate::{TokKind, Token, TranslateError};

/// Maximum simultaneous recursion (nested statements, parentheses, prefix
/// operators, right-associative chains, initializer lists).
const MAX_DEPTH: u32 = 128;
/// Maximum height of one expression tree (bounds left-associative chains
/// such as `a + b + ...` and postfix chains, which the parser builds
/// iteratively but later passes walk recursively).
const MAX_EXPR_HEIGHT: u32 = 512;

type R<T> = Result<T, TranslateError>;

/// Parse a preprocessed token stream into a [`Module`].
pub fn parse(tokens: &[Token]) -> Result<Module, TranslateError> {
    let mut p = Parser {
        toks: tokens,
        i: 0,
        depth: 0,
        structs: HashSet::new(),
    };
    p.module()
}

struct Parser<'t> {
    toks: &'t [Token],
    i: usize,
    depth: u32,
    /// Struct names declared so far (they start declarations and casts).
    structs: HashSet<String>,
}

const CONTROL_KEYWORDS: &[&str] = &[
    "if",
    "else",
    "for",
    "while",
    "do",
    "return",
    "break",
    "continue",
    "discard",
    "struct",
    "true",
    "false",
    "sampler_state",
    "typedef",
    "switch",
    "case",
    "default",
    "goto",
];

const QUALIFIERS: &[&str] = &[
    "const",
    "static",
    "uniform",
    "in",
    "out",
    "inout",
    "row_major",
    "column_major",
    "inline",
    "extern",
    "shared",
    "volatile",
    "precise",
];

/// The built-in type an identifier names, if any.
fn builtin_type(s: &str) -> Option<TypeKind> {
    Some(match s {
        "void" => TypeKind::Void,
        "sampler" => TypeKind::Sampler(SamplerKind::Any),
        "sampler2D" => TypeKind::Sampler(SamplerKind::S2D),
        "sampler3D" => TypeKind::Sampler(SamplerKind::S3D),
        "samplerCUBE" => TypeKind::Sampler(SamplerKind::Cube),
        "texture" | "texture2D" | "texture3D" | "textureCUBE" => TypeKind::Texture,
        _ => {
            let (scalar, rest) = [
                ("bool", Scalar::Bool),
                ("uint", Scalar::Uint),
                ("dword", Scalar::Uint),
                ("int", Scalar::Int),
                ("half", Scalar::Half),
                ("float", Scalar::Float),
                ("double", Scalar::Double),
            ]
            .iter()
            .find_map(|(p, sc)| s.strip_prefix(p).map(|r| (*sc, r)))?;
            let digit = |c: u8| (b'1'..=b'4').contains(&c).then(|| c - b'0');
            match rest.as_bytes() {
                [] => TypeKind::Scalar(scalar),
                [n] if (b'2'..=b'4').contains(n) => TypeKind::Vector(scalar, n - b'0'),
                [r, b'x', c] => TypeKind::Matrix(scalar, digit(*r)?, digit(*c)?),
                _ => return None,
            }
        }
    })
}

impl<'t> Parser<'t> {
    // ---- cursor -------------------------------------------------------

    fn peek(&self) -> Option<&'t Token> {
        self.toks.get(self.i)
    }

    fn peek_at(&self, k: usize) -> Option<&'t Token> {
        self.toks.get(self.i + k)
    }

    fn at_op(&self, op: &str) -> bool {
        self.peek().is_some_and(|t| t.is_op(op))
    }

    fn at_ident(&self, s: &str) -> bool {
        self.peek().and_then(Token::ident) == Some(s)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        let hit = self.at_op(op);
        if hit {
            self.i += 1;
        }
        hit
    }

    /// Position of the current token (or of the last token at end of input).
    fn pos(&self) -> Pos {
        match self.peek().or(self.toks.last()) {
            Some(t) => Pos {
                line: t.line,
                col: t.col,
            },
            None => Pos { line: 1, col: 1 },
        }
    }

    fn err<T>(&self, msg: impl Into<String>) -> R<T> {
        let p = self.pos();
        Err(TranslateError::new(p.line, p.col, msg))
    }

    /// Text of the current token for messages.
    fn describe(&self) -> String {
        match self.peek().map(|t| &t.kind) {
            None => "end of input".into(),
            Some(TokKind::Ident(s)) => format!("'{s}'"),
            Some(TokKind::Op(o)) => format!("'{o}'"),
            Some(TokKind::Int(i)) => format!("'{i}'"),
            Some(TokKind::Float(f)) => format!("'{f}'"),
        }
    }

    fn expect_op(&mut self, op: &str) -> R<()> {
        if self.eat_op(op) {
            Ok(())
        } else {
            self.err(format!("expected '{op}', found {}", self.describe()))
        }
    }

    /// A (non-reserved) identifier used as a name.
    fn expect_name(&mut self) -> R<String> {
        match self.peek().and_then(Token::ident) {
            Some(s) if !self.is_reserved(s) => {
                self.i += 1;
                Ok(s.to_string())
            }
            _ => self.err(format!("expected a name, found {}", self.describe())),
        }
    }

    fn is_reserved(&self, s: &str) -> bool {
        CONTROL_KEYWORDS.contains(&s) || QUALIFIERS.contains(&s) || self.type_of(s).is_some()
    }

    /// The type an identifier names: built-in or a declared struct.
    fn type_of(&self, s: &str) -> Option<TypeKind> {
        builtin_type(s).or_else(|| {
            self.structs
                .contains(s)
                .then(|| TypeKind::Struct(s.to_string()))
        })
    }

    /// Skip a balanced `open ... close` group starting at the current token.
    fn skip_balanced(&mut self, open: &str, close: &str) -> R<()> {
        self.expect_op(open)?;
        let mut depth = 1usize;
        while depth > 0 {
            match self.peek() {
                None => return self.err(format!("unterminated '{open}'")),
                Some(t) if t.is_op(open) => depth += 1,
                Some(t) if t.is_op(close) => depth -= 1,
                Some(t) if close == ">" && t.is_op(">>") => depth = depth.saturating_sub(2),
                Some(t) if close == ">" && t.is_op("<<") => depth += 2,
                _ => {}
            }
            self.i += 1;
        }
        Ok(())
    }

    // ---- nesting guard --------------------------------------------------

    fn enter(&mut self) -> R<()> {
        if self.depth >= MAX_DEPTH {
            return self.err(format!("nesting too deep (max {MAX_DEPTH})"));
        }
        self.depth += 1;
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    // ---- types and declarations -----------------------------------------

    fn quals(&mut self) -> Qualifiers {
        let mut q = Qualifiers::default();
        while let Some(s) = self.peek().and_then(Token::ident) {
            match s {
                "const" => q.is_const = true,
                "static" => q.is_static = true,
                "uniform" => q.uniform = true,
                "in" => q.is_in = true,
                "out" => q.out = true,
                "inout" => q.inout = true,
                "row_major" => q.layout = Some(MatrixLayout::RowMajor),
                "column_major" => q.layout = Some(MatrixLayout::ColumnMajor),
                s if QUALIFIERS.contains(&s) => {}
                _ => break,
            }
            self.i += 1;
        }
        q
    }

    fn type_name(&mut self) -> R<TypeName> {
        let pos = self.pos();
        match self
            .peek()
            .and_then(Token::ident)
            .and_then(|s| self.type_of(s))
        {
            Some(kind) => {
                self.i += 1;
                Ok(TypeName { pos, kind })
            }
            None => self.err(format!("expected a type, found {}", self.describe())),
        }
    }

    /// True if the current token starts a variable declaration: a
    /// qualifier, or a type followed by a name.
    fn at_decl_start(&self) -> bool {
        let Some(s) = self.peek().and_then(Token::ident) else {
            return false;
        };
        QUALIFIERS.contains(&s)
            || (self.type_of(s).is_some()
                && self
                    .peek_at(1)
                    .is_some_and(|t| matches!(t.kind, TokKind::Ident(_))))
    }

    /// `[N][]...` array suffixes.
    fn array_dims(&mut self) -> R<Vec<Option<Expr>>> {
        let mut dims = Vec::new();
        while self.eat_op("[") {
            if self.eat_op("]") {
                dims.push(None);
            } else {
                dims.push(Some(self.expr()?));
                self.expect_op("]")?;
            }
        }
        Ok(dims)
    }

    /// `: SEMANTIC`, `: register(...)`, `: packoffset(...)`, then an optional
    /// `<...>` annotation (discarded). Returns the last plain semantic.
    fn semantics_and_annotation(&mut self) -> R<Option<String>> {
        let mut sem = None;
        while self.eat_op(":") {
            match self.peek().and_then(Token::ident) {
                Some("register" | "packoffset") => {
                    self.i += 1;
                    self.skip_balanced("(", ")")?;
                }
                Some(s) => {
                    sem = Some(s.to_string());
                    self.i += 1;
                }
                None => return self.err(format!("expected a semantic, found {}", self.describe())),
            }
        }
        if self.at_op("<") {
            self.skip_balanced("<", ">")?;
        }
        Ok(sem)
    }

    /// Declarators after the type: `a[2] : SEM = init, b, ...` (no `;`).
    fn declarators(&mut self, first: Option<(Pos, String)>) -> R<Vec<Declarator>> {
        let mut vars = Vec::new();
        let mut next = first;
        loop {
            let (pos, name) = match next.take() {
                Some(n) => n,
                None => (self.pos(), self.expect_name()?),
            };
            let array = self.array_dims()?;
            let semantic = self.semantics_and_annotation()?;
            let init = if self.eat_op("=") {
                if self.at_ident("sampler_state") {
                    self.i += 1;
                    self.skip_balanced("{", "}")?;
                    None
                } else {
                    Some(self.init()?)
                }
            } else {
                None
            };
            vars.push(Declarator {
                pos,
                name,
                array,
                semantic,
                init,
            });
            if !self.eat_op(",") {
                return Ok(vars);
            }
        }
    }

    /// An initializer: expression or `{ ... }` list.
    fn init(&mut self) -> R<Init> {
        if !self.at_op("{") {
            return Ok(Init::Expr(self.expr()?));
        }
        self.enter()?;
        let r = self.init_list();
        self.leave();
        r
    }

    fn init_list(&mut self) -> R<Init> {
        let pos = self.pos();
        self.i += 1;
        let mut items = Vec::new();
        while !self.eat_op("}") {
            items.push(self.init()?);
            if !self.eat_op(",") {
                self.expect_op("}")?;
                break;
            }
        }
        Ok(Init::List { pos, items })
    }

    /// A variable declaration without its terminating `;`.
    fn var_decl(&mut self) -> R<VarDecl> {
        let pos = self.pos();
        let quals = self.quals();
        let ty = self.type_name()?;
        let vars = self.declarators(None)?;
        Ok(VarDecl {
            pos,
            quals,
            ty,
            vars,
        })
    }

    // ---- top level --------------------------------------------------------

    fn module(&mut self) -> R<Module> {
        let mut decls = Vec::new();
        while self.peek().is_some() {
            if self.eat_op(";") {
                continue;
            }
            if self.at_ident("shader_body") {
                decls.push(Decl::ShaderBody(self.shader_body()?));
                if self.toks[self.i..].iter().any(|t| t.is_op("}")) {
                    return self.err("unexpected tokens after the shader_body block");
                }
                break;
            }
            if self.at_ident("struct") {
                decls.push(Decl::Struct(self.struct_decl()?));
                continue;
            }
            decls.push(self.global_or_function()?);
        }
        Ok(Module { decls })
    }

    fn shader_body(&mut self) -> R<ShaderBody> {
        let pos = self.pos();
        self.i += 1;
        if !self.at_op("{") {
            return self.err(format!(
                "expected '{{' after shader_body, found {}",
                self.describe()
            ));
        }
        let body = self.block(true)?;
        Ok(ShaderBody { pos, body })
    }

    fn struct_decl(&mut self) -> R<StructDecl> {
        let pos = self.pos();
        self.i += 1;
        let name = self.expect_name()?;
        self.expect_op("{")?;
        let mut fields = Vec::new();
        while !self.eat_op("}") {
            let quals = self.quals();
            let ty = self.type_name()?;
            loop {
                let npos = self.pos();
                let fname = self.expect_name()?;
                let array = self.array_dims()?;
                let semantic = self.semantics_and_annotation()?;
                fields.push(Field {
                    pos: npos,
                    quals,
                    ty: ty.clone(),
                    name: fname,
                    array,
                    semantic,
                });
                if !self.eat_op(",") {
                    break;
                }
            }
            self.expect_op(";")?;
        }
        self.expect_op(";")?;
        self.structs.insert(name.clone());
        Ok(StructDecl { pos, name, fields })
    }

    fn global_or_function(&mut self) -> R<Decl> {
        let pos = self.pos();
        let quals = self.quals();
        let ty = self.type_name()?;
        let npos = self.pos();
        let name = self.expect_name()?;
        if self.at_op("(") {
            return Ok(Decl::Function(self.function(pos, quals, ty, name)?));
        }
        let vars = self.declarators(Some((npos, name)))?;
        self.expect_op(";")?;
        Ok(Decl::Global(VarDecl {
            pos,
            quals,
            ty,
            vars,
        }))
    }

    fn function(&mut self, pos: Pos, quals: Qualifiers, ret: TypeName, name: String) -> R<FnDecl> {
        self.expect_op("(")?;
        let mut params = Vec::new();
        if self.at_ident("void") && self.peek_at(1).is_some_and(|t| t.is_op(")")) {
            self.i += 1;
        }
        if !self.eat_op(")") {
            loop {
                params.push(self.param()?);
                if self.eat_op(")") {
                    break;
                }
                self.expect_op(",")?;
            }
        }
        let semantic = self.semantics_and_annotation()?;
        if !self.at_op("{") {
            return self.err(format!(
                "expected a function body, found {} (prototypes are not supported)",
                self.describe()
            ));
        }
        let body = self.block(false)?;
        Ok(FnDecl {
            pos,
            quals,
            ret,
            name,
            params,
            semantic,
            body,
        })
    }

    fn param(&mut self) -> R<Param> {
        let pos = self.pos();
        let quals = self.quals();
        let ty = self.type_name()?;
        let name = self.expect_name()?;
        let array = self.array_dims()?;
        let semantic = self.semantics_and_annotation()?;
        let default = if self.eat_op("=") {
            Some(self.expr()?)
        } else {
            None
        };
        Ok(Param {
            pos,
            quals,
            ty,
            name,
            array,
            semantic,
            default,
        })
    }
}

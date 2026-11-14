//! Resolved types of the type checker (Task 4.3).
//!
//! `half` and `double` are [`Scalar::Float`]; `uint`/`dword` are
//! [`Scalar::Int`] (ps_2/ps_3 have no native integers; fxc emulates both
//! with floats). Vectors have 2..=4 components and matrices 2..=4 rows and
//! columns: `float1`/`floatRx1`/`float1xC` are rejected when types are
//! lowered, because WGSL has no such types.
//!
//! Matrix convention (F24, binds Task 4.4): `Mat(s, r, c)` is HLSL
//! `floatRxC` — `r` rows of `c` components, `M[i]` is row `i` (a `Vec(s, c)`).
//! The emitter stores it transposed as WGSL `matRxC<f32>` (R columns of C
//! rows), so WGSL column `i` is HLSL row `i`; the types here are HLSL's.

use std::fmt;

/// Element type of scalars, vectors and matrices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scalar {
    Bool,
    Int,
    Float,
}

impl Scalar {
    pub fn name(self) -> &'static str {
        match self {
            Scalar::Bool => "bool",
            Scalar::Int => "int",
            Scalar::Float => "float",
        }
    }
}

/// A resolved type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Void,
    Bool,
    Int,
    Float,
    /// (element, components 2..=4).
    Vec(Scalar, u8),
    /// (element, rows, cols), HLSL `floatRxC`.
    Mat(Scalar, u8, u8),
    Sampler2D,
    Sampler3D,
    SamplerCube,
    /// (element, length).
    Array(Box<Ty>, u32),
    Struct(String),
}

/// Shape of a numeric type, independent of its element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Scalar,
    Vec(u8),
    Mat(u8, u8),
}

impl Shape {
    /// Number of components.
    pub fn components(self) -> u32 {
        match self {
            Shape::Scalar => 1,
            Shape::Vec(n) => n as u32,
            Shape::Mat(r, c) => r as u32 * c as u32,
        }
    }
}

impl Ty {
    /// The scalar type of element `s`.
    pub fn scalar(s: Scalar) -> Ty {
        match s {
            Scalar::Bool => Ty::Bool,
            Scalar::Int => Ty::Int,
            Scalar::Float => Ty::Float,
        }
    }

    /// The numeric type of element `s` and shape `shape`.
    pub fn numeric(s: Scalar, shape: Shape) -> Ty {
        match shape {
            Shape::Scalar => Ty::scalar(s),
            Shape::Vec(n) => Ty::Vec(s, n),
            Shape::Mat(r, c) => Ty::Mat(s, r, c),
        }
    }

    /// `Vec(s, n)`, or the scalar when `n == 1`.
    pub fn vec_or_scalar(s: Scalar, n: u8) -> Ty {
        if n == 1 {
            Ty::scalar(s)
        } else {
            Ty::Vec(s, n)
        }
    }

    /// Element type of a scalar, vector or matrix.
    pub fn elem(&self) -> Option<Scalar> {
        match self {
            Ty::Bool => Some(Scalar::Bool),
            Ty::Int => Some(Scalar::Int),
            Ty::Float => Some(Scalar::Float),
            Ty::Vec(s, _) | Ty::Mat(s, _, _) => Some(*s),
            _ => None,
        }
    }

    /// Shape of a scalar, vector or matrix.
    pub fn shape(&self) -> Option<Shape> {
        match self {
            Ty::Bool | Ty::Int | Ty::Float => Some(Shape::Scalar),
            Ty::Vec(_, n) => Some(Shape::Vec(*n)),
            Ty::Mat(_, r, c) => Some(Shape::Mat(*r, *c)),
            _ => None,
        }
    }

    /// True for scalars, vectors and matrices.
    pub fn is_numeric(&self) -> bool {
        self.elem().is_some()
    }

    pub fn is_scalar(&self) -> bool {
        self.shape() == Some(Shape::Scalar)
    }

    pub fn is_sampler(&self) -> bool {
        matches!(self, Ty::Sampler2D | Ty::Sampler3D | Ty::SamplerCube)
    }

    /// The same shape with element `s` (non-numeric types are unchanged).
    pub fn with_elem(&self, s: Scalar) -> Ty {
        match self.shape() {
            Some(shape) => Ty::numeric(s, shape),
            None => self.clone(),
        }
    }
}

impl fmt::Display for Ty {
    /// HLSL spelling, for diagnostics.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Void => f.write_str("void"),
            Ty::Bool | Ty::Int | Ty::Float => f.write_str(self.elem().unwrap().name()),
            Ty::Vec(s, n) => write!(f, "{}{n}", s.name()),
            Ty::Mat(s, r, c) => write!(f, "{}{r}x{c}", s.name()),
            Ty::Sampler2D => f.write_str("sampler2D"),
            Ty::Sampler3D => f.write_str("sampler3D"),
            Ty::SamplerCube => f.write_str("samplerCUBE"),
            Ty::Array(t, n) => write!(f, "{t}[{n}]"),
            Ty::Struct(name) => f.write_str(name),
        }
    }
}

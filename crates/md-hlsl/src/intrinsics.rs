//! The HLSL intrinsic table (Task 4.3).
//!
//! Overload resolution lives in `typeck/call.rs`; this module names the
//! intrinsics and classifies their signatures. After checking, a call's
//! arguments have these types (`T` is the resolved generic type: a `float`
//! scalar or `floatN`, or for the [`SigClass::Num`] classes `int`/`intN`
//! when every argument is integer; vectors of different lengths are
//! truncated to the shortest with a warning, scalars are splat):
//!
//! | class | intrinsics | arguments → result |
//! |---|---|---|
//! | `Float(1)` | acos asin atan ceil cos cosh ddx ddy degrees exp exp2 floor frac fwidth log log10 log2 radians round rsqrt saturate sin sinh sqrt tan tanh trunc | `T → T` |
//! | `Float(2)` | atan2 fmod ldexp pow step | `T, T → T` |
//! | `Float(3)` | lerp smoothstep | `T, T, T → T` |
//! | `Num(1/2/3)` | abs / min max / clamp | as `Float(n)` |
//! | `Sign` | sign | `T → int` of `T`'s shape (HLSL returns int) |
//! | `Reduce` | length | `T → float` (scalar allowed) |
//! | `Reduce2` | distance | `T, T → float` (scalar allowed) |
//! | `Dot` | dot | `T, T → float` (scalar `T` allowed, as the MilkDrop authoring guide documents `dot(float a, float b)`; the emitter lowers it to `a * b`) |
//! | `VecOnly(1)` | normalize | `floatN → floatN` |
//! | `VecOnly(2)` | reflect | `floatN, floatN → floatN` |
//! | `Refract` | refract | `floatN, floatN, float → floatN` |
//! | `Cross` | cross | `float3, float3 → float3` |
//! | `Bool` | all any | `T → bool`, the argument converted to `bool` of its shape |
//! | `OutLike(n)` | frexp modf (n=1), sincos (n=2) | `T, out T [, out T] → T` (sincos: `void`); out arguments are lvalues of exactly `T` |
//! | `Mul` | mul | see below |
//! | `Transpose` | transpose | `floatRxC → floatCxR` |
//! | `Tex` | tex2D… | below |
//!
//! `mul(a, b)` (arguments converted to `float` elements):
//! `mul(scalar, X) → X` and `mul(X, scalar) → X` (the scalar is NOT splat);
//! `mul(floatN, floatN) → float` (dot); `mul(vecR, matRxC) → vecC`;
//! `mul(matRxC, vecC) → vecR`; `mul(matRxC, matCxK) → matRxK`. When the
//! inner dimensions differ the longer side is truncated with a warning (the
//! vector by a swizzle, the matrix by a `Cast` to the smaller matrix). Under
//! the F24 transposed storage the emitter writes `mul(v, M)` as `M * v` and
//! `mul(M, v)` as `v * M`.
//!
//! Texture sampling returns `float4`; the first argument is a sampler
//! variable (global or parameter) of the listed kind, the rest are
//! converted like assignments: `tex2D(sampler2D, float2)`,
//! `tex2Dbias/tex2Dlod/tex2Dproj(sampler2D, float4)`,
//! `tex2Dgrad(sampler2D, float2, float2, float2)`, `tex3D(sampler3D, float3)`,
//! `tex3Dlod(sampler3D, float4)`, `texCUBE(samplerCUBE, float3)`. `texCUBE`
//! type-checks here; the emitter (Task 4.4, F26) rejects it because MilkDrop
//! binds no cube maps.
//!
//! Not covered (positioned errors): matrix arguments to the componentwise
//! intrinsics (HLSL allows `abs(float3x3)` etc.; WGSL builtins do not), and
//! scalar arguments to `normalize`/`reflect`/`refract`.

/// An HLSL intrinsic function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intrinsic {
    Abs,
    Acos,
    All,
    Any,
    Asin,
    Atan,
    Atan2,
    Ceil,
    Clamp,
    Cos,
    Cosh,
    Cross,
    Ddx,
    Ddy,
    Degrees,
    Distance,
    Dot,
    Exp,
    Exp2,
    Floor,
    Fmod,
    Frac,
    Frexp,
    Fwidth,
    Ldexp,
    Length,
    Lerp,
    Log,
    Log10,
    Log2,
    Max,
    Min,
    Modf,
    Mul,
    Normalize,
    Pow,
    Radians,
    Reflect,
    Refract,
    Round,
    Rsqrt,
    Saturate,
    Sign,
    Sin,
    Sincos,
    Sinh,
    Smoothstep,
    Sqrt,
    Step,
    Tan,
    Tanh,
    Tex2D,
    Tex2Dbias,
    Tex2Dlod,
    Tex2Dgrad,
    Tex2Dproj,
    Tex3D,
    Tex3Dlod,
    TexCube,
    Transpose,
    Trunc,
}

use Intrinsic as I;

const TABLE: &[(&str, Intrinsic)] = &[
    ("abs", I::Abs),
    ("acos", I::Acos),
    ("all", I::All),
    ("any", I::Any),
    ("asin", I::Asin),
    ("atan", I::Atan),
    ("atan2", I::Atan2),
    ("ceil", I::Ceil),
    ("clamp", I::Clamp),
    ("cos", I::Cos),
    ("cosh", I::Cosh),
    ("cross", I::Cross),
    ("ddx", I::Ddx),
    ("ddy", I::Ddy),
    ("degrees", I::Degrees),
    ("distance", I::Distance),
    ("dot", I::Dot),
    ("exp", I::Exp),
    ("exp2", I::Exp2),
    ("floor", I::Floor),
    ("fmod", I::Fmod),
    ("frac", I::Frac),
    ("frexp", I::Frexp),
    ("fwidth", I::Fwidth),
    ("ldexp", I::Ldexp),
    ("length", I::Length),
    ("lerp", I::Lerp),
    ("log", I::Log),
    ("log10", I::Log10),
    ("log2", I::Log2),
    ("max", I::Max),
    ("min", I::Min),
    ("modf", I::Modf),
    ("mul", I::Mul),
    ("normalize", I::Normalize),
    ("pow", I::Pow),
    ("radians", I::Radians),
    ("reflect", I::Reflect),
    ("refract", I::Refract),
    ("round", I::Round),
    ("rsqrt", I::Rsqrt),
    ("saturate", I::Saturate),
    ("sign", I::Sign),
    ("sin", I::Sin),
    ("sincos", I::Sincos),
    ("sinh", I::Sinh),
    ("smoothstep", I::Smoothstep),
    ("sqrt", I::Sqrt),
    ("step", I::Step),
    ("tan", I::Tan),
    ("tanh", I::Tanh),
    ("tex2D", I::Tex2D),
    ("tex2Dbias", I::Tex2Dbias),
    ("tex2Dlod", I::Tex2Dlod),
    ("tex2Dgrad", I::Tex2Dgrad),
    ("tex2Dproj", I::Tex2Dproj),
    ("tex3D", I::Tex3D),
    ("tex3Dlod", I::Tex3Dlod),
    ("texCUBE", I::TexCube),
    ("transpose", I::Transpose),
    ("trunc", I::Trunc),
];

/// Signature class of an intrinsic (see the module table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SigClass {
    /// `n` arguments of one generic float type `T`, result `T`.
    Float(u8),
    /// Like `Float`, but `T` stays `int` when every argument is integer.
    Num(u8),
    Sign,
    Reduce,
    Reduce2,
    Dot,
    VecOnly(u8),
    Refract,
    Cross,
    Bool,
    /// `T` followed by `n` out arguments of type `T`.
    OutLike(u8),
    Mul,
    Transpose,
    /// Sampler kind (2, 3, or 6 for cube) and coordinate argument widths.
    Tex(u8, &'static [u8]),
}

impl Intrinsic {
    /// The intrinsic spelled `name` (case-sensitive, as fxc).
    pub fn from_name(name: &str) -> Option<Intrinsic> {
        TABLE.iter().find(|(n, _)| *n == name).map(|(_, i)| *i)
    }

    /// The HLSL spelling.
    pub fn name(self) -> &'static str {
        TABLE
            .iter()
            .find(|(_, i)| *i == self)
            .map(|(n, _)| *n)
            .unwrap()
    }

    pub fn sig(self) -> SigClass {
        use SigClass as S;
        match self {
            I::Acos | I::Asin | I::Atan | I::Ceil | I::Cos | I::Cosh | I::Ddx | I::Ddy => {
                S::Float(1)
            }
            I::Degrees | I::Exp | I::Exp2 | I::Floor | I::Frac | I::Fwidth | I::Log => S::Float(1),
            I::Log10 | I::Log2 | I::Radians | I::Round | I::Rsqrt | I::Saturate => S::Float(1),
            I::Sin | I::Sinh | I::Sqrt | I::Tan | I::Tanh | I::Trunc => S::Float(1),
            I::Atan2 | I::Fmod | I::Ldexp | I::Pow | I::Step => S::Float(2),
            I::Lerp | I::Smoothstep => S::Float(3),
            I::Abs => S::Num(1),
            I::Min | I::Max => S::Num(2),
            I::Clamp => S::Num(3),
            I::Sign => S::Sign,
            I::Length => S::Reduce,
            I::Distance => S::Reduce2,
            I::Dot => S::Dot,
            I::Normalize => S::VecOnly(1),
            I::Reflect => S::VecOnly(2),
            I::Refract => S::Refract,
            I::Cross => S::Cross,
            I::All | I::Any => S::Bool,
            I::Frexp | I::Modf => S::OutLike(1),
            I::Sincos => S::OutLike(2),
            I::Mul => S::Mul,
            I::Transpose => S::Transpose,
            I::Tex2D => S::Tex(2, &[2]),
            I::Tex2Dbias | I::Tex2Dlod | I::Tex2Dproj => S::Tex(2, &[4]),
            I::Tex2Dgrad => S::Tex(2, &[2, 2, 2]),
            I::Tex3D => S::Tex(3, &[3]),
            I::Tex3Dlod => S::Tex(3, &[4]),
            I::TexCube => S::Tex(6, &[3]),
        }
    }

    /// Every intrinsic, in table order.
    pub fn all() -> impl Iterator<Item = Intrinsic> {
        TABLE.iter().map(|(_, i)| *i)
    }
}

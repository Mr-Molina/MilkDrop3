### Task 4.3: Type checker with HLSL implicit conversions

**Files:**
- Create: `crates/md-hlsl/src/{types.rs,typeck.rs,intrinsics.rs}`

**Interfaces:**
- Produces: `pub enum Ty { Void, Bool, Int, Float, Vec(Scalar, u8), Mat(Scalar, u8 /*rows*/, u8 /*cols*/), Sampler2D, Sampler3D, SamplerCube, Array(Box<Ty>, u32), Struct(String) }`, `pub struct TypedModule` (AST annotated with `Ty` and with every implicit conversion made explicit as `Expr::Cast` or `Expr::Splat`), `pub fn check(m: &Module) -> Result<TypedModule, TranslateError>`.
- Conversion rules pinned (HLSL legacy behavior presets depend on): scalar to vector splat in arithmetic and assignment (`float3 ret = 0;`, `ret *= 0.5`); vector truncation to a shorter vector in assignment and call arguments with a warning collected in `TypedModule.warnings` (`float3 a = float4(...)`); `int` and `bool` promote to `float`; `float` to `int` truncates; mixed `int`/`float` arithmetic is `float`; matrix-vector `mul` overloads: `mul(vecR, matRxC) -> vecC`, `mul(matRxC, vecC) -> vecR`, `mul(scalar, x)`; swizzle write masks; `half` is `float`.
- Intrinsic table (name, overloads, result): `abs acos all any asin atan atan2 ceil clamp cos cosh cross ddx ddy degrees distance dot exp exp2 floor fmod frac frexp fwidth ldexp length lerp log log10 log2 max min modf mul normalize pow radians reflect refract round rsqrt saturate sign sin sincos sinh smoothstep sqrt step tan tanh tex2D tex2Dbias tex2Dlod tex2Dgrad tex2Dproj tex3D tex3Dlod texCUBE transpose trunc`; `tex2D(s, uv)`, `tex2Dbias(s, float4)`, `tex2Dlod(s, float4)`, `tex3D(s, float3)`; `GetBlur1(uv)`, `GetBlur2`, `GetBlur3`, `GetMain(uv)`, `GetPixel(uv)` come from the preamble as ordinary functions.

- [ ] **Step 1:** Write failing tests `scalar_splats_into_vector_assignment`, `vector_truncation_warns_not_errors`, `mul_vec_mat_result_type`, `tex2d_returns_float4`, `unknown_identifier_errors_with_position`, `swizzle_write_mask_type`.
- [ ] **Step 2:** Run `cargo test -p md-hlsl typeck`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(hlsl): type checker with legacy implicit conversions`


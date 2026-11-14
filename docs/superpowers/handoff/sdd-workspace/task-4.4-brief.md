### Task 4.4: WGSL emitter

**Files:**
- Create: `crates/md-hlsl/src/{emit.rs,emit_expr.rs,layout.rs}`

**Interfaces:**
- Produces: `pub fn emit(m: &TypedModule, kind: ShaderKind, bindings: &Bindings) -> String` where `Bindings { samplers: Vec<SamplerBinding>, texsize_names: Vec<String> }` comes from Task 4.5; the uniform struct layout in `layout.rs` is shared with `md-render` as `pub const MD_UNIFORMS_SIZE: u64 = 1920` and `pub fn md_uniform_offset(name: &str) -> Option<u64>` (`_c0` at 0, `_c13` at 208, `_qa` at 224, `_qh` at 336, `rand_frame` 352, `rand_preset` 368, `rot_s1` 384, each matrix 64 bytes, `rot_rand4` at 1856).
- Emission rules: `floatN` -> `vecN<f32>`, `floatRxC` -> `matCxR<f32>`, `mul(v, M)` -> `(v * M)`, `mul(M, v)` -> `(M * v)`, `tex2D(s_i, uv)` -> `textureSample(tex_i, samp_i, uv)`, `tex2Dbias(s, c)` -> `textureSampleBias(tex, samp, c.xy, c.w)`, `tex2Dlod(s, c)` -> `textureSampleLevel(tex, samp, c.xy, c.w)`, `tex3D` -> `textureSample` on `texture_3d`, `saturate(x)` -> `clamp(x, 0.0, 1.0)` (vector forms splat), `lerp` -> `mix`, `frac` -> `fract`, `fmod(a,b)` -> `(a - b * trunc(a / b))`, `atan2` -> `atan2`, `rsqrt` -> `inverseSqrt`, `ddx/ddy` -> `dpdx/dpdy`, `log10(x)` -> `(log(x) / log(10.0))`, `sincos(x, s, c)` -> two statements, `round` -> `round`, `step`, `smoothstep`, `sign` -> `sign`, `any/all` on vectors of bool, `discard` -> `discard`; integer literals used in float context get `.0`; `float` -> `f32`, `int` -> `i32`, `bool` -> `bool`; swizzles pass through; out parameters become return tuples only for `sincos` (the only out intrinsic) and `inout` user-function parameters become pointers `ptr<function, T>`; `static` globals become module-scope `var<private>`; constants become `const`.
- Naming: every HLSL identifier is prefixed `h_` to avoid WGSL reserved words.

- [ ] **Step 1:** Write failing tests that assert exact emitted snippets for each rule above (one test per rule, 24 tests), plus `emitted_module_validates_with_naga` using `naga::front::wgsl::parse_str` and `naga::valid::Validator` on the default warp shader.
- [ ] **Step 2:** Run `cargo test -p md-hlsl emit`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(hlsl): WGSL emitter`


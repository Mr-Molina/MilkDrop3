//! Task 4.4: naga validation of emitted modules — the hand-wrapped default
//! warp fixture (F25), the shared uniform layout against naga's layouter,
//! and the whole corpus (every shader must validate).
//! Run with `cargo test -p md-hlsl --test emit_corpus -- --nocapture`.

mod common;

use common::{asset, emit_typed, fixture, naga_validate, wrap_preset};
use md_hlsl::layout::{md_uniform_offset, md_uniforms_wgsl, MD_UNIFORMS_SIZE, MD_UNIFORM_FIELDS};
use md_hlsl::ShaderKind;
use md_preset::parse_milk;

#[test]
fn emitted_module_validates_with_naga() {
    let src = format!("{}\n{}", asset("include.fx"), fixture("default_warp.hlsl"));
    let t = md_hlsl::check(&md_hlsl::parse(&md_hlsl::preprocess_tokens(&src).unwrap()).unwrap())
        .unwrap();
    let w = emit_typed(&t, ShaderKind::Warp).unwrap();
    let m = naga_validate(&w).unwrap_or_else(|e| panic!("{e}\n{w}"));
    let ep = &m.entry_points;
    assert_eq!(ep.len(), 1);
    assert_eq!(
        (ep[0].name.as_str(), ep[0].stage),
        ("fs_main", naga::ShaderStage::Fragment)
    );
    common::has(
        &w,
        &[
            "var h_ret: vec3<f32> = vec3<f32>(0.0);",
            "h_ret = textureSample(tex_0, samp_0, h__uv.xy).xyz;",
            "h_ret -= vec3<f32>(0.004);",
            "(*h__return_value) = vec4<f32>(h_ret.xyz, h__vDiffuse.w);",
        ],
    );
}

#[test]
fn md_uniforms_layout_matches_naga() {
    let src = format!(
        "{}@group(0) @binding(0) var<uniform> U: MdUniforms;\n",
        md_uniforms_wgsl()
    );
    let m = naga_validate(&src).unwrap();
    let (_, ty) = m
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("MdUniforms"))
        .unwrap();
    let naga::TypeInner::Struct { members, span } = &ty.inner else {
        panic!()
    };
    assert_eq!(*span as u64, MD_UNIFORMS_SIZE);
    assert_eq!(members.len(), MD_UNIFORM_FIELDS.len());
    for (mem, (name, kind)) in members.iter().zip(MD_UNIFORM_FIELDS) {
        assert_eq!(mem.name.as_deref(), Some(name));
        assert_eq!(
            mem.offset as u64,
            md_uniform_offset(name).unwrap(),
            "{name}"
        );
        let size = m.types[mem.ty].inner.size(m.to_ctx()) as u64;
        assert_eq!(size, kind.size(), "{name}");
    }
    // the brief's pinned offsets
    for (n, o) in [
        ("_c0", 0),
        ("_c13", 208),
        ("_qa", 224),
        ("_qh", 336),
        ("rand_frame", 352),
        ("rand_preset", 368),
        ("rot_s1", 384),
        ("rot_s2", 448),
        ("rot_rand4", 1856),
    ] {
        assert_eq!(md_uniform_offset(n), Some(o), "{n}");
    }
    assert_eq!(md_uniform_offset("texsize_main"), None);
    assert_eq!(md_hlsl::layout::texsizes_size(0), 16);
    assert_eq!(md_hlsl::layout::texsizes_size(3), 48);
    assert_eq!(md_hlsl::layout::D3D9_HALF_TEXEL, -0.5);
}

/// Every corpus shader (and the shipped defaults): check → bindings → emit
/// → naga. Reports counts and failures; every input must validate (66/66
/// at Task 4.4), so regressions are caught.
#[test]
fn corpus_emission_report() {
    let mut inputs = vec![
        ("warp_ps.fx".to_string(), true, asset("warp_ps.fx")),
        ("comp_ps.fx".to_string(), false, asset("comp_ps.fx")),
    ];
    for path in md_testkit::corpus_presets() {
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let p = parse_milk(&text).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        inputs.push((format!("{name} warp"), true, p.warp_shader));
        inputs.push((format!("{name} comp"), false, p.comp_shader));
    }
    let mut failures = Vec::new();
    for (what, warp, body) in &inputs {
        let kind = if *warp {
            ShaderKind::Warp
        } else {
            ShaderKind::Comp
        };
        let r = wrap_preset(body, *warp)
            .and_then(|m| md_hlsl::check(&m))
            .map_err(|e| format!("check: {e}"))
            .and_then(|t| emit_typed(&t, kind).map_err(|e| format!("emit: {e}")))
            .and_then(|w| {
                naga_validate(&w)
                    .map(|_| ())
                    .map_err(|e| format!("naga: {e}"))
            });
        if let Err(e) = r {
            failures.push(format!("{what}: {e}"));
        }
    }
    println!(
        "corpus emission: {}/{} validate with naga",
        inputs.len() - failures.len(),
        inputs.len()
    );
    for f in &failures {
        println!("FAIL {f}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Constructs the corpus barely exercises, each emitted and validated.
#[test]
fn assorted_constructs_validate_with_naga() {
    let cases: &[(&str, &str)] = &[
        ("", "float a, b, c = 1; a = b = c; _return_value = a;"),
        ("", "int y = 1; int x = y++ + ++y; _return_value = x;"),
        (
            "struct S { float3 p; float w[2]; };",
            "S s; s.p = 1; s.p.yz = _c0.xy; s.w[1] = 2; s.p.x += s.w[1]; _return_value = s.p.xyzz;",
        ),
        (
            "static const float K[3] = {1, 2, 3}; static const float3x3 M = {1,0,0, 0,1,0, 0,0,1};",
            "int i = (int)(_c0.x * 3); _return_value = mul(float3(K[i], 0, 1), M).xyzz;",
        ),
        ("", "float3x3 a = 1; float3x3 b = 2; float3x3 c = _c0.x > 0 ? a : b; _return_value = c[0].xyzz;"),
        ("", "int i = 5; i <<= 2; i = (i & 3) | (i ^ 7) | ~i | (i >> 1); _return_value = i;"),
        ("", "bool b = !(_c0.x > 0); int3 v = int3(1,2,3) * 2; bool3 e = v == int3(2,4,6); _return_value = any(e) && b;"),
        (
            "",
            "int i = 0; if (i++ > 0) { _return_value = 1; } else if (i++ > 1) { _return_value = 2; } \
             else if (_c0.x > 0) { _return_value = 3; } else { _return_value = 4; }",
        ),
        (
            "float find(float x) { for (int i = 0; i < 10; i++) { if (i > x) return i; } return -1; }",
            "_return_value = find(_c0.x);",
        ),
        (
            "void g(inout float2 p, out float q) { p *= 2; q = p.x; }",
            "float4 v = _c0; g(v.xy, v.w); g(v.zw, _return_value.x);",
        ),
        ("", "float4 v = 0; sincos(_c0.x, v.x, v.y); v.xy += 1; v.zw *= v.xy; _return_value = v;"),
        ("", "texsize_noise_lq.x = 3; _return_value = texsize_noise_lq;"),
        (
            "float4 unused() { return tex2D(sampler_noise_hq, 0); }",
            "_return_value = tex2D(sampler_noise_lq, _uv.xy * texsize_noise_lq.zw);",
        ),
        (
            "float4 helper(sampler2D s, sampler3D v, float3 p) { return tex2D(s, p.xy) + tex3D(v, p); }",
            "_return_value = helper(sampler_blur2, sampler_noisevol_lq, _uv.xyz) + \
             helper(sampler_fc_main, sampler_noisevol_hq, 0);",
        ),
        ("float4 foo = float4(1,2,3,4); float bar;", "bar += 1; _return_value = foo * bar;"),
        ("", "float x = 0; x++; x--; ++x; float3 v = 0; v++; _return_value = float4(v, x);"),
        ("", "int i = 3; float f = i / 2 + i % 2; f /= 0; _return_value = f;"),
        ("", "float2x2 m = float2x2(1,2,3,4); m /= m; m += m; m = -m; m = transpose(m); _return_value = m._11_12_21_22;"),
        ("", "int k = 0; do { k += 2; if (k > 5) break; else continue; } while (k < 100); _return_value = k;"),
        ("", "float4 a = _c0; a.wz = a.xy; a = a.x > 0 ? (a *= 2) : a; _return_value = a;"),
        ("", "float e; float3 e3; _return_value = float4(frexp(_c0.xyz, e3), modf(_c0.w, e)) + ldexp(_c1, _c2);"),
        ("", "_return_value = tex2Dbias(sampler_main, float4(_uv.xy, 0, _c0.x)) + tex2Dlod(sampler_main, 0) + tex2Dproj(sampler_main, _c1);"),
        ("", "float a = _c0.x; a = pow(abs(a), 2.2) + exp2(a) + log2(abs(a)+1) + degrees(a) + radians(a) + length(_c0.xy) + distance(_c0.xy, _c1.xy); _return_value = a;"),
        ("", "float3 n = normalize(_c0.xyz); _return_value = float4(reflect(n, _c1.xyz) + refract(n, _c1.xyz, 0.5) + cross(n, _c2.xyz), fwidth(_c0.x));"),
        ("", "int a = min(1, 2); int b = max(a, 3); int c = clamp(b, 0, 2); int d = abs(-c); _return_value = d;"),
    ];
    let mut failures = Vec::new();
    for (g, b) in cases {
        match common::try_emit(g, b) {
            Err(e) => failures.push(format!("{b}\n  emit: {e}")),
            Ok(w) => {
                if let Err(e) = naga_validate(&w) {
                    failures.push(format!("{b}\n  naga: {e}\n{}", common::body_of(&w)));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

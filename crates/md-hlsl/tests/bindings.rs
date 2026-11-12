//! Task 4.5: sampler / texsize binding resolution.
//! Run with `cargo test -p md-hlsl --test bindings`.

use md_hlsl::ast::Decl;
use md_hlsl::{
    check, parse, parse_rand_root, preprocess_tokens, resolve_bindings, Bindings, TranslateError,
    TypedModule,
};
use md_preset::parse_milk;

fn typed(src: &str) -> TypedModule {
    check(&parse(&preprocess_tokens(src).unwrap()).unwrap()).unwrap()
}

fn bind(src: &str) -> Bindings {
    resolve_bindings(&typed(src)).unwrap()
}

fn bind_err(src: &str) -> TranslateError {
    resolve_bindings(&typed(src)).unwrap_err()
}

fn include() -> String {
    std::fs::read_to_string(format!(
        "{}/../../assets/hlsl/include.fx",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// `include.fx` plus `extra` plus `void PS() { <body> }`.
fn with_preamble(extra: &str, body: &str) -> Bindings {
    bind(&format!("{}\n{extra}\nvoid PS() {{ {body} }}", include()))
}

fn sampler<'a>(b: &'a Bindings, name: &str) -> &'a md_hlsl::SamplerBinding {
    b.samplers
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} not bound: {:?}", b.samplers))
}

#[test]
fn prefix_fc_means_bilinear_clamp() {
    let b = with_preamble(
        "sampler sampler_fc_tex; sampler sampler_PW_tex2; sampler sampler_pc_t3;",
        "float3 a = tex2D(sampler_fc_tex, 0.5).xyz + tex2D(sampler_PW_tex2, 0.5).xyz \
         + tex2D(sampler_pc_t3, 0.5).xyz + tex2D(sampler_fc_main, 0.5).xyz;",
    );
    let s = sampler(&b, "sampler_fc_tex");
    assert_eq!(
        (s.root.as_str(), s.wrap, s.bilinear, s.dims),
        ("tex", false, true, 2)
    );
    let s = sampler(&b, "sampler_PW_tex2");
    assert_eq!((s.root.as_str(), s.wrap, s.bilinear), ("tex2", true, false));
    let s = sampler(&b, "sampler_pc_t3");
    assert_eq!((s.root.as_str(), s.wrap, s.bilinear), ("t3", false, false));
    // the preamble's fc_main is root `main`, bilinear clamp
    let s = sampler(&b, "sampler_fc_main");
    assert_eq!((s.root.as_str(), s.wrap, s.bilinear), ("main", false, true));
}

#[test]
fn reversed_prefix_cf_is_accepted() {
    let b = with_preamble(
        "sampler sampler_cf_a; sampler sampler_WF_b; sampler sampler_wp_c; sampler sampler_CP_d;",
        "float3 a = tex2D(sampler_cf_a, 0.5).xyz + tex2D(sampler_WF_b, 0.5).xyz \
         + tex2D(sampler_wp_c, 0.5).xyz + tex2D(sampler_CP_d, 0.5).xyz;",
    );
    let flags = |n: &str| {
        let s = sampler(&b, n);
        (s.root.clone(), s.wrap, s.bilinear)
    };
    assert_eq!(flags("sampler_cf_a"), ("a".into(), false, true));
    assert_eq!(flags("sampler_WF_b"), ("b".into(), true, true));
    assert_eq!(flags("sampler_wp_c"), ("c".into(), true, false));
    assert_eq!(flags("sampler_CP_d"), ("d".into(), false, false));
}

#[test]
fn unknown_two_letter_prefix_is_peeled_with_defaults() {
    // plugin.cpp peels any `XY_` (len > 3) even if unknown.
    let b = with_preamble(
        "sampler sampler_no_such;",
        "float3 a = tex2D(sampler_no_such, 0.5).xyz;",
    );
    let s = sampler(&b, "sampler_no_such");
    assert_eq!((s.root.as_str(), s.wrap, s.bilinear), ("such", true, true));
}

#[test]
fn blur_defaults_to_clamp() {
    let b = with_preamble(
        "sampler sampler_fw_blur2;",
        "float3 a = GetBlur1(0.5) + GetBlur3(0.5) + tex2D(sampler_fw_blur2, 0.5).xyz;",
    );
    let s = sampler(&b, "sampler_blur1");
    assert_eq!(
        (s.root.as_str(), s.wrap, s.bilinear),
        ("blur1", false, true)
    );
    assert!(!sampler(&b, "sampler_blur3").wrap);
    // an explicit prefix wins
    let s = sampler(&b, "sampler_fw_blur2");
    assert_eq!((s.root.as_str(), s.wrap, s.bilinear), ("blur2", true, true));
    assert_eq!(b.highest_blur, 3);
}

#[test]
fn plain_samplers_default_to_bilinear_wrap_and_dims_follow_type() {
    let b = with_preamble(
        "",
        "float3 a = tex2D(sampler_main, 0.5).xyz + tex2D(sampler_noise_hq, 0.5).xyz \
         + tex3D(sampler_noisevol_lq, float3(0.5, 0.5, 0.5)).xyz;",
    );
    let s = sampler(&b, "sampler_main");
    assert_eq!(
        (s.root.as_str(), s.wrap, s.bilinear, s.dims, s.slot),
        ("main", true, true, 2, 0)
    );
    assert_eq!(sampler(&b, "sampler_noise_hq").root, "noise_hq");
    let v = sampler(&b, "sampler_noisevol_lq");
    assert_eq!((v.dims, v.slot), (3, 2));
    assert_eq!(b.highest_blur, 0);
}

#[test]
fn unused_preamble_samplers_are_not_bound() {
    let b = with_preamble("", "float3 a = tex2D(sampler_noise_mq, 0.5).xyz;");
    assert_eq!(b.samplers.len(), 1, "{:?}", b.samplers);
    assert_eq!(b.samplers[0].name, "sampler_noise_mq");
    assert_eq!(b.samplers[0].slot, 0);
    assert!(b.texsize_names.is_empty());
}

#[test]
fn samplers_used_through_helper_functions_are_bound() {
    let b = with_preamble(
        "float3 helper(float2 p) { return tex2D(sampler_blur2, p).xyz; }\n\
         float3 unused(float2 p) { return tex2D(sampler_blur3, p).xyz; }",
        "float3 a = helper(0.5);",
    );
    assert_eq!(b.samplers.len(), 1);
    assert_eq!(b.samplers[0].name, "sampler_blur2");
    assert_eq!(b.highest_blur, 2);
}

#[test]
fn slots_follow_declaration_order_not_use_order() {
    let b = with_preamble(
        "",
        "float3 a = tex2D(sampler_blur1, 0.5).xyz + tex2D(sampler_noise_lq, 0.5).xyz \
         + tex2D(sampler_main, 0.5).xyz;",
    );
    let names: Vec<_> = b
        .samplers
        .iter()
        .map(|s| (s.name.as_str(), s.slot))
        .collect();
    assert_eq!(
        names,
        [
            ("sampler_main", 0),
            ("sampler_noise_lq", 1),
            ("sampler_blur1", 2)
        ]
    );
}

fn many_samplers(n: usize) -> String {
    let decls: String = (0..n)
        .map(|i| format!("sampler2D sampler_t{i:02}x;\n"))
        .collect();
    let uses: Vec<String> = (0..n)
        .map(|i| format!("tex2D(sampler_t{i:02}x, 0.5).xyz"))
        .collect();
    format!("{decls}void PS() {{ float3 a = {}; }}", uses.join(" + "))
}

#[test]
fn seventeenth_sampler_is_error() {
    assert_eq!(bind(&many_samplers(16)).samplers.len(), 16);
    let e = bind_err(&many_samplers(17));
    assert!(e.message.contains("too many samplers"), "{e}");
    assert!(e.message.contains("sampler_t16x"), "{e}");
    assert_eq!(e.line, 17);
}

#[test]
fn texsize_main_is_collected() {
    let b = with_preamble(
        "float4 texsize_main; float4 texsize_unused;",
        "float4 z = texsize_noise_hq; float4 y = texsize_main; z += texsize_noise_hq + texsize_main;",
    );
    assert_eq!(b.texsize_names, ["texsize_noise_hq", "texsize_main"]);
    assert_eq!(md_hlsl::texsize_root("texsize_main"), Some("main"));
}

#[test]
fn texsize_order_follows_callee_at_call_site() {
    let b = bind(
        "float4 texsize_a; float4 texsize_b; float4 texsize_c;\n\
         float f() { return texsize_b.x; }\n\
         void PS() { float x = texsize_a.x; x += f(); x += texsize_c.x; }",
    );
    assert_eq!(b.texsize_names, ["texsize_a", "texsize_b", "texsize_c"]);
}

#[test]
fn rand_roots() {
    assert_eq!(parse_rand_root("rand00"), Some((0, "")));
    assert_eq!(
        parse_rand_root("rand13_smalltiled"),
        Some((13, "smalltiled"))
    );
    assert_eq!(parse_rand_root("rand15_"), Some((15, "")));
    assert_eq!(parse_rand_root("rand16"), None);
    assert_eq!(parse_rand_root("rand1"), None);
    assert_eq!(parse_rand_root("rand01x"), None);
    assert_eq!(parse_rand_root("random"), None);
}

// ---- corpus -----------------------------------------------------------

const WARP_DEFINES: &str = "#define rad _rad_ang.x\n#define ang _rad_ang.y\n\
    #define uv _uv.xy\n#define uv_orig _uv.zw\n";
const COMP_DEFINES: &str = "#define rad _rad_ang.x\n#define ang _rad_ang.y\n\
    #define uv _uv.xy\n#define uv_orig _uv.xy\n#define hue_shader _vDiffuse.xyz\n";
const WARP_PARAMS: &str = "float4 _vDiffuse : COLOR, float4 _uv : TEXCOORD0, \
    float2 _rad_ang : TEXCOORD1, out float4 _return_value : COLOR0";
const COMP_PARAMS: &str = "float4 _vDiffuse : COLOR, float2 _uv : TEXCOORD0, \
    float2 _rad_ang : TEXCOORD1, out float4 _return_value : COLOR0";

/// include.fx + defines + body wrapped into `void PS(...)` as 4.6 will.
fn build_typed(body: &str, warp: bool) -> Result<TypedModule, TranslateError> {
    let (defines, params) = if warp {
        (WARP_DEFINES, WARP_PARAMS)
    } else {
        (COMP_DEFINES, COMP_PARAMS)
    };
    let wrapper = format!(
        "void PS({params}) {{ float3 ret = 0; _return_value = float4(ret.xyz, _vDiffuse.w); }}"
    );
    let Some(Decl::Function(mut f)) = parse(&preprocess_tokens(&wrapper)?)?
        .decls
        .into_iter()
        .next()
    else {
        panic!("wrapper")
    };
    let src = format!("{}\n{defines}{body}", include());
    let mut m = parse(&preprocess_tokens(&src)?)?;
    let Some(Decl::ShaderBody(sb)) = m.decls.pop() else {
        panic!("no shader_body")
    };
    let last = f.body.stmts.pop().unwrap();
    f.body.stmts.extend(sb.body.stmts);
    f.body.stmts.push(last);
    m.decls.push(Decl::Function(f));
    check(&m)
}

fn corpus_bindings(file: &str) -> (Bindings, Bindings) {
    let path = md_testkit::corpus_presets()
        .into_iter()
        .find(|p| p.file_name().unwrap() == file)
        .unwrap_or_else(|| panic!("{file} not in corpus"));
    let text = String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned();
    let p = parse_milk(&text).unwrap();
    let r = |body: &str, warp| resolve_bindings(&build_typed(body, warp).unwrap()).unwrap();
    (r(&p.warp_shader, true), r(&p.comp_shader, false))
}

fn summary(b: &Bindings) -> Vec<(String, String, u32, bool, bool, u8)> {
    b.samplers
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                s.root.clone(),
                s.slot,
                s.wrap,
                s.bilinear,
                s.dims,
            )
        })
        .collect()
}

fn row(
    n: &str,
    r: &str,
    slot: u32,
    wrap: bool,
    bil: bool,
    d: u8,
) -> (String, String, u32, bool, bool, u8) {
    (n.into(), r.into(), slot, wrap, bil, d)
}

#[test]
fn every_corpus_shader_resolves_bindings() {
    let mut bodies = Vec::new();
    for f in ["warp_ps.fx", "comp_ps.fx"] {
        let p = format!("{}/../../assets/hlsl/{f}", env!("CARGO_MANIFEST_DIR"));
        bodies.push((
            f.to_string(),
            f == "warp_ps.fx",
            std::fs::read_to_string(p).unwrap(),
        ));
    }
    for path in md_testkit::corpus_presets() {
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let p = parse_milk(&text).unwrap();
        bodies.push((format!("{} warp", path.display()), true, p.warp_shader));
        bodies.push((format!("{} comp", path.display()), false, p.comp_shader));
    }
    assert!(bodies.len() >= 62, "{}", bodies.len());
    let mut failures = Vec::new();
    for (what, warp, body) in &bodies {
        match build_typed(body, *warp).and_then(|m| resolve_bindings(&m)) {
            Ok(b) => {
                for (i, s) in b.samplers.iter().enumerate() {
                    assert_eq!(s.slot as usize, i, "{what}");
                }
            }
            Err(e) => failures.push(format!("{what}: {e}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn corpus_user_texture_preset() {
    let (warp, comp) = corpus_bindings("user_texture.milk");
    assert_eq!(
        summary(&warp),
        [row("sampler_main", "main", 0, true, true, 2)]
    );
    assert_eq!(
        summary(&comp),
        [
            row("sampler_main", "main", 0, true, true, 2),
            row("sampler_mytex", "mytex", 1, true, true, 2)
        ]
    );
    assert_eq!(comp.texsize_names, ["texsize_mytex"]);
}

#[test]
fn corpus_rand_presets() {
    let (warp, comp) = corpus_bindings("rand_tiles.milk");
    assert_eq!(
        summary(&warp),
        [
            row("sampler_main", "main", 0, true, true, 2),
            row(
                "sampler_rand01_smalltiled",
                "rand01_smalltiled",
                1,
                true,
                true,
                2
            ),
        ]
    );
    assert_eq!(warp.texsize_names, Vec::<String>::new());
    assert_eq!(
        summary(&comp)
            .iter()
            .map(|r| r.1.as_str())
            .collect::<Vec<_>>(),
        ["main", "rand02", "rand03"]
    );
    let (warp, _) = corpus_bindings("rand_gallery.milk");
    assert_eq!(
        summary(&warp),
        [
            row("sampler_main", "main", 0, true, true, 2),
            row("sampler_fc_rand04", "rand04", 1, false, true, 2),
            row("sampler_pw_rand05", "rand05", 2, true, false, 2),
        ]
    );
    assert_eq!(warp.texsize_names, ["texsize_rand05"]);
}

#[test]
fn corpus_noisevol_and_modes_and_blur_presets() {
    let (warp, comp) = corpus_bindings("noise_volume.milk");
    assert_eq!(
        summary(&warp),
        [
            row("sampler_main", "main", 0, true, true, 2),
            row("sampler_noisevol_hq", "noisevol_hq", 1, true, true, 3)
        ]
    );
    assert_eq!(warp.texsize_names, ["texsize_noisevol_hq"]);
    assert_eq!(comp.samplers.len(), 2);

    let (warp, _) = corpus_bindings("sampler_modes.milk");
    assert_eq!(
        summary(&warp),
        [
            row("sampler_fc_main", "main", 0, false, true, 2),
            row("sampler_pc_main", "main", 1, false, false, 2),
            row("sampler_fw_main", "main", 2, true, true, 2),
            row("sampler_pw_main", "main", 3, true, false, 2),
        ]
    );
    let (warp, comp) = corpus_bindings("blur3_bloom.milk");
    assert_eq!((warp.highest_blur, comp.highest_blur), (1, 3));
}

//! Corpus smoke test: preprocess + tokenize every corpus preset's warp and
//! comp shader text prepended with `include.fx`, plus the shipped default
//! shader bodies; (Task 4.2) parse the same inputs; and (Task 4.3) type-check
//! them wrapped as Task 4.6 will.

mod common;

use common::asset;
use md_preset::parse_milk;

#[test]
fn include_fx_and_default_shaders_preprocess() {
    let inc = asset("include.fx");
    assert!(md_hlsl::preprocess_tokens(&inc).unwrap().len() > 100);
    for f in ["warp_ps.fx", "comp_ps.fx"] {
        md_hlsl::preprocess_tokens(&format!("{inc}\n{}", asset(f)))
            .unwrap_or_else(|e| panic!("{f}: {e}"));
    }
}

#[test]
fn every_corpus_shader_tokenizes() {
    let inc = asset("include.fx");
    let files = md_testkit::corpus_presets();
    assert!(files.len() >= 30, "only {} corpus presets", files.len());
    let mut failures = Vec::new();
    let mut shaders = 0;
    for path in &files {
        let text = String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned();
        let preset = match parse_milk(&text) {
            Ok(p) => p,
            Err(e) => {
                failures.push(format!("{}: parse: {e}", path.display()));
                continue;
            }
        };
        for (what, body) in [("warp", &preset.warp_shader), ("comp", &preset.comp_shader)] {
            shaders += 1;
            let src = format!("{inc}\n{body}");
            if let Err(e) = md_hlsl::preprocess_tokens(&src) {
                failures.push(format!("{} {what}: {e}", path.display()));
            }
        }
    }
    assert!(shaders >= 60, "{shaders}");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Task 4.2: include.fx + every corpus shader text (and the shipped defaults)
/// parses, and the body yields exactly one trailing `shader_body` decl — the
/// raw form Task 4.6 wraps into `void PS(...)`.
#[test]
fn every_corpus_shader_parses() {
    use md_hlsl::ast::Decl;
    let inc = asset("include.fx");
    let mut bodies = vec![
        ("warp_ps.fx".to_string(), asset("warp_ps.fx")),
        ("comp_ps.fx".to_string(), asset("comp_ps.fx")),
    ];
    for path in md_testkit::corpus_presets() {
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let preset = parse_milk(&text).unwrap();
        bodies.push((format!("{} warp", path.display()), preset.warp_shader));
        bodies.push((format!("{} comp", path.display()), preset.comp_shader));
    }
    assert!(bodies.len() >= 62, "{}", bodies.len());
    let mut failures = Vec::new();
    for (what, body) in &bodies {
        let src = format!("{inc}\n{body}");
        let parsed = md_hlsl::preprocess_tokens(&src).and_then(|t| md_hlsl::parse(&t));
        match parsed {
            Ok(m) => {
                let n = m
                    .decls
                    .iter()
                    .filter(|d| matches!(d, Decl::ShaderBody(_)))
                    .count();
                if n != 1 || !matches!(m.decls.last(), Some(Decl::ShaderBody(_))) {
                    failures.push(format!("{what}: {n} shader_body decls"));
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

/// Task 4.3: the same inputs, with the 4.6 defines prepended and the
/// `shader_body` wrapped at AST level into `void PS(...)` exactly as
/// `plugin.cpp::LoadShaderFromMemory` does ([`common::wrap_preset`]),
/// type-check without error.
#[test]
fn every_corpus_shader_type_checks() {
    let mut bodies = vec![
        ("warp_ps.fx".to_string(), true, asset("warp_ps.fx")),
        ("comp_ps.fx".to_string(), false, asset("comp_ps.fx")),
    ];
    for path in md_testkit::corpus_presets() {
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        let preset = parse_milk(&text).unwrap();
        bodies.push((format!("{} warp", path.display()), true, preset.warp_shader));
        bodies.push((
            format!("{} comp", path.display()),
            false,
            preset.comp_shader,
        ));
    }
    assert!(bodies.len() >= 62, "{}", bodies.len());
    let mut failures = Vec::new();
    for (what, warp, body) in &bodies {
        let m = common::wrap_preset(body, *warp).unwrap_or_else(|e| panic!("{what}: {e}"));
        if let Err(e) = md_hlsl::check(&m) {
            failures.push(format!("{what}: {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

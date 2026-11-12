//! Corpus smoke test: preprocess + tokenize every corpus preset's warp and
//! comp shader text prepended with `include.fx`, plus the shipped default
//! shader bodies.

use md_preset::parse_milk;

fn asset(name: &str) -> String {
    let p = format!("{}/../../assets/hlsl/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

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

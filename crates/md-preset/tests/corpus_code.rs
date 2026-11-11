//! Every corpus preset parses, and every non-empty code block, in the
//! `strip_code`'d form the C++ compiles (RecompileExpressions skips empty
//! blocks), compiles with md-eval.

use std::cell::RefCell;
use std::rc::Rc;

use md_eval::{GlobalMem, Program, Vm};
use md_preset::{parse_milk, Preset};

fn blocks(p: &Preset) -> Vec<(String, String)> {
    let mut out = vec![
        ("per_frame_init".to_string(), p.per_frame_init_eel()),
        ("per_frame".to_string(), p.per_frame_eel()),
        ("per_pixel".to_string(), p.per_pixel_eel()),
    ];
    for (i, w) in p.waves.iter().enumerate() {
        out.push((format!("wave_{i}_init"), w.init_eel()));
        out.push((format!("wave_{i}_per_frame"), w.per_frame_eel()));
        out.push((format!("wave_{i}_per_point"), w.per_point_eel()));
    }
    for (i, s) in p.shapes.iter().enumerate() {
        out.push((format!("shape_{i}_init"), s.init_eel()));
        out.push((format!("shape_{i}_per_frame"), s.per_frame_eel()));
    }
    out
}

#[test]
fn corpus_presets_parse_and_code_compiles() {
    let files = md_testkit::corpus_presets();
    assert!(files.len() >= 30, "only {} corpus presets", files.len());
    let mut failures = Vec::new();
    let mut compiled = 0;
    for path in &files {
        let bytes = std::fs::read(path).expect("read preset");
        let text = String::from_utf8_lossy(&bytes);
        let preset = match parse_milk(&text) {
            Ok(p) => p,
            Err(e) => {
                failures.push(format!("{}: parse: {e}", path.display()));
                continue;
            }
        };
        assert!(!preset.warp_shader.is_empty() && !preset.comp_shader.is_empty());
        let mut vm = Vm::new(Rc::new(RefCell::new(GlobalMem::new())));
        for (name, code) in blocks(&preset) {
            if code.is_empty() {
                continue;
            }
            match Program::compile(&mut vm, &code) {
                Ok(_) => compiled += 1,
                Err(e) => failures.push(format!("{}: {name}: {e:?}\n    {code}", path.display())),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(compiled > 20, "only {compiled} code blocks compiled");
}

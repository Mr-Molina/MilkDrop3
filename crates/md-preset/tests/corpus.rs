//! Corpus tests (Task 2.5; the compile check moved here from Task 2.3's
//! `corpus_code.rs`): every corpus preset parses, and every non-empty code
//! block, in the `strip_code`'d form the C++ compiles (RecompileExpressions
//! skips empty blocks), compiles with md-eval.

mod common;

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
fn every_corpus_preset_parses_without_error() {
    let corpus = common::corpus_texts();
    let mut failures = Vec::new();
    for (path, text) in &corpus {
        match parse_milk(text) {
            // Import always leaves shader text (read or generated).
            Ok(p) if p.warp_shader.is_empty() || p.comp_shader.is_empty() => {
                failures.push(format!("{}: empty shader text", path.display()));
            }
            Ok(_) => {}
            Err(e) => failures.push(format!("{}: {e}", path.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "{}/{} corpus presets failed:\n{}",
        failures.len(),
        corpus.len(),
        failures.join("\n")
    );
}

#[test]
fn corpus_presets_code_compiles() {
    let mut failures = Vec::new();
    let mut compiled = 0;
    for (path, text) in common::corpus_texts() {
        let Ok(preset) = parse_milk(&text) else {
            continue; // reported by every_corpus_preset_parses_without_error
        };
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

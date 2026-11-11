//! Every EEL code block in the corpus must compile (lex, parse, lower).

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use md_eval::{GlobalMem, Program, Vm};

#[test]
fn every_corpus_expression_compiles() {
    let all = common::corpus_blocks();
    let blocks = all.len();
    let mut failures = Vec::new();
    for b in &all {
        let mut vm = Vm::new(Rc::new(RefCell::new(GlobalMem::new())));
        if let Err(e) = Program::compile(&mut vm, &b.code) {
            failures.push(format!(
                "{}: {}: {e}\n    {}",
                b.path.display(),
                b.prefix,
                b.code
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {blocks} blocks failed to compile:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(blocks > 20, "only {blocks} code blocks found");
}

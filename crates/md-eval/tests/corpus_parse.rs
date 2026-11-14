//! Every EEL code block in the corpus must lex and parse. Extraction lives
//! in `common`.

mod common;

use md_eval::{parse, Lexer};

#[test]
fn corpus_code_blocks_parse() {
    let all = common::corpus_blocks();
    let blocks = all.len();
    let mut failures = Vec::new();
    for b in &all {
        if let Err(e) = Lexer::tokenize(&b.code).and_then(|t| parse(&t)) {
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
        "{} of {blocks} blocks failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(blocks > 20, "only {blocks} code blocks found");
}

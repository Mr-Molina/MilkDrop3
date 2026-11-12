//! Task 4.9 triage: translate every corpus shader (as
//! `tests/corpus.rs::every_corpus_shader_translates_and_validates` does),
//! group the failures by message prefix (quoted names and numbers
//! normalised) and print the counts, most frequent first, then the full
//! `preset | shader | line:col | message` table.
//!
//! `cargo run -p md-hlsl --example triage`; point `MD_EXTRA_PRESETS` at a
//! directory of `.milk` files to triage a real preset pack as well.

#[path = "../tests/common/corpus.rs"]
mod corpus;

fn main() {
    let r = corpus::run_corpus();
    let failed_shaders = r.failures.iter().filter(|f| f.shader != "preset").count();
    let failed_presets = r.failures.len() - failed_shaders;
    println!(
        "{} presets ({} failed to parse), {} shaders: {} translate, {} fail",
        r.presets,
        failed_presets,
        r.translated,
        r.translated - failed_shaders,
        failed_shaders
    );
    if r.failures.is_empty() {
        return;
    }
    println!("\ncount | message prefix");
    for (prefix, n) in corpus::group_failures(&r.failures) {
        println!("{n:5} | {prefix}");
    }
    println!("\n{}", corpus::failure_table(&r.failures));
}

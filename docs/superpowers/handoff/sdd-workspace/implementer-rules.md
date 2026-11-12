# Rules for every implementer on the MilkDrop3 Rust conversion

Repo: /home/user/MilkDrop3, branch rust-experiment. The C++ reference tree is code/ (read it to pin semantics).

Read, in order:
1. Your task brief (path given in your dispatch) — your requirements, exact values verbatim.
2. shared-context.md in this directory — Global Constraints, workspace layout (A.1), crate graph (A.2), public interfaces (A.3), decisions (A.4).
2b. milestone-<M>-context.md in this directory (M = your milestone number) — the milestone intro, often with pinned semantics that bind every task in it.
3. /home/user/MilkDrop3/docs/superpowers/ledger/2026-11-11-rust-conversion-preflight.md section 4 — ADOPTED rulings. Every ruling that names your task (or the crate/interface you touch) binds you and overrides the brief where they conflict.
4. /home/user/MilkDrop3/docs/superpowers/ledger/2026-11-11-rust-conversion.md — grep for `Ruling:` lines naming your task or crate; they bind you too.

Working rules:
- TDD where the brief lists tests: write them, see them fail, implement, see them pass. Record RED/GREEN evidence.
- Each task adds the [workspace.dependencies] pins it first needs (caret version + Cargo.lock). Never add a crate that compiles C/C++ (`cc`, `cmake`, `bindgen` in its build graph) or a license outside MIT/Apache-2.0/BSD/Zlib/ISC/MPL-2.0/Unicode (OFL allowed for the bundled font only). Never add dssim-core, blake3, cargo-fuzz/libfuzzer-sys.
- `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, and the tests you touched must pass with pristine output before you commit. Run the full `cargo test --workspace` once before committing.
- Keep files under ~600 lines; one responsibility per file.
- Commit with the message given in the brief, ending with exactly these trailer lines:
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01SUnx2i2kYPom424TCZtfvS
- Do NOT push. Do NOT edit docs/superpowers/ (plan, ledger).
- Stay on branch rust-experiment: never `git checkout`/`switch` another commit in /home/user/MilkDrop3 (use `git archive` or a worktree under the scratchpad to inspect old code).
- You do not dispatch subagents of any kind (no helpers, no reviewers). Self-review = read your own diff.
- If unclear or blocked: stop and report NEEDS_CONTEXT / BLOCKED with specifics. Don't guess on architecture.

Report: write the full report to the report path given in your dispatch (what you implemented, TDD RED/GREEN evidence with commands and output, files changed, self-review findings, concerns). Then reply with ONLY (under 15 lines): Status (DONE | DONE_WITH_CONCERNS | BLOCKED | NEEDS_CONTEXT), commits (short SHA + subject), one-line test summary, concerns, report path.

If you are resumed with review findings: fix them, re-run the covering tests, APPEND a fix report (changes, covering tests, command, output) to the same report file, commit, and reply with the same short contract.

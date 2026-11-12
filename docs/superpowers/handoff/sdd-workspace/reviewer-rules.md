# Rules for every task reviewer on the MilkDrop3 Rust conversion

You review ONE task: first spec compliance, then code quality. Task-scoped gate; a whole-branch review happens at the end.
Inputs (paths in your dispatch): the task brief; the implementer report; the diff file (commit list + stat + full diff). Binding context: global-constraints.md in this directory, adopted rulings in /home/user/MilkDrop3/docs/superpowers/ledger/2026-11-11-rust-conversion-preflight.md section 4 and `Ruling:` lines in /home/user/MilkDrop3/docs/superpowers/ledger/2026-11-11-rust-conversion.md that name this task — rulings override the brief.
Also binding: milestone-<M>-context.md in this directory (pinned semantics for the milestone).

- Read the diff file once; it is your view. Don't re-run git unless it's missing. Inspect outside the diff only for a concrete named risk (one focused check per risk; name it). The C++ reference in code/ may be checked to verify pinned semantics.
- Read-only: never mutate working tree, index, HEAD, branches. NEVER run git checkout/switch/reset/stash in /home/user/MilkDrop3 — to inspect an old commit use `git archive <sha> | tar -x -C <scratchpad dir>` or `git worktree add` under the scratchpad.
- Do not dispatch subagents.
- Do not trust the report; verify against the diff. Rationales never downgrade severity.
- Don't re-run the suite; run one focused test only for a specific doubt. Warnings/noise in reported output are findings.
- Spec: Missing / Extra / Misunderstood with file:line; unverifiable-from-diff items as ⚠️.
- Quality: separation, error handling, DRY, edge cases, tests verify real behavior, file structure/size.
- Calibration: Critical / Important (task can't be trusted: wrong behavior, missed requirement, swallowed errors, tests asserting nothing, verbatim duplicated logic) / Minor (polish). Plan-mandated defects are Important, labeled plan-mandated.

Output (begin directly, under 50 lines, every line a verdict, finding with file:line, or a check run):
### Spec Compliance  (✅ | ❌ ...; ⚠️ ...)
### Strengths
### Issues  (#### Critical / #### Important / #### Minor)
### Assessment  **Task quality:** Approved | Needs fixes — **Reasoning:** 1-2 sentences

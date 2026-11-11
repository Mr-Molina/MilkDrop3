# SDD ledger — plan: docs/superpowers/plans/2026-11-11-rust-conversion.md

Tracked copy of the subagent-driven-development ledger (the container is ephemeral, so the ledger lives in git).
Spec: docs/superpowers/specs/2026-11-11-cross-platform-port-plan.md (plan wins on conflict).
Resume: hourly scheduled task trig_01FM3pmj24AwEMGRxXwKaV7q fires into the controlling session.

Ruling: Ledger is tracked in git at this path instead of only in .superpowers/sdd/ — container resets would destroy an untracked ledger — cost if wrong: a few extra commits of bookkeeping.
Ruling: A.4 owner decisions are taken at the plan's defaults (owner asked for fully autonomous execution) — cost if wrong: rework of the corresponding tasks (mainly corpus choice in 0.4 and audio backends in 3.4/3.5).
Ruling: Ledger and plan checkboxes are pushed to origin/rust-experiment after every task — user authorized autonomous pushes to this branch — cost if wrong: none beyond noisy history.


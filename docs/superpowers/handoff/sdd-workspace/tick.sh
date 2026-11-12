#!/usr/bin/env bash
# tick.sh N — mark all checkboxes of Task N in the plan as done
set -e
plan=docs/superpowers/plans/2026-11-11-rust-conversion.md
awk -v n="$1" '
  /^```/ { f=!f }
  !f && /^#+[ \t]+(Task|Milestone|## )/ { if ($0 ~ /^#+[ \t]+Task[ \t]+/) intask = ($0 ~ ("^#+[ \t]+Task[ \t]+" n "([^0-9.]|$)")); else if ($0 ~ /^## /) intask=0 }
  intask { sub(/^- \[ \]/, "- [x]") } { print }' "$plan" > "$plan.tmp" && mv "$plan.tmp" "$plan"

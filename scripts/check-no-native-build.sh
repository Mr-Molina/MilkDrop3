#!/usr/bin/env bash
# Native-code guard (Task 0.3). Fails if:
#   1. C/C++ source files exist in the repository (outside code/, which Task 9.1 deletes
#      together with the exemption below);
#   2. cc, cmake or bindgen is reachable in the dependency graph (all targets, normal and
#      build edges, transitive) through any parent other than the allow-listed wrappers;
#   3. the C-compiling features of those allow-listed wrappers are enabled.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

fail=0
errfile=$(mktemp)
trap 'rm -f "$errfile"' EXIT
err() { echo "check-no-native-build: $*" >&2; fail=1; }

# --- 1. C/C++ sources -------------------------------------------------------
# TODO(Task 9.1): remove the `-path ./code` exemption when code/ is deleted.
sources=$(find . \( -path ./code -o -path ./target -o -path ./.git \) -prune -o \
  -type f \( -name '*.c' -o -name '*.cc' -o -name '*.cpp' -o -name '*.h' -o -name '*.hpp' \) -print)
if [ -n "$sources" ]; then
  err "C/C++ source files found outside code/:"
  echo "$sources" >&2
fi

# --- 2. compiler crates in the graph ----------------------------------------
# Allowed direct dependents (wrappers) per crate. Their C compilation is feature-gated
# and checked in step 3.
allowed_parents_for() {
  case "$1" in
    cc) echo "wayland-backend android-activity" ;;
    *) echo "" ;;
  esac
}

for crate in cc cmake bindgen; do
  rc=0
  out=$(cargo tree --workspace --target all -e normal,build -i "$crate" \
    --depth 1 --prefix none 2>"$errfile") || rc=$?
  if [ "$rc" -ne 0 ]; then
    if grep -q 'did not match any packages' "$errfile"; then
      continue # crate is not in the graph at all
    fi
    err "cargo tree failed for '$crate':"
    cat "$errfile" >&2
    continue
  fi
  allowed=" $(allowed_parents_for "$crate") "
  # First line is the crate itself; the rest are its direct dependents.
  while IFS= read -r line; do
    parent=${line%% *}
    [ -z "$parent" ] && continue
    case "$allowed" in
      *" $parent "*) ;;
      *) err "'$crate' is reachable through '$parent' (not an allow-listed wrapper)" ;;
    esac
  done < <(printf '%s\n' "$out" | tail -n +2)
done

# --- 3. C-compiling features of the allow-listed wrappers -------------------
check_feature_off() {
  local crate=$1 feature=$2 rc=0 out
  out=$(cargo tree --workspace --target all -e features -i "$crate" --prefix none 2>"$errfile") || rc=$?
  if [ "$rc" -ne 0 ]; then
    if grep -q 'did not match any packages' "$errfile"; then
      return 0
    fi
    err "cargo tree failed for '$crate':"
    cat "$errfile" >&2
    return 0
  fi
  if printf '%s\n' "$out" | grep -Fq "$crate feature \"$feature\""; then
    err "feature '$feature' of '$crate' is enabled; it compiles C/C++ code"
  fi
}

check_feature_off wayland-backend log
check_feature_off android-activity game-activity

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "check-no-native-build: OK"

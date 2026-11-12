### Task 2.4: Writer and round trip

**Files:**
- Create: `crates/md-preset/src/write.rs`

**Interfaces:**
- Produces: `Preset::to_milk(&self) -> String` emitting `[preset00]`, then scalars in the order `CState::Export` writes them, floats with `%.3f` for most and `%.6f` for `zoom`, `rot`, `cx`, `cy`, `dx`, `dy`, `warp`, `sx`, `sy` (match `state.cpp` formats; record each in a comment), then code blocks as numbered lines; lines starting with `[` or `;` in code get a backtick prefix.

- [ ] **Step 1:** Write failing tests `round_trip_preserves_every_field` (`parse_milk(p.to_milk()) == p` for the all-keys fixture), `code_line_starting_with_bracket_gets_backtick`, `corpus_round_trip_is_stable` (for every corpus preset, `parse(write(parse(x))) == parse(x)`).
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(preset): writer with round-trip tests`


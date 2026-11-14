### Task 2.1: INI reader

**Files:**
- Create: `crates/md-preset/src/ini.rs`

**Interfaces:**
- Produces: `pub struct Ini { pub fn parse(text: &str) -> Ini; pub fn get(&self, section: &str, key: &str) -> Option<&str>; pub fn get_f32(&self, s: &str, k: &str, default: f32) -> f32; pub fn get_i32(..); pub fn get_bool(..); pub fn code_block(&self, section: &str, prefix: &str) -> String /* joins prefix1, prefix2, ... until the first missing number, with '\n' */ }`.
- Rules: case-insensitive section and key lookup; first occurrence wins; values are not trimmed of interior whitespace but leading and trailing whitespace is removed; a value may contain `=`; lines starting with `;` are comments; `\r\n` and a UTF-8 BOM are accepted.

- [ ] **Step 1:** Write failing tests `first_duplicate_key_wins`, `value_may_contain_equals`, `code_block_stops_at_gap` (`per_frame_1`, `per_frame_2`, `per_frame_4` -> two lines), `backtick_prefix_is_stripped`, `parses_crlf_bom_and_trailing_junk`.
- [ ] **Step 2:** Run `cargo test -p md-preset ini`. Expected: FAIL.
- [ ] **Step 3:** Implement with a `HashMap<String, Vec<(String, String)>>` keyed by lowercased section.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(preset): INI reader`


### Finding Verdicts
- **1. [Important] C++ line citations wrong by 20-30 lines** - ADDRESSED. Every citation in shapes.rs (2, 6, 7, 19, 41, 43, 46, 48, 59, 63, 93, 109, 111, 136, 178), shapes_draw.rs (13, 15, 16, 17, 73) and tests/shapes.rs (53, 101, 127, 156, 159, 177, 185, 257, 325, 352, 356, 396) checked with sed against milkdropfs.cpp; each cited line holds the named construct (sides clamp 2288-2290, additive 2294, UV 2321-2322, close 2325, textured 2327-2334/2330, border 2352-2385, thick 2371, tex inc 2372-2373, offsets 2376-2382, LINESTRIP 2383, reps 2224-2227, alpha_mult 2228-2230, colours 2304-2313, per-frame vars 2400-2440, NSEEL 2267-2273, first-frame clear 971-981). No stale numbers remain.
- **2. [Folded] (int) cast parity** - ADDRESSED. shapes.rs:82-90 `c_int` is identical in logic to md-eval mem.rs:30-39 `eel_int` (trunc; NaN/out-of-range -> i32::MIN). Used at every C++ `(int)` site in DrawCustomShapes: sides (shapes.rs:120, C++ 2288), textured (2327), additive (2294), thick (2371), and all colour bytes via `byte` (96; C++ 2304-2313, 2358-2362, border included through `color`). No remaining `as i32` except inside c_int (grep). The per-frame var handling (2400-2440) has no `(int)` casts. Unit tests shapes.rs:208-245 cover NaN, +/-3e9, infinity, sides 3e9 -> 3, NaN flags true, colour 1e7 -> 0.
- **3. [Folded] blend and aspect_y tests** - ADDRESSED. tests/shapes.rs:384-413 `additive_selects_one_instead_of_inv_src_alpha` asserts ~(128,127) for alpha and (>250,127) for additive, so swapping to ONE/ONE or dropping the flag fails it. The UV test (127-140) uses aspect_y 0.75 and asserts u = 0.5 + h*0.75 while v = 0.5 + h.

### New Breakage in the Fix Diff
None Critical/Important.
- Minor: shapes.rs:82 duplicates `eel_int` (md-render cannot depend on md-eval; stated in the doc comment).
- Minor: shapes.rs:109 "2297-2325" starts at the v2 declaration (fan code is 2299-2325); tests/shapes.rs:391-394 comment wording is muddled ("is not needed").
- Report shows commands and pass counts (workspace tests, clippy, fmt); not re-run per rules.

### Out-of-Scope Observations
None.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage.

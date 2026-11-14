### Finding Verdicts
- **[Important] golden near-black grain, guard too weak** — ADDRESSED. tests/golden/rust/default_60.png viewed: bright concentric rainbow rings with an 8-spoke centre and dark vignette (clear structure). Preset tests/presets/ps2/shader_rings.milk is self-authored, shader-only (wavecode/shapecode all enabled=0, comp_1..9 only), documented at tests/presets/README.md (added table row). golden_basic.rs:~44-58 adds the discrimination assertion: all-black 640x480 vs golden, dssim = 1/ssim.max(1e-12) - 1, which matches md-testkit image_diff.rs:57 and ssim is pub (image_diff.rs:151); asserts > 0.02. Lit-pixel guard is retained.
- **[Important] run()/main.rs untested** — ADDRESSED for run(). run_writes_the_requested_pngs (golden_basic.rs) parses argv, calls render_cmd::run, asserts exactly frame_3.png and frame_6.png, return length 2, and decoded size 64x48. main.rs's own exit-code/print wiring is still untested (a thin wrapper; the report discloses it). Not blocking.
- **[Minor, folded] parse errors name the flag** — ADDRESSED. cli.rs:~48-53 maps the u32::try_from error with `{flag}: {e}`; the u64 parse already did. bad_numeric_flags_name_the_flag covers both --frames (non-numeric) and --every (overflow).

### New Breakage in the Fix Diff
None. Checks: the report's tests were run (5 passed in milkdrop, 585 workspace); the black-frame assertion's formula matches the production one; temp dir uses pid-suffixed name and is cleaned before and after. Minor nit only: the GOLDEN_PRESET doc comment says noise_grain is the only other non-black preset; fine.

### Out-of-Scope Observations
None.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage.

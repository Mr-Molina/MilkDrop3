### Finding Verdicts
- **[Important] Golden-suite CI step ran on all three OSes; must be Linux-only with MD_REQUIRE_GPU=1 and lavapipe selection** — ADDRESSED. ci.yml:66-71: `if: runner.os == 'Linux'`, step-level env `MD_REQUIRE_GPU: "1"` and `WGPU_ADAPTER_NAME: llvmpipe`. ci.yml:23-27 installs `mesa-vulkan-drivers libvulkan1` (Linux only) before it; ci.yml:31-35 sets the same two variables for the Linux job via GITHUB_ENV, so names and values match (step env is redundant but harmless and keeps the step self-contained).
- **[Folded] .gitignore ignores `*.diff.png` without hiding a committed baseline** — ADDRESSED. .gitignore:9 `tests/golden/rust/**/*.diff.png`. image_diff.rs:76 writes `golden_path.with_extension("diff.png")`, i.e. `frame_<n>.diff.png` beside the baseline, which the pattern covers. `find tests/golden -name '*.diff.png'` returns nothing, so no committed baseline is hidden.
- **[Folded] tests/golden/README.md notes that bless skips within-threshold drift** — ADDRESSED. README.md:66-69 matches image_diff.rs:61-72 (early return when dssim <= max_dssim; rewrite only on a missing file or a threshold breach) and :76 for the diff-image name. `max_dssim = 0` advice is accurate.

### New Breakage in the Fix Diff
None.

### Out-of-Scope Observations
- The matrix guard at ci.yml:24 uses `matrix.os == 'ubuntu-latest'` while the other steps use `runner.os`. Harmless today; would diverge if the matrix gains another Linux runner label.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage.

### Finding Verdicts
1. Rating picks uniform, ratio 4.5..5.5 — ADDRESSED. library.rs `pick_random`: CDF draw is `(next_u64() >> 11) / 2^53` (uniform f64 [0,1)) times total; uniform pick uses `below()` rejection sampling (`zone = MAX - (MAX % n + 1) % n`, correct, no modulo bias, n=1 safe). No CRT 15-bit shift, no `% 14345`, `CDF_STEPS` removed. Float round-up to `hi == n` still clamped by `hi.min(n-1)`. library_tests.rs rating_cdf_prefers_high_ratings is back to 4.5..5.5; the "5.85" comment is gone. These are the only two draws in the file.
2. Progressive API and insta flag removed — ADDRESSED. `read_more_ratings`, `ratings_pending`, `progress` and the flag are gone; `scan(dir, max_ps)` reads every rating through `read_file_info` (plugin.cpp:7539-7610). No other crate uses them (grep).
3. PSVERSION cap — ADDRESSED. C++ 7566-7580 checks line 1 `strncmp "MILKDROP_PRESET_VERSION"`, line 2 `strncmp "PSVERSION"`, `sscanf(&p[10], "%d")` default 2, skip if `> nMaxPSVersion`. The key is `PSVERSION` only, with no max of warp/comp; Rust `ps_version_exceeds` matches (`PSVERSION_WARP=` on line 2 gives 2, as in C++). A missing PSVERSION line never skips; no header (MD1) never skips. Cap mapping None/2_0/2_X/3_0/4_0 -> 0/2/3/4/5 matches plugin.h:94-98. The 2 KiB probe is shared with the rating. Test ps_version_cap_skips_presets_above_it expects 5/6/7/3 files across the four caps, which agrees with the fixtures.
4. Option<usize> for current — ADDRESSED. library.rs `next_sequential`/`prev_sequential` take `Option<usize>`; None is the C++ -1. Walked the C++ 6981-6986 and 6898-6904 cases: None, first/last file, past-end and dirs == 0 all match.
5. Relative-dir `..` — ADDRESSED. `*..` is decided on `canonicalize(dir).parent()`, so the root rule holds and relative dirs get it. `path_of` falls back to `dir.join("..")` when the parent is empty, so no empty path. Test relative_dir_gets_dotdot_without_empty_path covers it.
6. Unopenable .milk skipped — ADDRESSED. `read_file_info` returns None on `File::open` failure and `scan` omits the entry (7540-7542). Dangling-symlink test is valid, also as root.
7. next_sequential precondition — ADDRESSED. Both return None when the library has no file; test sequential_without_files_is_none.

Report names the covering test command and output (`cargo test -p md-core -- library_tests`, 18 passed; full gate clean); consistent with the diff.

### New Breakage in the Fix Diff
#### Minor
- library.rs `ps_version_exceeds`: `split('\n')` treats a blank line between the header and PSVERSION as line 2, but C++ `NextLine` (plugin.cpp:7397-7410) skips all consecutive \r/\n, so `HEADER\n\nPSVERSION=3` is capped in C++ and not here. Only hand-edited presets hit it. Fix: skip empty lines when advancing.
- library_tests.rs relative_dir_gets_dotdot_without_empty_path scans the crate cwd and asserts `*..` is entry 0. It would break if the cwd held a directory whose starred name sorts before `*..` (e.g. `-x`). Fragile, not wrong today.
- `*..` uses the canonical parent while `path_of` uses the lexical one; they differ only through symlinked dirs.

### Out-of-Scope Observations
None.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage.

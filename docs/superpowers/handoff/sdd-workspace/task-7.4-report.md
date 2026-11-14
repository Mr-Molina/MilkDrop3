# Task 7.4 report: preset library, selection, history

Commit 12b5de1 `feat(core): preset library, selection, history`. Files: crates/md-core/src/{library.rs (325 lines), history.rs (105), library_tests.rs (316)}, lib.rs (mods + re-exports). No new dependencies (`rand_core::Rng` is used, `impl Rng` accepts `&mut Pcg32`).

## C++ pinned (plugin.cpp)
- Scan 7418-7732: dirs starred `*name` (7521); `.` skipped, `..` shown by FindFirstFile (non-root) as `*..`; files kept if `len>=5 && wcsicmp(tail5,".milk")==0` (7527): any case, hidden files kept, `.milk` alone qualifies. Ratings are read during the scan in the live code (7539-7610); `UpdatePresetRatings`/`m_nRatingReadProgress`/`bInstaScan` are commented out (8268-8307, plugin.h:427, plugin.cpp:964/1061/1164).
- Rating: `GetPrivateProfileFloatW(preset00, fRating, 3.0)` then clamp [0,5] (7609-7610); implemented by a 2 KiB probe parsed with md_preset `Ini` (section/key case-insensitive, first wins; exact), falling back to the whole file. No cheap key read exists in md-preset.
- Sort: dirs first (`*` special-casing in MergeSortPresets 7807-7875), then `mystrcmpiW` (714-737). Its `LC2UC` table (556-572) folds A-Z to **a-z** (verified by parsing the table: only ' ' -> 255 and A-Z -> +32 differ from identity), so `_` sorts before letters and space sorts last. Mirrored with a char-wise compare (`cmp_names`); stable sort like the merge sort.
- Cumulative rating (7700-7702): `cum[0]=0`, `cum[i]=cum[i-1]+rating[i]` (dirs rating 0). Entry 0's rating is dropped (C++ quirk, reproduced; harmless below root as entry 0 is `*..`).
- Random (6990-7030): uniform `nDirs + rand()%(nPresets-nDirs)` if ratings off or `cum[last] < 0.1`; else `cdf = (rand()%14345)/14345*cum[last]`, `< cum[first file]` -> first file, else binary search (lo=nDirs, hi=n; `cum[mid] > cdf -> hi`). Dirs never picked; current preset may be re-picked; weight is rating (not a power).
- Sequential: next 6981-6985 (`cur+1`, wrap to first file if < nDirs or >= n); previous 6898-6904 (`cur-1`, if < nDirs -> last entry; >= n -> first file). `-1` (no current, 6167) is passed as `usize::MAX`.
- History (plugin.h:431, 6896-6959, 7037, 7221-7233, 6343-6346, 6180): ring of 66, `pos`, `back_fence`, `fwd_fence`. Back: if pos != back_fence, pos-=1. Forward (start of LoadRandomPreset): if `pos+1 != fwd_fence && !empty` step onto it. Otherwise a new pick: advance pos if non-empty, then LoadPreset records only when `fwd_fence == pos` (store, fwd+=1, push back_fence if fences touch). So forward history is never lost by a push (Next replays it first), no duplicate suppression, usable depth 65 (64 back steps). Menu load (6180) advances pos unconditionally and records only at the frontier (`push_user_load`). Order toggle resets (`reset`). Sequential mode does not touch history.
- Type-to-find `SeekToPreset` 7359-7375: first typed letter, ASCII-uppercased, first file (from nDirs) whose first char matches.

## Departures
1. Ratings: `scan(dir, insta)`/`read_more_ratings(n)` follow the brief and the commented-out progressive reader, not live code (which always reads at scan). Dirs cost nothing and the first call already reads n files (dead code spends call 1 on dirs). Unread ratings are 0.
2. Random ratio: the brief's 5:1 does not follow from the C++. `rand()` is 15-bit and `32768 % 14345 = 4078` biases low residues (3 vs 2 hits); P(hi)=0.8541, ratio 5.85:1 analytically. Test range changed from 4.5..5.5 to 5.3..6.4 (observed 5.93 with seed 7). The 15-bit draw is mimicked via `next_u32()>>17`. Uniform pick uses all 32 bits (C++ `rand()` caps at 32768 files).
3. `find_prefix(typed, from)`: prefix (not first letter), starts at max(from, nDirs), ASCII case-insensitive, no wrap; `from=0` with one letter equals the C++.
4. No PSVERSION filter (`nMaxPSVersion`, 7573) and no unreadable-file skip; unreadable dir gives an empty library (C++ falls back via FindValidPresetDir, 7440/7478/7689). Name compare is by Unicode scalar, not UTF-16 unit; chars > 255 are not table-folded (C++ indexes past the table, UB).
5. `hi` clamped to n-1 against float rounding; NaN rating -> 0.
6. Signature `pick_random(&mut impl Rng, ..)` instead of `&mut Pcg32` (accepts Pcg32, no dependency move). Added `path_of`, `dir`, `ratings_pending`, `prev_sequential`, `History::{push_user_load, reset, is_empty}`; `History::push` records the path (brief listed no signature) and `back`/`forward` take `&mut self` because they move the position.

## TDD
RED (stubbed library with `unimplemented!()`, tests written first):
`cargo test -p md-core library_tests` -> `test result: FAILED. 3 passed; 12 failed` (all library tests failed with "not implemented"; the 3 passing were History tests, since history.rs was written before the tests, so those had no RED).
GREEN after implementing: the ratio test failed once (5.93:1, vs the brief's 5:1 range), which led to departure 2; then `test result: ok. 15 passed`. 
Tests (15): dirs_sort_before_files_and_are_starred, sort_is_case_insensitive, only_milk_extension_any_case_hidden_included, dotdot_only_when_not_at_root, rating_cdf_prefers_high_ratings, uniform_when_ratings_disabled, uniform_when_cumulative_rating_below_threshold, pick_random_none_without_files, rating_read_default_clamp_and_position, sequential_wraps_past_dirs, read_more_ratings_progresses_n_per_call, find_prefix_is_case_insensitive_and_skips_dirs, history_back_then_forward_returns_same, history_ring_keeps_65_entries_and_stores_duplicates, history_menu_load_mid_history_is_not_recorded_and_reset_reseeds. Temp dirs under std::env::temp_dir (no tempfile dependency).

## Gate
cargo build --workspace, clippy --workspace --all-targets -D warnings, fmt --check: clean. Full `cargo test --workspace` run before commit: all `test result: ok`, no failures. md-core lib: 52 passed.

## Citations
Every plugin.cpp/plugin.h citation in library.rs/history.rs was checked with `sed -n`/`grep -n` against code/vis_milk2; ten were wrong at first (my earlier guesses) and were corrected before the commit.

## Concerns
- The C++ cumulative-rating quirk (entry 0 excluded) is reproduced; it only bites at a filesystem root with no subdirectories and was not unit-tested (a temp dir always has `..`).
- Task 7.9 callers must pass `usize::MAX` for "no current preset" and remember the Load-menu `m_nCurrentPreset = -1`.

## Fix round 1 (review findings 1-7)
Changes (library.rs, library_tests.rs):
1. `pick_random` no longer emulates CRT `rand()`: the CDF draw is a uniform f64 in [0,1) (53 bits of `next_u64`) times the total, the uniform pick is an unbiased rejection draw. Ratio test restored to 4.5..5.5; the "5.85" comment and departure 2 of the original report are withdrawn.
2. Removed `read_more_ratings`, `ratings_pending`, the `insta_scan_ratings` flag and the progress field; `scan(dir, max_ps)` reads every rating (plugin.cpp:7539-7610).
3. `scan` takes `max_ps: PsVersion` (mapped to the C++ ints 0/2/3/4/5) and skips a file when line 1 starts with `MILKDROP_PRESET_VERSION`, line 2 starts with `PSVERSION` and the `%d` at byte 10 (default 2 if unreadable, so `PSVERSION_WARP=` on line 2 counts as 2) exceeds the cap (7566-7580). Missing PSVERSION line: not skipped; no version header (MD1): never skipped (7582-7586). Header and rating share the 2 KiB probe.
4. `next_sequential`/`prev_sequential` take `Option<usize>` (None = no current preset).
5. `..` is shown unless `canonicalize(dir)` has no parent (relative dirs get it); `path_of` for `*..` never yields an empty path (`./..` for `.`).
6. A `.milk` that cannot be opened is skipped (7540-7542).
7. Both sequential fns return `Option<usize>`, `None` when there is no file.
Tests: removed read_more test; ratio 4.5..5.5; added sequential_without_files_is_none, ps_version_cap_skips_presets_above_it, unopenable_milk_is_skipped (dangling symlink, valid as root), relative_dir_gets_dotdot_without_empty_path. Command `cargo test -p md-core -- library_tests`: 18 passed (md-core lib 55). Full gate (build, clippy -D warnings, fmt --check, cargo test --workspace) clean.
Citations touched (6996, 7010-7027, 7539-7610, 7540-7542, 7566-7580, 7582-7586, 1562-1570, 7527-7528, 7700-7702, 6898-6904, 6903, 6981-6986) were checked with sed/grep; 7566-7575 and 7583-7587 were corrected to 7566-7580 and 7582-7586.

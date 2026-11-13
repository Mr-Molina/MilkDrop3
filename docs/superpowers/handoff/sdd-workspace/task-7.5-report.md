# Task 7.5 report: preset switching, blending, hard cuts, timers

Commit 7b93128 `feat(core): preset switching, blending, hard cuts`. Files: crates/md-core/src/{switcher.rs (412), timing.rs (~160), switcher_tests.rs (~475)}, lib.rs (modules, re-exports), crates/md-render/src/renderer.rs (one removed call, see D1).

## Implemented
- `ShaderLoader` trait (md-core): `type Gpu`, `compile(&Preset) -> Result<Gpu,String>`, `preset_loaded()`, `blend_geometry() -> ((gx,gy),(aspx,aspy))`. Implemented for `Renderer` (compile = `load_preset`, preset_loaded = `textures_mut().preset_loaded()`, geometry from `mesh()`). Tests use a fake, no GPU.
- `Switcher<G>`: current/old `Option<PresetState>`, pending (state, blend_time, tick, gpu), gpu/old_gpu, `blend_pattern: Vec<VertInfo>`, `Timers`, `locked_by_user/code`. API: `load_preset(Preset, blend_time, now, &InitInputs, loader, rng) -> Result<bool>`, `load_path`, `tick`, `finish_pending`, `update_timers(now, fps, frame, &SoundData, &SwitchConfig, rng) -> Vec<SwitchEvent>`, `blend_t`, `progress`, `bump_hard_cut_thresh`, accessors.
- Instant load (blend 0 or no current preset): `InitMode::Instant` with `progress` replaced by the OUTGOING timers' progress, swap at once, compile, timers reset, `preset_loaded()` (plugin.cpp:7245-7281; 7259 Import after swap 7251-7253; 7267-7268 timers; 7280 OnFinishedLoadingPreset).
- Blended load: `InitMode::Blended` into the pending slot, tick = 1 (7293, 7295). `tick` runs once per frame after the frame: tick 2 compiles BOTH shaders (F44; C++ 7315-7318 compiles one per tick at 2 and 5), tick 5 no-op, tick 8 = `finish_pending` (7320-7352): swap states and shaders, `randomize_blend_pattern(rng, gx, gy, aspect, &mut blend_pattern)` (7334), start blend (7337), timers reset (7339-7340), `preset_loaded()`; counter ++ otherwise (7355-7356).
- Blend start: `Preset::start_blend` on the new preset plus the old-preset freeze (state.cpp:826-835): if old `mv_a >= 0.001` and new `< 0.001`, the old preset's `mv_x..mv_b` (8 fields, not `mv_a`) are `set` to the new preset's start-of-blend value, cancelling their blend (Task 2.5 ruling).
- Timers (`timing.rs`, `Timers::update`), in the C++ order: lock freeze (plugin.cpp:3920-3932: start = now, next = -1 each frame when either lock is set, so `next` is always ahead and a locked preset never advances); frame 0: start = now (milkdropfs.cpp:765-769) and thresh = 2*loudness_thresh (825-827); `next < 0` -> `next = now + blend_time_auto + time_between_presets + time_between_presets_rand * (draw%1000)*0.001` (771-775); advance due when `next < now`, strict (782), suppressed while pending (784-785); blend progress `(now - start)/duration`, blending off when > 1 (814-821); hard cut when `fps > 1 && !disabled && !locked_user && !locked_code` (828): `imm_rel` sum > `thresh*3` -> event `LoadRandom{blend_time:0}` unless pending (832-833), thresh *= 2 even when pending (834), else decay `(t - L)*exp(-1.3863/(halflife*fps)) + L` (839-845). No decay when disabled or locked or fps <= 1 (as C++).
- `SwitchConfig` (minimal; md-platform config is 8.1) with C++ defaults (plugin.cpp:939-946). `SwitchEvent::LoadRandom{blend_time}`: the caller picks from `PresetLibrary` (`Option<usize>` current, scan with PS cap) and calls `load_preset`/`load_path`; the Switcher does not re-implement selection or history.
- `blend_t(now)` = `(now - blend_start)/duration` while blending else 1; `progress(now)` = existing `preset_progress(now, start, next)` (milkdropfs.cpp:494). `update_timers` writes `current.blend_progress` and ends the blend as 814-821.

## Where the blend pattern lives (5.6/5.7)
In `Switcher.blend_pattern`, sized `(gx+1)(gy+1)` from `ShaderLoader::blend_geometry` at tick 8, passed by the caller to `WarpMesh::compute_uvs(.., Some((switcher.blend_pattern(), switcher.blend_t(now))))`. Why: the renderer already owns the mesh and takes the pattern as a slice parameter; keeping the buffer in md-core needs no renderer change and no extra trait method beyond the geometry query.

## Departures (C++ wins or ruling applies)
- D1 (Task 5.4): `Renderer::load_preset` used to call `textures.preset_loaded()` itself at compile time; with the trait method that would double-count (halving the protect window) and fire at tick 2 instead of completion (C++ increments in OnFinishedLoadingPreset, plugin.cpp:7307). I removed the call from `load_preset`; `ShaderLoader::preset_loaded` (called once per completed load, instant or tick 8) does it. No md-render test depended on it (full workspace passes). Any other direct caller of `load_preset` must call `preset_loaded()` itself.
- D2: `current` is `Option` (C++ INVALID_PRESET_DESC forces blend 0, 7241-7243), not `PresetState`.
- D3: `load_preset` takes a parsed `Preset` (`load_path` reads a file; missing file is an Err, 7211-7218); returns `Ok(false)` for any load while pending. C++ guards only auto/hard-cut callers (milkdropfs.cpp:784, 832); a direct user load would restart the pending one (7295). Ruling-driven.
- D4: `update_timers` returns events rather than loading (library pick belongs to the caller); extra args `frame` (GetFrame()) and `rng`.
- D5: not ported: warp/comp shader locks (ApplyFlags 7255-7257, 7290-7291), history push, "not found" UI message, blocking user loads by `locked_by_code` (VK_SPACE, plugin.cpp:5986: caller's job in 7.9). `bump_hard_cut_thresh` exposes the `*= 2` of 6191/6471.
- D6: `finish_pending` (device-reset jump, 3768-3771) compiles shaders if tick 2 never ran (the C++ would have none).
- D7: a failed `compile` leaves `gpu = None` and sets `Switcher.last_error`; the swap still happens.
- D8: an instant load keeps the previous preset in `old()`/`old_gpu()` without a blend, as the C++ keeps m_pOldState/m_OldShaders; callers must hand the old gpu to the renderer only while `is_blending()`.
- D9: `Timers::update` takes a `TimerFrame` struct (clippy too_many_arguments).
- Brief's flat fields (`next_preset_time`, `hard_cut_thresh`) are nested in `Timers` (accessors on Switcher).

## TDD
RED (tests written first, modules absent), `cargo test -p md-core --lib switcher`:
```
error[E0583]: file not found for module `switcher`
error[E0583]: file not found for module `timing`
error: could not compile `md-core` (lib test) due to 2 previous errors
```
GREEN: `test result: ok. 19 passed` (switcher tests; first run had 1 failure, a wrong expectation in my own test (thresh decays on frame 0 before doubling), fixed in the test). Tests cover the brief's six plus: first load instant; instant uses Instant init with outgoing progress (q1=bass, q2=progress=0.5); blended uses Blended (q=0); fake loader sees one compile of both shaders at tick 2 and nothing at 5 (counts `[0,1,1,1,1,1,1,1]`); preset_loaded per load (not before the swap); mv freeze on the old preset (and not frozen when new is visible); random advance range; no advance while pending; hard-cut double while pending without a load; hard cuts disabled/low fps; both locks; blend_t/progress formulas and blend end; finish_pending.

Gate: `cargo fmt --check`, `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace`: no failures (md-core lib 105 passed).

## Citations
Every C++ line citation in the code and this report was checked with `sed -n` against code/vis_milk2 before commit; my first guesses for plugin.cpp were off (I had read from a shifted offset) and were corrected: LoadPreset 7203-7299, tick 7313-7357, RandomizeBlendPattern 7043-7185, OnFinishedLoadingPreset 7302-7308.

## Concerns
- `update_timers` mid-blend `blend_t` is not clamped (C++ leaves > 1 for the frame the blend ends).
- C++ `rand()%1000` replaced by `next_u32()%1000` (A.4).
- Gpu `old_gpu` semantics (D8) need care in 7.9.

## Fix round 1
Changes (all findings addressed):
1. `load_preset` no longer drops loads while pending (returns `Result<()>`). Blended replaces the pending state, restarts ticks at 1 and recompiles (plugin.cpp:7285-7298; the leak not mirrored). Instant applies at once, pending stays and lands at tick 8 blending from the instant preset. `update_timers` still skips auto-advance/hard-cut while pending (milkdropfs.cpp:784, 832). Tests: `blended_load_during_pending_replaces_it_and_restarts_ticks`, `instant_load_during_pending_applies_at_once_and_pending_still_lands`; `no_auto_advance_while_a_load_is_pending` kept.
2. `bump_hard_cut_thresh` removed; module doc explains the dead member vs function-static (plugin.cpp:6191, 6471; milkdropfs.cpp:825).
3. headless.rs comment fixed and it now calls `textures_mut().preset_loaded()` after `load_preset` (previously implicit); `Renderer::load_preset` doc says direct callers must call it.
4. `finish_pending` docs the device-reset `m_bBlending = false` (plugin.cpp:3768-3774) for 7.9.
5. Advance and hard cut can both fire when `blend_time_auto == 0` (a blended advance makes the load pending so the cut skips); test `zero_blend_auto_can_emit_advance_and_hard_cut_in_one_frame`.
6. `update_timers` documents no-redraw-frames (milkdropfs.cpp:777, plugin.cpp:3917-3919).
7. `old()`, `old_mut()`, `old_gpu()` return Some only while `is_blending()`; test `old_is_hidden_unless_blending`; instant-load test asserts both None.
8. mv_a assertion now `value() == 0.0 && is_blending()`; fake loader aspect is (0.75, 1.0).
Citations touched (3768-3774, 7285-7298, 7298, 777, 3917-3919, 825, 784/832, 7307) checked with sed.
Covering: `cargo test -p md-core switcher`: 22 passed. Full gate: fmt, build, clippy -D warnings clean; `cargo test --workspace` no failures.

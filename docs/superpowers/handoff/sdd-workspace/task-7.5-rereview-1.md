### Finding Verdicts
1. User loads during a pending load - ADDRESSED. switcher.rs load_preset no longer has the guard (returns Result<()>). Blended: `self.pending = Some(Pending{tick:1, gpu:None,..})` replaces the old Pending, which Rust drops with its gpu (no leak; C++ 7285-7298 matched). Instant: swaps current/old, `old_gpu = gpu.take()`, compiles, timers.reset, preset_loaded() once (7251-7280); `pending` untouched, so tick 8 swaps current (the instant preset) into old and blends from it (checked vs plugin.cpp:7320-7352). InitMode::Instant uses outgoing progress; Blended uses InitMode::Blended; the mv freeze runs only in finish_pending (state.cpp path), so a replaced pending never froze anything. Tests blended_load_during_pending_replaces_it_and_restarts_ticks (7 ticks pending, 8th lands, one recompile "c") and instant_load_during_pending_applies_at_once_and_pending_still_lands cover both paths. Matches the Ruling lines (ledger:459).
2. bump_hard_cut_thresh removed - ADDRESSED. Method gone from switcher.rs; no remaining callers (grep); module doc cites the member vs function-static shadowing (plugin.cpp:6191, 6471; milkdropfs.cpp:825). Matches ledger ruling (ledger:460).
3. headless.rs / load_preset doc (preset_loaded ownership) - ADDRESSED. headless.rs:33 calls textures_mut().preset_loaded() after load_preset; comment corrected; renderer.rs load_preset doc says direct callers must call it. Only other non-test caller is headless (grep).
4. finish_pending device-reset note - ADDRESSED (plugin.cpp:3774 m_bBlending=false named; field path `preset.transition.blending` exists). Doc prose is clumsy, see Minor.
5. Two loads in one frame at blend_time_auto == 0 - ADDRESSED and correct. C++ mfs:784 guards the advance with m_nLoadingPreset==0 and 832 re-reads it afterwards: a blended advance (LoadPreset sets m_nLoadingPreset=1, 7295) makes the cut skip; blend 0 applies at once (7251-7280) and leaves it 0, so the cut also loads. update_timers: `advance_pends = !events.is_empty() && blend_time_auto != 0.0` reproduces this (switcher.rs:205-206); the `!is_pending()` term covers the already-pending case. Threshold doubling is in Timers::update and still happens regardless. Test zero_blend_auto_can_emit_advance_and_hard_cut_in_one_frame covers both sides.
6. update_timers redraw-frame doc - ADDRESSED (cites mfs:777, plugin.cpp:3917-3919).
7. old()/old_mut()/old_gpu() gated on is_blending() - ADDRESSED; test old_is_hidden_unless_blending; instant-load test asserts both None. Internal old/old_gpu still kept for tick 8, as intended.
8. mv_a assertion and fake aspect - ADDRESSED. Assertion is now `new.mv_a.value() == 0.0 && is_blending()` (value() is the target, blend.rs:44, b's mv_a was set 0.0); Fake aspect is (0.75, 1.0).
Fix report names covering tests (22 passed, full gate clean); consistent with the diff.

### New Breakage in the Fix Diff
None Critical or Important.
- Minor: switcher.rs finish_pending doc: the sentence "Also what the C++ does to jump to the new preset when the device is reset" is now split by the inserted note and reads awkwardly (rustfmt-neutral prose only).
- Minor: update_timers doc has an oddly wrapped line ("The caller loads / what the events ask"); cosmetic.
- Minor: advance_pends assumes LoadRandomPreset always succeeds; if the caller finds no preset for a blended advance the C++ would also have m_nLoadingPreset==0 and cut. Caller (7.9) edge; not a defect in this diff.

### Out-of-Scope Observations
None.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage.

### Spec Compliance
✅ (a) msg keys/defaults = ReadCustomMessages (plugin.cpp:8358-8397, text-gated 8421, overrides 8440-8453; face default "arial", `font` default 0) → messages.rs `from_ini`/`CustomMessage::defaults`; n>99→99 (8460), n<0 `rand()%count+1` (8470-8479); exactly 5 jitter draws x,y (8497-8498), R,G,B (8515-8517), `(r%1037)/1037*2-1`, `(int)` truncation + clamp 0..255 (8518-8523) → messages.rs `launch`; caller's Rng, no CRT emulation (A.4).
✅ (a) fade = fade-in only, fraction of duration: `CosineInterp(min(1, fProgress/fFadeTime))` (mfs:4667); timeline progress/burn/off (mfs:1059-1067, 1123-1129) via md-render `SuperText::plan`; test `message_runs_on_the_supertext_timeline` (rename of the brief's `message_fades_after_time` justified).
✅ (a) one m_supertext: song title never writes fFadeTime (8546-8565) → `SuperTextSlot::start` (supertext.rs:165) keeps it; later launch replaces earlier, no burn-in; pinned by `message_interrupting_a_song_title_replaces_it_as_the_cpp`.
✅ (b) TITLE_FONT_SIZES = g_title_font_sizes (mfs:45-53, 36 entries); band = (X, Y*17/21 - Y*1/21) (mfs:309-313); " %s " (mfs:243); search lo=0, hi=N-1, break at lo==hi-1 using mid==lo, `temp.right >= rect.right || h > band_h` → hi (mfs:328-375) — `fit_font_px` matches incl. 512 never chosen.
✅ (c) keys img, colorkey_lo then colorkey (8601-8602), colorkey_hi unread, init_N/code_N (8604-8639; brief's per_frame_N is wrong — C++ wins), 8190-char cap identical, StripLinefeedCharsAndComments (texmgr.cpp:174-199) mirrored; 16 slots (texmgr.h:33), first empty else smallest nStartFrame, ties lowest, KillTex before LoadTex (8641-8667); variable set = RegisterBuiltInVariables (texmgr.cpp:276-310), values (mfs:3479-3488); defaults before init (texmgr.cpp:216-231); done kills after draw (mfs:3756-3759) via tick/reap; burn passed through to SpriteFrame (md-render sprites.rs:156).
✅ (c) GlobalMem shared with presets: ns-eel2 `nseel_globalregs[100]` (nseel-eval.c:45) and static gmembuf (nseel-ram.c:103) are process-wide; test `sprites_share_registers_and_gmegabuf_with_presets`.
✅ (d) Verified: texmgr.cpp:241-259 strips and returns true, never calls NSEEL_code_compile, so m_codehandle stays NULL and neither RunInitCode (233-236) nor DrawUserSprites (mfs:3490-3496) executes code. Upstream structure (per-slot tex_eel_ctx texmgr.cpp:66, var_* pointers, TEXMGR_WARN_ERROR_IN_INIT/REG_CODE texmgr.h, IDS strings plugin.rc:821/823) implies compile+run; Rust compiles init, sets defaults, runs, compiles per-frame in the same Vm — matches the Ruling.
✅ (f) Determinism: only rand is the caller's `impl Rng`; seeded tests pin draw order and count. Load errors: 7.0 s/Misc/not bold for missing img (8583-8584), 6.0 s/Misc/bold for BADFILE/OUTOFMEM/code warnings, exact plugin.rc strings (791-828) → `SpriteToast` for `Ui::add_error` (ui/mod.rs:355).
⚠️ GetPrivateProfileInt edge rules (negative → UINT wrap, non-numeric → default vs 0) for colorkey are Win32 behaviour not in the tree (sprites.rs:166-178); unverifiable.
⚠️ Upstream RecompileExpressions' `if (buf[0])` empty-code guard (Rust `compile` returns Ok(None)) is not in the snapshot; assumed.

### Strengths
- Brief errors (per_frame_N, Vec<TextDraw>, fade-out) resolved to the C++ and listed; ~25 citations re-checked — my sed spot-check of 50+ found only off-by-one-line test comments.
- Kill-before-load, the 5-draws-always rule and the shared-supertext quirk each have a test against the C++.

### Issues
#### Critical
(none)
#### Important
1. Both code warnings at once differ from the C++: `switch(ret & TEXMGR_WARNING_MASK)` (plugin.cpp:8674-8686) matches only 0x10 or 0x20; with both bits (0x30) it takes `default:` and shows NO toast. sprites.rs:466-483 pushes both, and sprites_tests.rs:307-322 `code_errors_warn_but_keep_the_sprite` asserts both. Fix: one toast when only one fails, none when both fail; correct the test.
#### Minor
2. Undocumented departure: a second sprite from an already-loaded file memcpy's the td_tex incl. tex_eel_ctx and var_* (texmgr.cpp:76-85), so RunInitCode resets the first sprite's context (NSEEL_VM_resetvars frees its vars, nseel-compiler.c:1716-1735 — use-after-free). The per-sprite Vm (sprites.rs:460) is the right choice; note it in the module doc.
3. Two copies of m_supertext: `SuperTextSlot` (supertext.rs) never gets the end-of-anim reset (mfs:1127-1128) or the failed-redraw reset (mfs:945-946) that md-render does (title.rs:321, 331), so `is_active()` (supertext.rs:184) stays true after a message finishes. Add a 7.9 note: the renderer's SuperText is authoritative (spawn gate mfs:788-805 reads fStartTime/fDuration/bRedrawSuperText), or drop/sync the md-core copy.
4. `TitleLaunch` stores fade twice (`st.fade_time` and `fade_time: Option`); song_title's `st.fade_time: 0.0` would clobber the kept value if 7.9 copies `launch.st` into the renderer wholesale (harmless today: song titles use the pow branch, mfs:4665).
5. `base_dir.join(img)` (sprites.rs:420) is a path join, not the C++ string concat; a leading `\`/`/` or Windows `sub\x.png` in a relative `img=` resolves differently on Unix.
6. sprites.rs is 611 lines (limit ~600); `Refs` could move out.
7. Test comment citations off by a few lines: "8473-8481" for `sel` (it is 8470), ":8590-8596" (the `if` is 8589).
- (e) `Sprites<H>` is fine (TextureId has no public ctor). The GPU-less gaps hide no defect: `TickedSprite::frame` just calls the tested `SpriteFrame::from_vars`, and `StoreTextures` only delegates to load_sprite/kill_sprite. Mapping every TextureError to BadFile is acceptable.

### Assessment
**Task quality:** Needs fixes — **Reasoning:** Messages, fitting, slots, variables, GlobalMem sharing and the run-sprite-code Ruling all match the C++. But the combined-warning toast contradicts LaunchSprite's switch, and a test pins that wrong behaviour.

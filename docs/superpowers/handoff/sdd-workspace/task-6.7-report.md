# Task 6.7 report: song title texture and animation

Commit: dbd5236 `feat(render): song title texture and animation` (BASE 295b53a). Status: DONE_WITH_CONCERNS.

## What was implemented
- `crates/md-render/src/title_mesh.rs` (pure): `SuperText` (td_supertext, plugin.h:134-155), `SONG_TITLE_DURATION = 1.7` (plugin.cpp:966), `TitlePlan` (per-frame state machine), `title_passes` (mesh, warp, scale, colour ramp, nudges), `title_font_px`.
- `crates/md-render/src/title.rs` (GPU): `Renderer::{set_title_font, set_title_font_bundled, set_song_title_duration, start_song_title, render_title_texture, title_texture, supertext, supertext_mut, draw_title_anim}`, burn-in, per-frame hooks.
- Hooks: `frame.rs` (redraw + plan at begin_frame start; burn-in after `draw_sprites_fixed`, before the composite; screen draw recorded into `FrameCtx.title`; state off after the frame), `comp.rs` (title drawn after the composite, before the 6.6 text), `renderer.rs` (`title` field, `invalidate()` on a main-texture resize), `text_draw.rs` (`sync_atlas` made `pub(crate)`; 6.6 glyph code reused, not forked), `lib.rs` exports.

## Pinned from the C++ (every line cited was re-read with `sed -n` before commit; I corrected about 20 first-draft citations that were off)
- Title texture: `max(tex_w, tex_h)` x `/4` (plugin.cpp:2177-2178), NOT a fixed 1024x256. Device-limited by clamping to max_texture_dimension_2d (the C++ retries smaller sizes on alloc failure, :2192-2208).
- Font: songtitle slot at `nSize * title_w / 256` px, min 6 (plugin.cpp:2228-2229): "double size" holds only for a 512 texture; it scales with the texture. Always anti-aliased (D3DX font uses ANTIALIASED_QUALITY, :2247).
- Fit (mfs:399-448): CALCRECT in the 1/21..17/21 band; if wider than the texture and more than 8 chars, cut to `(int)(len*0.91*W/width)` chars + "..."; drawn DT_CENTER in a rect centred on row `H/2`. No fitting/scaling beyond that. The font-size binary search (`g_title_font_sizes`) is custom-message only (mfs:315-397), left to 7.8.
- Mesh: 16 x 8 vertices (128), 15 x 7 quads = 630 indices, not "32x8" (mfs:4496-4497, 4533-4545, 4636-4649). `v = (y/7 - 0.5)*0.75 + 0.5` (VERT_CLIP, mfs:43).
- Duration 1.7 s (`m_fSongTitleAnimDuration`, plugin.cpp:966, ini :1217).
- Distortion (mfs:4547-4569): `t2 = max(0,1-1.5p)^1.8 * 1.3`, six sine terms of GetTime(), then scale `1.01/(p^0.21+0.01)`; position size `50/fontSizeUsed * 1.5^(fontSize-2)` clamped to 0.88 (mfs:4501-4511); y *= `m_fInvAspectY`.
- Colour/blend (mfs:4660-4710): two passes. Pass 0 shadow colour `(t,t,t,t)`, blend ZERO/INVSRCCOLOR; pass 1 text colour `t*rgb/255, t`, blend ONE/ONE; `t = p^0.3` for a song title. Nudges accumulate on the same vertices: +2 px (pass 0), then -4 more (pass 1), net text -2 px up-left of the unnudged mesh.
- Where drawn / frame order (mfs:1059-1067, 1123-1129): burn-in into the main texture (warp target) on the first frame with progress >= 1, after DrawSprites (darken_center, borders) and before the composite, so the next warp carries it; plus every active frame over the composite (window-sized intermediate), after the composite and under the 6.6 text. On that final frame the screen draw uses `min(p, 0.9999)` and the state then goes off.
- Active range: nothing while `start_time < 0`; screen draw for every frame from launch; burn-in only on the finishing frame.

## Departures from the brief
1. No `seed`: the C++ has no per-animation random values. The only variation is the warp wobble, a function of `GetTime()`. `draw_title_anim(ctx, progress, now)` takes the clock; determinism needs no RNG. (Custom messages' `rand()` is in LaunchCustomMessage, 7.8's.)
2. Size 1024x256 -> `max(tex_w,tex_h)` x /4; "32x8-ish" -> 16x8; "double size" -> `nSize*W/256`.
3. Drawn twice (burn-in into main, and over the composite), not once; both from `begin_frame` automatically (needs `FrameInput::now` = GetTime()). `draw_title_anim` stays public for an explicit pose.
4. Fixed-function y flip: `PrepareFor2DDrawing`'s ortho (2,-2) (support.cpp:179) flips y for this draw, as comp_fixed already does, so vertices carry `-y`. The first literal port rendered the title upside down on screen; the test `title_is_upright_on_screen_and_burnt_into_main` pins it.
5. Texture redraw follows `bRedrawSuperText`: `start_song_title` marks it and `begin_frame` renders (mfs:942-947), failure turns the animation off; `render_title_texture(text)` also renders immediately (own submit) and is what the tests call. A main-texture resize invalidates and redraws.
6. Custom-message mesh branch (mfs:4574-4634) is ported and unit-tested (position, growth, tint) but custom-message texture fitting is not; `supertext_mut()` is the 7.8 entry point.
7. Sampler LinearClamp (the C++ inherits the previous draw's state; the title mesh stays inside the texture). Characters are Rust `char`s, not UTF-16 units, for the "..." cut.
8. `assets/shaders/title.wgsl` is identical to `fixed_tex_modulate.wgsl`; the draw uses `FixedMode::TexModulate`. Left in place, unused (not mine to delete; flag for cleanup).
9. Glyph alpha in the title texture: SRCALPHA/INVSRCALPHA applies to the alpha channel too, so antialiased edge texels have alpha < 255; only RGB is used by the animation (diffuse alpha is selected).

## TDD evidence
RED (stubs returning false / drawing nothing, `MD_REQUIRE_GPU=1 cargo test -p md-render --test title`): 0 passed, 4 failed
```
test anim_draws_nothing_outside_its_active_range ... FAILED
test same_inputs_give_identical_pixels ... FAILED  (assertion failed: r.render_title_texture("Rag"))
test title_texture_holds_glyph_coverage_for_a_known_string ... FAILED
test title_anim_at_progress_half_is_visible ... FAILED  (assertion failed: r.start_song_title("Rag", 0.0))
```
GREEN: same command, 5 passed (adds `title_is_upright_on_screen_and_burnt_into_main`, written after the upside-down finding); `--lib title_mesh`: 9 passed (plan range, 16x8/630, final pose and nudges, colour ramp, warp vs now, scale, custom branch, font px, zero duration).
Tests required by the brief/dispatch: `title_anim_at_progress_half_is_visible`; glyph coverage (known string "Rag": size, centring, band, black clear); nothing outside the active range (off state, finishing frame burns, state off, decay-0 frame black; plus CPU plan tests); determinism (same `now` identical pixels, different `now` differs).

## Gate
`cargo fmt --check` OK; `cargo build --workspace` OK; `cargo clippy --workspace --all-targets -- -D warnings` clean; `MD_REQUIRE_GPU=1 cargo test --workspace`: no failures (lavapipe). `tests/golden/rust/default_60.png` unchanged (apps/milkdrop/tests/golden_basic.rs passes; no title in the headless render, inactive path draws nothing). Files: title.rs 393 lines, title_mesh.rs 492, tests/title.rs 243, renderer.rs 514, frame.rs 441.

## Concerns / notes for later tasks
- 7.x md-core: pass `FrameInput::now` as the same clock given to `start_song_title`; convert songtitle `FontConfig` -> `FontSpec` and call `set_title_font` (system fonts) before the first title; `start_song_title` returns false without it.
- 7.8: custom message must also fill `font_size_used` via its own fitting (render path is song-title only) and call `supertext_mut()`; `render_title_texture` is currently song-title only.
- A dropped `FrameCtx` still advances the title state (C++ has no dropped frames).
- Title pixel orientation is validated against the C++ convention (ortho flip) and by reading the output, not against a D3D9 reference image.

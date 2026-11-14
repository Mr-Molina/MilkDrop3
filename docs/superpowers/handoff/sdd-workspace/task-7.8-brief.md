### Task 7.8: Custom messages and sprites

**Files:**
- Create: `crates/md-core/src/{messages.rs,sprites.rs}`

**Interfaces:**
- Produces: `pub struct CustomMessages { fonts: [MsgFont; 16], messages: [Option<CustomMessage>; 100] }` loaded from `milk2_msg.ini` (`[font00..15]` keys `face bold ital r g b`; `[message00..99]` keys `text font size x y randx randy growth time fade r g b`) with `launch(n, now)` and `active(&self, now) -> Vec<TextDraw>` implementing the grow/fade timeline; `pub struct Sprites { slots: [Option<SpriteSlot>; 16] }` loaded from `milk2_img.ini` (`[img00..99]` keys `img colorkey colorkey_lo colorkey_hi init_N per_frame_N`) with `launch(n, textures, now, frame)` (evicting the oldest slot by start frame when full) and `tick(&mut self, now, frame, sound) -> Vec<SpriteFrame>` running each sprite's per-frame program with the variables `time frame fps progress bass bass_att mid mid_att treb treb_att x y sx sy repeatx repeaty rot flipx flipy r g b a blendmode done burn`; a sprite whose `done` becomes true is killed.

- [ ] **Step 1:** Write failing tests `message_defaults_match_cpp` (size 50, x 0.5, y 0.5, growth 1.0, face "arial"), `message_fades_after_time`, `sprite_slot_eviction_picks_oldest`, `sprite_done_kills_it`, `sprite_init_runs_once`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): custom messages and sprites`


### Finding Verdicts
1. **[Important] kill_sprite dropped a shared TextureId** - ADDRESSED. textures.rs:336-352 `new_instance`/`kill_sprite`: per-slot ids map to a shared sprite-cache texture; the shared texture is removed only when no slot still maps to it (`values().any`), a second kill finds no slot and returns (no-op, matches texmgr.cpp:157 guard). Bookkeeping: slot ids = INSTANCE_ID_BASE + live + killed = number of slots ever created, so unique and never reused; no leak (slot removed on kill, texture removed with last slot, by_key removed in TexCache::remove so a reload decodes afresh); no dangling (`texture()` returns None for a dead slot, or for a slot whose texture is gone). Ranges: disk cache from 0, shared sprite textures from 1<<31 (room for 2^30 before 3<<30), slots from 3<<30; no collision. preset_loaded() (textures.rs:224) bumps only `self.cache`; the sprite cache has no make_room/evict path, so eviction cannot touch sprite textures or slots. Test sprites.rs:314-348 covers distinct handles, kill one of two, repeated dead kills, last kill, reload.
2. **[Folded] upright-image GPU test** - ADDRESSED. sprites.rs:351-383: top-half red/bottom-half blue file, asserts mean red row < mean blue row - 4.
3. **[Folded] blend modes 2/3/4 readbacks** - ADDRESSED. Checked against the equations (grey dst 100, white tint, a=0.5, tolerance 3): mode 2 src=tex*0.5=(50,100,25)+100 = (150,200,125) correct. Mode 3 s*s+d(1-s): R 100, G 178.4 (expects 179), B 90.2 (expects 90) correct. Mode 4 alpha=128/255*0.5=0.251: G 200*.251+100*.749=125.1, B 87.4 (expects 88), R 100 correct; mode 0 control G 150, B 75 discriminates. Backdrop assert guards the decal.
4. **[Folded] record_comp named title/sprites params** - ADDRESSED. comp.rs signature and uses (title, sprites at record_title/record_sprites/burns), frame.rs call site updated.

### New Breakage in the Fix Diff
None. Minor (non-blocking): `texture()` (textures.rs:361) still resolves a raw shared id in [1<<31, 3<<30), unreachable from outside; kill_sprite's `any` scan is O(slots), negligible.

### Out-of-Scope Observations
None.

### Verdict
**Fix round:** All findings addressed, no new Critical/Important breakage. (Not re-run; fix report cites MD_REQUIRE_GPU=1 workspace tests, 11 sprites tests, clippy/fmt clean.)

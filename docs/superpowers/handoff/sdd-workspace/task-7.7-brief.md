### Task 7.7: UI state machine, menus, overlays

**Files:**
- Create: `crates/md-core/src/ui/{mod.rs,mode.rs,menu.rs,menu_tree.rs,load_screen.rs,save_screen.rs,overlays.rs}`

**Interfaces:**
- Produces: `pub enum UiMode { .. }` (the 15 modes), `pub struct Ui { mode, menu: MenuTree, load: LoadScreen, save: SaveScreen, toasts: Vec<Toast>, show_fps, show_preset_name, show_rating, show_help, show_debug }`, `Ui::handle(&mut self, cmd: Command, ctx: &mut UiCtx) -> Vec<UiAction>` (`UiAction::{LoadPreset(PathBuf), SavePreset(PathBuf), SetValue(..), ChangeDir(PathBuf), ..}`), `Ui::draw(&self, q: &mut TextQueue, layout: &ScreenLayout)` emitting the same overlays the C++ draws: preset name lower-left, fps upper-right, rating, "Press F1 for help", help screen, the load screen with `MAX_PRESETS_PER_PAGE = 128` paging and type-to-search, the menu with tooltips and `[value]` editing, error toasts with durations, the debug info block.
- `MenuTree` is built from a declarative table in `menu_tree.rs` mirroring `menu.cpp`'s `AddChildMenu`/`AddItem` calls (each item: label, type, target field, min, max, step, tooltip string from the former `.rc` table).

- [ ] **Step 1:** Write failing tests `escape_from_menu_returns_to_regular`, `load_screen_search_jumps_to_prefix`, `menu_float_item_increments_by_step`, `menu_tree_item_count_matches_cpp` (count recorded from `menu.cpp`), `save_overwrite_requires_confirmation`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): UI modes, menus, overlays`


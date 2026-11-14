//! `CMilkMenu` (menu.h:55-123, menu.cpp): the runtime of the menu tree, its
//! navigation and the rules for editing a value. The tree itself is the table
//! in [`super::menu_tree`] and [`super::menu_custom`]; drawing is in
//! [`super::menu_draw`].
//!
//! The menus live in one arena ([`MenuTree::menus`]); a menu names its parent
//! and children by index, where the C++ keeps pointers
//! (`m_pParentMenu`, `m_ppChildMenu`). Index order is the order
//! `CPlugin::BuildMenus` creates them: [`ROOT`], [`MOTION`], [`CUSTOM_SHAPES`],
//! [`CUSTOM_WAVES`], [`WAVEFORM`], [`AUGMENT`], [`POST`], then the four
//! custom wave menus and the four custom shape menus.

use super::field::{FieldId, FieldValue};
use super::menu_custom::{SHAPECODE_ITEMS, WAVECODE_ITEMS};
use super::menu_tree::{
    ItemDef, ItemKind, Target, AUGMENT_ITEMS, MOTION_ITEMS, POST_ITEMS, PRESET_ITEMS, WAVE_ITEMS,
};

/// Arena index of `m_menuPreset`, the root (`m_pCurMenu` starts here,
/// plugin.cpp:6665).
pub const ROOT: usize = 0;
/// `m_menuMotion`.
pub const MOTION: usize = 1;
/// `m_menuCustomShape`.
pub const CUSTOM_SHAPES: usize = 2;
/// `m_menuCustomWave`.
pub const CUSTOM_WAVES: usize = 3;
/// `m_menuWave`.
pub const WAVEFORM: usize = 4;
/// `m_menuAugment`.
pub const AUGMENT: usize = 5;
/// `m_menuPost`.
pub const POST: usize = 6;
/// First of the four `m_menuWavecode`.
pub const WAVECODE0: usize = 7;
/// First of the four `m_menuShapecode`.
pub const SHAPECODE0: usize = 11;
/// `MAX_CUSTOM_WAVES` and `MAX_CUSTOM_SHAPES`.
pub const SLOTS: usize = 4;

/// One item of a menu (`CMilkMenuItem`, menu.h:55-81).
#[derive(Clone, Debug)]
pub struct MenuItem {
    /// The table row.
    pub def: &'static ItemDef,
    /// `m_bEnabled`: a disabled item is not drawn and not selectable.
    pub enabled: bool,
    /// `m_nLastCursorPos`: where the cursor was when the string was last
    /// accepted (menu.cpp:418).
    pub last_cursor: usize,
    /// `m_original_value`: the value before editing, times 10000 and
    /// truncated for floats (menu.cpp:534-550).
    pub original: i64,
}

/// One menu (`CMilkMenu`, menu.h:83-123).
#[derive(Clone, Debug)]
pub struct Menu {
    /// `m_szMenuName`.
    pub name: String,
    /// `m_pParentMenu`.
    pub parent: Option<usize>,
    /// `m_ppChildMenu`.
    pub children: Vec<usize>,
    /// The linked list of items.
    pub items: Vec<MenuItem>,
    /// `m_nCurSel`: sub-menus come first, then items.
    pub cur_sel: usize,
    /// `m_bEditingCurSel`.
    pub editing: bool,
    /// `m_bEnabled`.
    pub enabled: bool,
    /// The custom wave or shape this menu edits, for the field ids.
    pub slot: u8,
}

impl Menu {
    fn new(name: String, parent: Option<usize>, defs: &'static [ItemDef], slot: u8) -> Menu {
        Menu {
            name,
            parent,
            children: Vec::new(),
            items: defs
                .iter()
                .map(|def| MenuItem {
                    def,
                    enabled: true,
                    last_cursor: 0,
                    original: 0,
                })
                .collect(),
            cur_sel: 0,
            editing: false,
            enabled: true,
            slot,
        }
    }

    /// `m_nChildMenus + m_nChildItems`.
    pub fn len(&self) -> usize {
        self.children.len() + self.items.len()
    }

    /// Whether the menu has no row at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `ItemIsEnabled(j)` (menu.cpp:87-104), `j` counting sub-menus first.
    pub fn row_enabled(&self, j: usize, menus: &[Menu]) -> bool {
        if j < self.children.len() {
            return menus[self.children[j]].enabled;
        }
        self.items
            .get(j - self.children.len())
            .is_some_and(|i| i.enabled)
    }

    /// The highlighted item, when an item (not a sub-menu) is (`GetCurItem`).
    pub fn cur_item(&self) -> Option<&MenuItem> {
        self.items
            .get(self.cur_sel.checked_sub(self.children.len())?)
    }

    /// The field the highlighted item edits.
    pub fn cur_field(&self) -> Option<FieldId> {
        field_of(self.cur_item()?.def.target, self.slot)
    }
}

/// The field id of a table target for a menu with custom slot `slot`.
pub fn field_of(t: Target, slot: u8) -> Option<FieldId> {
    match t {
        Target::None => None,
        Target::Preset(f) => Some(FieldId::Preset(f)),
        Target::Wave(f) => Some(FieldId::Wave(slot, f)),
        Target::Shape(f) => Some(FieldId::Shape(slot, f)),
    }
}

/// The whole tree and where the user is in it.
#[derive(Clone, Debug)]
pub struct MenuTree {
    /// The arena.
    pub menus: Vec<Menu>,
    /// `m_pCurMenu`.
    pub cur: usize,
}

impl Default for MenuTree {
    fn default() -> MenuTree {
        MenuTree::new()
    }
}

impl MenuTree {
    /// `CPlugin::BuildMenus` (plugin.cpp:6661-6871).
    pub fn new() -> MenuTree {
        let mut m = vec![
            Menu::new("--edit current preset".into(), None, &PRESET_ITEMS, 0),
            Menu::new("--MOTION".into(), Some(ROOT), &MOTION_ITEMS, 0),
            Menu::new("--drawing: custom shapes".into(), Some(ROOT), &[], 0),
            Menu::new("--drawing: custom waves".into(), Some(ROOT), &[], 0),
            Menu::new(
                "--drawing: simple waveform".into(),
                Some(ROOT),
                &WAVE_ITEMS,
                0,
            ),
            Menu::new(
                "--drawing: borders, motion vectors".into(),
                Some(ROOT),
                &AUGMENT_ITEMS,
                0,
            ),
            Menu::new(
                "--post-processing, misc.".into(),
                Some(ROOT),
                &POST_ITEMS,
                0,
            ),
        ];
        for i in 0..SLOTS {
            m.push(Menu::new(
                format!("--custom wave {}", i + 1),
                Some(CUSTOM_WAVES),
                &WAVECODE_ITEMS,
                i as u8,
            ));
        }
        for i in 0..SLOTS {
            m.push(Menu::new(
                format!("--custom shape {}", i + 1),
                Some(CUSTOM_SHAPES),
                &SHAPECODE_ITEMS,
                i as u8,
            ));
        }
        m[ROOT].children = vec![MOTION, CUSTOM_SHAPES, CUSTOM_WAVES, WAVEFORM, AUGMENT, POST];
        m[CUSTOM_SHAPES].children = (SHAPECODE0..SHAPECODE0 + SLOTS).collect();
        m[CUSTOM_WAVES].children = (WAVECODE0..WAVECODE0 + SLOTS).collect();
        MenuTree {
            menus: m,
            cur: ROOT,
        }
    }

    /// The menu the user is in.
    pub fn current(&self) -> &Menu {
        &self.menus[self.cur]
    }

    /// Total items over all menus (the `AddItem` calls, loops unrolled).
    pub fn item_count(&self) -> usize {
        self.menus.iter().map(|m| m.items.len()).sum()
    }

    /// `EnableItem` (menu.cpp:107-148): enable or disable a sub-menu or item
    /// by name. A disabled row that held the selection moves it (up first,
    /// then down).
    pub fn enable_item(&mut self, menu: usize, name: &str, enable: bool) {
        let kids: Vec<usize> = self.menus[menu].children.clone();
        let mut found = false;
        for k in kids {
            if self.menus[k].name == name {
                self.menus[k].enabled = enable;
                found = true;
                break;
            }
        }
        if !found {
            match self.menus[menu]
                .items
                .iter_mut()
                .find(|i| i.def.label == name)
            {
                Some(i) => i.enabled = enable,
                None => return,
            }
        }
        if !enable {
            self.settle(menu);
        }
    }

    /// The loops after a disable (menu.cpp:116-120).
    fn settle(&mut self, menu: usize) {
        let n = self.menus[menu].len();
        let en = |t: &MenuTree, j: usize| t.menus[menu].row_enabled(j, &t.menus);
        let mut sel = self.menus[menu].cur_sel;
        while sel > 0 && !en(self, sel) {
            sel -= 1;
        }
        if sel == 0 && !en(self, 0) {
            while sel + 1 < n && !en(self, sel) {
                sel += 1;
            }
        }
        self.menus[menu].cur_sel = sel;
    }

    /// `SetMenusForPresetVersion` (plugin.cpp:6635-6659): which items make
    /// sense for the preset's pixel shader versions.
    pub fn set_for_preset_version(&mut self, warp: i32, comp: i32) {
        let max = warp.max(comp);
        let root = [
            ("[ edit warp shader ]", warp > 0),
            ("[ edit composite shader ]", comp > 0),
        ];
        for (n, e) in root {
            self.enable_item(ROOT, n, e);
        }
        let post = [
            ("sustain level", warp == 0),
            ("texture wrap", warp == 0),
            ("gamma adjustment", comp == 0),
            ("hue shader", comp == 0),
            ("video echo: alpha", comp == 0),
            (" video echo: zoom", comp == 0),
            (" video echo: orientation", comp == 0),
            ("filter: invert", comp == 0),
            ("filter: brighten", comp == 0),
            ("filter: darken", comp == 0),
            ("filter: solarize", comp == 0),
            ("blur1: edge darken amount", max > 0),
            ("blur1: min color value", max > 0),
            ("blur1: max color value", max > 0),
            ("blur2: min color value", max > 0),
            ("blur2: max color value", max > 0),
            ("blur3: min color value", max > 0),
            ("blur3: max color value", max > 0),
        ];
        for (n, e) in post {
            self.enable_item(POST, n, e);
        }
    }

    /// `VK_UP` (menu.cpp:437-450).
    pub fn up(&mut self) {
        let menu = self.cur;
        let n = self.menus[menu].len();
        let en = |t: &MenuTree, j: usize| t.menus[menu].row_enabled(j, &t.menus);
        let mut sel = self.menus[menu].cur_sel;
        if sel != 0 {
            loop {
                sel -= 1;
                if !(sel > 0 && !en(self, sel)) {
                    break;
                }
            }
        }
        while sel + 1 < n && !en(self, sel) {
            sel += 1;
        }
        self.menus[menu].cur_sel = sel;
    }

    /// `VK_DOWN` (menu.cpp:451-464).
    pub fn down(&mut self) {
        let menu = self.cur;
        let n = self.menus[menu].len();
        let en = |t: &MenuTree, j: usize| t.menus[menu].row_enabled(j, &t.menus);
        let mut sel = self.menus[menu].cur_sel;
        if sel + 1 != n {
            loop {
                sel += 1;
                if !(sel + 1 < n && !en(self, sel)) {
                    break;
                }
            }
        }
        sel = sel.min(n.saturating_sub(1));
        while sel > 0 && !en(self, sel) {
            sel -= 1;
        }
        self.menus[menu].cur_sel = sel;
    }
}

/// `VK_UP`/`VK_DOWN`/`VK_PRIOR`/`VK_NEXT` while a value is edited
/// (menu.cpp:637-698): the new value of `v` for an item of `kind` with the
/// bounds `min..max`. `dec` is Down or PageDown; `mult` is 10 for the page
/// keys and 0.1 for Shift+arrow.
pub fn step_value(kind: ItemKind, v: f32, min: f32, max: f32, dec: bool, mult: f32) -> f32 {
    match kind {
        ItemKind::Int => {
            let mult = if mult < 1.0 { 1.0 } else { mult };
            let step = (if dec { -mult } else { mult }) as i32;
            let mut i = v as i32 + step;
            if (i as f32) < min {
                i = min as i32;
            }
            if (i as f32) > max {
                i = max as i32;
            }
            i as f32
        }
        ItemKind::Float | ItemKind::Blendable => {
            let inc = (max - min) * 0.01 * mult;
            clamp_c(v + if dec { -inc } else { inc }, min, max)
        }
        ItemKind::LogFloat | ItemKind::LogBlendable => {
            let f = if dec {
                (1.0f32 / 1.01).powf(mult)
            } else {
                1.01f32.powf(mult)
            };
            clamp_c(v * f, min, max)
        }
        _ => v,
    }
}

/// `if (v < min) v = min; if (v > max) v = max;`, in that order.
fn clamp_c(mut v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        v = min;
    }
    if v > max {
        v = max;
    }
    v
}

/// `m_original_value` of a value (menu.cpp:534-550): ints as they are,
/// floats times 10000, truncated.
pub fn original_of(kind: ItemKind, v: &FieldValue) -> i64 {
    match kind {
        ItemKind::Int => i64::from(v.as_i32()),
        _ => (v.as_f32() * 10000.0) as i64,
    }
}

/// The value `Escape` puts back (menu.cpp:599-627).
pub fn restore_of(kind: ItemKind, original: i64) -> FieldValue {
    match kind {
        ItemKind::Int => FieldValue::Int(original as i32),
        _ => FieldValue::Float(original as f32 * 0.0001),
    }
}

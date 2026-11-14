//! Tests of what the UI's screens queue (Task 7.7): the load list and its
//! paging, the menu, the prompts, toasts and the shader editor.

use md_render::{FontId, QueuedMessage};

use crate::commands::{Command, Mods};
use crate::ui::{ErrCategory, ScreenLayout, UiMode};
use crate::ui_draw_tests::{dark_boxes, find, frame, layout, queue, texts, FONT, H, W};
use crate::ui_tests::{ctx, Harness};

#[test]
fn load_screen_lists_rows_under_a_prompt_with_a_tooltip() {
    let mut h = Harness::new();
    h.ch('l');
    h.ui.load.pos = h.row("beta.milk");
    let mut ctx = ctx!(h);
    ctx.current_preset = Some(h.row("gamma.milk"));
    ctx.locked_by_user = true;
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    let t = texts(&q);

    // the prompt and the directory, upper left, simple font, black boxes
    let prompt = find(&t, "Load which preset?");
    assert_eq!(prompt.len(), 1);
    assert_eq!((prompt[0].rect.left, prompt[0].rect.top), (10, 10));
    assert_eq!(prompt[0].font, FontId::Simple);
    let dir = find(&t, "Directory of: ");
    assert_eq!(dir[0].rect.top, 10 + FONT);

    // rows: dirs as [ name ], files without extension; colours by role
    let sub = find(&t, " [ sub ] ");
    assert_eq!(sub[0].color, [0x88, 0xCC, 0xFF, 0xFF]);
    assert_eq!(find(&t, " [ .. ] (parent directory) ").len(), 1);
    let beta = find(&t, " beta ");
    assert_eq!(beta[0].color, [0xFF, 0x50, 0x50, 0xFF], "selected");
    let gamma = find(&t, " gamma <locked> ");
    assert_eq!(
        gamma[0].color,
        [0xCC, 0xFF, 0x00, 0xFF],
        "the running preset"
    );
    assert_eq!(find(&t, " alpha ")[0].color, [0xCC, 0xCC, 0xCC, 0xFF]);
    // rows go down one line each, in list order
    let alpha_y = find(&t, " alpha ")[0].rect.top;
    assert_eq!(find(&t, " beta ")[0].rect.top, alpha_y + FONT);

    // a page tooltip in the lower right, in the tooltip font, over a box
    let tip = find(&t, "(page 1 of 1)");
    assert_eq!(tip.len(), 1);
    assert_eq!(tip[0].font, FontId::Tooltip);
    assert_eq!(tip[0].rect.right, W - 10 - 10);
    assert_eq!(tip[0].rect.bottom, H - 10 - 10);
    assert!(
        dark_boxes(&q).len() >= 2,
        "the list box and the tooltip box"
    );
}

#[test]
fn load_screen_pages_at_most_128_rows() {
    let names: Vec<String> = (0..300).map(|i| format!("p{i:03}.milk")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut h = Harness::with_files(&refs);
    let total = h.lib.entries().len();
    assert_eq!(total, 302);
    h.ch('l');
    // a very tall window: the cap, not the height, sets the page
    let tall = ScreenLayout::uniform(W, 6000, FONT);
    let mut q = queue();
    let ctx = ctx!(h);
    h.ui.draw(&mut q, &tall, &ctx);
    assert_eq!(h.ui.load.lines, 128);
    let rows = texts(&q)
        .iter()
        .filter(|d| d.text.starts_with(" p") || d.text.starts_with(" ["))
        .count();
    assert_eq!(rows, 128);
    assert_eq!(find(&texts(&q), "(page 1 of 3)").len(), 1);

    // PageDown moves a page at the next draw (plugin.cpp:4975)
    h.send(Command::PageDown(Mods::NONE));
    assert_eq!(h.ui.load.pos, 0, "applied by the draw, as in the C++");
    let mut q = queue();
    let ctx = ctx!(h);
    h.ui.draw(&mut q, &tall, &ctx);
    assert_eq!(h.ui.load.pos, 128);
    assert_eq!(find(&texts(&q), "(page 2 of 3)").len(), 1);
    h.send(Command::PageDown(Mods::NONE));
    let mut q = queue();
    let ctx = ctx!(h);
    h.ui.draw(&mut q, &tall, &ctx);
    assert_eq!(h.ui.load.pos, 256);
    h.send(Command::PageDown(Mods::NONE));
    let mut q = queue();
    let ctx = ctx!(h);
    h.ui.draw(&mut q, &tall, &ctx);
    assert_eq!(h.ui.load.pos, 301, "clamped to the last row");
    h.send(Command::PageUp(Mods::NONE));
    let mut q = queue();
    let ctx = ctx!(h);
    h.ui.draw(&mut q, &tall, &ctx);
    assert_eq!(h.ui.load.pos, 301 - 128);

    // a short window pages by what fits: (720 - 10 - 2*16 - 8 - 8 ...)/16
    h.ui.load.pos = 0;
    let mut q = queue();
    let ctx = ctx!(h);
    h.ui.draw(&mut q, &layout(), &ctx);
    // rect: top 10 + 2 prompt lines + 16/2 + 4, bottom 720 - 10 - 4:
    // (706 - 54 - 8) / 16
    assert_eq!(h.ui.load.lines, 40);
}

#[test]
fn load_screen_with_no_presets_says_so_and_leaves() {
    let mut h = Harness::new();
    // a missing directory lists nothing (Task 7.4)
    let missing = h.tmp.0.join("missing");
    h.lib = crate::library::PresetLibrary::scan(&missing, md_preset::PsVersion::Ps3_0);
    assert!(h.lib.entries().is_empty());
    h.ui.mode = UiMode::Load;
    let (q, _) = frame(&mut h);
    // nothing but the message itself, drawn the same frame (plugin.cpp:4937-4943)
    assert_eq!(texts(&q).len(), 1);
    assert_eq!(h.ui.mode, UiMode::Regular);
    let t = &h.ui.toasts[0];
    assert!(t.msg.starts_with("ERROR: No preset files found in ") && t.msg.ends_with("*.milk"));
    assert!(
        (t.expire - t.birth - 6.0).abs() < 1e-6,
        "6 seconds, bold, misc"
    );
    assert!(t.bold && t.category == ErrCategory::Misc);
}

#[test]
fn menu_draws_names_with_the_current_row_hilited_and_a_tooltip() {
    let mut h = Harness::new();
    h.ch('m');
    let (q, _) = frame(&mut h);
    let t = texts(&q);
    // sub-menus then items, simple font, menu colours
    let names: Vec<_> = t
        .iter()
        .filter(|d| d.font == FontId::Simple)
        .map(|d| d.text.as_str())
        .collect();
    assert_eq!(
        &names[..3],
        [
            "--MOTION",
            "--drawing: custom shapes",
            "--drawing: custom waves"
        ]
    );
    let rows: Vec<_> = t.iter().filter(|d| d.font == FontId::Simple).collect();
    let first = rows[0];
    assert_eq!(first.color, [0xFF, 0x44, 0x00, 0xFF], "MENU_HILITE_COLOR");
    assert_eq!(rows[1].color, [0xCC, 0xCC, 0xCC, 0xFF]);
    assert_eq!(
        (first.rect.left, first.rect.top),
        (14, 14),
        "inset by PLAYLIST_INNER_MARGIN"
    );
    assert_eq!(rows[1].rect.top, 14 + FONT);
    // the navigation tooltip for a sub-menu, in the tooltip font, lower right
    let tip = find(&t, "navigation: ESC: exit");
    assert_eq!(tip[0].font, FontId::Tooltip);
    assert_eq!(tip[0].rect.bottom, H - 20);
    assert_eq!(dark_boxes(&q).len(), 2, "menu box + tooltip box");

    // on an item: "[ON]/[OFF]" for a bool and the item's own tooltip
    h.send(Command::Escape);
    h.ui.mode = UiMode::Menu;
    h.ui.menu.cur = crate::ui::menu::WAVEFORM;
    h.ui.menu.menus[crate::ui::menu::WAVEFORM].cur_sel = 10; // "use dots"
    let (q, _) = frame(&mut h);
    let t = texts(&q);
    assert_eq!(find(&t, "use dots [OFF]").len(), 1);
    assert_eq!(find(&t, "draw thick [OFF]").len(), 1);
    assert_eq!(find(&t, "wave type").len(), 1);
    let tip = crate::ui::menu_tips::USE_DOTS_TT;
    assert_eq!(
        find(&t, tip)[0].font,
        FontId::Tooltip,
        "the item's own tooltip"
    );
}

#[test]
fn editing_a_menu_value_shows_the_value_in_brackets_spaces() {
    let mut h = Harness::new();
    h.ui.mode = UiMode::Menu;
    h.ui.menu.cur = crate::ui::menu::POST;
    let i = h.ui.menu.menus[crate::ui::menu::POST]
        .items
        .iter()
        .position(|i| i.def.label == "blur1: edge darken amount")
        .unwrap();
    h.ui.menu.menus[crate::ui::menu::POST].cur_sel = i;
    h.preset.blur1_edge_darken.set(0.25);
    h.send(Command::Enter(Mods::NONE));
    let (q, _) = frame(&mut h);
    let t = texts(&q);
    let shown: Vec<_> = t
        .iter()
        .filter(|d| d.font == FontId::Simple)
        .map(|d| d.text.as_str())
        .collect();
    assert_eq!(
        shown,
        [
            "(use up/down arrow keys, PGUP, PGDN to change value)",
            "Current value of blur1: edge darken amount:",
            " 0.250 ",
        ]
    );
    assert_eq!(find(&t, " 0.250 ")[0].color, [0xFF, 0x44, 0x00, 0xFF]);
}

#[test]
fn prompts_draw_their_questions() {
    let mut h = Harness::new();
    h.ch('l');
    h.ui.load.pos = h.row("beta.milk");
    h.send(Command::Delete(Mods::NONE));
    let (q, _) = frame(&mut h);
    let t = texts(&q);
    assert_eq!(
        t[0].text,
        "Are you SURE you want to delete this preset? [y/N]"
    );
    assert_eq!(t[1].text, "(preset to delete: beta.milk)");
    assert_eq!(t[1].rect.top, t[0].rect.top + FONT);

    let mut h = Harness::new();
    h.ch('s');
    let mut ctx = ctx!(h);
    ctx.warp_shader_lock = true;
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    let t = texts(&q);
    assert_eq!(t[0].text, "Save as:");
    assert_eq!(t[1].text, "(keep in mind... warp shader was locked.)");
    assert!(
        t.iter().any(|d| d.text.contains("current preset")),
        "the text being edited"
    );

    for _ in 0..14 {
        h.send(Command::PrevPreset);
    }
    h.type_str("alpha");
    h.send(Command::Enter(Mods::NONE));
    let mut ctx = ctx!(h);
    ctx.warp_shader_lock = true;
    ctx.comp_shader_lock = true;
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    let t = texts(&q);
    assert_eq!(t[0].text, "This file already exists.  Overwrite it?  [y/N]");
    assert_eq!(t[1].text, "(file in question: alpha.milk)");
    assert_eq!(
        t[2].text,
        " WARNING: do not forget WARP shader was LOCKED! "
    );
    assert_eq!(t[2].color, [255, 255, 255, 255]);
    assert_eq!(
        t[3].text,
        " WARNING: do not forget COMPOSITE shader was LOCKED! "
    );
}

#[test]
fn upgrade_prompt_depends_on_the_shader_versions() {
    let mut h = Harness::new();
    h.ui.mode = UiMode::UpgradePixelShader;
    let say = |h: &mut Harness, warp: i32, comp: i32| {
        h.preset.warp_ps = warp;
        h.preset.comp_ps = comp;
        let (q, _) = frame(h);
        texts(&q).into_iter().map(|d| d.text).collect::<Vec<_>>()
    };
    let t = say(&mut h, 0, 0);
    assert_eq!(t[0], "This preset does not currently use pixel shaders.");
    assert_eq!(
        t[1],
        "Do you want to upgrade it to use pixel shader 2?  [y/N]"
    );
    let t = say(&mut h, 2, 3);
    assert_eq!(t[0], "This preset has mixed versions of shaders in it.");
    assert_eq!(
        t[1],
        "Do you want to upgrade all shaders to use (at least) pixel shader 2.X?  [y/N]"
    );
    let t = say(&mut h, 4, 4);
    assert!(t[0].starts_with("This preset already uses the highest pixel shader version (4)"));
    assert_eq!(t[1], "Press ESC to return.");
}

#[test]
fn toasts_go_last_in_the_upper_right_and_fade() {
    let mut h = Harness::new();
    h.ui.show_fps = true;
    h.ui.add_error("shuffle is now ON", 3.0, ErrCategory::Notify, false, h.now);
    h.ui.add_error("bad shader", 6.0, ErrCategory::Preset, true, h.now);
    let mut ctx = ctx!(h);
    ctx.fps = 60.0;
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    let t = texts(&q);
    // after the fps (two draws), then the toasts: one box each
    let n = t.len();
    assert_eq!(t[n - 2].text, "shuffle is now ON ");
    assert_eq!(t[n - 1].text, "bad shader ");
    assert_eq!(t[n - 2].rect.top, 10 + FONT, "below the fps line");
    assert_eq!(t[n - 1].rect.top, 10 + 2 * FONT);
    assert_eq!(t[n - 1].rect.right, W - 10);
    assert_eq!(t[n - 1].font, FontId::Simple);
    let boxes: Vec<_> = q
        .messages()
        .iter()
        .filter_map(|m| match m {
            QueuedMessage::Box { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    // a plain message has a black box; a bold one starts at red 200 and
    // fades (cr = 200 - 199 * age^4, plugin.cpp:5105)
    assert_eq!(boxes, [[0, 0, 0, 255], [200, 0, 0, 255]]);
    h.now += 3.0;
    let ctx = ctx!(h);
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    let boxes: Vec<_> = q
        .messages()
        .iter()
        .filter_map(|m| match m {
            QueuedMessage::Box { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert_eq!(boxes.len(), 1, "the 3 s notification is over");
    assert_eq!(boxes[0][0], (200.0 - 199.0 * 0.5f32.powi(4)) as u8);
    // warnings disabled: no toasts at all
    h.ui.show_fps = false;
    let mut ctx = ctx!(h);
    ctx.warnings_disabled = true;
    let mut q = queue();
    h.ui.draw(&mut q, &layout(), &ctx);
    assert!(q.is_empty());
}

#[test]
fn shader_editor_shows_the_quick_reference_on_f9() {
    let mut h = Harness::new();
    h.ui.mode = UiMode::Menu;
    let i = h.ui.menu.menus[0].children.len()
        + h.ui.menu.menus[0]
            .items
            .iter()
            .position(|i| i.def.label == "[ edit warp shader ]")
            .unwrap();
    h.ui.menu.menus[0].cur_sel = i;
    h.send(Command::Enter(Mods::NONE));
    assert_eq!(h.ui.mode, UiMode::EditMenuString);
    let (q, _) = frame(&mut h);
    assert!(!find(&texts(&q), "press F9 to show shader Quick Reference").is_empty());
    assert!(find(&texts(&q), "sampler_billy").is_empty());
    h.send(Command::ToggleShaderHelp);
    let (q, _) = frame(&mut h);
    let t = texts(&q);
    assert!(!find(&t, "press F9 to hide").is_empty());
    assert!(!find(&t, "uv_orig").is_empty(), "the warp shader's inputs");
    assert!(
        find(&t, "hue_shader").is_empty(),
        "not the composite shader's"
    );
    // the code editor boxes its text; its tooltip is the page number
    assert_eq!(h.ui.edit.tooltip, " Page 1 ");
}

#[test]
fn edit_prompt_draws_the_text_with_a_cursor() {
    let mut h = Harness::new();
    h.ch('s');
    let (q, _) = frame(&mut h);
    let t = texts(&q);
    let line = find(&t, "current preset");
    assert_eq!(line.len(), 1);
    assert!(line[0].text.starts_with("    current preset"));
    assert_eq!(line[0].color, [0xCC, 0xCC, 0xCC, 0xFF]);
    assert_eq!(dark_boxes(&q).len(), 1);
    // selection: [brackets] around it
    h.send(Command::Left(Mods::SHIFT));
    h.send(Command::Left(Mods::SHIFT));
    let (q, _) = frame(&mut h);
    assert!(
        !find(&texts(&q), "current prese[t]").is_empty()
            || !find(&texts(&q), "current pres[et]").is_empty()
    );
}

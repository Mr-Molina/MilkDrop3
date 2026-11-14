//! Tests for user sprites (Task 7.8).

use crate::sprites::{
    colorkey, SpriteInputs, SpriteLoadError, SpriteTextures, Sprites, NUM_SPRITE_SLOTS,
};
use crate::ui::ErrCategory;
use md_eval::{GlobalMem, Vm};
use md_preset::ini::Ini;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Default)]
struct Fake {
    loads: Vec<(PathBuf, u32)>,
    killed: Vec<u32>,
    fail: Option<SpriteLoadError>,
    next: u32,
}

impl SpriteTextures for Fake {
    type Handle = u32;
    fn load(&mut self, path: &Path, ck: u32) -> Result<u32, SpriteLoadError> {
        if let Some(e) = self.fail {
            return Err(e);
        }
        self.loads.push((path.to_owned(), ck));
        self.next += 1;
        Ok(self.next)
    }
    fn kill(&mut self, h: u32) {
        self.killed.push(h);
    }
}

fn global() -> Rc<RefCell<GlobalMem>> {
    Rc::new(RefCell::new(GlobalMem::new()))
}

fn sprites(text: &str) -> Sprites<u32> {
    Sprites::parse(text, PathBuf::from("/md/"), global())
}

fn inputs(now: f32, frame: i32) -> SpriteInputs {
    SpriteInputs {
        now,
        frame,
        fps: 30.0,
        progress: 0.0,
        imm_rel: [1.0, 2.0, 3.0],
        avg_rel: [4.0, 5.0, 6.0],
    }
}

const SIMPLE: &str = "[img00]\nimg=a.png\n[img01]\nimg=b.png\n[img02]\nimg=C:\\abs\\c.png\n";

#[test]
fn colour_key_maps_like_the_cpp() {
    let ck = |t: &str| colorkey(&Ini::parse(t), "img00");
    // colorkey_lo first, `colorkey` overrides it (plugin.cpp:142-144, 8601-8602).
    assert_eq!(ck("[img00]\n"), 0);
    assert_eq!(ck("[img00]\ncolorkey_lo=65280\n"), 0x00FF00);
    assert_eq!(ck("[img00]\ncolorkey=255\n"), 255);
    assert_eq!(ck("[img00]\ncolorkey_lo=1\ncolorkey=2\n"), 2);
    // colorkey_hi is ignored (the old range was replaced by one colour).
    assert_eq!(ck("[img00]\ncolorkey_hi=2105376\n"), 0);
    // GetPrivateProfileInt reads decimal only: "0x.." reads as 0.
    assert_eq!(ck("[img00]\ncolorkey=0x00ff00\n"), 0);
    // The UINT result wraps for negatives and takes the whole 32 bits.
    assert_eq!(ck("[img00]\ncolorkey=-1\n"), 0xFFFF_FFFF);
    assert_eq!(ck("[img00]\ncolorkey=4294967295\n"), 0xFFFF_FFFF);
    // Another section is not read.
    assert_eq!(colorkey(&Ini::parse("[img01]\ncolorkey=9\n"), "img00"), 0);
}

#[test]
fn launch_passes_the_colour_key_and_resolves_the_path() {
    let mut s = sprites(
        "[img00]\nimg=pics/a.png\ncolorkey_lo=7\ncolorkey=1193046\n[img01]\nimg=D:\\x\\y.png\n",
    );
    let mut t = Fake::default();
    let r = s.launch(0, &mut t, 1.0, 5);
    assert_eq!(r.slot, Some(0));
    assert!(r.toasts.is_empty());
    assert_eq!(t.loads[0], (PathBuf::from("/md/pics/a.png"), 1_193_046));
    // `img[1] == ':'` is an absolute path: used as it is (:8590-8596).
    s.launch(1, &mut t, 1.0, 5);
    assert_eq!(t.loads[1].0, PathBuf::from("D:\\x\\y.png"));
}

#[test]
fn a_load_failure_creates_no_sprite() {
    let mut s = sprites(SIMPLE);
    let mut t = Fake {
        fail: Some(SpriteLoadError::BadFile),
        ..Fake::default()
    };
    let r = s.launch(0, &mut t, 0.0, 0);
    assert_eq!(r.slot, None);
    assert_eq!(s.len(), 0);
    assert_eq!(
        r.toasts,
        vec![crate::sprites::SpriteToast {
            msg: "sprite #0 error: image file missing or corrupt".into(),
            duration: 6.0,
            category: ErrCategory::Misc,
            bold: true
        }]
    );
    assert!(s.tick(&inputs(1.0, 1)).is_empty());
    t.fail = Some(SpriteLoadError::OutOfMemory);
    let r = s.launch(1, &mut t, 0.0, 0);
    assert_eq!(
        r.toasts[0].msg,
        "sprite #1 error: out of memory, unable to load image"
    );
    assert!(s.is_empty());
}

#[test]
fn an_undefined_sprite_raises_the_cpp_error() {
    let mut s = sprites(SIMPLE);
    let mut t = Fake::default();
    let r = s.launch(9, &mut t, 0.0, 0);
    assert_eq!(r.slot, None);
    assert!(t.loads.is_empty());
    assert_eq!(r.toasts.len(), 1);
    assert_eq!(
        r.toasts[0].msg,
        "sprite #9 error: could not find 'img=' setting, or sprite is not defined"
    );
    assert_eq!(
        (r.toasts[0].duration, r.toasts[0].category, r.toasts[0].bold),
        (7.0, ErrCategory::Misc, false)
    );
}

#[test]
fn sprite_slot_eviction_picks_oldest() {
    let mut s = sprites(SIMPLE);
    let mut t = Fake::default();
    // Fill 16 slots; slot k starts at frame 100 - k so slot 15 is oldest...
    // no: oldest = smallest start frame.
    let frames = [
        50, 40, 90, 30, 70, 60, 80, 20, 110, 120, 130, 140, 150, 160, 170, 180,
    ];
    for (k, f) in frames.iter().enumerate() {
        let r = s.launch(0, &mut t, 0.0, *f);
        assert_eq!(r.slot, Some(k), "first empty slot");
    }
    assert_eq!(s.len(), NUM_SPRITE_SLOTS);
    // Full: the oldest (frame 20, slot 7, handle 8) is killed and reused.
    let r = s.launch(1, &mut t, 0.0, 500);
    assert_eq!(r.slot, Some(7));
    assert_eq!(t.killed, vec![8]);
    assert_eq!(s.slot_number(7), Some(1));
    assert_eq!(s.len(), NUM_SPRITE_SLOTS);
    // Next oldest is frame 30 (slot 3); ties keep the lowest index.
    let r = s.launch(2, &mut t, 0.0, 600);
    assert_eq!(r.slot, Some(3));
    // A freed slot is taken before anyone is evicted.
    s.kill_number(2, &mut t);
    assert_eq!(s.len(), NUM_SPRITE_SLOTS - 1);
    assert_eq!(s.launch(0, &mut t, 0.0, 700).slot, Some(3));
}

#[test]
fn eviction_ties_keep_the_lowest_index() {
    let mut s = sprites(SIMPLE);
    let mut t = Fake::default();
    for _ in 0..NUM_SPRITE_SLOTS {
        s.launch(0, &mut t, 0.0, 10);
    }
    assert_eq!(s.launch(1, &mut t, 0.0, 11).slot, Some(0));
}

#[test]
fn eviction_frees_the_old_slot_even_if_the_new_load_fails() {
    // KillTex(nSlot) runs before LoadTex (plugin.cpp:8663-8667).
    let mut s = sprites(SIMPLE);
    let mut t = Fake::default();
    for f in 0..NUM_SPRITE_SLOTS as i32 {
        s.launch(0, &mut t, 0.0, f + 1);
    }
    t.fail = Some(SpriteLoadError::BadFile);
    let r = s.launch(1, &mut t, 0.0, 99);
    assert_eq!(r.slot, None);
    assert_eq!(t.killed, vec![1]);
    assert_eq!(s.len(), NUM_SPRITE_SLOTS - 1);
}

#[test]
fn sprite_done_kills_it() {
    let mut s = sprites("[img00]\nimg=a.png\ncode_1=done=above(frame,2);\n");
    let mut t = Fake::default();
    s.launch(0, &mut t, 0.0, 10);
    let f = s.tick(&inputs(0.1, 11));
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].vars.done, 0.0);
    s.reap(&mut t);
    assert_eq!(s.len(), 1);
    assert!(t.killed.is_empty());
    let f = s.tick(&inputs(0.2, 13)); // frame - start = 3 > 2
    assert_eq!(f.len(), 1, "the done sprite still draws this frame");
    assert_eq!(f[0].vars.done, 1.0);
    // It is killed after the frame is drawn (DrawUserSprites kills after
    // its draw): until reap() its texture lives.
    assert!(t.killed.is_empty());
    s.reap(&mut t);
    assert_eq!(t.killed, vec![1]);
    assert!(s.is_empty());
    assert!(s.tick(&inputs(0.3, 14)).is_empty());
}

#[test]
fn sprite_init_runs_once() {
    let mut s = sprites(
        "[img00]\nimg=a.png\ninit_1=n=10;\ninit_2=reg00=reg00+1;\ncode_1=n=n+1; // counts\ncode_2=x=n/100;\n",
    );
    let g = s.global().clone();
    let mut t = Fake::default();
    s.launch(0, &mut t, 0.0, 0);
    assert_eq!(g.borrow().regs[0], 1.0);
    let a = s.tick(&inputs(0.1, 1));
    let b = s.tick(&inputs(0.2, 2));
    assert_eq!(a[0].vars.x, 0.11);
    assert_eq!(b[0].vars.x, 0.12);
    assert_eq!(g.borrow().regs[0], 1.0, "init did not run again");
}

#[test]
fn outputs_start_at_the_texmgr_defaults_and_persist() {
    // RunInitCode sets the outputs before the init code (texmgr.cpp:216-231);
    // with no code the sprite is centred, unit size, opaque, burnt in.
    let mut s = sprites(SIMPLE);
    let mut t = Fake::default();
    s.launch(0, &mut t, 0.0, 0);
    let v = s.tick(&inputs(0.1, 1))[0].vars;
    assert_eq!(
        (v.x, v.y, v.sx, v.sy, v.rot, v.repeatx, v.repeaty),
        (0.5, 0.5, 1.0, 1.0, 0.0, 1.0, 1.0)
    );
    assert_eq!((v.flipx, v.flipy, v.blendmode), (0.0, 0.0, 0.0));
    assert_eq!(
        (v.r, v.g, v.b, v.a, v.burn, v.done),
        (1.0, 1.0, 1.0, 1.0, 1.0, 0.0)
    );
    // Init may set outputs, per-frame code may leave them alone.
    let mut s = sprites("[img00]\nimg=a.png\ninit_1=x=0.25;burn=0;\n");
    s.launch(0, &mut t, 0.0, 0);
    let v = s.tick(&inputs(0.1, 1))[0].vars;
    assert_eq!((v.x, v.burn), (0.25, 0.0));
}

#[test]
fn sprite_programs_see_the_cpp_variable_set() {
    // texmgr.cpp:RegisterBuiltInVariables: the ten inputs and the sixteen
    // outputs, nothing else; the values are those of DrawUserSprites.
    let code = "\
code_1=x=time;y=frame;sx=fps;sy=progress;
code_2=r=bass;g=mid;b=treb;a=bass_att*mid_att*treb_att;
code_3=rot=bass_att+mid_att+treb_att;repeatx=mid_att;repeaty=treb_att;
code_4=flipx=1;flipy=0;blendmode=3;burn=0;
";
    let mut s = sprites(&format!("[img00]\nimg=a.png\n{code}"));
    let mut t = Fake::default();
    s.launch(0, &mut t, 2.0, 100);
    let mut inp = inputs(2.5, 103);
    inp.progress = 0.75;
    let v = s.tick(&inp)[0].vars;
    assert_eq!(v.x, 0.5); // now - start_time
    assert_eq!(v.y, 3.0); // frame - start_frame
    assert_eq!(v.sx, 30.0);
    assert_eq!(v.sy, 0.75); // m_pState->m_fBlendProgress
    assert_eq!((v.r, v.g, v.b), (1.0, 2.0, 3.0)); // imm_rel
    assert_eq!(v.a, 120.0); // avg_rel
    assert_eq!(v.rot, 15.0);
    assert_eq!((v.repeatx, v.repeaty), (5.0, 6.0));
    assert_eq!(
        (v.flipx, v.flipy, v.blendmode, v.burn),
        (1.0, 0.0, 3.0, 0.0)
    );
}

#[test]
fn code_lines_join_without_a_separator_and_comments_end_at_the_line() {
    // StripLinefeedCharsAndComments drops the line feeds (texmgr.cpp:176-199)
    // and cuts `//` (and `\\`) comments at the end of their line.
    let mut s =
        sprites("[img00]\nimg=a.png\ncode_1=x=0.1; // c1\ncode_2=y=0.2; \\\\ c2\ncode_3=sx=0.3;\n");
    let mut t = Fake::default();
    let r = s.launch(0, &mut t, 0.0, 0);
    assert!(r.toasts.is_empty());
    let v = s.tick(&inputs(0.0, 0))[0].vars;
    assert_eq!((v.x, v.y, v.sx), (0.1, 0.2, 0.3));
    // A line without its `;` runs into the next one, as in the C++.
    let mut s = sprites("[img00]\nimg=a.png\ncode_1=x=0.1\ncode_2=y=0.2;\n");
    let r = s.launch(0, &mut t, 0.0, 0);
    assert_eq!(
        r.toasts[0].msg,
        "sprite #0 warning: error in per-frame code"
    );
    assert_eq!(r.slot, Some(0), "the sprite still exists");
}

#[test]
fn code_errors_warn_but_keep_the_sprite() {
    let mut t = Fake::default();
    let mut s = sprites("[img00]\nimg=a.png\ninit_1=x=;\ncode_1=y=;\n");
    let r = s.launch(0, &mut t, 0.0, 0);
    assert_eq!(r.slot, Some(0));
    let msgs: Vec<_> = r.toasts.iter().map(|t| t.msg.as_str()).collect();
    assert_eq!(
        msgs,
        vec![
            "sprite #0 warning: error in initialization code ",
            "sprite #0 warning: error in per-frame code"
        ]
    );
    assert!(r
        .toasts
        .iter()
        .all(|t| (t.duration, t.category, t.bold) == (6.0, ErrCategory::Misc, true)));
    // Defaults still draw it.
    assert_eq!(s.tick(&inputs(0.0, 0)).len(), 1);
}

#[test]
fn sprites_share_registers_and_gmegabuf_with_presets() {
    // reg00..99 and gmegabuf are process-wide in ns-eel2, so a sprite and
    // a preset context see the same ones.
    let g = global();
    let mut s: Sprites<u32> = Sprites::parse(
        "[img00]\nimg=a.png\ninit_1=reg05=42;gmegabuf(7)=9;\n",
        PathBuf::from("/md/"),
        g.clone(),
    );
    s.launch(0, &mut Fake::default(), 0.0, 0);
    let mut vm = Vm::new(g);
    let r = vm.var("reg05");
    assert_eq!(vm.get(r), 42.0);
    assert_eq!(vm.global.borrow().gmegabuf.get(7.0), 9.0);
}

#[test]
fn kills_by_number_newest_oldest_and_all() {
    let mut s = sprites(SIMPLE);
    let mut t = Fake::default();
    s.launch(0, &mut t, 0.0, 30); // slot 0, handle 1
    s.launch(1, &mut t, 0.0, 10); // slot 1, handle 2
    s.launch(0, &mut t, 0.0, 20); // slot 2, handle 3
    s.launch(2, &mut t, 0.0, 40); // slot 3, handle 4
    s.kill_newest(&mut t); // frame 40
    assert_eq!(t.killed, vec![4]);
    s.kill_oldest(&mut t); // frame 10
    assert_eq!(t.killed, vec![4, 2]);
    s.kill_number(0, &mut t); // both sprite-0 slots
    assert_eq!(t.killed, vec![4, 2, 1, 3]);
    assert!(s.is_empty());
    s.kill_newest(&mut t); // nothing left: no-op
    s.launch(0, &mut t, 0.0, 1);
    s.launch(1, &mut t, 0.0, 2);
    s.kill_all(&mut t);
    assert!(s.is_empty());
    assert_eq!(t.killed.len(), 6);
}

//! User sprites: `milk2_img.ini`, `LaunchSprite`, `texmgr` and the
//! per-frame program run of `DrawUserSprites` (`plugin` =
//! code/vis_milk2/plugin.cpp, `texmgr` = texmgr.cpp, `mfs` = milkdropfs.cpp).
//!
//! There is no GPU here: the image goes through [`SpriteTextures`]
//! ([`StoreTextures`] is the md-render one), and [`Sprites::tick`] returns
//! each sprite's output variables ([`SpriteVars`]) with its texture handle;
//! for md-render's `TextureId` [`TickedSprite::frame`] makes the
//! [`SpriteFrame`] that `Renderer::draw_sprites` takes.
//!
//! * **Slots.** 16 (`NUM_TEX`, texmgr.h:33). `LaunchSprite(n, -1)` takes the
//!   first empty slot, else the one with the smallest start frame (ties keep
//!   the lowest index), which is killed first (plugin:8641-8665), before the
//!   image loads, so a failed load still costs the old sprite.
//! * **Colour key.** `colorkey_lo` (default 0) is read first and `colorkey`
//!   overrides it (plugin:142-144, 8597-8602): one packed colour,
//!   `GetPrivateProfileInt` rules (decimal; the UINT result wraps for
//!   negatives); `colorkey_hi` is not read. [`colorkey`] returns the number
//!   `LoadTex` ORs into `0xFF000000` (texmgr:109), which is what
//!   `TextureStore::load_sprite` takes.
//! * **Code.** `init_1`, `init_2`, ... and `code_1`, `code_2`, ... (the brief's
//!   `per_frame_N` is not a key) read until a key is missing or 8191
//!   characters are used (plugin:8604-8639). Lines are joined WITHOUT a
//!   separator and `//` and `\\` comments cut at the end of their line
//!   (`StripLinefeedCharsAndComments`, texmgr:174-199), so a line needs its
//!   own `;`. The init code runs once at launch after the outputs get their
//!   defaults (texmgr:216-236), the per-frame code every frame
//!   (mfs:3490-3496). Variables persist between frames (one VM per sprite).
//!   The VM shares `reg00..99` and `gmegabuf` with the presets through the
//!   `GlobalMem` it is built with (ns-eel2 keeps both process-wide).
//! * **Variables** (texmgr:`RegisterBuiltInVariables`, 276-310; values mfs:3479-3488):
//!   inputs `time` (seconds since launch) `frame` (frames since launch) `fps`
//!   `progress` (the preset BLEND progress, `m_fBlendProgress`) `bass mid
//!   treb` (`imm_rel`) `bass_att mid_att treb_att` (`avg_rel`); outputs `x y
//!   sx sy repeatx repeaty rot flipx flipy r g b a blendmode done burn`. The
//!   inputs are 0 while the init code runs.
//! * **Kill.** A sprite whose `done` is non-zero still draws this frame and is
//!   killed afterwards (mfs:3756-3759): [`Sprites::tick`] marks it and
//!   [`Sprites::reap`] frees it once the host has drawn the frame (killing
//!   the texture first would make `draw_sprites` skip it).
//!
//! Departure: each sprite has its own VM. In the C++ a second sprite from an
//! already-loaded file copies the first one's `tex_eel_ctx` (texmgr:76-85) and
//! the new launch then resets that shared context (a use-after-free); not
//! reproduced.
//!
//! Departure: this tree's `texmgr::RecompileExpressions` (texmgr:241-259)
//! strips comments and returns true without ever compiling, so its sprite
//! programs never run. The upstream MilkDrop 2 compiles there; this port
//! compiles and runs them as the task requires, and raises the two
//! "error in ... code" warnings for programs that do not compile.

use crate::sprite_vars::Refs;
use crate::ui::ErrCategory;
use md_eval::{GlobalMem, Program, Vm};
use md_preset::ini::Ini;
use md_render::{Gpu, SpriteFrame, SpriteVars, TextureId, TextureStore};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// `NUM_TEX` (texmgr.h:33).
pub const NUM_SPRITE_SLOTS: usize = 16;
/// `LINEFEED_CONTROL_CHAR`: the byte the C++ ends each code line with.
const LINEFEED: char = '\u{1}';
/// The size of `initcode` and `code` (plugin:8569).
const CODE_BUF: isize = 8192;

/// Why an image did not load (`texmgr` return codes 9 and 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteLoadError {
    /// `TEXMGR_ERR_BADFILE`: missing, unreadable or not an image.
    BadFile,
    /// `TEXMGR_ERR_OUTOFMEM`.
    OutOfMemory,
}

/// Where sprite images live: md-render's [`TextureStore`], or a fake.
pub trait SpriteTextures {
    type Handle: Copy;
    /// `LoadTex`'s image part: load `path` with colour key `colorkey`.
    fn load(&mut self, path: &Path, colorkey: u32) -> Result<Self::Handle, SpriteLoadError>;
    /// `KillTex`.
    fn kill(&mut self, h: Self::Handle);
}

/// [`SpriteTextures`] over md-render's store.
pub struct StoreTextures<'a> {
    pub gpu: &'a Gpu,
    pub store: &'a mut TextureStore,
}

impl SpriteTextures for StoreTextures<'_> {
    type Handle = TextureId;
    fn load(&mut self, path: &Path, colorkey: u32) -> Result<TextureId, SpriteLoadError> {
        // D3DX reports any decode or size failure as a bad file; md-render
        // has no out-of-memory result.
        self.store
            .load_sprite(self.gpu, path, colorkey)
            .map_err(|_| SpriteLoadError::BadFile)
    }
    fn kill(&mut self, h: TextureId) {
        self.store.kill_sprite(h);
    }
}

/// An `AddError` the launch raises (route it through `Ui::add_error`).
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteToast {
    pub msg: String,
    /// Seconds.
    pub duration: f32,
    pub category: ErrCategory,
    pub bold: bool,
}

fn toast(msg: String, duration: f32, bold: bool) -> SpriteToast {
    SpriteToast {
        msg,
        duration,
        category: ErrCategory::Misc,
        bold,
    }
}

/// What a launch did.
#[derive(Clone, Debug, PartialEq)]
pub struct LaunchResult {
    /// The slot the sprite took; `None` when it failed (no sprite exists).
    pub slot: Option<usize>,
    pub toasts: Vec<SpriteToast>,
}

/// The per-frame inputs (`GetTime()` etc.).
#[derive(Clone, Copy, Debug)]
pub struct SpriteInputs {
    /// `GetTime()`.
    pub now: f32,
    /// `GetFrame()`.
    pub frame: i32,
    /// `GetFps()`.
    pub fps: f32,
    /// `m_pState->m_fBlendProgress`.
    pub progress: f32,
    /// `mysound.imm_rel`.
    pub imm_rel: [f32; 3],
    /// `mysound.avg_rel`.
    pub avg_rel: [f32; 3],
}

/// One sprite after its per-frame code ran.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickedSprite<H> {
    pub slot: usize,
    /// The sprite's `[imgNN]` number (`nUserData`).
    pub number: i32,
    pub vars: SpriteVars,
    pub handle: H,
}

impl TickedSprite<TextureId> {
    /// The clamped frame `Renderer::draw_sprites` draws; `done` and `burn`
    /// are passed through.
    pub fn frame(&self) -> SpriteFrame {
        SpriteFrame::from_vars(&self.vars, self.handle)
    }
}

/// `swprintf("%s%s", m_szMilkdrop2Path, img)` (plugin:8589-8595): the INI
/// value is appended to the directory as text. `img[1] == ':'` (a drive
/// path) is used as is; otherwise `\\` separators become `/`, and a leading
/// separator is dropped so it cannot make the path absolute.
fn sprite_path(base_dir: &Path, img: &str) -> PathBuf {
    if img.chars().nth(1) == Some(':') {
        return PathBuf::from(img);
    }
    let mut p = base_dir.to_string_lossy().replace('\\', "/");
    if !p.is_empty() && !p.ends_with('/') {
        p.push('/');
    }
    p.push_str(img.replace('\\', "/").trim_start_matches('/'));
    PathBuf::from(p)
}

/// `GetPrivateProfileInt` as an unsigned value: leading decimal digits (after
/// optional blanks and a sign), wrapping for negatives; `None` when absent or
/// not a number.
fn profile_uint(ini: &Ini, section: &str, key: &str) -> Option<u32> {
    let s = ini.get(section, key)?.trim_start();
    let (neg, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        // "0x00ff00" reads as 0 (the 0 is a digit); "abc" as the default.
        return None;
    }
    let v: i64 = digits.parse().ok()?;
    Some(if neg { (-v) as u32 } else { v as u32 })
}

/// The colour key of `[section]`: `colorkey_lo` (default 0), overridden by
/// `colorkey` (plugin:8601-8602).
pub fn colorkey(ini: &Ini, section: &str) -> u32 {
    let lo = profile_uint(ini, section, "colorkey_lo").unwrap_or(0);
    profile_uint(ini, section, "colorkey").unwrap_or(lo)
}

/// `init_N` / `code_N` lines of a section joined as the C++ buffer is
/// (plugin:8604-8639), then stripped (texmgr:174-199).
fn read_code(ini: &Ini, section: &str, prefix: &str) -> String {
    let mut buf = String::new();
    let mut char_pos: isize = 0;
    for line in 1.. {
        let Some(t) = ini.get(section, &format!("{prefix}{line}")) else {
            break;
        };
        let len = t.len() as isize;
        if len >= CODE_BUF - 1 - char_pos - 1 {
            break;
        }
        buf.push_str(t);
        buf.push(LINEFEED);
        char_pos += len + 1;
    }
    strip_code(&buf)
}

/// `StripLinefeedCharsAndComments`: drop the line-feed bytes, and everything
/// from `//` or `\\` to the end of its line.
fn strip_code(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let (mut out, mut comment) = (String::new(), false);
    for (i, &ch) in c.iter().enumerate() {
        let next = c.get(i + 1).copied();
        if comment {
            comment = ch != LINEFEED;
        } else if (ch == '\\' && next == Some('\\')) || (ch == '/' && next == Some('/')) {
            comment = true;
        } else if ch != LINEFEED {
            out.push(ch);
        }
    }
    out
}

struct Slot<H> {
    number: i32,
    handle: H,
    start_time: f32,
    start_frame: i32,
    vm: Vm,
    refs: Refs,
    program: Option<Program>,
    /// `done` was set by the last tick: kill at [`Sprites::reap`].
    dying: bool,
}

/// The running sprites and the `milk2_img.ini` they launch from.
pub struct Sprites<H> {
    ini: Ini,
    base_dir: PathBuf,
    global: Rc<RefCell<GlobalMem>>,
    slots: [Option<Slot<H>>; NUM_SPRITE_SLOTS],
}

impl<H: Copy> Sprites<H> {
    /// `base_dir` is `m_szMilkdrop2Path`, which relative `img=` paths are
    /// appended to; `global` is the presets' `GlobalMem`.
    pub fn new(ini: Ini, base_dir: PathBuf, global: Rc<RefCell<GlobalMem>>) -> Self {
        Sprites {
            ini,
            base_dir,
            global,
            slots: std::array::from_fn(|_| None),
        }
    }

    /// [`new`](Self::new) from the file's text.
    pub fn parse(text: &str, base_dir: PathBuf, global: Rc<RefCell<GlobalMem>>) -> Self {
        Sprites::new(Ini::parse(text), base_dir, global)
    }

    /// The shared registers.
    pub fn global(&self) -> &Rc<RefCell<GlobalMem>> {
        &self.global
    }

    /// Sprites alive (a slot with a surface).
    pub fn len(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The `[imgNN]` number of the sprite in `slot`.
    pub fn slot_number(&self, slot: usize) -> Option<i32> {
        self.slots.get(slot)?.as_ref().map(|s| s.number)
    }

    /// `LaunchSprite(n, -1)` (plugin:8567-8713) at `now` and `frame`
    /// (`GetTime()`, `GetFrame()`).
    pub fn launch<T: SpriteTextures<Handle = H>>(
        &mut self,
        n: i32,
        textures: &mut T,
        now: f32,
        frame: i32,
    ) -> LaunchResult {
        let mut toasts = Vec::new();
        let fail = |toasts| LaunchResult { slot: None, toasts };
        let section = format!("img{n:02}");

        let img = self.ini.get(&section, "img").unwrap_or("");
        if img.is_empty() {
            toasts.push(toast(
                format!(
                    "sprite #{n} error: could not find 'img=' setting, or sprite is not defined"
                ),
                7.0,
                false,
            ));
            return fail(toasts);
        }
        // `img[1] != ':'`: not "x:\..." so the plugin directory is prepended.
        let path = sprite_path(&self.base_dir, img);
        let ck = colorkey(&self.ini, &section);
        let init = read_code(&self.ini, &section, "init_");
        let code = read_code(&self.ini, &section, "code_");

        // First empty slot, else the oldest start frame (plugin:8641-8665).
        let start_of = |s: &Option<Slot<H>>| s.as_ref().map_or(0, |s| s.start_frame);
        let (mut slot, mut oldest, mut oldest_frame) = (None, 0, start_of(&self.slots[0]));
        for x in 0..NUM_SPRITE_SLOTS {
            if self.slots[x].is_none() {
                slot = Some(x);
                break;
            } else if start_of(&self.slots[x]) < oldest_frame {
                oldest = x;
                oldest_frame = start_of(&self.slots[x]);
            }
        }
        let slot = slot.unwrap_or_else(|| {
            self.kill_slot(oldest, textures);
            oldest
        });

        let handle = match textures.load(&path, ck) {
            Ok(h) => h,
            Err(e) => {
                let msg = match e {
                    SpriteLoadError::BadFile => {
                        format!("sprite #{n} error: image file missing or corrupt")
                    }
                    SpriteLoadError::OutOfMemory => {
                        format!("sprite #{n} error: out of memory, unable to load image")
                    }
                };
                toasts.push(toast(msg, 6.0, true));
                return fail(toasts);
            }
        };

        // RunInitCode: variables, init program, output defaults, run it.
        let mut vm = Vm::new(self.global.clone());
        let refs = Refs::register(&mut vm);
        let init_prog = compile(&mut vm, &init);
        refs.set_defaults(&mut vm);
        let init_failed = init_prog.is_err();
        if let Ok(Some(p)) = &init_prog {
            p.run(&mut vm);
        }
        let program = compile(&mut vm, &code);
        let code_failed = program.is_err();
        let program = program.unwrap_or(None);
        // LoadTex ORs the two warning bits and LaunchSprite's switch matches
        // 0x10 or 0x20 only (plugin:8674-8686): both failing (0x30) shows
        // nothing.
        if init_failed != code_failed {
            let msg = if init_failed {
                format!("sprite #{n} warning: error in initialization code ")
            } else {
                format!("sprite #{n} warning: error in per-frame code")
            };
            toasts.push(toast(msg, 6.0, true));
        }
        self.slots[slot] = Some(Slot {
            number: n,
            handle,
            start_time: now,
            start_frame: frame,
            vm,
            refs,
            program,
            dying: false,
        });
        LaunchResult {
            slot: Some(slot),
            toasts,
        }
    }

    /// The per-frame part of `DrawUserSprites` (mfs:3473-3499, 3542-3556):
    /// set the inputs, run each sprite's code, and return the sprites in
    /// slot order with their outputs. A sprite whose `done` is non-zero is
    /// marked and freed by [`reap`](Self::reap).
    pub fn tick(&mut self, inp: &SpriteInputs) -> Vec<TickedSprite<H>> {
        let mut out = Vec::new();
        for (i, s) in self.slots.iter_mut().enumerate() {
            let Some(s) = s else { continue };
            let (vm, r) = (&mut s.vm, &s.refs);
            let sets = [
                (r.time, f64::from(inp.now - s.start_time)),
                (r.frame, f64::from(inp.frame - s.start_frame)),
                (r.fps, f64::from(inp.fps)),
                (r.progress, f64::from(inp.progress)),
                (r.bass, f64::from(inp.imm_rel[0])),
                (r.mid, f64::from(inp.imm_rel[1])),
                (r.treb, f64::from(inp.imm_rel[2])),
                (r.bass_att, f64::from(inp.avg_rel[0])),
                (r.mid_att, f64::from(inp.avg_rel[1])),
                (r.treb_att, f64::from(inp.avg_rel[2])),
            ];
            for (v, x) in sets {
                vm.set(v, x);
            }
            if let Some(p) = &s.program {
                p.run(vm);
            }
            let vars = r.outputs(vm);
            s.dying = vars.done != 0.0;
            out.push(TickedSprite {
                slot: i,
                number: s.number,
                vars,
                handle: s.handle,
            });
        }
        out
    }

    /// `KillSprite` for every sprite the last [`tick`](Self::tick) marked
    /// done: call it after the frame's sprites were drawn.
    pub fn reap<T: SpriteTextures<Handle = H>>(&mut self, textures: &mut T) {
        for i in 0..NUM_SPRITE_SLOTS {
            if self.slots[i].as_ref().is_some_and(|s| s.dying) {
                self.kill_slot(i, textures);
            }
        }
    }

    fn kill_slot<T: SpriteTextures<Handle = H>>(&mut self, i: usize, textures: &mut T) {
        if let Some(s) = self.slots[i].take() {
            textures.kill(s.handle);
        }
    }

    /// The sprite-kill numeric input: every slot of sprite `n`
    /// (plugin:6289-6293).
    pub fn kill_number<T: SpriteTextures<Handle = H>>(&mut self, n: i32, t: &mut T) {
        for i in 0..NUM_SPRITE_SLOTS {
            if self.slots[i].as_ref().is_some_and(|s| s.number == n) {
                self.kill_slot(i, t);
            }
        }
    }

    /// Delete in sprite mode: the newest sprite, the largest start frame
    /// (plugin:6076-6093).
    pub fn kill_newest<T: SpriteTextures<Handle = H>>(&mut self, t: &mut T) {
        self.kill_extreme(t, |a, b| a > b);
    }

    /// Shift+Delete: the oldest sprite.
    pub fn kill_oldest<T: SpriteTextures<Handle = H>>(&mut self, t: &mut T) {
        self.kill_extreme(t, |a, b| a < b);
    }

    fn kill_extreme<T: SpriteTextures<Handle = H>>(
        &mut self,
        t: &mut T,
        better: impl Fn(i32, i32) -> bool,
    ) {
        let mut pick: Option<(usize, i32)> = None;
        for (i, s) in self.slots.iter().enumerate() {
            if let Some(s) = s {
                if pick.is_none_or(|(_, f)| better(s.start_frame, f)) {
                    pick = Some((i, s.start_frame));
                }
            }
        }
        if let Some((i, _)) = pick {
            self.kill_slot(i, t);
        }
    }

    /// Ctrl+Shift+Delete and Ctrl+K: every sprite.
    pub fn kill_all<T: SpriteTextures<Handle = H>>(&mut self, t: &mut T) {
        for i in 0..NUM_SPRITE_SLOTS {
            self.kill_slot(i, t);
        }
    }
}

/// Compile stripped code: `Ok(None)` when empty, `Err` when it does not
/// compile.
fn compile(vm: &mut Vm, src: &str) -> Result<Option<Program>, ()> {
    if src.trim().is_empty() {
        return Ok(None);
    }
    Program::compile(vm, src).map(Some).map_err(|_| ())
}

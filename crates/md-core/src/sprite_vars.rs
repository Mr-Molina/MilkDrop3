//! The variables of a sprite's VM (texmgr.cpp:276-310) and their defaults.

use md_eval::{VarRef, Vm};
use md_render::SpriteVars;

/// The variables of `RegisterBuiltInVariables` (texmgr:276-310).
pub(crate) struct Refs {
    pub(crate) time: VarRef,
    pub(crate) frame: VarRef,
    pub(crate) fps: VarRef,
    pub(crate) progress: VarRef,
    pub(crate) bass: VarRef,
    pub(crate) bass_att: VarRef,
    pub(crate) mid: VarRef,
    pub(crate) mid_att: VarRef,
    pub(crate) treb: VarRef,
    pub(crate) treb_att: VarRef,
    pub(crate) x: VarRef,
    pub(crate) y: VarRef,
    pub(crate) sx: VarRef,
    pub(crate) sy: VarRef,
    pub(crate) repeatx: VarRef,
    pub(crate) repeaty: VarRef,
    pub(crate) rot: VarRef,
    pub(crate) flipx: VarRef,
    pub(crate) flipy: VarRef,
    pub(crate) r: VarRef,
    pub(crate) g: VarRef,
    pub(crate) b: VarRef,
    pub(crate) a: VarRef,
    pub(crate) blendmode: VarRef,
    pub(crate) done: VarRef,
    pub(crate) burn: VarRef,
}

impl Refs {
    pub(crate) fn register(vm: &mut Vm) -> Refs {
        let mut v = |n: &str| vm.var(n);
        Refs {
            time: v("time"),
            frame: v("frame"),
            fps: v("fps"),
            progress: v("progress"),
            bass: v("bass"),
            bass_att: v("bass_att"),
            mid: v("mid"),
            mid_att: v("mid_att"),
            treb: v("treb"),
            treb_att: v("treb_att"),
            x: v("x"),
            y: v("y"),
            sx: v("sx"),
            sy: v("sy"),
            repeatx: v("repeatx"),
            repeaty: v("repeaty"),
            rot: v("rot"),
            flipx: v("flipx"),
            flipy: v("flipy"),
            r: v("r"),
            g: v("g"),
            b: v("b"),
            a: v("a"),
            blendmode: v("blendmode"),
            done: v("done"),
            burn: v("burn"),
        }
    }

    /// The outputs' defaults (texmgr:216-231) as a [`SpriteVars`].
    pub(crate) fn set_defaults(&self, vm: &mut Vm) {
        let d = SpriteVars::default();
        for (r, x) in [
            (self.x, d.x),
            (self.y, d.y),
            (self.sx, d.sx),
            (self.sy, d.sy),
            (self.repeatx, d.repeatx),
            (self.repeaty, d.repeaty),
            (self.rot, d.rot),
            (self.flipx, d.flipx),
            (self.flipy, d.flipy),
            (self.r, d.r),
            (self.g, d.g),
            (self.b, d.b),
            (self.a, d.a),
            (self.blendmode, d.blendmode),
            (self.done, d.done),
            (self.burn, d.burn),
        ] {
            vm.set(r, x);
        }
    }

    pub(crate) fn outputs(&self, vm: &Vm) -> SpriteVars {
        SpriteVars {
            x: vm.get(self.x),
            y: vm.get(self.y),
            sx: vm.get(self.sx),
            sy: vm.get(self.sy),
            rot: vm.get(self.rot),
            repeatx: vm.get(self.repeatx),
            repeaty: vm.get(self.repeaty),
            flipx: vm.get(self.flipx),
            flipy: vm.get(self.flipy),
            blendmode: vm.get(self.blendmode),
            r: vm.get(self.r),
            g: vm.get(self.g),
            b: vm.get(self.b),
            a: vm.get(self.a),
            burn: vm.get(self.burn),
            done: vm.get(self.done),
        }
    }
}

//! The Task 1.8 per-pixel workload, shared by `benches/per_pixel.rs` and the
//! `tests/perf_guard.rs` regression guard.
//!
//! A representative per-pixel (warp mesh per-vertex) program run once per
//! vertex of the default 64 x 64 mesh, i.e. 65 x 65 = 4225 evaluations, with
//! the per-frame values restored before each vertex as MilkDrop does.

use md_eval::{GlobalMem, Program, VarRef, Vm};
use std::cell::RefCell;
use std::rc::Rc;

pub const SRC: &str =
    "zoom = zoom + 0.1*sin(rad*6 + time); rot = rot + 0.02*ang; dx = dx + 0.001*cos(ang*3)";

/// Mesh side in vertices (`64 x 64` grid -> 65 x 65 vertices).
const SIDE: usize = 65;

/// Evaluations per [`Workload::frame`].
pub const EVALS: usize = SIDE * SIDE;

pub struct Workload {
    vm: Vm,
    prog: Program,
    zoom: VarRef,
    rot: VarRef,
    dx: VarRef,
    rad: VarRef,
    ang: VarRef,
    verts: Vec<(f64, f64)>,
}

impl Workload {
    pub fn new() -> Workload {
        let mut vm = Vm::new(Rc::new(RefCell::new(GlobalMem::new())));
        let prog = Program::compile(&mut vm, SRC).expect("per-pixel program compiles");
        let [zoom, rot, dx, rad, ang, time] =
            ["zoom", "rot", "dx", "rad", "ang", "time"].map(|n| vm.var(n));
        vm.set(time, 1.25);
        // rad/ang per vertex the way the warp mesh computes them
        let verts = (0..EVALS)
            .map(|i| {
                let x = (i % SIDE) as f64 / (SIDE - 1) as f64 * 2.0 - 1.0;
                let y = (i / SIDE) as f64 / (SIDE - 1) as f64 * 2.0 - 1.0;
                ((x * x + y * y).sqrt() / 2f64.sqrt(), y.atan2(x))
            })
            .collect();
        Workload {
            vm,
            prog,
            zoom,
            rot,
            dx,
            rad,
            ang,
            verts,
        }
    }

    /// Run the program over every vertex; returns the sum of the results
    /// (the value of the last statement, `dx`) so the work is observable.
    pub fn frame(&mut self) -> f64 {
        let mut acc = 0.0;
        for &(r, a) in &self.verts {
            self.vm.set(self.zoom, 1.0);
            self.vm.set(self.rot, 0.0);
            self.vm.set(self.dx, 0.0);
            self.vm.set(self.rad, r);
            self.vm.set(self.ang, a);
            acc += self.prog.run(&mut self.vm);
        }
        acc
    }
}

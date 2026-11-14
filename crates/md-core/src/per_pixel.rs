//! The per-pixel (per-vertex) program as a `WarpMesh::compute_uvs` runner:
//! the `m_pp_codehandle` branch of `ComputeGridAlphaValues`
//! (code/vis_milk2/milkdropfs.cpp:1752-1791).
//!
//! The once-per-frame inputs and `q1..q32` were written into the per-pixel
//! `Vm` by [`crate::run_per_frame`] (milkdropfs.cpp:640-655, 673-674); per
//! vertex the runner writes `x y rad ang` and the ten motion values from
//! the [`PerPixelVars`] it is given (the C++ copies them from `var_pf_*`,
//! 1756-1769, and `compute_uvs` seeds `PerPixelVars` from the per-frame
//! vars it is passed), runs the code and reads the motion values back
//! (1782-1791). Variables are resolved once (`pv_refs`), so a call does no
//! allocation or name lookup. User variables persist from vertex to vertex
//! and frame to frame, as in C++.
//!
//! During a blend the old preset's runner is built from the old
//! `PresetState` the same way and passed with the old per-frame vars for
//! `rep` 1; both states are borrowed mutably, one runner at a time.

use crate::state::PresetState;
use md_render::PerPixelVars;

/// A runner over `state`'s per-pixel program. With no per-pixel code it
/// leaves every vertex unchanged (the C++ then keeps the per-frame
/// values); callers may skip it when `state.pv_program` is `None`.
pub fn per_pixel_runner(state: &mut PresetState) -> impl FnMut(&mut PerPixelVars) + '_ {
    let PresetState {
        pv,
        pv_refs: r,
        pv_program,
        ..
    } = state;
    move |v: &mut PerPixelVars| {
        let Some(prog) = pv_program.as_ref() else {
            return;
        };
        macro_rules! put {
            ($($f:ident),* $(,)?) => { $( pv.set(r.$f, v.$f); )* };
        }
        macro_rules! get {
            ($($f:ident),* $(,)?) => { $( v.$f = pv.get(r.$f); )* };
        }
        put!(x, y, rad, ang, zoom, zoomexp, rot, warp, cx, cy, dx, dy, sx, sy);
        prog.run(pv);
        get!(zoom, zoomexp, rot, warp, cx, cy, dx, dy, sx, sy);
    }
}

//! The variables each expression context registers, verbatim from
//! `CState::RegisterBuiltInVariables` (code/vis_milk2/state.cpp:262-514).
//!
//! One `Vm` per context, as the C++ has one `NSEEL_VM` per context:
//! per-frame (`m_pf_eel`, state.cpp:267-348), per-pixel (`m_pv_eel`,
//! 355-390), and per custom wave a per-frame and a per-point VM
//! (`m_wave[i].m_pf_eel` 398-424, `m_pp_eel` 427-457) and per custom shape
//! a per-frame VM (`m_shape[i].m_pf_eel`, 466-511). `q1..q32` are
//! registered in every context (`sprintf(buf, "q%d", vi+1)`), `t1..t8` in
//! the wave and shape ones (`NUM_Q_VAR` 32 and `NUM_T_VAR` 8, state.h:49-50).
//!
//! Names are case-insensitive and keep their first 16 characters
//! (md-eval's convention), so `blur1_edge_darken` is one variable
//! `blur1_edge_dark`.
//!
//! Departures from the task 7.1 brief's lists (the C++ wins): the shape
//! context has no `thickoutline` (the shape's outline flag is exposed as
//! `thick`, milkdropfs.cpp:2427); the per-pixel context has only the
//! motion outputs, the inputs, `x y rad ang` and `q` (no `monitor`, wave,
//! border, echo or blur variables); wave and shape per-frame contexts also
//! carry `t1..t8`, and wave per-frame has `samples` while per-point has
//! `sample`.

use md_eval::{VarRef, Vm};
use md_render::PerFrameVars;

fn indexed<const N: usize>(vm: &mut Vm, prefix: &str) -> [VarRef; N] {
    std::array::from_fn(|i| vm.var(&format!("{prefix}{}", i + 1)))
}

fn indexed_names(prefix: &str, n: usize) -> impl Iterator<Item = String> + '_ {
    (1..=n).map(move |i| format!("{prefix}{i}"))
}

/// Declares a refs struct (`field = "name"` per scalar, plus `q`, and `t`
/// when the third form is used), its `register` constructor and the
/// `*_names` list. Field order is the C++ registration order.
macro_rules! var_refs {
    ($(#[$m:meta])* $name:ident, $names:ident, [$($f:ident = $s:literal),* $(,)?] $(, $t:ident)?) => {
        $(#[$m])*
        #[derive(Clone, Debug)]
        pub struct $name {
            $(pub $f: VarRef,)*
            /// `q1..q32`.
            pub q: [VarRef; 32],
            $(
                /// `t1..t8`.
                pub $t: [VarRef; 8],
            )?
        }

        impl $name {
            /// Register every variable in `vm` (creating them at 0.0).
            pub fn register(vm: &mut Vm) -> Self {
                Self {
                    $($f: vm.var($s),)*
                    q: indexed(vm, "q"),
                    $($t: indexed(vm, "t"),)?
                }
            }
        }

        /// Every registered name, in order.
        pub fn $names() -> Vec<String> {
            let mut v: Vec<String> = vec![$($s.to_string()),*];
            v.extend(indexed_names("q", 32));
            $( let _ = stringify!($t); v.extend(indexed_names("t", 8)); )?
            v
        }
    };
}

var_refs!(
    /// The per-frame context (`var_pf_*`, state.cpp:267-348). Field names
    /// match [`PerFrameVars`].
    PerFrameRefs, per_frame_names,
    [
        zoom = "zoom", zoomexp = "zoomexp", rot = "rot", warp = "warp",
        cx = "cx", cy = "cy", dx = "dx", dy = "dy", sx = "sx", sy = "sy",
        time = "time", fps = "fps", bass = "bass", mid = "mid", treb = "treb",
        bass_att = "bass_att", mid_att = "mid_att", treb_att = "treb_att",
        frame = "frame", decay = "decay",
        wave_a = "wave_a", wave_r = "wave_r", wave_g = "wave_g", wave_b = "wave_b",
        wave_x = "wave_x", wave_y = "wave_y", wave_mystery = "wave_mystery",
        wave_mode = "wave_mode", progress = "progress",
        ob_size = "ob_size", ob_r = "ob_r", ob_g = "ob_g", ob_b = "ob_b", ob_a = "ob_a",
        ib_size = "ib_size", ib_r = "ib_r", ib_g = "ib_g", ib_b = "ib_b", ib_a = "ib_a",
        mv_x = "mv_x", mv_y = "mv_y", mv_dx = "mv_dx", mv_dy = "mv_dy", mv_l = "mv_l",
        mv_r = "mv_r", mv_g = "mv_g", mv_b = "mv_b", mv_a = "mv_a",
        monitor = "monitor", echo_zoom = "echo_zoom", echo_alpha = "echo_alpha",
        echo_orient = "echo_orient", wave_usedots = "wave_usedots",
        wave_thick = "wave_thick", wave_additive = "wave_additive",
        wave_brighten = "wave_brighten", darken_center = "darken_center",
        gamma = "gamma", wrap = "wrap", invert = "invert", brighten = "brighten",
        darken = "darken", solarize = "solarize",
        meshx = "meshx", meshy = "meshy", pixelsx = "pixelsx", pixelsy = "pixelsy",
        aspectx = "aspectx", aspecty = "aspecty",
        blur1_min = "blur1_min", blur2_min = "blur2_min", blur3_min = "blur3_min",
        blur1_max = "blur1_max", blur2_max = "blur2_max", blur3_max = "blur3_max",
        blur1_edge_darken = "blur1_edge_darken",
    ]
);

var_refs!(
    /// The per-pixel (per-vertex) context (`var_pv_*`, state.cpp:355-390).
    PerPixelRefs, per_pixel_names,
    [
        zoom = "zoom", zoomexp = "zoomexp", rot = "rot", warp = "warp",
        cx = "cx", cy = "cy", dx = "dx", dy = "dy", sx = "sx", sy = "sy",
        time = "time", fps = "fps", bass = "bass", mid = "mid", treb = "treb",
        bass_att = "bass_att", mid_att = "mid_att", treb_att = "treb_att",
        frame = "frame", x = "x", y = "y", rad = "rad", ang = "ang",
        progress = "progress",
        meshx = "meshx", meshy = "meshy", pixelsx = "pixelsx", pixelsy = "pixelsy",
        aspectx = "aspectx", aspecty = "aspecty",
    ]
);

var_refs!(
    /// A custom wave's per-frame context (state.cpp:398-424).
    WaveFrameRefs, wave_frame_names,
    [
        time = "time", fps = "fps", frame = "frame", progress = "progress",
        bass = "bass", mid = "mid", treb = "treb",
        bass_att = "bass_att", mid_att = "mid_att", treb_att = "treb_att",
        r = "r", g = "g", b = "b", a = "a", samples = "samples",
    ],
    t
);

var_refs!(
    /// A custom wave's per-point context (state.cpp:427-457).
    WavePointRefs, wave_point_names,
    [
        time = "time", fps = "fps", frame = "frame", progress = "progress",
        bass = "bass", mid = "mid", treb = "treb",
        bass_att = "bass_att", mid_att = "mid_att", treb_att = "treb_att",
        sample = "sample", value1 = "value1", value2 = "value2",
        x = "x", y = "y", r = "r", g = "g", b = "b", a = "a",
    ],
    t
);

var_refs!(
    /// A custom shape's per-frame context (state.cpp:466-511).
    ShapeFrameRefs, shape_frame_names,
    [
        time = "time", fps = "fps", frame = "frame", progress = "progress",
        bass = "bass", mid = "mid", treb = "treb",
        bass_att = "bass_att", mid_att = "mid_att", treb_att = "treb_att",
        x = "x", y = "y", rad = "rad", ang = "ang", tex_ang = "tex_ang",
        tex_zoom = "tex_zoom", sides = "sides", textured = "textured",
        instance = "instance", num_inst = "num_inst", additive = "additive",
        thick = "thick", r = "r", g = "g", b = "b", a = "a",
        r2 = "r2", g2 = "g2", b2 = "b2", a2 = "a2",
        border_r = "border_r", border_g = "border_g", border_b = "border_b",
        border_a = "border_a",
    ],
    t
);

impl PerFrameRefs {
    /// Write every per-frame variable of `v` into `vm`
    /// (`LoadPerFrameEvallibVars`, milkdropfs.cpp:460-554, which assigns
    /// all of them every frame, `q` and `monitor` from the init capture).
    pub fn load(&self, vm: &mut Vm, v: &PerFrameVars) {
        macro_rules! set {
            ($($f:ident),* $(,)?) => { $( vm.set(self.$f, v.$f); )* };
        }
        set!(
            time,
            fps,
            frame,
            progress,
            bass,
            mid,
            treb,
            bass_att,
            mid_att,
            treb_att,
            meshx,
            meshy,
            pixelsx,
            pixelsy,
            aspectx,
            aspecty,
            zoom,
            zoomexp,
            rot,
            warp,
            cx,
            cy,
            dx,
            dy,
            sx,
            sy,
            decay,
            wave_a,
            wave_r,
            wave_g,
            wave_b,
            wave_x,
            wave_y,
            wave_mystery,
            wave_mode,
            wave_usedots,
            wave_thick,
            wave_additive,
            wave_brighten,
            ob_size,
            ob_r,
            ob_g,
            ob_b,
            ob_a,
            ib_size,
            ib_r,
            ib_g,
            ib_b,
            ib_a,
            mv_x,
            mv_y,
            mv_dx,
            mv_dy,
            mv_l,
            mv_r,
            mv_g,
            mv_b,
            mv_a,
            echo_zoom,
            echo_alpha,
            echo_orient,
            darken_center,
            gamma,
            wrap,
            invert,
            brighten,
            darken,
            solarize,
            blur1_min,
            blur2_min,
            blur3_min,
            blur1_max,
            blur2_max,
            blur3_max,
            blur1_edge_darken,
            monitor,
        );
        for (r, x) in self.q.iter().zip(&v.q) {
            vm.set(*r, *x);
        }
    }

    /// Read every per-frame variable out of `vm` into `out` (the values
    /// the renderer reads after `RunPerFrameEquations`). The preset
    /// scalars of [`PerFrameVars`] that have no variable are left alone.
    pub fn store(&self, vm: &Vm, out: &mut PerFrameVars) {
        macro_rules! get {
            ($($f:ident),* $(,)?) => { $( out.$f = vm.get(self.$f); )* };
        }
        get!(
            time,
            fps,
            frame,
            progress,
            bass,
            mid,
            treb,
            bass_att,
            mid_att,
            treb_att,
            meshx,
            meshy,
            pixelsx,
            pixelsy,
            aspectx,
            aspecty,
            zoom,
            zoomexp,
            rot,
            warp,
            cx,
            cy,
            dx,
            dy,
            sx,
            sy,
            decay,
            wave_a,
            wave_r,
            wave_g,
            wave_b,
            wave_x,
            wave_y,
            wave_mystery,
            wave_mode,
            wave_usedots,
            wave_thick,
            wave_additive,
            wave_brighten,
            ob_size,
            ob_r,
            ob_g,
            ob_b,
            ob_a,
            ib_size,
            ib_r,
            ib_g,
            ib_b,
            ib_a,
            mv_x,
            mv_y,
            mv_dx,
            mv_dy,
            mv_l,
            mv_r,
            mv_g,
            mv_b,
            mv_a,
            echo_zoom,
            echo_alpha,
            echo_orient,
            darken_center,
            gamma,
            wrap,
            invert,
            brighten,
            darken,
            solarize,
            blur1_min,
            blur2_min,
            blur3_min,
            blur1_max,
            blur2_max,
            blur3_max,
            blur1_edge_darken,
            monitor,
        );
        for (r, x) in self.q.iter().zip(out.q.iter_mut()) {
            *x = vm.get(*r);
        }
    }
}

//! Defaults from `CState::Default()` (code/vis_milk2/state.cpp:515-702).
//!
//! Every value cites its line as `state.cpp:<line>@6f7cd5d` (the `code/`
//! snapshot commit, per the F61 ruling).
//!
//! Note: `Preset::defaults()` is the state before any file is read. Parsing a
//! file that lacks a key does NOT always yield these values -- see
//! [`crate::parse_milk`] for the `wave_r..wave_y` and `mv_a` fallbacks.

use crate::blend::Blendable;
use crate::preset::Preset;
use crate::shape::ShapeData;
use crate::transition::Transition;
use crate::wave::WaveData;

impl Default for WaveData {
    fn default() -> WaveData {
        WaveData {
            enabled: 0,               // state.cpp:595@6f7cd5d
            samples: 512,             // state.cpp:596@6f7cd5d
            sep: 0,                   // state.cpp:597@6f7cd5d
            scaling: 1.0,             // state.cpp:598@6f7cd5d
            smoothing: 0.5,           // state.cpp:599@6f7cd5d
            r: 1.0,                   // state.cpp:600@6f7cd5d
            g: 1.0,                   // state.cpp:601@6f7cd5d
            b: 1.0,                   // state.cpp:602@6f7cd5d
            a: 1.0,                   // state.cpp:603@6f7cd5d
            spectrum: 0,              // state.cpp:604@6f7cd5d
            use_dots: 0,              // state.cpp:605@6f7cd5d
            draw_thick: 0,            // state.cpp:606@6f7cd5d
            additive: 0,              // state.cpp:607@6f7cd5d
            init: String::new(),      // state.cpp:638@6f7cd5d
            per_frame: String::new(), // state.cpp:639@6f7cd5d
            per_point: String::new(), // state.cpp:640@6f7cd5d
        }
    }
}

impl Default for ShapeData {
    fn default() -> ShapeData {
        ShapeData {
            enabled: 0,               // state.cpp:611@6f7cd5d
            sides: 4,                 // state.cpp:612@6f7cd5d
            additive: 0,              // state.cpp:613@6f7cd5d
            thick_outline: 0,         // state.cpp:614@6f7cd5d
            textured: 0,              // state.cpp:615@6f7cd5d
            instances: 1,             // state.cpp:616@6f7cd5d
            tex_zoom: 1.0,            // state.cpp:617@6f7cd5d
            tex_ang: 0.0,             // state.cpp:618@6f7cd5d
            x: 0.5,                   // state.cpp:619@6f7cd5d
            y: 0.5,                   // state.cpp:620@6f7cd5d
            rad: 0.1,                 // state.cpp:621@6f7cd5d
            ang: 0.0,                 // state.cpp:622@6f7cd5d
            r: 1.0,                   // state.cpp:623@6f7cd5d
            g: 0.0,                   // state.cpp:624@6f7cd5d
            b: 0.0,                   // state.cpp:625@6f7cd5d
            a: 1.0,                   // state.cpp:626@6f7cd5d
            r2: 0.0,                  // state.cpp:627@6f7cd5d
            g2: 1.0,                  // state.cpp:628@6f7cd5d
            b2: 0.0,                  // state.cpp:629@6f7cd5d
            a2: 0.0,                  // state.cpp:630@6f7cd5d
            border_r: 1.0,            // state.cpp:631@6f7cd5d
            border_g: 1.0,            // state.cpp:632@6f7cd5d
            border_b: 1.0,            // state.cpp:633@6f7cd5d
            border_a: 0.1,            // state.cpp:634@6f7cd5d
            init: String::new(),      // state.cpp:644@6f7cd5d
            per_frame: String::new(), // state.cpp:645@6f7cd5d
        }
    }
}

impl Default for Preset {
    fn default() -> Preset {
        Preset::defaults()
    }
}

impl Preset {
    /// The state `CState::Default(STATE_ALL)` leaves behind.
    pub fn defaults() -> Preset {
        let b = Blendable::new;
        Preset {
            warp_ps: 0, // state.cpp:689@6f7cd5d
            comp_ps: 0, // state.cpp:696@6f7cd5d

            // general
            rating: 3.0,                // state.cpp:538@6f7cd5d
            decay: b(0.98),             // state.cpp:539@6f7cd5d
            gamma_adj: b(2.0),          // state.cpp:540@6f7cd5d
            video_echo_zoom: b(2.0),    // state.cpp:541@6f7cd5d
            video_echo_alpha: b(0.0),   // state.cpp:542@6f7cd5d
            video_echo_orientation: 0,  // state.cpp:543@6f7cd5d
            red_blue_stereo: false,     // state.cpp:544@6f7cd5d
            brighten: false,            // state.cpp:545@6f7cd5d
            darken: false,              // state.cpp:546@6f7cd5d
            tex_wrap: true,             // state.cpp:547@6f7cd5d
            darken_center: false,       // state.cpp:548@6f7cd5d
            solarize: false,            // state.cpp:549@6f7cd5d
            invert: false,              // state.cpp:550@6f7cd5d
            shader: b(0.0),             // state.cpp:551@6f7cd5d
            blur1_min: b(0.0),          // state.cpp:552@6f7cd5d
            blur2_min: b(0.0),          // state.cpp:553@6f7cd5d
            blur3_min: b(0.0),          // state.cpp:554@6f7cd5d
            blur1_max: b(1.0),          // state.cpp:555@6f7cd5d
            blur2_max: b(1.0),          // state.cpp:556@6f7cd5d
            blur3_max: b(1.0),          // state.cpp:557@6f7cd5d
            blur1_edge_darken: b(0.25), // state.cpp:558@6f7cd5d

            // wave
            wave_mode: 0,                    // state.cpp:565@6f7cd5d
            additive_waves: false,           // state.cpp:567@6f7cd5d
            wave_dots: false,                // state.cpp:568@6f7cd5d
            wave_thick: false,               // state.cpp:569@6f7cd5d
            wave_alpha: b(0.8),              // state.cpp:570@6f7cd5d
            wave_scale: b(1.0),              // state.cpp:571@6f7cd5d
            wave_smoothing: b(0.75),         // state.cpp:572@6f7cd5d
            wave_param: b(0.0),              // state.cpp:573@6f7cd5d
            mod_wave_alpha_by_volume: false, // state.cpp:574@6f7cd5d
            mod_wave_alpha_start: b(0.75),   // state.cpp:575@6f7cd5d
            mod_wave_alpha_end: b(0.95),     // state.cpp:576@6f7cd5d
            wave_r: b(1.0),                  // state.cpp:577@6f7cd5d
            wave_g: b(1.0),                  // state.cpp:578@6f7cd5d
            wave_b: b(1.0),                  // state.cpp:579@6f7cd5d
            wave_x: b(0.5),                  // state.cpp:580@6f7cd5d
            wave_y: b(0.5),                  // state.cpp:581@6f7cd5d
            maximize_wave_color: true,       // state.cpp:582@6f7cd5d
            mv_x: b(12.0),                   // state.cpp:583@6f7cd5d
            mv_y: b(9.0),                    // state.cpp:584@6f7cd5d
            mv_dx: b(0.0),                   // state.cpp:585@6f7cd5d
            mv_dy: b(0.0),                   // state.cpp:586@6f7cd5d
            mv_l: b(0.9),                    // state.cpp:587@6f7cd5d
            mv_r: b(1.0),                    // state.cpp:588@6f7cd5d
            mv_g: b(1.0),                    // state.cpp:589@6f7cd5d
            mv_b: b(1.0),                    // state.cpp:590@6f7cd5d
            mv_a: b(1.0),                    // state.cpp:591@6f7cd5d
            waves: Default::default(),       // state.cpp:593-607@6f7cd5d
            shapes: Default::default(),      // state.cpp:609-634@6f7cd5d

            // motion
            warp_anim_speed: 1.0,          // state.cpp:653@6f7cd5d
            warp_scale: b(1.0),            // state.cpp:654@6f7cd5d
            zoom_exponent: b(1.0),         // state.cpp:655@6f7cd5d
            zoom: b(1.0),                  // state.cpp:656@6f7cd5d
            rot: b(0.0),                   // state.cpp:657@6f7cd5d
            cx: b(0.5),                    // state.cpp:658@6f7cd5d
            cy: b(0.5),                    // state.cpp:659@6f7cd5d
            dx: b(0.0),                    // state.cpp:660@6f7cd5d
            dy: b(0.0),                    // state.cpp:661@6f7cd5d
            warp: b(1.0),                  // state.cpp:662@6f7cd5d
            sx: b(1.0),                    // state.cpp:663@6f7cd5d
            sy: b(1.0),                    // state.cpp:664@6f7cd5d
            ob_size: b(0.01),              // state.cpp:665@6f7cd5d
            ob_r: b(0.0),                  // state.cpp:666@6f7cd5d
            ob_g: b(0.0),                  // state.cpp:667@6f7cd5d
            ob_b: b(0.0),                  // state.cpp:668@6f7cd5d
            ob_a: b(0.0),                  // state.cpp:669@6f7cd5d
            ib_size: b(0.01),              // state.cpp:670@6f7cd5d
            ib_r: b(0.25),                 // state.cpp:671@6f7cd5d
            ib_g: b(0.25),                 // state.cpp:672@6f7cd5d
            ib_b: b(0.25),                 // state.cpp:673@6f7cd5d
            ib_a: b(0.0),                  // state.cpp:674@6f7cd5d
            per_frame_init: String::new(), // state.cpp:677@6f7cd5d
            per_frame: String::new(),      // state.cpp:678@6f7cd5d
            per_pixel: String::new(),      // state.cpp:679@6f7cd5d

            // shaders
            warp_shader: String::new(), // state.cpp:688@6f7cd5d
            comp_shader: String::new(), // state.cpp:695@6f7cd5d

            // m_bBlending = false (state.cpp:533@6f7cd5d), m_nOldWaveMode = -1
            // (state.cpp:566@6f7cd5d); the rest is not set by Default().
            transition: Transition::default(),
        }
    }
}

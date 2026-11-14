//! `ApplyShaderParams` (code/vis_milk2/milkdropfs.cpp:3819-3965): the
//! [`MdUniforms`] every preset warp and composite shader reads, shared by
//! the warp pass (Task 5.6) and the composite pass (Task 5.7).
//!
//! The C++ sets only the constants a shader references; here the whole
//! block is filled every time (unreferenced fields are ignored by the
//! translated WGSL). Textures and `texsize_*` are bound at load time
//! ([`crate::ShaderProgram`]), so they are not part of this filler.
//!
//! Inputs split as in the C++: [`FrameShaderInputs`] are plugin-global
//! (`GetTime`, `mysound`, `m_rand_frame`, window size, …);
//! [`PresetShaderInputs`] come from the `CState` being drawn (`pState`:
//! the old preset on blend pass 0), i.e. its q values, blur range,
//! rotation parameters, `m_rand_preset` and preset start time.

use crate::blendpattern::{frand, CPP_6_28};
use crate::frame_vars::PerFrameVars;
use crate::uniforms::MdUniforms;
use rand_core::Rng;

/// One preset's random rotation (`m_xlate[k]`, `m_rot_base[k]`,
/// `m_rot_speed[k]`, state.h:360-362), drawn at preset load by
/// [`randomize_rot_params`] (`CState::RandomizePresetVars`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RotParams {
    pub xlate: [f32; 3],
    pub base: [f32; 3],
    pub speed: [f32; 3],
}

/// A row-vector 4×4 matrix (D3DX convention: `v' = v * M`).
type Mat4 = [[f32; 4]; 4];

fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, o) in row.iter_mut().enumerate() {
            *o = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

/// `D3DXMatrixRotationX/Y/Z` and `D3DXMatrixTranslation` composed as the
/// C++ does: `temp = mx * mxlate * mz * my` (milkdropfs.cpp:3932-3941),
/// returned as the four HLSL rows of the `float4x3` constant (the first
/// three columns of each row).
fn rot_xlate(angles: [f32; 3], xlate: [f32; 3]) -> [[f32; 3]; 4] {
    let (sx, cx) = angles[0].sin_cos();
    let (sy, cy) = angles[1].sin_cos();
    let (sz, cz) = angles[2].sin_cos();
    let mx = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cx, sx, 0.0],
        [0.0, -sx, cx, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let my = [
        [cy, 0.0, -sy, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [sy, 0.0, cy, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mz = [
        [cz, sz, 0.0, 0.0],
        [-sz, cz, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut mt = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    mt[3] = [xlate[0], xlate[1], xlate[2], 1.0];
    let m = mul(&mul(&mul(&mx, &mt), &mz), &my);
    m.map(|r| [r[0], r[1], r[2]])
}

impl RotParams {
    /// `rot_*` matrix `k < 20` at shader time `time`
    /// (milkdropfs.cpp:3928-3945): rotation `base + speed * time`.
    pub fn matrix(&self, time: f32) -> [[f32; 3]; 4] {
        let a = [0, 1, 2].map(|i| self.base[i] + self.speed[i] * time);
        rot_xlate(a, self.xlate)
    }
}

/// `rot_rand1..4` (milkdropfs.cpp:3947-3964): fresh random angles
/// `FRAND * 6.28` and translation `FRAND` on every call.
pub fn random_rot_matrix<R: Rng + ?Sized>(rng: &mut R) -> [[f32; 3]; 4] {
    let a = [0; 3].map(|_| frand(rng) * CPP_6_28);
    let t = [0; 3].map(|_| frand(rng));
    rot_xlate(a, t)
}

/// `CState::RandomizePresetVars` (state.cpp:1875-1900): `rand_preset`
/// (four `FRAND`) and the 20 rotation parameters, `k = 0..20` with
/// `rot_mult = 0.9 * (k/8)^3.2`; entries 20..24 stay zero (`rot_rand*`
/// is random per draw, not per preset).
pub fn randomize_preset_vars<R: Rng + ?Sized>(rng: &mut R) -> ([f32; 4], [RotParams; 24]) {
    let rand_preset = [0; 4].map(|_| frand(rng));
    let mut rot = [RotParams::default(); 24];
    for (k, r) in rot.iter_mut().enumerate().take(20) {
        let rot_mult = 0.9 * (k as f32 / 8.0).powf(3.2);
        r.xlate = [0; 3].map(|_| frand(rng) * 2.0 - 1.0);
        r.base = [0; 3].map(|_| frand(rng) * CPP_6_28);
        r.speed = [0; 3].map(|_| (frand(rng) * 2.0 - 1.0) * rot_mult);
    }
    (rand_preset, rot)
}

/// `GetSafeBlurMinMax` (milkdropfs.cpp:1468-1499), including its quirk:
/// a range narrower than 0.1 collapses to `min = max = avg - 0.05`.
/// Returns `(blur_min, blur_max)` for levels 1..3.
pub fn safe_blur_min_max(v: &PerFrameVars) -> ([f32; 3], [f32; 3]) {
    let mut mn = [v.blur1_min, v.blur2_min, v.blur3_min].map(|x| x as f32);
    let mut mx = [v.blur1_max, v.blur2_max, v.blur3_max].map(|x| x as f32);
    const MIN_DIST: f32 = 0.1;
    for i in 0..3 {
        if i > 0 {
            mx[i] = mx[i - 1].min(mx[i]);
            mn[i] = mn[i - 1].max(mn[i]);
        }
        if mx[i] - mn[i] < MIN_DIST {
            let avg = (mn[i] + mx[i]) * 0.5;
            mn[i] = avg - MIN_DIST * 0.5;
            mx[i] = avg - MIN_DIST * 0.5;
        }
    }
    (mn, mx)
}

/// The plugin-global inputs of `ApplyShaderParams`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameShaderInputs {
    /// `GetTime() - m_fStartTime` (`_c8`..`_c11` and the rotations).
    pub time: f32,
    pub fps: f32,
    pub frame: u32,
    /// `(GetTime() - m_fPresetStartTime) / (m_fNextPresetTime - m_fPresetStartTime)`.
    pub progress: f32,
    /// `mysound.imm_rel` (`_c3`) and `mysound.avg_rel` (`_c4`).
    pub imm_rel: [f32; 3],
    pub avg_rel: [f32; 3],
    /// `m_rand_frame`.
    pub rand_frame: [f32; 4],
    /// `GetWidth()`, `GetHeight()`: the window, for `_c0` and `_c12`.
    pub window: (u32, u32),
}

/// The per-preset (`pState`) inputs of `ApplyShaderParams`.
#[derive(Clone, Copy, Debug)]
pub struct PresetShaderInputs<'a> {
    /// `var_pf_q*` and `var_pf_blur*` after the per-frame code.
    pub vars: &'a PerFrameVars,
    /// Entries `0..20` are used; `rot_rand1..4` are drawn per call.
    pub rot: &'a [RotParams; 24],
    /// `pState->m_rand_preset`.
    pub rand_preset: [f32; 4],
    /// `GetTime() - pState->GetPresetStartTime()`.
    pub time_since_preset_start: f32,
}

/// Fill [`MdUniforms`] as `ApplyShaderParams` sets its constants, for
/// main textures of `tex` = `(m_nTexSizeX, m_nTexSizeY)`. `rng` draws the
/// four `rot_rand*` matrices.
pub fn shader_uniforms<R: Rng + ?Sized>(
    frame: &FrameShaderInputs,
    preset: &PresetShaderInputs,
    tex: (u32, u32),
    rng: &mut R,
) -> MdUniforms {
    let mut u = MdUniforms::default();
    let tsp = preset.time_since_preset_start;
    let tsp_wrapped = tsp - ((tsp / 10000.0) as i32 * 10000) as f32;
    let time = frame.time;
    let (ww, wh) = (frame.window.0.max(1) as f32, frame.window.1.max(1) as f32);
    // The C++ takes log2 of the width for both (milkdropfs.cpp:3867-3868).
    let mip_x = ww.ln() / 2.0f32.ln();
    let mip_y = ww.ln() / 2.0f32.ln();
    let mip_avg = 0.5 * (mip_x + mip_y);
    let (mut aspect_x, mut aspect_y) = (1.0f32, 1.0f32);
    if ww > wh {
        aspect_y = wh / ww;
    } else {
        aspect_x = ww / wh;
    }
    let (bmin, bmax) = safe_blur_min_max(preset.vars);
    let (tw, th) = (tex.0.max(1) as f32, tex.1.max(1) as f32);
    let [ib, im, it] = frame.imm_rel;
    let [ab, am, at] = frame.avg_rel;
    let wave = |f: fn(f32) -> f32, k: [f32; 4], p: [f32; 4]| {
        [0, 1, 2, 3].map(|i| 0.5 + 0.5 * f(time * k[i] + p[i]))
    };
    let fast = ([0.329, 1.293, 5.070, 20.051], [1.2, 3.9, 2.5, 5.4]);
    let slow = ([0.0050, 0.0085, 0.0133, 0.0217], [2.7, 5.3, 4.5, 3.8]);
    u.c = [
        [aspect_x, aspect_y, 1.0 / aspect_x, 1.0 / aspect_y],
        [0.0; 4],
        [tsp_wrapped, frame.fps, frame.frame as f32, frame.progress],
        // `0.3333f*(a, b, c)` is the comma operator: a third of treb.
        [ib, im, it, 0.3333 * it],
        [ab, am, at, 0.3333 * at],
        [bmax[0] - bmin[0], bmin[0], bmax[1] - bmin[1], bmin[1]],
        [bmax[2] - bmin[2], bmin[2], bmin[0], bmax[0]],
        [tw, th, 1.0 / tw, 1.0 / th],
        wave(f32::cos, fast.0, fast.1),
        wave(f32::sin, fast.0, fast.1),
        wave(f32::cos, slow.0, slow.1),
        wave(f32::sin, slow.0, slow.1),
        [mip_x, mip_y, mip_avg, 0.0],
        [bmin[1], bmax[1], bmin[2], bmax[2]],
    ];
    for (i, q4) in u.q.iter_mut().enumerate() {
        *q4 = [0, 1, 2, 3].map(|j| preset.vars.q[i * 4 + j] as f32);
    }
    u.rand_frame = frame.rand_frame;
    u.rand_preset = preset.rand_preset;
    for k in 0..20 {
        u.rot[k] = preset.rot[k].matrix(time);
    }
    for k in 20..24 {
        u.rot[k] = random_rot_matrix(rng);
    }
    u
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::SeedableRng;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn rotation_composes_x_xlate_z_y() {
        // Pure translation: identity rows plus the translation row.
        let r = RotParams {
            xlate: [0.1, 0.2, 0.3],
            ..Default::default()
        };
        assert_eq!(
            r.matrix(5.0),
            [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.1, 0.2, 0.3]
            ]
        );
        // Z by 90°: row 0 = (cos, sin, 0) = (0, 1, 0); the translation row
        // is rotated by Z then Y: (1, 0, 0) -> (0, 1, 0).
        let r = RotParams {
            xlate: [1.0, 0.0, 0.0],
            base: [0.0, 0.0, std::f32::consts::FRAC_PI_2],
            speed: [0.0; 3],
        };
        let m = r.matrix(0.0);
        assert!(
            close(m[0], [0.0, 1.0, 0.0]) && close(m[3], [0.0, 1.0, 0.0]),
            "{m:?}"
        );
        // speed * time adds to base: Y by 90° at t = 1.
        let r = RotParams {
            speed: [0.0, std::f32::consts::FRAC_PI_2, 0.0],
            ..Default::default()
        };
        let m = r.matrix(1.0);
        assert!(
            close(m[0], [0.0, 0.0, -1.0]) && close(m[2], [1.0, 0.0, 0.0]),
            "{m:?}"
        );
    }

    #[test]
    fn blur_min_max_keeps_levels_nested_and_has_the_cpp_quirk() {
        let mut v = PerFrameVars::default();
        (v.blur1_min, v.blur1_max) = (0.1, 0.9);
        (v.blur2_min, v.blur2_max) = (0.0, 1.0);
        (v.blur3_min, v.blur3_max) = (0.5, 0.55);
        let (mn, mx) = safe_blur_min_max(&v);
        assert_eq!((mn[1], mx[1]), (0.1, 0.9)); // clamped inside level 1
                                                // Level 3: 0.05 apart -> both avg - 0.05 (the C++ writes `avg - d/2` twice).
        assert!((mn[2] - 0.475).abs() < 1e-6 && mn[2] == mx[2]);
    }

    #[test]
    fn uniforms_follow_apply_shader_params() {
        let mut v = PerFrameVars::default();
        v.q[0] = 1.5;
        v.q[31] = -2.0;
        let rot = [RotParams::default(); 24];
        let frame = FrameShaderInputs {
            time: 0.0,
            fps: 60.0,
            frame: 7,
            progress: 0.25,
            imm_rel: [1.0, 2.0, 3.0],
            avg_rel: [0.5, 0.25, 0.75],
            rand_frame: [0.1, 0.2, 0.3, 0.4],
            window: (800, 400),
        };
        let preset = PresetShaderInputs {
            vars: &v,
            rot: &rot,
            rand_preset: [0.9; 4],
            time_since_preset_start: 10003.5,
        };
        let mut rng = rand_pcg::Pcg32::seed_from_u64(1);
        let u = shader_uniforms(&frame, &preset, (640, 480), &mut rng);
        assert_eq!(u.c[0], [1.0, 0.5, 1.0, 2.0]);
        assert_eq!(u.c[2], [3.5, 60.0, 7.0, 0.25]);
        assert_eq!(u.c[3], [1.0, 2.0, 3.0, 0.3333 * 3.0]);
        assert_eq!(u.c[4][3], 0.3333 * 0.75);
        assert_eq!(u.c[7], [640.0, 480.0, 1.0 / 640.0, 1.0 / 480.0]);
        assert!((u.c[8][0] - (0.5 + 0.5 * 1.2f32.cos())).abs() < 1e-6);
        assert!((u.c[11][3] - (0.5 + 0.5 * 3.8f32.sin())).abs() < 1e-6);
        let l = 800f32.log2();
        assert!((u.c[12][0] - l).abs() < 1e-5 && (u.c[12][1] - l).abs() < 1e-5);
        assert_eq!(u.q[0][0], 1.5);
        assert_eq!(u.q[7][3], -2.0);
        assert_eq!(
            (u.rand_frame, u.rand_preset),
            ([0.1, 0.2, 0.3, 0.4], [0.9; 4])
        );
        // rot_rand rows are random rotations: unit-length rotation rows.
        let r = u.rot[20];
        let len: f32 = r[0].iter().map(|x| x * x).sum();
        assert!((len - 1.0).abs() < 1e-4);
    }

    #[test]
    fn preset_vars_follow_randomize_preset_vars() {
        let mut rng = rand_pcg::Pcg32::seed_from_u64(4);
        let (rp, rot) = randomize_preset_vars(&mut rng);
        assert!(rp.iter().all(|x| (0.0..=1.0).contains(x)));
        // k = 0: rot_mult = 0, so no spin.
        assert_eq!(rot[0].speed, [0.0; 3]);
        assert!(rot[19].speed.iter().any(|&s| s != 0.0));
        assert_eq!(rot[20], RotParams::default());
        assert!(rot[..20]
            .iter()
            .all(|r| r.xlate.iter().all(|x| (-1.0..=1.0).contains(x))));
    }
}

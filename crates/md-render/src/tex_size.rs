//! Main and blur texture sizes (`AllocateMyDX9Stuff`,
//! code/vis_milk2/plugin.cpp:1844-2007@6f7cd5d) and their allocation.
//! Every size is clamped to `1..=max_texture_dimension_2d` (F56).

use crate::gpu::Gpu;
use crate::texture::Texture2d;

/// How the main texture size follows the window (`m_nTexSizeX/Y`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexSize {
    /// `-1` ("auto", the C++ default): the window size.
    Auto,
    /// `-2`: the nearest power of two (`GetNearestPow2Size`), square.
    AutoPow2,
    /// `tex_w` × `tex_h` exactly, whatever the window (md-render's mode
    /// for tests and fixed-size headless renders; not snapped to 16).
    Fixed,
}

/// Sizes of the six blur textures for a `w`×`h` main texture, exactly as
/// `AllocateMyDX9Stuff` (plugin.cpp:1987-2007@6f7cd5d): levels 0, 1 halve, then
/// every even level halves again (each at least 16), widths rounded to
/// `((w + 3) / 16) * 16`, heights to `((h + 3) / 4) * 4`. Textures 1, 3, 5
/// are what presets see as `blur1..3` (¼, ⅛, 1/16 of the main texture —
/// the Milestone 5 text's "½, ¼, ⅛" names textures 0, 2, 4).
pub fn blur_texture_sizes(w: u32, h: u32) -> [(u32, u32); 6] {
    let (mut w, mut h) = (w, h);
    let mut out = [(0, 0); 6];
    for (i, o) in out.iter_mut().enumerate() {
        if i % 2 == 0 || i < 2 {
            w = (w / 2).max(16);
            h = (h / 2).max(16);
        }
        *o = (((w + 3) / 16) * 16, h.div_ceil(4) * 4);
    }
    out
}

/// `GetNearestPow2Size` (plugin.cpp:1509-1518).
fn nearest_pow2_size(w: u32, h: u32) -> u32 {
    let (w, h) = (w as f32, h as f32);
    let exp = (w.max(h) * 0.75 + 0.25 * w.min(h)).ln() / 2.0f32.ln();
    let bias = if exp + 0.55 >= 11.0 { 0.5 } else { 0.55 };
    2.0f32.powf((exp + bias) as i32 as f32) as u32
}

/// The main texture size for a `window` (`AllocateMyDX9Stuff`,
/// plugin.cpp:1844-1889): the window ([`TexSize::Auto`]) or its nearest
/// power of two ([`TexSize::AutoPow2`]), clipped to `max`, scaled by
/// `100 / canvas_stretch`, the power of two recomputed after stretching,
/// then rounded up to a multiple of 16 — and finally clamped to
/// `1..=max` (F56). [`TexSize::Fixed`] returns `fixed` clamped.
pub fn main_texture_size(
    window: (u32, u32),
    mode: TexSize,
    fixed: (u32, u32),
    canvas_stretch: u32,
    max: u32,
) -> (u32, u32) {
    let max = max.max(1);
    let clamp = |(w, h): (u32, u32)| (w.clamp(1, max), h.clamp(1, max));
    let (w, h) = match mode {
        TexSize::Fixed => return clamp(fixed),
        TexSize::Auto => clamp(window),
        TexSize::AutoPow2 => {
            let p = nearest_pow2_size(window.0.max(1), window.1.max(1));
            clamp((p, p))
        }
    };
    let stretch = u64::from(if canvas_stretch == 0 {
        100
    } else {
        canvas_stretch
    });
    let scale = |x: u32| (u64::from(x) * 100 / stretch).clamp(1, u64::from(max)) as u32;
    let (mut w, mut h) = (scale(w), scale(h));
    if mode == TexSize::AutoPow2 {
        let p = nearest_pow2_size(w, h);
        (w, h) = (p, p);
    }
    let snap = |x: u32| (u64::from(x).div_ceil(16) * 16) as u32;
    clamp((snap(w), snap(h)))
}

/// The ping-pong and blur textures for a `w`×`h` main texture. All are
/// single-level, as in the C++ (plugin.cpp:1904, 1989: `AUTOGENMIPMAP`
/// commented out), so sampling main always reads level 0.
pub(crate) fn main_textures(
    gpu: &Gpu,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
) -> ([Texture2d; 2], Vec<Texture2d>) {
    let max = gpu.device().limits().max_texture_dimension_2d.max(1);
    let vs = [0, 1].map(|_| Texture2d::new_target(gpu, w, h, format, false));
    let blur = blur_texture_sizes(w, h)
        .iter()
        .map(|&(bw, bh)| Texture2d::new_target(gpu, bw.min(max), bh.min(max), format, false))
        .collect();
    (vs, blur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_sizes_follow_allocate_my_dx9_stuff() {
        // The C++ comment's example: main 1024 -> 512, 256, 128, 128, 64, 64.
        let s = blur_texture_sizes(1024, 1024);
        assert_eq!(s.map(|(w, _)| w), [512, 256, 128, 128, 64, 64]);
        assert_eq!(s.map(|(_, h)| h), [512, 256, 128, 128, 64, 64]);
        // Rounding and the 16 minimum.
        let s = blur_texture_sizes(100, 30);
        assert_eq!(s[0], (48, 16)); // w 50 -> 48, h max(16, 15) = 16
        assert_eq!(s[5], (16, 16));
    }

    #[test]
    fn main_texture_size_follows_allocate_my_dx9_stuff() {
        use TexSize::{Auto, AutoPow2, Fixed};
        let m = 8192;
        // Auto: the window snapped up to 16.
        assert_eq!(
            main_texture_size((100, 30), Auto, (0, 0), 100, m),
            (112, 32)
        );
        assert_eq!(main_texture_size((0, 0), Auto, (0, 0), 100, m), (16, 16));
        // Canvas stretch 200 halves it before the snap; 0 means 100.
        assert_eq!(
            main_texture_size((1000, 600), Auto, (0, 0), 200, m),
            (512, 304)
        );
        assert_eq!(main_texture_size((96, 64), Auto, (0, 0), 0, m), (96, 64));
        // Pow2: log2(800*0.75 + 600*0.25) = 9.55, +0.55 -> 2^10.
        assert_eq!(nearest_pow2_size(800, 600), 1024);
        assert_eq!(
            main_texture_size((800, 600), AutoPow2, (0, 0), 100, m),
            (1024, 1024)
        );
        // Above 2^11 the bias drops to 0.5: log2(1700) = 10.73 -> 2^11.
        assert_eq!(nearest_pow2_size(1700, 1700), 2048);
        // Fixed: exact, only clamped.
        assert_eq!(
            main_texture_size((5, 5), Fixed, (100, 30), 100, m),
            (100, 30)
        );
        assert_eq!(main_texture_size((5, 5), Fixed, (0, 99999), 100, m), (1, m));
        // Everything clamps to the device maximum (F56).
        assert_eq!(
            main_texture_size((u32::MAX, 10), Auto, (0, 0), 100, m),
            (m, 16)
        );
        assert_eq!(
            main_texture_size((9000, 9000), AutoPow2, (0, 0), 1, m),
            (m, m)
        );
    }
}

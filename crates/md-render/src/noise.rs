//! The generated noise textures shaders sample as `sampler_noise_lq`,
//! `sampler_noisevol_hq`, ... — a port of `CPlugin::AddNoiseTex` and
//! `CPlugin::AddNoiseVol` (code/vis_milk2/plugin.cpp:2427-2715).
//!
//! Generation, per texel: four values `rand() % RANGE + RANGE/2` with
//! `RANGE = 216` when `zoom > 1`, else 256, packed into a D3D
//! `A8R8G8B8` DWORD with `(v0 << 24) | (v1 << 16) | (v2 << 8) | v3`
//! (plugin.cpp:2482-2485). The values reach 383 (or 323), so they do NOT
//! fit a byte and the C++ does not clamp: each channel byte is `v & 0xFF`
//! with bit 8 of the value below it ORed into its bit 0 (B carries into G,
//! G into R, R into A; bit 8 of A falls off the DWORD) — preflight F33.
//! [`pack_texel`] reproduces that with plain `u32` shifts. Each row is then
//! shuffled by `size` random swaps (plugin.cpp:2488-2495).
//!
//! Smoothing for `zoom > 1` (plugin.cpp:2499-2540, 2620-2690): the texels on
//! the lattice `x % zoom == 0` (and `y`, `z`) keep their random values;
//! first every lattice row is filled across X by cubic interpolation of
//! the four surrounding lattice texels (wrapping), then every column along Y
//! (on lattice slices for volumes), then — volumes only — every texel along
//! Z. [`dw_cubic_interpolate`] interpolates each byte of the DWORD
//! separately, clamps to `[0, 1]` and truncates `f * 255`.
//!
//! Randomness: the C++ reseeds `rand()` from the performance counter for
//! every row; per A.4 we do not reproduce its stream — one
//! [`rand_pcg::Pcg32`] seeded from the store seed drives every texture in
//! the order of [`NOISE_2D`] then [`NOISE_3D`]. [`crand`] keeps `rand()`'s
//! 15-bit range so `% RANGE` has the same (slight) bias.

use rand_core::Rng;

/// One generated noise texture: name, edge length and zoom factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoiseSpec {
    pub name: &'static str,
    pub size: u32,
    pub zoom: u32,
}

/// The 2D noise textures, in creation order (plugin.cpp:2369-2372).
pub const NOISE_2D: [NoiseSpec; 4] = [
    NoiseSpec {
        name: "noise_lq",
        size: 256,
        zoom: 1,
    },
    NoiseSpec {
        name: "noise_lq_lite",
        size: 32,
        zoom: 1,
    },
    NoiseSpec {
        name: "noise_mq",
        size: 256,
        zoom: 4,
    },
    NoiseSpec {
        name: "noise_hq",
        size: 256,
        zoom: 8,
    },
];

/// The noise volumes, in creation order (plugin.cpp:2374-2375).
pub const NOISE_3D: [NoiseSpec; 2] = [
    NoiseSpec {
        name: "noisevol_lq",
        size: 32,
        zoom: 1,
    },
    NoiseSpec {
        name: "noisevol_hq",
        size: 32,
        zoom: 4,
    },
];

/// Whether the 2D noise textures get a full mip chain: yes — AddNoiseTex
/// creates them with `Levels = 0` and `D3DUSAGE_AUTOGENMIPMAP`.
pub const NOISE_2D_MIPS: bool = true;

/// Whether the noise volumes get mips: no — AddNoiseVol creates them with
/// `Levels = 1` ("DX9 doesn't do auto mipgen w/volume textures",
/// plugin.cpp:2573-2575), so shaders always sample level 0.
pub const NOISE_3D_MIPS: bool = false;

/// `rand()` stand-in: 15 bits (`0..=32767`, `RAND_MAX` on MSVC).
pub fn crand<R: Rng + ?Sized>(rng: &mut R) -> u32 {
    rng.next_u32() >> 17
}

/// `RANGE` of AddNoiseTex/AddNoiseVol: 216 when smoothed, else 256.
pub fn noise_range(zoom: u32) -> u32 {
    if zoom > 1 {
        216
    } else {
        256
    }
}

/// Pack four channel values exactly as plugin.cpp:2482-2485 does
/// (`v[0]` is the `<< 24` alpha term, `v[3]` the unshifted blue term);
/// values above 255 carry bit 8 into the next channel's bit 0.
pub fn pack_texel(v: [u32; 4]) -> u32 {
    (v[0] << 24) | (v[1] << 16) | (v[2] << 8) | v[3]
}

/// A D3D `A8R8G8B8` DWORD as `Rgba8Unorm` bytes.
pub fn dword_to_rgba(d: u32) -> [u8; 4] {
    let [b, g, r, a] = d.to_le_bytes();
    [r, g, b, a]
}

/// `fCubicInterpolate` (plugin.cpp:2389).
fn f_cubic_interpolate(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let a0 = y3 - y2 - y0 + y1;
    let a1 = y0 - y1 - a0;
    let a2 = y2 - y0;
    let a3 = y1;
    a0 * t * t2 + a1 * t2 + a2 * t + a3
}

/// `dwCubicInterpolate` (plugin.cpp:2402): per byte, clamped, truncated.
pub fn dw_cubic_interpolate(y0: u32, y1: u32, y2: u32, y3: u32, t: f32) -> u32 {
    let mut ret = 0;
    for shift in [0, 8, 16, 24] {
        let ch = |y: u32| ((y >> shift) & 0xFF) as f32 / 255.0;
        let f = f_cubic_interpolate(ch(y0), ch(y1), ch(y2), ch(y3), t).clamp(0.0, 1.0);
        ret |= ((f * 255.0) as u32) << shift;
    }
    ret
}

/// One row of random texels followed by `row.len()` random swaps.
fn fill_row<R: Rng + ?Sized>(rng: &mut R, row: &mut [u32], range: u32) {
    for px in row.iter_mut() {
        let mut v = [0; 4];
        for c in &mut v {
            *c = crand(rng) % range + range / 2;
        }
        *px = pack_texel(v);
    }
    let n = row.len() as u32;
    for _ in 0..n {
        let x1 = (crand(rng) % n) as usize;
        let x2 = (crand(rng) % n) as usize;
        row.swap(x1, x2);
    }
}

/// Interpolate texel `i` along one axis: `p` is its coordinate on that
/// axis, `at(q)` the flat index of coordinate `q` on the same line.
fn cubic_along(d: &[u32], s: usize, z: usize, p: usize, at: impl Fn(usize) -> usize) -> u32 {
    let base = (p / z) * z + s;
    let t = (p % z) as f32 / z as f32;
    dw_cubic_interpolate(
        d[at((base - z) % s)],
        d[at(base % s)],
        d[at((base + z) % s)],
        d[at((base + 2 * z) % s)],
        t,
    )
}

/// The `size`² DWORDs of AddNoiseTex (row-major, top row first).
pub fn noise_2d<R: Rng + ?Sized>(rng: &mut R, size: u32, zoom: u32) -> Vec<u32> {
    let (s, z) = (size as usize, zoom.max(1) as usize);
    let range = noise_range(zoom);
    let mut d = vec![0u32; s * s];
    for row in d.chunks_mut(s) {
        fill_row(rng, row, range);
    }
    if z > 1 {
        // Across X, on the lattice rows only.
        for y in (0..s).step_by(z) {
            for x in (0..s).filter(|x| x % z != 0) {
                d[y * s + x] = cubic_along(&d, s, z, x, |q| y * s + q);
            }
        }
        // Down Y, on every column.
        for x in 0..s {
            for y in (0..s).filter(|y| y % z != 0) {
                d[y * s + x] = cubic_along(&d, s, z, y, |q| q * s + x);
            }
        }
    }
    d
}

/// The `size`³ DWORDs of AddNoiseVol (x fastest, then y, then z).
pub fn noise_3d<R: Rng + ?Sized>(rng: &mut R, size: u32, zoom: u32) -> Vec<u32> {
    let (s, z) = (size as usize, zoom.max(1) as usize);
    let range = noise_range(zoom);
    let mut d = vec![0u32; s * s * s];
    for row in d.chunks_mut(s) {
        fill_row(rng, row, range);
    }
    if z > 1 {
        let idx = |x: usize, y: usize, zz: usize| (zz * s + y) * s + x;
        // Across X, on lattice rows of lattice slices.
        for zz in (0..s).step_by(z) {
            for y in (0..s).step_by(z) {
                for x in (0..s).filter(|x| x % z != 0) {
                    d[idx(x, y, zz)] = cubic_along(&d, s, z, x, |q| idx(q, y, zz));
                }
            }
        }
        // Down Y, on the lattice slices.
        for zz in (0..s).step_by(z) {
            for x in 0..s {
                for y in (0..s).filter(|y| y % z != 0) {
                    d[idx(x, y, zz)] = cubic_along(&d, s, z, y, |q| idx(x, q, zz));
                }
            }
        }
        // Through Z, everywhere.
        for x in 0..s {
            for y in 0..s {
                for zz in (0..s).filter(|zz| zz % z != 0) {
                    d[idx(x, y, zz)] = cubic_along(&d, s, z, zz, |q| idx(x, y, q));
                }
            }
        }
    }
    d
}

/// DWORDs to tightly packed `Rgba8Unorm` bytes.
pub fn dwords_to_rgba8(d: &[u32]) -> Vec<u8> {
    d.iter().flat_map(|&x| dword_to_rgba(x)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::SeedableRng;

    fn rng(seed: u64) -> rand_pcg::Pcg32 {
        rand_pcg::Pcg32::seed_from_u64(seed)
    }

    /// Preflight F33: the C++ packs values up to 383 (zoom 1) or 323
    /// (zoom > 1) without clamping; each channel byte is `v & 0xFF` with
    /// bit 8 of the next-lower channel ORed into bit 0.
    #[test]
    fn noise_values_carry_bit8_into_next_channel_for_zoom() {
        // v = [A, R, G, B] terms = [200, 256, 300, 383]:
        // B = 0x7F; G = 0x2C | 1 (383 > 255); R = 0x00 | 1 (300 > 255);
        // A = 0xC8 | 1 (256 > 255); A's own bit 8 would fall off.
        assert_eq!(pack_texel([200, 256, 300, 383]), 0xC901_2D7F);
        assert_eq!(dword_to_rgba(0xC901_2D7F), [0x01, 0x2D, 0x7F, 0xC9]);
        assert_eq!(pack_texel([383, 128, 128, 128]), 0x7F80_8080);
        assert_eq!(
            (noise_range(1), noise_range(4), noise_range(8)),
            (256, 216, 216)
        );

        // zoom > 1: lattice texels are never rewritten by smoothing, and
        // their bytes come from v in [108, 324): (v & 0xFF) | carry lies
        // in [0, 67] or [108, 255] — and values >= 256 do occur.
        let s = 256;
        let d = noise_2d(&mut rng(1), s, 4);
        let mut wrapped = 0;
        for y in (0..s as usize).step_by(4) {
            for x in (0..s as usize).step_by(4) {
                for b in dword_to_rgba(d[y * s as usize + x]) {
                    assert!(b <= 67 || b >= 108, "lattice byte {b}");
                    wrapped += usize::from(b <= 67);
                }
            }
        }
        assert!(wrapped > 1000, "only {wrapped} wrapped bytes: clamped?");

        // zoom 1: B (no carry in) is odd half the time; G, R, A get a carry
        // half the time, so they are odd 3/4 of the time.
        let d = noise_2d(&mut rng(2), s, 1);
        let n = d.len() as f64;
        let odd = |c: usize| d.iter().filter(|&&x| dword_to_rgba(x)[c] & 1 == 1).count() as f64 / n;
        let (r, g, b, a) = (odd(0), odd(1), odd(2), odd(3));
        assert!((b - 0.5).abs() < 0.02, "B odd fraction {b}");
        for (name, f) in [("R", r), ("G", g), ("A", a)] {
            assert!((f - 0.75).abs() < 0.02, "{name} odd fraction {f}");
        }
    }

    #[test]
    fn cubic_interpolates_each_byte_and_clamps() {
        // t = 0 returns y1 (truncation of y1/255*255 may lose 1).
        let y1 = 0x40_80_C0_FF;
        let r = dw_cubic_interpolate(0, y1, 0, 0, 0.0).to_le_bytes();
        for (got, want) in r.iter().zip(y1.to_le_bytes()) {
            assert!(want - got <= 1, "{got} vs {want}");
        }
        // Overshoot clamps to 255 / 0 per byte: y0=0, y1=y2=255, y3=0 at
        // t=0.5 is 255*1.125 -> 255; the inverse undershoots -> 0.
        let hi = dw_cubic_interpolate(0, 0xFF, 0xFF, 0, 0.5);
        assert_eq!(hi & 0xFF, 0xFF);
        let lo = dw_cubic_interpolate(0xFF, 0, 0, 0xFF, 0.5);
        assert_eq!(lo & 0xFF, 0);
    }

    fn roughness(d: &[u32], s: usize) -> f64 {
        let mut sum = 0u64;
        for y in 0..s {
            for x in 0..s {
                let a = dword_to_rgba(d[y * s + x])[0];
                let b = dword_to_rgba(d[y * s + (x + 1) % s])[0];
                sum += u64::from(a.abs_diff(b));
            }
        }
        sum as f64 / (s * s) as f64
    }

    #[test]
    fn zoomed_noise_is_smoothed_and_deterministic() {
        let lq = noise_2d(&mut rng(3), 256, 1);
        let hq = noise_2d(&mut rng(3), 256, 8);
        assert!(roughness(&hq, 256) * 4.0 < roughness(&lq, 256));
        assert_eq!(noise_2d(&mut rng(3), 256, 8), hq);
        assert_ne!(noise_2d(&mut rng(4), 256, 8), hq);
    }

    #[test]
    fn noise_volume_is_smoothed_along_every_axis() {
        let s = 32usize;
        let d = noise_3d(&mut rng(5), s as u32, 4);
        assert_eq!(d.len(), s * s * s);
        let r = |i: usize| i32::from(dword_to_rgba(d[i])[0]);
        let mean_step = |stride: usize| {
            (0..s * s * s - stride)
                .map(|i| (r(i) - r(i + stride)).abs())
                .sum::<i32>() as f64
                / (s * s * s - stride) as f64
        };
        let raw = noise_3d(&mut rng(5), s as u32, 1);
        let raw_step = (0..raw.len() - 1)
            .map(|i| {
                (i32::from(dword_to_rgba(raw[i])[0]) - i32::from(dword_to_rgba(raw[i + 1])[0]))
                    .abs()
            })
            .sum::<i32>() as f64
            / (raw.len() - 1) as f64;
        for stride in [1, s, s * s] {
            let step = mean_step(stride);
            assert!(
                step * 2.0 < raw_step,
                "stride {stride}: {step} vs {raw_step}"
            );
        }
    }

    #[test]
    fn specs_match_plugin_cpp() {
        let names: Vec<_> = NOISE_2D.iter().map(|s| (s.name, s.size, s.zoom)).collect();
        assert_eq!(
            names,
            [
                ("noise_lq", 256, 1),
                ("noise_lq_lite", 32, 1),
                ("noise_mq", 256, 4),
                ("noise_hq", 256, 8)
            ]
        );
        let vols: Vec<_> = NOISE_3D.iter().map(|s| (s.name, s.size, s.zoom)).collect();
        assert_eq!(vols, [("noisevol_lq", 32, 1), ("noisevol_hq", 32, 4)]);
        assert_eq!((NOISE_2D_MIPS, NOISE_3D_MIPS), (true, false));
    }
}

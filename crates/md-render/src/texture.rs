//! 2D and 3D textures with optional full mip chains.
//!
//! Formats per the Milestone 5 context: `Rgba8Unorm` (8 bits per channel)
//! or `Rgba16Float` (16) for render targets, `Rgba8Unorm` for uploaded
//! images and noise; mip chains are always complete (down to 1×1).

use crate::gpu::Gpu;
use crate::mips::generate_mips;

/// Number of levels of a full mip chain for a `w`×`h` (×`d`) texture:
/// `floor(log2(max)) + 1`.
pub fn full_mip_count(w: u32, h: u32, d: u32) -> u32 {
    32 - w.max(h).max(d).max(1).leading_zeros()
}

/// A 2D texture and a view of all its levels.
#[derive(Clone, Debug)]
pub struct Texture2d {
    pub tex: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub w: u32,
    pub h: u32,
    /// Number of mip levels (1 without mips).
    pub mips: u32,
    pub format: wgpu::TextureFormat,
}

impl Texture2d {
    /// A render target (`w`, `h` clamped to at least 1) usable as a colour
    /// attachment, a sampled texture and a copy source/destination; with
    /// `mips` it gets a full chain for [`generate_mips`].
    pub fn new_target(
        gpu: &Gpu,
        w: u32,
        h: u32,
        format: wgpu::TextureFormat,
        mips: bool,
    ) -> Texture2d {
        let (w, h) = (w.max(1), h.max(1));
        let mips = if mips { full_mip_count(w, h, 1) } else { 1 };
        let tex = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("md-render texture2d"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        Texture2d {
            tex,
            view,
            w,
            h,
            mips,
            format,
        }
    }

    /// An `Rgba8Unorm` texture holding `rgba` (`w * h * 4` bytes, rows top
    /// to bottom); with `mips` the chain is generated on the GPU.
    ///
    /// # Panics
    /// If `rgba.len() != w * h * 4` or a dimension is 0.
    pub fn from_rgba8(gpu: &Gpu, w: u32, h: u32, rgba: &[u8], mips: bool) -> Texture2d {
        assert!(w > 0 && h > 0, "empty texture {w}x{h}");
        assert_eq!(rgba.len(), (w * h * 4) as usize, "rgba8 data for {w}x{h}");
        let t = Texture2d::new_target(gpu, w, h, wgpu::TextureFormat::Rgba8Unorm, mips);
        write_level(gpu, &t.tex, 0, (w, h, 1), rgba);
        generate_mips(gpu, &t);
        t
    }

    /// Size of mip `level`.
    pub fn mip_size(&self, level: u32) -> (u32, u32) {
        ((self.w >> level).max(1), (self.h >> level).max(1))
    }

    /// A view of the single mip `level` (render attachments need one).
    pub fn level_view(&self, level: u32) -> wgpu::TextureView {
        self.tex.create_view(&wgpu::TextureViewDescriptor {
            base_mip_level: level,
            mip_level_count: Some(1),
            ..Default::default()
        })
    }
}

/// A 3D `Rgba8Unorm` texture (the `noisevol_*` volumes) and a view of it.
#[derive(Clone, Debug)]
pub struct Texture3d {
    pub tex: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub w: u32,
    pub h: u32,
    pub d: u32,
    pub mips: u32,
}

impl Texture3d {
    /// A volume holding `rgba` (`w * h * d * 4` bytes, x fastest, then y,
    /// then z). With `mips` the full chain is built on the CPU by 2×2×2 box
    /// averaging (clamped at odd edges) — volumes are small and 3D levels
    /// cannot be blitted as 2D targets.
    ///
    /// # Panics
    /// If the data length does not match or a dimension is 0.
    pub fn from_rgba8(gpu: &Gpu, w: u32, h: u32, d: u32, rgba: &[u8], mips: bool) -> Texture3d {
        assert!(w > 0 && h > 0 && d > 0, "empty volume {w}x{h}x{d}");
        assert_eq!(
            rgba.len(),
            (w * h * d * 4) as usize,
            "rgba8 data for {w}x{h}x{d}"
        );
        let mips = if mips { full_mip_count(w, h, d) } else { 1 };
        let tex = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("md-render texture3d"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: d,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut level = rgba.to_vec();
        let mut size = (w, h, d);
        for l in 0..mips {
            write_level(gpu, &tex, l, size, &level);
            if l + 1 < mips {
                (level, size) = box_down_3d(&level, size);
            }
        }
        let view = tex.create_view(&Default::default());
        Texture3d {
            tex,
            view,
            w,
            h,
            d,
            mips,
        }
    }
}

/// Upload tightly packed RGBA8 `data` to mip `level` of size `size`.
fn write_level(gpu: &Gpu, tex: &wgpu::Texture, level: u32, size: (u32, u32, u32), data: &[u8]) {
    let (w, h, d) = size;
    gpu.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: level,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: d,
        },
    );
}

/// Halve an RGBA8 volume (each dimension to `max(1, n/2)`) by averaging
/// the 2×2×2 source block of each texel (rounded to nearest).
fn box_down_3d(src: &[u8], (w, h, d): (u32, u32, u32)) -> (Vec<u8>, (u32, u32, u32)) {
    let (nw, nh, nd) = ((w / 2).max(1), (h / 2).max(1), (d / 2).max(1));
    let at = |x: u32, y: u32, z: u32, c: u32| {
        u32::from(src[(((z.min(d - 1) * h + y.min(h - 1)) * w + x.min(w - 1)) * 4 + c) as usize])
    };
    let mut out = Vec::with_capacity((nw * nh * nd * 4) as usize);
    for z in 0..nd {
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0;
                    for (dx, dy, dz) in (0..8).map(|i| (i & 1, (i >> 1) & 1, i >> 2)) {
                        sum += at(2 * x + dx, 2 * y + dy, 2 * z + dz, c);
                    }
                    out.push(((sum + 4) / 8) as u8);
                }
            }
        }
    }
    (out, (nw, nh, nd))
}

//! Task 5.2: GPU context, textures, mip generation and the dynamic vertex
//! buffer on a real (headless) device.
//!
//! Every test asks [`gpu_or_skip`]: without an adapter it prints a note and
//! returns early, unless `MD_REQUIRE_GPU=1` (CI), where it fails.

use md_render::{
    generate_mips, gpu_or_skip, read_texture_level, vertices_to_bytes, DynamicVertexBuffer, Gpu,
    Texture2d, Texture3d, Vertex,
};

#[test]
fn headless_device_is_available_on_ci() {
    let Some(gpu) = gpu_or_skip("headless_device_is_available_on_ci") else {
        return;
    };
    // Gpu owns clones (F6): it outlives the tuple it was built from and
    // clones share the device.
    let g2 = gpu.clone();
    drop(gpu);
    let buf = g2.device().create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: false,
    });
    assert_eq!(buf.size(), 64);
}

#[test]
fn gpu_new_clones_borrowed_handles() {
    let Some(owner) = gpu_or_skip("gpu_new_clones_borrowed_handles") else {
        return;
    };
    let gpu = Gpu::new(owner.device(), owner.queue());
    drop(owner);
    gpu.queue().submit([]);
    gpu.wait_idle();
}

fn level_bytes(gpu: &Gpu, t: &Texture2d, level: u32) -> Vec<u8> {
    read_texture_level(gpu, &t.tex, level)
}

#[test]
fn mip_generation_halves_dimensions() {
    let Some(gpu) = gpu_or_skip("mip_generation_halves_dimensions") else {
        return;
    };
    let (w, h) = (64u32, 16u32);
    // 1-pixel checkerboard of black/white: every 2x2 box averages to grey.
    let data: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let (x, y) = (i % w, i / w);
            let v = if (x + y) % 2 == 0 { 255 } else { 0 };
            [v, v, v, 255]
        })
        .collect();
    let t = Texture2d::from_rgba8(&gpu, w, h, &data, true);
    assert_eq!(t.mips, 7, "full chain for 64x16: 64,32,16,8,4,2,1");
    assert_eq!(t.tex.mip_level_count(), 7);
    for level in 0..t.mips {
        let (lw, lh) = t.mip_size(level);
        assert_eq!(
            (lw, lh),
            ((w >> level).max(1), (h >> level).max(1)),
            "level {level}"
        );
        let bytes = level_bytes(&gpu, &t, level);
        assert_eq!(bytes.len() as u32, lw * lh * 4, "level {level}");
        if level == 0 {
            assert_eq!(bytes, data);
        } else {
            for px in bytes.chunks(4) {
                assert!(
                    (126..=129).contains(&px[0]) && px[0] == px[1] && px[1] == px[2],
                    "level {level}: {px:?} not mid-grey"
                );
                assert_eq!(px[3], 255);
            }
        }
    }
}

#[test]
fn mip_generation_works_for_rgba16f_targets() {
    let Some(gpu) = gpu_or_skip("mip_generation_works_for_rgba16f_targets") else {
        return;
    };
    let t = Texture2d::new_target(&gpu, 32, 32, wgpu::TextureFormat::Rgba16Float, true);
    assert_eq!(t.mips, 6);
    assert_eq!(t.format, wgpu::TextureFormat::Rgba16Float);
    // Clear level 0 to 0.5 via a render pass, then build mips.
    let mut enc = gpu.device().create_command_encoder(&Default::default());
    {
        let view = t.tex.create_view(&wgpu::TextureViewDescriptor {
            base_mip_level: 0,
            mip_level_count: Some(1),
            ..Default::default()
        });
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.5,
                        g: 0.25,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    gpu.queue().submit([enc.finish()]);
    generate_mips(&gpu, &t);
    let last = level_bytes(&gpu, &t, 5);
    assert_eq!(last.len(), 8, "1x1 of 4 half floats");
    let half = |i: usize| u16::from_le_bytes([last[2 * i], last[2 * i + 1]]);
    // f16 bit patterns: 0.5 = 0x3800, 0.25 = 0x3400, 1.0 = 0x3C00.
    assert_eq!(
        [half(0), half(1), half(2), half(3)],
        [0x3800, 0x3400, 0x3C00, 0x3C00]
    );
}

#[test]
fn texture_without_mips_has_one_level() {
    let Some(gpu) = gpu_or_skip("texture_without_mips_has_one_level") else {
        return;
    };
    let t = Texture2d::new_target(&gpu, 100, 60, wgpu::TextureFormat::Rgba8Unorm, false);
    assert_eq!((t.w, t.h, t.mips), (100, 60, 1));
    generate_mips(&gpu, &t); // no-op
    let t = Texture2d::new_target(&gpu, 100, 60, wgpu::TextureFormat::Rgba8Unorm, true);
    assert_eq!(t.mips, 7, "floor(log2(100)) + 1");
}

#[test]
fn texture3d_uploads_full_mip_chain() {
    let Some(gpu) = gpu_or_skip("texture3d_uploads_full_mip_chain") else {
        return;
    };
    let n = 8u32;
    let data: Vec<u8> = (0..n * n * n)
        .flat_map(|i| {
            let v = if i % 2 == 0 { 200 } else { 100 };
            [v, v, v, 255]
        })
        .collect();
    let t = Texture3d::from_rgba8(&gpu, n, n, n, &data, true);
    assert_eq!((t.w, t.h, t.d, t.mips), (8, 8, 8, 4));
    assert_eq!(t.tex.dimension(), wgpu::TextureDimension::D3);
    let top = read_texture_level(&gpu, &t.tex, 0);
    assert_eq!(top, data);
    let last = read_texture_level(&gpu, &t.tex, 3);
    assert_eq!(
        last,
        vec![150, 150, 150, 255],
        "box average of the 8^3 volume"
    );
}

fn quad(seed: f32) -> Vec<Vertex> {
    (0..4)
        .map(|i| Vertex {
            pos: [seed + i as f32, 0.0, 0.0],
            color: [i as u8, 1, 2, 3],
            ..Vertex::default()
        })
        .collect()
}

fn read_buffer(gpu: &Gpu, buf: &wgpu::Buffer, offset: u64, size: u64) -> Vec<u8> {
    let staging = gpu.device().create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device().create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(buf, offset, &staging, 0, size);
    gpu.queue().submit([enc.finish()]);
    md_render::map_read(gpu, &staging)
}

#[test]
fn dynamic_vertex_buffer_is_a_per_frame_ring() {
    let Some(gpu) = gpu_or_skip("dynamic_vertex_buffer_is_a_per_frame_ring") else {
        return;
    };
    let mut vb = DynamicVertexBuffer::new(&gpu, 16);
    vb.begin_frame();
    let (o1, s1) = {
        let s = vb.write(&quad(0.0));
        (s.offset(), s.size())
    };
    let (o2, s2) = {
        let s = vb.write(&quad(10.0));
        (s.offset(), s.size())
    };
    assert_eq!((o1, s1), (0, 160));
    assert_eq!((o2, s2), (160, 160), "second write follows the first");
    assert_eq!(vb.used_bytes(), 320);
    let bytes = read_buffer(&gpu, vb.buffer(), 0, 320);
    assert_eq!(&bytes[..160], &vertices_to_bytes(&quad(0.0))[..]);
    assert_eq!(&bytes[160..], &vertices_to_bytes(&quad(10.0))[..]);
    vb.begin_frame();
    assert_eq!(vb.used_bytes(), 0);
    assert_eq!(
        vb.write(&quad(20.0)).offset(),
        0,
        "next frame restarts the ring"
    );
}

#[test]
fn dynamic_vertex_buffer_grows_on_overflow() {
    let Some(gpu) = gpu_or_skip("dynamic_vertex_buffer_grows_on_overflow") else {
        return;
    };
    let mut vb = DynamicVertexBuffer::new(&gpu, 6);
    assert_eq!(vb.capacity_bytes(), 240);
    vb.begin_frame();
    let first = vb.buffer().clone();
    vb.write(&quad(0.0));
    // 4 + 4 vertices overflow 6: a new, larger buffer takes the write at 0.
    let (off, size) = {
        let s = vb.write(&quad(5.0));
        (s.offset(), s.size())
    };
    assert_eq!((off, size), (0, 160));
    assert!(vb.capacity_bytes() >= 2 * 240);
    assert_ne!(vb.buffer(), &first, "grew into a new buffer");
    // The old buffer (still referenced by earlier draws) keeps its data.
    assert_eq!(
        read_buffer(&gpu, &first, 0, 160),
        vertices_to_bytes(&quad(0.0))
    );
    assert_eq!(
        read_buffer(&gpu, vb.buffer(), 0, 160),
        vertices_to_bytes(&quad(5.0))
    );
    // A single write larger than double the capacity also fits.
    let big: Vec<Vertex> = (0..100).map(|_| Vertex::default()).collect();
    assert_eq!(vb.write(&big).size(), 4000);
    assert!(vb.capacity_bytes() >= 4000);
    // Empty writes return a valid (non-empty) slice to draw 0 vertices from.
    assert_eq!(vb.write(&[]).size(), 40);
}

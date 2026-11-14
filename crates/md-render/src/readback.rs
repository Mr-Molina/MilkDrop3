//! Blocking GPU→CPU readback, for tests and the headless `render` path.

use crate::gpu::Gpu;

/// Map `buf` (usage `MAP_READ`, all prior work submitted), wait, and copy
/// out its whole contents.
pub fn map_read(gpu: &Gpu, buf: &wgpu::Buffer) -> Vec<u8> {
    let (tx, rx) = std::sync::mpsc::channel();
    buf.map_async(wgpu::MapMode::Read, .., move |r| {
        let _ = tx.send(r);
    });
    gpu.wait_idle();
    rx.recv()
        .expect("map_async callback dropped")
        .expect("buffer mapping failed");
    let out = buf.get_mapped_range(..).expect("mapped range").to_vec();
    buf.unmap();
    out
}

/// The texels of mip `level` of `tex` (2D or 3D, any uncompressed format,
/// usage `COPY_SRC`), tightly packed: rows top to bottom, then slices.
pub fn read_texture_level(gpu: &Gpu, tex: &wgpu::Texture, level: u32) -> Vec<u8> {
    let size = tex.size().mip_level_size(level, tex.dimension());
    let layers = if tex.dimension() == wgpu::TextureDimension::D3 {
        size.depth_or_array_layers
    } else {
        1
    };
    let texel = tex
        .format()
        .block_copy_size(None)
        .expect("uncompressed colour format");
    let row = size.width * texel;
    let padded =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let staging = gpu.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("md-render readback"),
        size: u64::from(padded * size.height * layers),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device().create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: level,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(size.height),
            },
        },
        wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: layers,
        },
    );
    gpu.queue().submit([enc.finish()]);
    map_read(gpu, &staging)
        .chunks(padded as usize)
        .flat_map(|r| &r[..row as usize])
        .copied()
        .collect()
}

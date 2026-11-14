//! A per-frame ring of vertex data.

use crate::gpu::Gpu;
use crate::vertex::{vertices_to_bytes, Vertex};

/// A vertex buffer that hands out consecutive ranges within a frame.
///
/// Call [`begin_frame`](Self::begin_frame) once per frame (after the
/// previous frame was submitted); each [`write`](Self::write) uploads with
/// `Queue::write_buffer` to the next free range and returns its slice. Since
/// all of a frame's writes land before that frame's command buffers run,
/// ranges are never reused within a frame.
///
/// Overflow: when a write does not fit, the buffer **grows** — a new buffer
/// of `max(2 × capacity, needed)` bytes replaces it and the write goes to
/// its offset 0. Slices handed out earlier keep the old buffer alive (wgpu
/// handles are reference counted), so draws already recorded this frame are
/// unaffected. The larger buffer is kept for later frames.
#[derive(Debug)]
pub struct DynamicVertexBuffer {
    gpu: Gpu,
    buf: wgpu::Buffer,
    cursor: u64,
}

impl DynamicVertexBuffer {
    /// A ring with room for `capacity_verts` vertices (at least 1).
    pub fn new(gpu: &Gpu, capacity_verts: usize) -> DynamicVertexBuffer {
        let size = capacity_verts.max(1) as u64 * Vertex::SIZE;
        DynamicVertexBuffer {
            gpu: gpu.clone(),
            buf: Self::alloc(gpu, size),
            cursor: 0,
        }
    }

    fn alloc(gpu: &Gpu, size: u64) -> wgpu::Buffer {
        gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("md-render dynamic vertices"),
            size,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    /// Start a new frame: the whole buffer is free again.
    pub fn begin_frame(&mut self) {
        self.cursor = 0;
    }

    /// Upload `verts` to the next free range and return it. An empty
    /// `verts` returns a valid one-vertex slice (wgpu slices cannot be
    /// empty) that the caller draws zero vertices from.
    pub fn write(&mut self, verts: &[Vertex]) -> wgpu::BufferSlice<'_> {
        if verts.is_empty() {
            let at = if self.cursor + Vertex::SIZE <= self.capacity_bytes() {
                self.cursor
            } else {
                0
            };
            return self.buf.slice(at..at + Vertex::SIZE);
        }
        let bytes = vertices_to_bytes(verts);
        let len = bytes.len() as u64;
        if self.cursor + len > self.capacity_bytes() {
            let size = (2 * self.capacity_bytes()).max(len);
            self.buf = Self::alloc(&self.gpu, size);
            self.cursor = 0;
        }
        let at = self.cursor;
        self.gpu.queue().write_buffer(&self.buf, at, &bytes);
        self.cursor += len;
        self.buf.slice(at..at + len)
    }

    /// The current buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buf
    }

    /// Size of the current buffer in bytes.
    pub fn capacity_bytes(&self) -> u64 {
        self.buf.size()
    }

    /// Bytes written to the current buffer this frame.
    pub fn used_bytes(&self) -> u64 {
        self.cursor
    }
}

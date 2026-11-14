//! Little-endian byte packing for GPU buffers (F11: md-render serializes by
//! hand instead of `bytemuck`, keeping `forbid(unsafe_code)`).

/// Appends little-endian scalars to a byte vector.
pub(crate) struct ByteWriter<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> ByteWriter<'a> {
    pub(crate) fn new(out: &'a mut Vec<u8>) -> Self {
        ByteWriter { out }
    }

    pub(crate) fn f32s(&mut self, xs: &[f32]) -> &mut Self {
        for x in xs {
            self.out.extend_from_slice(&x.to_le_bytes());
        }
        self
    }

    pub(crate) fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.out.extend_from_slice(b);
        self
    }
}

/// Serialize into a fixed-size array with `f`, which must write exactly `N`
/// bytes.
pub(crate) fn to_array<const N: usize>(f: impl FnOnce(&mut ByteWriter)) -> [u8; N] {
    let mut v = Vec::with_capacity(N);
    f(&mut ByteWriter::new(&mut v));
    v.try_into()
        .unwrap_or_else(|v: Vec<u8>| panic!("serialized {} bytes, expected {N}", v.len()))
}

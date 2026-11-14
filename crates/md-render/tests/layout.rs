//! Task 5.2: CPU-side byte layouts of the md-render GPU types (no device).
//!
//! F11: everything is serialized by hand (little-endian, no bytemuck);
//! F29: every offset comes from `md_hlsl::layout`; F24: HLSL `float4x3`
//! rows become WGSL `mat4x3` columns.

use md_hlsl::layout::{
    md_uniform_offset, texsizes_size, BLUR_UNIFORMS_SIZE, BUILTIN_VS_PARAMS_SIZE, D3D9_PIXEL_SHIFT,
    MD_UNIFORMS_SIZE, MD_UNIFORM_FIELDS,
};
use md_render::{
    pack_mat4x3, texsizes_bytes, vertices_to_bytes, BlurUniforms, MdUniforms, Vertex, VsParams,
};

fn f32_at(bytes: &[u8], off: u64) -> f32 {
    let o = off as usize;
    f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap())
}

fn floats_at(bytes: &[u8], off: u64, n: usize) -> Vec<f32> {
    (0..n).map(|i| f32_at(bytes, off + 4 * i as u64)).collect()
}

/// A uniform block with a distinct marker in every float of every field.
fn marked() -> MdUniforms {
    let mut u = MdUniforms::default();
    for (i, c) in u.c.iter_mut().enumerate() {
        *c = std::array::from_fn(|j| (100 * i + j) as f32);
    }
    for (i, q) in u.q.iter_mut().enumerate() {
        *q = std::array::from_fn(|j| (2000 + 100 * i + j) as f32);
    }
    u.rand_frame = [3000.0, 3001.0, 3002.0, 3003.0];
    u.rand_preset = [3100.0, 3101.0, 3102.0, 3103.0];
    for (k, m) in u.rot.iter_mut().enumerate() {
        *m =
            std::array::from_fn(|r| std::array::from_fn(|c| (10000 + 100 * k + 10 * r + c) as f32));
    }
    u
}

/// The expected floats (16 for a mat4x3, 4 for a vec4) of field `name` of
/// [`marked`], mapped by name independently of the serializer.
fn expected(u: &MdUniforms, name: &str) -> Vec<f32> {
    if let Some(i) = name.strip_prefix("_c") {
        return u.c[i.parse::<usize>().unwrap()].to_vec();
    }
    if let Some(l) = name.strip_prefix("_q") {
        let i = (l.as_bytes()[0] - b'a') as usize;
        return u.q[i].to_vec();
    }
    match name {
        "rand_frame" => return u.rand_frame.to_vec(),
        "rand_preset" => return u.rand_preset.to_vec(),
        _ => {}
    }
    let rest = name.strip_prefix("rot_").unwrap();
    let digit = rest.find(|ch: char| ch.is_ascii_digit()).unwrap();
    let group = ["s", "d", "f", "vf", "uf", "rand"]
        .iter()
        .position(|g| *g == &rest[..digit])
        .unwrap();
    let k = group * 4 + rest[digit..].parse::<usize>().unwrap() - 1;
    pack_mat4x3(u.rot[k]).iter().flatten().copied().collect()
}

#[test]
fn uniform_struct_is_1920_bytes() {
    assert_eq!(MdUniforms::SIZE, MD_UNIFORMS_SIZE);
    assert_eq!(MdUniforms::default().to_bytes().len() as u64, 1920);
    let mut out = vec![0xAA; 3];
    MdUniforms::default().write_to(&mut out);
    assert_eq!(out.len(), 3 + 1920, "write_to appends");
}

#[test]
fn every_uniform_field_lands_at_md_uniform_offset() {
    let u = marked();
    let bytes = u.to_bytes();
    for (name, kind) in MD_UNIFORM_FIELDS {
        let off = md_uniform_offset(name).unwrap();
        let want = expected(&u, name);
        assert_eq!(want.len() as u64 * 4, kind.size(), "{name}");
        assert_eq!(floats_at(&bytes, off, want.len()), want, "{name} at {off}");
    }
}

#[test]
fn mat4x3_packing_puts_hlsl_rows_in_wgsl_columns() {
    let rows = [
        [1.0, 2.0, 3.0],
        [4.0, 5.0, 6.0],
        [7.0, 8.0, 9.0],
        [10.0, 11.0, 12.0],
    ];
    let cols = pack_mat4x3(rows);
    for i in 0..4 {
        assert_eq!(cols[i][..3], rows[i], "WGSL column {i} is HLSL row {i}");
        assert_eq!(cols[i][3], 0.0, "column {i} padded to 16 bytes");
    }
    // In the block: column i of rot_s1 starts at offset + 16 i.
    let mut u = MdUniforms::default();
    u.rot[0] = rows;
    let bytes = u.to_bytes();
    let base = md_uniform_offset("rot_s1").unwrap();
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(floats_at(&bytes, base + 16 * i as u64, 3), row.to_vec());
        assert_eq!(f32_at(&bytes, base + 16 * i as u64 + 12), 0.0);
    }
}

#[test]
fn vertex_layout_matches_builtin_shader_table() {
    // assets/shaders/README.md: stride 40, pos@0 off 0, color@1 off 12,
    // uv@2 off 16, rad_ang@3 off 32.
    assert_eq!(Vertex::SIZE, 40);
    assert_eq!(std::mem::size_of::<Vertex>(), 40);
    let l = Vertex::LAYOUT;
    assert_eq!(l.array_stride, 40);
    assert_eq!(l.step_mode, wgpu::VertexStepMode::Vertex);
    let got: Vec<_> = l
        .attributes
        .iter()
        .map(|a| (a.shader_location, a.offset, a.format))
        .collect();
    use wgpu::VertexFormat::*;
    assert_eq!(
        got,
        vec![
            (0, 0, Float32x3),
            (1, 12, Unorm8x4),
            (2, 16, Float32x4),
            (3, 32, Float32x2)
        ]
    );
}

#[test]
fn vertex_bytes_are_little_endian_in_field_order() {
    let v = Vertex {
        pos: [1.0, -2.0, 0.5],
        color: [10, 20, 30, 40],
        uv: [0.25, 0.75, 1.5, -1.5],
        rad_ang: [0.125, 3.0],
    };
    let b = v.to_bytes();
    assert_eq!(floats_at(&b, 0, 3), vec![1.0, -2.0, 0.5]);
    assert_eq!(&b[12..16], &[10, 20, 30, 40]);
    assert_eq!(floats_at(&b, 16, 4), vec![0.25, 0.75, 1.5, -1.5]);
    assert_eq!(floats_at(&b, 32, 2), vec![0.125, 3.0]);
    let all = vertices_to_bytes(&[v, Vertex::default()]);
    assert_eq!(all.len(), 80);
    assert_eq!(&all[..40], &b[..]);
    assert!(all[40..].iter().all(|&x| x == 0), "unused fields are zero");
}

#[test]
fn d3dcolor_is_swizzled_to_rgba_bytes() {
    // D3DCOLOR 0xAARRGGBB.
    assert_eq!(
        Vertex::color_from_d3dcolor(0x80FF4010),
        [0xFF, 0x40, 0x10, 0x80]
    );
    assert_eq!(Vertex::color_from_d3dcolor(0xFF000000), [0, 0, 0, 0xFF]);
}

#[test]
fn vs_params_layout() {
    let p = VsParams::for_target(256, 128);
    assert_eq!(p.texel_size, [1.0 / 256.0, 1.0 / 128.0]);
    assert_eq!(p.pixel_shift, D3D9_PIXEL_SHIFT);
    let b = p.to_bytes();
    assert_eq!(b.len() as u64, BUILTIN_VS_PARAMS_SIZE);
    assert_eq!(
        floats_at(&b, 0, 4),
        vec![1.0 / 256.0, 1.0 / 128.0, 0.5, 0.0]
    );
}

#[test]
fn blur_uniforms_layout() {
    let mut u = BlurUniforms::default();
    for (n, c) in u.c.iter_mut().enumerate() {
        *c = std::array::from_fn(|j| (10 * n + j) as f32);
    }
    let b = u.to_bytes();
    assert_eq!(b.len() as u64, BLUR_UNIFORMS_SIZE);
    for n in 0..7 {
        assert_eq!(
            floats_at(&b, 16 * n as u64, 4),
            u.c[n].to_vec(),
            "_c{n} at 16*{n}"
        );
    }
}

#[test]
fn texsizes_block_is_never_empty() {
    assert_eq!(texsizes_bytes(&[]).len() as u64, texsizes_size(0));
    assert_eq!(texsizes_bytes(&[]).len(), 16);
    let v = [
        [256.0, 128.0, 1.0 / 256.0, 1.0 / 128.0],
        [1.0, 2.0, 3.0, 4.0],
    ];
    let b = texsizes_bytes(&v);
    assert_eq!(b.len() as u64, texsizes_size(2));
    assert_eq!(floats_at(&b, 16, 4), v[1].to_vec());
}

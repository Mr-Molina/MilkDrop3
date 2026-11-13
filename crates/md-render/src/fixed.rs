//! D3D9 fixed-function emulation: the texture-stage modes, primitive
//! topologies and sampler states of the C++'s non-shader draws, and the
//! CPU conversions for the primitives wgpu lacks.
//!
//! F31: wgpu has `PointList` (always 1 px) but no `TriangleFan`. Fans become
//! triangle lists ([`fan_to_list`]); points become two-triangle quads of the
//! configured point size ([`points_to_quads`]), for every size: D3D9
//! rasterizes a point as a screen-aligned square of `D3DRS_POINTSIZE`
//! pixels under the triangle fill rules, which a quad reproduces exactly
//! (with the shared +0.5 px raster shift of the built-in vertex shaders).
//!
//! Point sizes the C++ uses: `DrawCustomWaves` sets `D3DRS_POINTSIZE` to
//! `(tex_w >= 1024 ? 2 : 1) + (bDrawThick ? 1 : 0)` for dot waves and back
//! to 1 afterwards (code/vis_milk2/milkdropfs.cpp:2650-2652, 2670-2671);
//! `DrawWave` draws dots at the D3D9 default size 1.0 and fattens them by
//! redrawing offset copies instead (milkdropfs.cpp:3258-3287).

use crate::vertex::Vertex;

/// The D3D9 texture stage 0 state of a fixed-function draw (one built-in
/// shader each; see `assets/shaders/README.md`, "Fixed-function
/// equivalents").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FixedMode {
    /// No texture: the vertex colour (`fixed_color.wgsl`).
    Color,
    /// The C++'s standard stage state, COLOROP `MODULATE(DIFFUSE, TEXTURE)`
    /// and ALPHAOP `SELECTARG1 DIFFUSE`: `(tex.rgb * color.rgb, color.a)`
    /// (`fixed_tex_modulate.wgsl`).
    TexModulate,
    /// Sprite blend mode 4 (milkdropfs.cpp:3714-3721): COLOROP and ALPHAOP
    /// `MODULATE`: `tex * color` (`fixed_tex_modulate_alpha.wgsl`).
    TexModulateAlpha,
    /// `SELECTARG1 TEXTURE`: the texture alone (`fixed_tex_select.wgsl`);
    /// no C++ draw uses it, md-render's own copies do.
    TexSelect,
}

impl FixedMode {
    /// Every mode, for iteration.
    pub const ALL: [FixedMode; 4] = [
        FixedMode::Color,
        FixedMode::TexModulate,
        FixedMode::TexModulateAlpha,
        FixedMode::TexSelect,
    ];

    /// Whether the mode samples a texture (bind group 1).
    pub fn textured(self) -> bool {
        self != FixedMode::Color
    }

    /// The built-in WGSL source (`vs_main`, `fs_main`).
    pub fn wgsl(self) -> &'static str {
        match self {
            FixedMode::Color => include_str!("../../../assets/shaders/fixed_color.wgsl"),
            FixedMode::TexModulate => {
                include_str!("../../../assets/shaders/fixed_tex_modulate.wgsl")
            }
            FixedMode::TexModulateAlpha => {
                include_str!("../../../assets/shaders/fixed_tex_modulate_alpha.wgsl")
            }
            FixedMode::TexSelect => include_str!("../../../assets/shaders/fixed_tex_select.wgsl"),
        }
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// The D3D9 primitive type of a draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Topo {
    /// `D3DPT_TRIANGLELIST`.
    TriList,
    /// `D3DPT_TRIANGLESTRIP`.
    TriStrip,
    /// `D3DPT_TRIANGLEFAN`: converted to a list by [`fan_to_list`].
    TriFan,
    /// `D3DPT_LINESTRIP` (1 px wide in both APIs).
    LineStrip,
    /// `D3DPT_LINELIST` (1 px wide in both APIs).
    LineList,
    /// `D3DPT_POINTLIST`: expanded to quads by [`points_to_quads`].
    Points,
}

impl Topo {
    /// The wgpu topology the converted vertices are drawn with.
    pub fn primitive(self) -> wgpu::PrimitiveTopology {
        match self {
            Topo::TriList | Topo::TriFan | Topo::Points => wgpu::PrimitiveTopology::TriangleList,
            Topo::TriStrip => wgpu::PrimitiveTopology::TriangleStrip,
            Topo::LineStrip => wgpu::PrimitiveTopology::LineStrip,
            Topo::LineList => wgpu::PrimitiveTopology::LineList,
        }
    }

    /// The topology a pipeline is built for: fans and points share the
    /// triangle-list pipeline.
    pub fn canonical(self) -> Topo {
        match self {
            Topo::TriFan | Topo::Points => Topo::TriList,
            t => t,
        }
    }
}

/// Sampler state of a textured fixed-function draw.
///
/// Linear: MAG/MIN/MIP `D3DTEXF_LINEAR`, as the C++ sets for every
/// textured pass (code/vis_milk2/support.cpp:88-93; milkdropfs.cpp:913-918,
/// 1888-1893, 1532-1534, 3467-3469, 3805-3807, 3981-3986). Point: all three
/// `D3DTEXF_POINT`, as for the `pw_`/`pc_` sampler prefixes
/// (milkdropfs.cpp:3843-3846). Wrap/Clamp: `D3DTADDRESS_WRAP`/`_CLAMP` on
/// U and V (milkdropfs.cpp:889-897, 1906-1908, 1529-1530, 3841-3842; W
/// follows). The mip filter only matters for textures with mips (noise,
/// uploaded images; not the single-level main textures); with one level it is inert, as in
/// D3D9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerKind {
    LinearWrap,
    LinearClamp,
    PointWrap,
    PointClamp,
}

impl SamplerKind {
    /// Every kind, for iteration.
    pub const ALL: [SamplerKind; 4] = [
        SamplerKind::LinearWrap,
        SamplerKind::LinearClamp,
        SamplerKind::PointWrap,
        SamplerKind::PointClamp,
    ];

    /// The wgpu sampler descriptor for this D3D9 sampler state.
    pub fn descriptor(self) -> wgpu::SamplerDescriptor<'static> {
        let (filter, mip) = match self {
            SamplerKind::LinearWrap | SamplerKind::LinearClamp => {
                (wgpu::FilterMode::Linear, wgpu::MipmapFilterMode::Linear)
            }
            SamplerKind::PointWrap | SamplerKind::PointClamp => {
                (wgpu::FilterMode::Nearest, wgpu::MipmapFilterMode::Nearest)
            }
        };
        let address = match self {
            SamplerKind::LinearWrap | SamplerKind::PointWrap => wgpu::AddressMode::Repeat,
            SamplerKind::LinearClamp | SamplerKind::PointClamp => wgpu::AddressMode::ClampToEdge,
        };
        wgpu::SamplerDescriptor {
            label: Some("md-render fixed sampler"),
            address_mode_u: address,
            address_mode_v: address,
            address_mode_w: address,
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: mip,
            ..Default::default()
        }
    }
}

/// A `D3DPT_TRIANGLEFAN` as a triangle list: `(v0, v[i], v[i+1])` for
/// `i = 1..n-1`, i.e. `n - 2` triangles (none for fewer than 3 vertices).
/// Winding is irrelevant: the C++ and md-render draw with culling off.
pub fn fan_to_list(verts: &[Vertex]) -> Vec<Vertex> {
    if verts.len() < 3 {
        return Vec::new();
    }
    verts
        .windows(2)
        .skip(1)
        .flat_map(|w| [verts[0], w[0], w[1]])
        .collect()
}

/// A `D3DPT_POINTLIST` as a triangle list: each point becomes a square of
/// `size` pixels (at least 1, D3D9's `D3DRS_POINTSIZE_MIN` default) centred
/// on it in a `target` (W, H) render target — half-extent `size / W` by
/// `size / H` in NDC — as two triangles (six vertices) carrying the point's
/// colour, UVs and depth.
pub fn points_to_quads(verts: &[Vertex], size: f32, target: (u32, u32)) -> Vec<Vertex> {
    let size = size.max(1.0);
    let hx = size / target.0.max(1) as f32;
    let hy = size / target.1.max(1) as f32;
    let mut out = Vec::with_capacity(verts.len() * 6);
    for v in verts {
        let at = |dx: f32, dy: f32| Vertex {
            pos: [v.pos[0] + dx, v.pos[1] + dy, v.pos[2]],
            ..*v
        };
        let (a, b, c, d) = (at(-hx, hy), at(hx, hy), at(-hx, -hy), at(hx, -hy));
        out.extend_from_slice(&[a, b, c, c, b, d]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_fixed_wgsl_parses() {
        for m in FixedMode::ALL {
            let module = naga::front::wgsl::parse_str(m.wgsl()).expect("parse");
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .expect("validate");
        }
    }

    #[test]
    fn sampler_descriptors_follow_d3d9_states() {
        let d = SamplerKind::LinearWrap.descriptor();
        assert_eq!(d.mag_filter, wgpu::FilterMode::Linear);
        assert_eq!(d.mipmap_filter, wgpu::MipmapFilterMode::Linear);
        assert_eq!(d.address_mode_u, wgpu::AddressMode::Repeat);
        let d = SamplerKind::PointClamp.descriptor();
        assert_eq!(d.min_filter, wgpu::FilterMode::Nearest);
        assert_eq!(d.mipmap_filter, wgpu::MipmapFilterMode::Nearest);
        assert_eq!(d.address_mode_v, wgpu::AddressMode::ClampToEdge);
    }

    #[test]
    fn fan_and_points_share_the_list_pipeline() {
        assert_eq!(Topo::TriFan.canonical(), Topo::TriList);
        assert_eq!(Topo::Points.canonical(), Topo::TriList);
        assert_eq!(Topo::LineStrip.canonical(), Topo::LineStrip);
        assert_eq!(
            Topo::Points.primitive(),
            wgpu::PrimitiveTopology::TriangleList
        );
    }
}

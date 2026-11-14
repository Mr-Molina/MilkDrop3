//! Task 4.8: the hand-written built-in WGSL passes in `assets/shaders/`.
//!
//! Every file must parse and validate under naga; every vertex shader must
//! read the single Task 5.2 `Vertex` layout (F30); `warp_vs`/`comp_vs` must
//! produce exactly the `VsOut` a translated preset fragment shader reads;
//! the blur and parameter blocks must have the sizes/offsets
//! `md_hlsl::layout` publishes for md-render; every vertex shader must apply
//! the D3D9 rasterization shift (F29, round-1 ruling: +0.5 px position).

mod common;

use common::asset;
use md_hlsl::layout::{
    BLUR_UNIFORMS_SIZE, BUILTIN_VS_PARAMS_BINDING, BUILTIN_VS_PARAMS_SIZE, D3D9_PIXEL_SHIFT,
};
use md_hlsl::{translate, PsVersion, ShaderInput, ShaderKind};
use naga::{Binding, BuiltIn, Module, ShaderStage, TypeInner, VectorSize};

/// Every file Task 4.8 creates.
const FILES: [&str; 10] = [
    "warp_vs.wgsl",
    "comp_vs.wgsl",
    "blur_vs.wgsl",
    "blur1_ps.wgsl",
    "blur2_ps.wgsl",
    "fixed_color.wgsl",
    "fixed_tex_modulate.wgsl",
    "fixed_tex_modulate_alpha.wgsl",
    "fixed_tex_select.wgsl",
    "text.wgsl",
];

/// The files with a vertex entry point.
const VERTEX_FILES: [&str; 8] = [
    "warp_vs.wgsl",
    "comp_vs.wgsl",
    "blur_vs.wgsl",
    "fixed_color.wgsl",
    "fixed_tex_modulate.wgsl",
    "fixed_tex_modulate_alpha.wgsl",
    "fixed_tex_select.wgsl",
    "text.wgsl",
];

/// The position shift every vertex shader carries, verbatim; mirrored on
/// the CPU by [`d3d9_raster`].
const RASTER_FN: &str = "fn d3d9_raster(clip: vec4<f32>) -> vec4<f32> {
    let s = 2.0 * vs_params.pixel_shift * vec2<f32>(vs_params.texel_size.x, -vs_params.texel_size.y);
    return vec4<f32>(clip.xy + s * clip.w, clip.zw);
}";

fn shader_dir() -> String {
    format!("{}/../../assets/shaders", env!("CARGO_MANIFEST_DIR"))
}

fn source(name: &str) -> String {
    let p = format!("{}/{name}", shader_dir());
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

#[track_caller]
fn module(name: &str) -> Module {
    let src = source(name);
    common::naga_validate(&src).unwrap_or_else(|e| panic!("{name}:\n{e}"))
}

#[track_caller]
fn entry(m: &Module, stage: ShaderStage) -> &naga::EntryPoint {
    m.entry_points
        .iter()
        .find(|e| e.stage == stage)
        .unwrap_or_else(|| panic!("no {stage:?} entry point"))
}

/// One shader-interface member: name, binding, type.
type Io = (String, Binding, TypeInner);

/// Flatten a (possibly struct) value of type `ty` with optional `binding`.
fn flatten(
    m: &Module,
    name: Option<&str>,
    ty: naga::Handle<naga::Type>,
    b: &Option<Binding>,
) -> Vec<Io> {
    match (&m.types[ty].inner, b) {
        (TypeInner::Struct { members, .. }, None) => members
            .iter()
            .map(|mem| {
                (
                    mem.name.clone().unwrap_or_default(),
                    mem.binding.clone().expect("struct member without binding"),
                    m.types[mem.ty].inner.clone(),
                )
            })
            .collect(),
        (inner, Some(b)) => vec![(
            name.unwrap_or_default().to_string(),
            b.clone(),
            inner.clone(),
        )],
        _ => panic!("interface value without binding"),
    }
}

fn outputs(m: &Module, stage: ShaderStage) -> Vec<Io> {
    let ep = entry(m, stage);
    let r = ep
        .function
        .result
        .as_ref()
        .expect("entry point has no result");
    flatten(m, None, r.ty, &r.binding)
}

fn inputs(m: &Module, stage: ShaderStage) -> Vec<Io> {
    let ep = entry(m, stage);
    ep.function
        .arguments
        .iter()
        .flat_map(|a| flatten(m, a.name.as_deref(), a.ty, &a.binding))
        .collect()
}

fn location(b: &Binding) -> Option<u32> {
    match b {
        Binding::Location { location, .. } => Some(*location),
        Binding::BuiltIn(_) => None,
    }
}

fn vec_f32(size: VectorSize) -> TypeInner {
    TypeInner::Vector {
        size,
        scalar: naga::Scalar::F32,
    }
}

/// `(group, binding, name)` of every resource global, sorted.
fn resources(m: &Module) -> Vec<(u32, u32, String)> {
    let mut v: Vec<_> = m
        .global_variables
        .iter()
        .filter_map(|(_, g)| {
            g.binding
                .as_ref()
                .map(|b| (b.group, b.binding, g.name.clone().unwrap_or_default()))
        })
        .collect();
    v.sort();
    v
}

/// Size and member offsets of the struct type of global `name`.
fn uniform_layout(m: &Module, name: &str) -> (u32, Vec<(String, u32)>) {
    let g = m
        .global_variables
        .iter()
        .find(|(_, g)| g.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no global {name}"))
        .1;
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(m.to_ctx()).unwrap();
    let size = layouter[g.ty].size;
    let TypeInner::Struct { members, .. } = &m.types[g.ty].inner else {
        panic!("{name} is not a struct");
    };
    let offs = members
        .iter()
        .map(|mem| (mem.name.clone().unwrap_or_default(), mem.offset))
        .collect();
    (size, offs)
}

fn translated(kind: ShaderKind, body_file: &str) -> Module {
    let t = translate(&ShaderInput {
        kind,
        body: &asset(body_file),
        preamble: &asset("include.fx"),
        ps_version: PsVersion::Ps2_0,
    })
    .unwrap_or_else(|e| panic!("translate {body_file}: {e}"));
    common::naga_validate(&t.wgsl).unwrap()
}

#[test]
fn every_builtin_wgsl_validates() {
    for f in FILES {
        module(f);
    }
    // No stray files: everything in assets/shaders is either listed or the README.
    for e in std::fs::read_dir(shader_dir()).unwrap() {
        let n = e.unwrap().file_name().into_string().unwrap();
        assert!(
            n == "README.md" || FILES.contains(&n.as_str()),
            "unexpected file assets/shaders/{n}"
        );
    }
}

#[test]
fn every_vertex_shader_reads_the_single_vertex_layout() {
    // F30: pos @0 vec3, color @1 vec4 (Unorm8x4), uv @2 vec4, rad_ang @3 vec2.
    let want = [
        (0, vec_f32(VectorSize::Tri)),
        (1, vec_f32(VectorSize::Quad)),
        (2, vec_f32(VectorSize::Quad)),
        (3, vec_f32(VectorSize::Bi)),
    ];
    for f in FILES {
        let m = module(f);
        if !m
            .entry_points
            .iter()
            .any(|e| e.stage == ShaderStage::Vertex)
        {
            continue;
        }
        let mut got: Vec<(u32, TypeInner)> = inputs(&m, ShaderStage::Vertex)
            .into_iter()
            .map(|(_, b, t)| (location(&b).expect("builtin vertex input"), t))
            .collect();
        got.sort_by_key(|g| g.0);
        assert_eq!(got, want, "{f}");
    }
}

#[test]
fn warp_vs_and_comp_vs_output_the_preset_vs_out() {
    for (vs, kind, body) in [
        ("warp_vs.wgsl", ShaderKind::Warp, "warp_ps.fx"),
        ("comp_vs.wgsl", ShaderKind::Comp, "comp_ps.fx"),
    ] {
        let out = outputs(&module(vs), ShaderStage::Vertex);
        let fs_in = inputs(&translated(kind, body), ShaderStage::Fragment);
        // Identical member names, bindings and types, in order.
        assert_eq!(out, fs_in, "{vs} vs translated {body}");
        let names: Vec<&str> = out.iter().map(|o| o.0.as_str()).collect();
        assert_eq!(names, ["pos", "diffuse", "uv", "rad_ang"], "{vs}");
        assert_eq!(
            out[0].1,
            Binding::BuiltIn(BuiltIn::Position { invariant: false })
        );
        let locs: Vec<Option<u32>> = out[1..].iter().map(|o| location(&o.1)).collect();
        assert_eq!(locs, [Some(0), Some(1), Some(2)], "{vs}");
        assert_eq!(out[2].2, vec_f32(VectorSize::Quad));
    }
}

#[test]
fn vertex_outputs_feed_their_fragment_shaders() {
    let pairs = [
        ("blur_vs.wgsl", "blur1_ps.wgsl"),
        ("blur_vs.wgsl", "blur2_ps.wgsl"),
        ("fixed_color.wgsl", "fixed_color.wgsl"),
        ("fixed_tex_modulate.wgsl", "fixed_tex_modulate.wgsl"),
        (
            "fixed_tex_modulate_alpha.wgsl",
            "fixed_tex_modulate_alpha.wgsl",
        ),
        ("fixed_tex_select.wgsl", "fixed_tex_select.wgsl"),
        ("text.wgsl", "text.wgsl"),
    ];
    for (vs, fs) in pairs {
        let out = outputs(&module(vs), ShaderStage::Vertex);
        for (name, b, t) in inputs(&module(fs), ShaderStage::Fragment) {
            if location(&b).is_none() {
                continue;
            }
            assert!(
                out.iter().any(|(_, ob, ot)| *ob == b && *ot == t),
                "{fs} input {name} ({b:?}) has no matching {vs} output"
            );
        }
    }
}

#[test]
fn builtin_vertex_params_do_not_collide_with_preset_bindings() {
    // Preset fragment shaders use (0,0) MdUniforms, (0,1) TexSizes and
    // group 1 for textures; the built-in VS block sits at (0, 2).
    let preset = resources(&translated(ShaderKind::Warp, "warp_ps.fx"));
    for vs in VERTEX_FILES {
        let m = module(vs);
        let r = resources(&m);
        assert!(
            r.contains(&(0, BUILTIN_VS_PARAMS_BINDING, "vs_params".to_string())),
            "{vs}: {r:?}"
        );
        assert!(!preset
            .iter()
            .any(|p| (p.0, p.1) == (0, BUILTIN_VS_PARAMS_BINDING)));
        let (size, offs) = uniform_layout(&m, "vs_params");
        assert_eq!(u64::from(size), BUILTIN_VS_PARAMS_SIZE, "{vs}");
        assert_eq!(
            offs,
            [
                ("texel_size".into(), 0),
                ("pixel_shift".into(), 8),
                ("pad".into(), 12)
            ]
        );
    }
    assert_eq!(
        resources(&module("fixed_color.wgsl")),
        [(0, BUILTIN_VS_PARAMS_BINDING, "vs_params".to_string())]
    );
}

#[test]
fn blur_uniform_block_mirrors_constant_registers() {
    for fs in ["blur1_ps.wgsl", "blur2_ps.wgsl"] {
        let m = module(fs);
        assert_eq!(
            resources(&m),
            [
                (0, 0, "blur".to_string()),
                (1, 0, "tex_main".to_string()),
                (1, 1, "samp_main".to_string()),
            ],
            "{fs}"
        );
        let (size, offs) = uniform_layout(&m, "blur");
        assert_eq!(u64::from(size), BLUR_UNIFORMS_SIZE);
        let want: Vec<(String, u32)> = (0..7).map(|i| (format!("_c{i}"), 16 * i)).collect();
        assert_eq!(offs, want, "{fs}");
    }
}

#[test]
fn textured_fixed_passes_bind_one_texture_in_group_1() {
    for f in [
        "fixed_tex_modulate.wgsl",
        "fixed_tex_modulate_alpha.wgsl",
        "fixed_tex_select.wgsl",
        "text.wgsl",
    ] {
        let r = resources(&module(f));
        assert!(r.contains(&(1, 0, "tex".to_string())), "{f}: {r:?}");
        assert!(r.contains(&(1, 1, "samp".to_string())), "{f}: {r:?}");
        assert_eq!(r.len(), 3, "{f}: {r:?}");
    }
}

#[test]
fn every_vertex_shader_applies_the_d3d9_raster_shift() {
    for f in VERTEX_FILES {
        assert!(source(f).contains(RASTER_FN), "{f}: d3d9_raster differs");
        let m = module(f);
        let ep = entry(&m, ShaderStage::Vertex);
        let calls = ep
            .function
            .body
            .iter()
            .filter(|s| {
                matches!(s, naga::Statement::Call { function, .. }
                    if m.functions[*function].name.as_deref() == Some("d3d9_raster"))
            })
            .count();
        assert_eq!(calls, 1, "{f}: vs_main must call d3d9_raster once");
    }
    // Every vertex shader is listed; the others have no vertex stage.
    for f in FILES {
        let has_vs = module(f)
            .entry_points
            .iter()
            .any(|e| e.stage == ShaderStage::Vertex);
        assert_eq!(has_vs, VERTEX_FILES.contains(&f), "{f}");
    }
}

/// The C++'s 2D projection `D3DXMatrixOrthoLH(2, -2, 0, 1)`
/// (support.cpp:179), verbatim in every fixed-function vertex shader.
const ORTHO_FN: &str = "fn d3d9_ortho2d(p: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(p.x, -p.y, p.z, 1.0);
}";

#[test]
fn fixed_function_vertex_shaders_apply_the_ortho_projection_once() {
    // Task 6.7a: the fixed-function shaders flip y once, before the raster
    // shift; the preset-shader VS (their own C++ vertex shaders), blur and
    // text (D3DX's font sprite, positions already in clip space) do not.
    let fixed = [
        "fixed_color.wgsl",
        "fixed_tex_modulate.wgsl",
        "fixed_tex_modulate_alpha.wgsl",
        "fixed_tex_select.wgsl",
    ];
    for f in VERTEX_FILES {
        let src = source(f);
        assert_eq!(src.contains(ORTHO_FN), fixed.contains(&f), "{f}");
        if fixed.contains(&f) {
            assert!(
                src.contains("o.pos = d3d9_raster(d3d9_ortho2d(v.pos));"),
                "{f}: the shift must follow the projection"
            );
        } else {
            assert!(!src.contains("d3d9_ortho2d"), "{f}");
        }
    }
}

/// A 2-vector (NDC, screen or UV).
type V2 = [f32; 2];

/// CPU mirror of the WGSL `d3d9_raster` ([`RASTER_FN`]).
fn d3d9_raster(clip: [f32; 4], texel_size: [f32; 2], pixel_shift: f32) -> [f32; 4] {
    let s = [
        2.0 * pixel_shift * texel_size[0],
        2.0 * pixel_shift * -texel_size[1],
    ];
    [
        clip[0] + s[0] * clip[3],
        clip[1] + s[1] * clip[3],
        clip[2],
        clip[3],
    ]
}

/// Viewport transform (identical in D3D9 and wgpu): NDC -> screen pixels,
/// y down.
fn screen(ndc: [f32; 2], w: f32, h: f32) -> [f32; 2] {
    [(ndc[0] + 1.0) * 0.5 * w, (1.0 - ndc[1]) * 0.5 * h]
}

/// Attribute of an axis-aligned quad with screen corners `s0` (attr `a0`)
/// and `s1` (attr `a1`), linearly interpolated at screen point `p`; `None`
/// when `p` lies outside the quad (not covered).
fn sample(s0: [f32; 2], s1: [f32; 2], a0: [f32; 2], a1: [f32; 2], p: [f32; 2]) -> Option<[f32; 2]> {
    let mut out = [0.0; 2];
    for k in 0..2 {
        let (lo, hi) = (s0[k].min(s1[k]), s0[k].max(s1[k]));
        if p[k] < lo || p[k] >= hi {
            return None;
        }
        out[k] = a0[k] + (p[k] - s0[k]) / (s1[k] - s0[k]) * (a1[k] - a0[k]);
    }
    Some(out)
}

#[test]
fn raster_shift_reproduces_d3d9_sampling_on_a_4x4_target() {
    let (w, h) = (4.0_f32, 4.0_f32);
    let texel = [1.0 / w, 1.0 / h];
    // A shifted vertex lands +0.5 px right and down on screen.
    let p = d3d9_raster([-1.0, 1.0, 0.0, 1.0], texel, D3D9_PIXEL_SHIFT);
    assert_eq!(screen([p[0], p[1]], w, h), [0.5, 0.5]);
    // With w != 1 the shift still moves NDC by (+1/W, -1/H).
    let q = d3d9_raster([-2.0, 2.0, 0.0, 2.0], texel, D3D9_PIXEL_SHIFT);
    assert_eq!([q[0] / q[3], q[1] / q[3]], [-1.0 + 1.0 / w, 1.0 - 1.0 / h]);

    // Quads built as the C++ builds them: (NDC corners, UV corners).
    let hx = 0.5 / w;
    let hy = 0.5 / h;
    let cases: [(&str, V2, V2, V2, V2); 3] = [
        // warp mesh: uv = ndc mapped to 0..1 plus the C++'s +0.5 texel.
        (
            "warp",
            [-1.0, 1.0],
            [1.0, -1.0],
            [hx, hy],
            [1.0 + hx, 1.0 + hy],
        ),
        // comp grid: sx = (u - 0.5/W)*2 - 1, sy = -((v - 0.5/H)*2 - 1).
        (
            "comp",
            [(0.0 - hx) * 2.0 - 1.0, -((0.0 - hy) * 2.0 - 1.0)],
            [(1.0 - hx) * 2.0 - 1.0, -((1.0 - hy) * 2.0 - 1.0)],
            [0.0, 0.0],
            [1.0, 1.0],
        ),
        // blur quad: v = 0 at the bottom edge.
        ("blur", [-1.0, -1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 1.0]),
    ];
    for (name, n0, n1, a0, a1) in cases {
        let d3d = (screen(n0, w, h), screen(n1, w, h));
        let s0 = d3d9_raster([n0[0], n0[1], 0.0, 1.0], texel, D3D9_PIXEL_SHIFT);
        let s1 = d3d9_raster([n1[0], n1[1], 0.0, 1.0], texel, D3D9_PIXEL_SHIFT);
        let wg = (screen([s0[0], s0[1]], w, h), screen([s1[0], s1[1]], w, h));
        for j in 0..4 {
            for i in 0..4 {
                // D3D9 samples pixel (i, j) at (i, j); wgpu at (i+.5, j+.5).
                let (x, y) = (i as f32, j as f32);
                let want = sample(d3d.0, d3d.1, a0, a1, [x, y]);
                let got = sample(wg.0, wg.1, a0, a1, [x + 0.5, y + 0.5]);
                assert_eq!(got, want, "{name} pixel ({i},{j})");
            }
        }
    }
    // The comp grid covers every pixel (incl. the last column/row) in both.
    let n0 = [(0.0 - hx) * 2.0 - 1.0, -((0.0 - hy) * 2.0 - 1.0)];
    let n1 = [(1.0 - hx) * 2.0 - 1.0, -((1.0 - hy) * 2.0 - 1.0)];
    let s0 = d3d9_raster([n0[0], n0[1], 0.0, 1.0], texel, D3D9_PIXEL_SHIFT);
    let s1 = d3d9_raster([n1[0], n1[1], 0.0, 1.0], texel, D3D9_PIXEL_SHIFT);
    let (a, b) = (screen([s0[0], s0[1]], w, h), screen([s1[0], s1[1]], w, h));
    assert_eq!(
        sample(a, b, [0.0; 2], [1.0; 2], [3.5, 3.5]),
        Some([0.875, 0.875])
    );
}

#[test]
fn fixed_function_alpha_ops_match_the_cpp_stage_states() {
    // COLOROP MODULATE(DIFFUSE, TEXTURE), ALPHAOP SELECTARG1 DIFFUSE.
    assert!(source("fixed_tex_modulate.wgsl")
        .contains("return vec4<f32>(t.rgb * in.color.rgb, in.color.a);"));
    // Sprite mode 4: ALPHAOP MODULATE(DIFFUSE, TEXTURE).
    assert!(source("fixed_tex_modulate_alpha.wgsl")
        .contains("return textureSample(tex, samp, in.uv) * in.color;"));
    assert!(source("fixed_tex_select.wgsl").contains("return textureSample(tex, samp, in.uv);"));
    assert!(source("fixed_color.wgsl").contains("return in.color;"));
}

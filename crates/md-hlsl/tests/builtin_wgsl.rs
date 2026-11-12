//! Task 4.8: the hand-written built-in WGSL passes in `assets/shaders/`.
//!
//! Every file must parse and validate under naga; every vertex shader must
//! read the single Task 5.2 `Vertex` layout (F30); `warp_vs`/`comp_vs` must
//! produce exactly the `VsOut` a translated preset fragment shader reads;
//! the blur and parameter blocks must have the sizes/offsets
//! `md_hlsl::layout` publishes for md-render.

mod common;

use common::asset;
use md_hlsl::layout::{BLUR_UNIFORMS_SIZE, BUILTIN_VS_PARAMS_BINDING, BUILTIN_VS_PARAMS_SIZE};
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
    "fixed_tex_select.wgsl",
    "text.wgsl",
    "title.wgsl",
];

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
        ("fixed_tex_select.wgsl", "fixed_tex_select.wgsl"),
        ("text.wgsl", "text.wgsl"),
        ("title.wgsl", "title.wgsl"),
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
    for vs in [
        "warp_vs.wgsl",
        "comp_vs.wgsl",
        "blur_vs.wgsl",
        "fixed_tex_modulate.wgsl",
        "fixed_tex_select.wgsl",
        "text.wgsl",
        "title.wgsl",
    ] {
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
                ("half_texel".into(), 8),
                ("pad".into(), 12)
            ]
        );
    }
    assert!(resources(&module("fixed_color.wgsl")).is_empty());
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
        "fixed_tex_select.wgsl",
        "text.wgsl",
        "title.wgsl",
    ] {
        let r = resources(&module(f));
        assert!(r.contains(&(1, 0, "tex".to_string())), "{f}: {r:?}");
        assert!(r.contains(&(1, 1, "samp".to_string())), "{f}: {r:?}");
        assert_eq!(r.len(), 3, "{f}: {r:?}");
    }
}

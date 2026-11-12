# assets/hlsl

HLSL text that MilkDrop pastes in front of (or around) preset shaders. These are
data, not code, and are kept byte-for-byte as they appear upstream.

Origin: `code/resources/Milkdrop2/data/` in this repository (MilkDrop 2 by
Nullsoft, Inc.; BeatDrop/MilkDrop3 changes on top). License: Nullsoft's
3-clause BSD license, the same one reproduced in the top-level `NOTICE`.

| File | Use |
|---|---|
| `include.fx` | preamble pasted before every preset shader (uniforms, macros, samplers) |
| `warp_ps.fx`, `comp_ps.fx` | default warp / composite pixel-shader bodies (fallbacks) |
| `warp_vs.fx`, `comp_vs.fx`, `blur_vs.fx` | vertex shaders (reference for hand-written WGSL) |
| `blur1_ps.fx`, `blur2_ps.fx` | blur pass pixel shaders (reference for hand-written WGSL) |

Do not reformat these files; tests (`crates/md-hlsl/tests`) read `include.fx` directly.

### Task 6.2: Built-in waveform

**Files:**
- Create: `crates/md-render/src/wave.rs`

**Interfaces:**
- Produces: `pub fn build_wave(mode: u32, sound: &SoundData, p: &WaveParams, aspect: (f32,f32), time: f32, out: &mut Vec<Vertex>) -> Topo` for modes 0..8 (circle, x-y spectrum, centered spiro, derivative, explosive hash, two spectrum lines, line, double line, spectrum line) with flags `256`, `512`, `1024`, `2048` as in `DrawWave`'s `switch`; `WaveParams { r, g, b, a, x, y, mystery, use_dots, thick, additive, maximize_color, mod_alpha_by_volume, mod_alpha_start, mod_alpha_end, scale, smoothing }`; alpha modulation by volume `alpha *= (vol - start) / (end - start)` clamped, `maximize_color` scales rgb so the max channel is 1; smoothing `wave[i] = wave[i] * (1 - s) + wave[i-1] * s` after scaling by `scale / 128`; `thick` draws four offset copies as the C++ `draw fat dots` loop; `use_dots` draws points.
- `Renderer::draw_wave(ctx, ...)` draws with `FixedMode::Color` and `BlendMode::Additive` when `additive` else `Alpha`.

- [ ] **Step 1:** Write failing tests `mode_0_circle_vertex_count_is_samples_plus_one`, `mode_6_line_spans_width`, `alpha_modulation_clamps`, `maximize_color_scales_max_channel_to_one`, `thick_quadruples_vertices`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement from the C++ geometry per mode (the implementer transcribes each mode's vertex formulas into Rust with the C++ line reference in a comment).
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(render): built-in waveforms`


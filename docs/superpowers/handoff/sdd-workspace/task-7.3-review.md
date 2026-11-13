### Spec Compliance
✅ Interfaces: `eval_custom_waves(&mut PresetState, &FrameInputs)` / `eval_custom_shapes(..)` (waves.rs:44, shapes.rs:38), exported lib.rs:24,26; brief's four tests present. Departure "no `sound` arg" (= `inputs.sound`) accepted: no second source of truth.
✅ (a) Shapes: loop `instance < instances` (mfs:2264) = `0..instances.max(0)` (shapes.rs:85); every instance reloads inputs, q from `*pState->var_pf_q` (mfs:2413), t from `t_values_after_init_code` (mfs:2415), the shape's file values, `num_inst`/`instance` (mfs:2425-2426) = shapes.rs:86-101; save-back commented out (mfs:2276-2286) so t restarts per instance and frame; user vars persist (Vm not reset). Draw reads only var_pf_* (sed of mfs:2300-2398: x y rad ang tex_* r..border_a textured thick), all returned (shapes.rs:105-128).
✅ (b) Waves: load (mfs:2442-2463) = waves.rs:74-87; pp read-only inputs copied from the pre-program pf values (mfs:2530-2539, before 2541) = waves.rs:91-93; pf program then q/t pf->pp (mfs:2541-2546) = waves.rs:94-102. Per point only sample/value1/value2, x=0.5f+value1 (float sum), y, and r g b a from the per-frame RESULT `var_pf_r..a` (mfs:2598-2606) = waves.rs:120-128; report's departure 3 is correct C++ parity. q/t/user vars carry point to point; pp t restarts each frame via the pf copy (test t_flows_... pins it).
✅ (c) Draw condition `nSamples>=2 || (bUseDots && nSamples>=1)` after `min(512,(int)samples)` (mfs:2548-2551) is inside md-render's `custom_wave_inputs` (custom_waves.rs:57-60, `c_int(..).min(512)`); empty -> wave omitted (waves.rs:114-116), which matches the C++ drawing nothing. Per-frame program and q/t copy still run first, as in C++.
✅ (d) q read from `state.pf` (waves.rs:46, shapes.rs:39): C++ resets var_pf_q to q_init (mfs:492), runs per-frame in RunPerFrameEquations (mfs:873, both states 623-630), only per-vertex q is derived after (mfs:674), nothing writes var_pf_q before DrawCustomShapes/Waves (mfs:1054-1055). Correct given run_per_frame first.
✅ (e) Disabled skipped (mfs:2234/2517 = shapes.rs:51, waves.rs:49). Compile failure: C++ leaves the handle NULL and the wave/shape enabled (state.cpp ~1772-1786, 1806); `NSEEL_code_execute(NULL)` returns (nseel-compiler.c:1664) = `Option<Program>` None no-op; test failed_programs_behave_as_no_ops.
✅ wave_scale = own preset's `wave_scale.eval(-1.0)` (waves.rs:45; mfs:2555); old state uses its own. Casts: samples via md-render `c_int`; shape sides/additive/textured/thick via `c_int` in build_shape (md-render shapes.rs:104,159,179-180); md-core casts nothing.
✅ (f) Per point: 9 `set`, one run, 6 `get` through pre-resolved VarRefs; no lookup or allocation (waves.rs:119-137).
✅ Citations sed-checked: 2264, 2267, 2270-2273, 2276-2286, 2288-2290, 2400-2440, 2413, 2415, 2425-2426, 2442-2463, 2457, 2496, 2517, 2530-2539, 2541, 2543-2546, 2548-2551, 2598-2606, 2601-2602, 2609, 2612-2619, 2624-2633, 2680 all land on the cited code.

### Strengths
- Exact C++ ordering subtleties preserved (pp inputs before the pf program; r g b a from the pf result; per-frame run even for an undrawn wave).
- Tests pin real behaviour with exact values: t reset per instance/frame, user-var persistence, q from preset result, 512 cap/truncation/dots, own wave_scale vs md-render output.

### Issues
#### Critical
None.
#### Important
None.
#### Minor
1. Entry-point guidance for 7.9 omits the cross-state order that shared globals make observable (report "Entry point"; ledger note 440; waves.rs:34-37, shapes.rs:29-32). reg00..reg99 are VM-global (nseel-eval.c:145, 201, 248) and gmegabuf is process-wide (md-eval mem.rs:10-13), so the C++ order is: per-frame new, old (mfs:623-630) -> per-vertex mesh -> shapes new, old (mfs:1054, 2224-2227) -> waves new, old (mfs:1055, 2507-2510). The report's "same sequence on the OLD state" and "run_per_frame then eval_custom_waves/eval_custom_shapes" suggest per-state grouping and waves before shapes. Controller: amend note 440 to state this order.
2. Duplicated 10-value input array in waves.rs:53-64 and shapes.rs:41-52 (and waves.rs rebuilds it per wave, loop-invariant); a shared helper would do.
3. `out` Vecs without capacity (shapes.rs:50, waves.rs:47); per-wave allocations in `custom_wave_inputs` (md-render, already deferred in 6.3/6.4).
4. wave_scale test (custom_eval_tests.rs:113) cannot tell blended from unblended (no blend set up); the code is right by construction (`eval(-1.0)`).
5. One-sample dot wave gives `sample` = NaN (0 * 1/0, mfs:2592-2595, mirrored); the test checks only the count. Acceptable, undocumented.

### Assessment
**Task quality:** Approved — **Reasoning:** Every evaluation rule I checked (per-instance reload, t/q flow, per-point reset set, draw condition, casts, compile failure) matches the C++ line for line, and the tests pin each one. The only substantive item is the cross-state call-order note for 7.9, which belongs in the ledger.

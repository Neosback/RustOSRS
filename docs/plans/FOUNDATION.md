# Foundation: structure, debugging, performance, scripting

Goal: a base that is easy to extend, easy to debug and fast, without touching the parity-critical
core. This records the current layering, what to adopt (`glam`, indirect drawing, `mlua`) and
what not to, with the measurements that justify each call.

## 1. Layers (today) and the rules between them

```
osrs-core     pure client semantics (fixed-point maths, lighting, animation stepping, definitions)
osrs-cache    cache + decoders                      (IO, no rendering)
osrs-scene    scene building: terrain, placement, normals   (pure, deterministic)
osrs-world    cache-backed scene assembly, streaming, animation worker, picker data
osrs-render   wgpu renderer (Reference profile), geometry packing, overlays, timing
osrs-editor   eframe app: chrome, camera, picking UI, plugins, settings
osrs-reference  oracle/fixture runner (tests, tools)
```

Rules that keep it extensible:
1. Dependencies point downward only. `osrs-core`/`osrs-scene` never see `wgpu`, `egui` or the filesystem.
2. Everything parity-critical stays integer/fixed-point exactly like the client. Float maths belongs to
   presentation (camera, picking, overlays, HD shading).
3. The editor reaches optional features only through `EditorPlugin` (`plugin.rs`): menus, Properties
   sections, overlays, hotkeys, guide lines, persisted settings. `app.rs` knows about no specific tool.
4. Anything slow runs off the render thread behind a small worker facade (`RegionStreamer`,
   `AnimationWorker`). The render thread only uploads.
5. Non-reference behaviour is always an explicit, named option, never silent.

## 2. Debugging toolkit (exists / to add)

Exists: per-phase frame timings and worst-frame readout, GPU scene timing (timestamp queries), picker
data dumps (copy object / tile / neighbourhood), `render_frame` with `RENDER_AT`, ~40 probe examples,
`anim_census`, oracle diff tests, `RUSTOSRS_*` smoke hooks (teleport, autofly, vsync, settings path).

To add, in order of payoff:
1. `tracing` spans around region build, streaming, animation rebuild, upload (feature `profiling` -> Tracy
   or `tracing-chrome`), so a hitch is a timeline instead of a guess.
2. A pixel-diff harness: `render_frame` at fixed cameras -> PNG -> compare against stored goldens with a
   tolerance report. This is the safety net for every renderer refactor below.
3. Debug views in the Shaders section: bias, UV, normals, overdraw, alpha-model order (RuneLite has
   `ZBUF_DEBUG` / `BIAS_DEBUG` / `FRAG_UVS` switches to copy).
4. A command log (every `EditorCommand` and edit) for reproducing a session.

## 3. Evaluations

### glam: adopt, at the boundaries
`gpu/mod.rs` and `pick.rs` hand-roll 4x4 matrices and ray maths. `glam` (`Mat4`, `Vec3`, `DMat4` for the
f64 picking path) removes ~300 lines of error-prone code and gives SIMD for free. Do **not** use it in
`osrs-core`/`osrs-scene` (integer client maths must not drift). `bytemuck` interop is built in. Cost: one
dependency, no behaviour change; verify with the pixel-diff harness.

### Indirect drawing: not yet, with a trigger
Measured (release, M-series, vsync off): at rest CPU 2-3 ms/frame, GPU scene pass 1.1-2.7 ms for ~5 M
vertices in ~1,100 zones; the earlier slowdowns were animation rebuilds, fixed by the worker thread. Draw
calls are not the bottleneck. wgpu's `MULTI_DRAW_INDIRECT` is not native on Metal (it falls back to a loop),
so the win on this machine would be smaller than on Vulkan/DX12.
Plan when needed (trigger: `encode` > 3 ms or > ~5k draws, e.g. a 7x7 region load radius):
1. slab-allocate zone vertex data (Bevy `mesh/allocator.rs` is the model) so a region is a few buffers,
2. keep per-zone origin in a storage buffer indexed by `instance_index`,
3. build `DrawIndexedIndirectArgs` on the CPU first (still one `draw_indexed_indirect` per slab), and move
   culling to a compute pass only if CPU culling shows up in the profile.
Gate each step with the pixel-diff harness; the alpha pass already uses a per-frame index buffer.

### mlua (Lua) scripting: yes, after the command API settles
Use case: batch edits, the auto-tiler's rule sets, user plugins, test scenarios. Design so scripts see the
**same surface as Rust plugins**: read-only `EditorView` queries + `EditorCommand`s, never internals.
* crate: `mlua` with `lua54`, `vendored`, `send`, `serialize` (no system Lua; one thread owns the `Lua`).
* sandbox: load only the safe stdlib subset, instruction-count hook for runaway loops, no `io`/`os` by default.
* loading: `~/Library/Application Support/RustOSRS/scripts/*.lua`, hot reload via a file watcher.
* alternative: `rhai` is pure Rust and easier to sandbox but has a smaller ecosystem; pick Lua for
  familiarity and speed, revisit if embedding friction appears.
Prerequisite: the edit/command layer with undo (section 4); scripting before that would freeze a bad API.

## 4. Next structural pieces (no new tools yet)
1. **Edit model**: `EditCommand` (do/undo, serialisable) over a region-scoped world (`MapEdit` of tiles,
   locs); a pure function from (region data, edits) to region data feeding the existing build pipeline.
2. **Selection model**: multi-select with tile and loc targets, shared by picker, outliner and tools.
3. **Viewport tool trait** (activate/handle input/draw overlay/commit) on top of `EditorPlugin`.
4. **Asset/definition provider trait** so the editor can read the pinned cache, a server cache, or a
   work-in-progress cache the same way (OpenRune server projects use packed caches).

# RustOSRS GPU Data and Render Pass Contracts

Status: **Checkpoint 5 normative renderer blueprint**  
Decision class: `PROJECT_DECISION`

This document defines the renderer-facing ABI between extracted OSRS scene data and `wgpu`. It is deliberately not an OSRS semantic specification.

## 1. Principles

1. GPU formats are disposable derivatives.
2. Semantic face metadata survives extraction until the renderer has consumed it.
3. Static batching must not erase the information required for parity diagnostics.
4. Dynamic ordering must not mutate the canonical mesh.
5. The renderer must be able to rebuild every GPU resource from the current render extraction snapshot.

## 2. Canonical render mesh

Before GPU packing, a renderer-owned `RenderMesh` representation must be able to express:

- positions in render-local `f32` coordinates converted from exact semantic integer coordinates;
- triangle indices;
- color/HSL payload required by the current render profile;
- semantic texture ID/material handle;
- UV or texture-triangle inputs;
- per-face alpha;
- per-face priority or model default priority;
- authored face bias;
- semantic/renderable ID;
- optional normals/tangents for diagnostics/enhanced presentation;
- flags such as hidden/suppressed face state already resolved by semantic construction.

No field above may be discarded solely because the first GPU pipeline does not use it yet.

## 3. Position conversion

Semantic model/local coordinates remain integer through semantic construction.

Render extraction converts them once to `f32` at the GPU boundary.

Rules:

- one semantic local unit maps to one render-space unit by default;
- scene-origin rebasing is allowed to keep viewport-visible values numerically small;
- rebasing must be represented as an explicit per-scene/per-zone origin, never baked back into semantic coordinates;
- extraction must not apply hidden half-tile offsets or corrective rotations.

A later global render scale may be introduced only if the conversion is explicit and tested.

## 4. Vertex formats

The renderer uses a small family of explicit vertex formats instead of one universal packed struct.

### 4.1 Static scene vertex

Minimum logical fields:

```text
position      : vec3<f32>
color_or_hsl  : u32
material_id   : u32
uv            : vec2<f32>
face_meta     : u32
```

`face_meta` may pack renderer-consumed alpha/bias/diagnostic bits, but the CPU-side extraction retains the full semantic fields independently.

### 4.2 Dynamic/ordered vertex

Uses the same logical material/color contract as static geometry so static and dynamic paths shade consistently.

Dynamic geometry may use a separate GPU buffer layout optimized for frequent upload.

### 4.3 Diagnostic vertex streams

Normals, wireframe edges, selection bounds, gizmos, and similar diagnostics use separate buffers/pipelines. They must not force production static geometry to carry editor-only attributes.

## 5. Indexing

Indexed triangle rendering is the default.

The render compiler may de-index geometry when profiling shows a material performance advantage, but must retain object/face provenance through side tables.

Canonical GPU-facing winding is normalized to **counter-clockwise front faces**.

Any source-space conversion required to achieve that normalization happens exactly once during render extraction. Asset-specific mirroring remains semantic behavior under `MODEL-BUILD-*`; the renderer does not use negative-scale instance transforms as a substitute.

## 6. Material table

Semantic texture IDs map through a renderer-owned material table.

A material entry can contain:

- source semantic texture ID;
- texture page + layer;
- sampler class;
- animation vector/speed metadata;
- reference brightness/sampling inputs;
- enhanced-profile sampling options;
- debug/provenance identity.

No fixed semantic assumption such as `TEXTURE_COUNT = 256` is allowed.

## 7. Texture storage

The baseline GPU texture system uses **paged 2D texture arrays**.

Reasons:

- OSRS reference textures are naturally same-dimension sampled images in the audited renderer path;
- array layers avoid atlas bleeding and preserve wrap/animation behavior;
- paging avoids treating one historical texture-count constant as a universal limit;
- page size can respect adapter limits.

The material table maps semantic texture IDs to page/layer coordinates.

If an adapter cannot support the preferred layout, initialization must either select a documented compatible fallback or fail with a capability diagnostic. It must not silently drop textures.

## 8. Samplers

At minimum the renderer supports:

- reference-compatible sampler behavior;
- enhanced sampler behavior with anisotropic filtering when supported.

Sampler choice belongs to render profile, not semantic texture identity.

## 9. Per-frame uniform groups

Separate uniform/storage groups should distinguish update frequency.

### Frame globals

Typical contents:

- viewport size;
- deterministic render tick;
- render-profile flags;
- diagnostic mode;
- fog/presentation values where enabled.

### Camera globals

- view matrix;
- reverse-Z projection matrix;
- camera position;
- scene-origin rebase;
- near-plane/reference projection inputs.

### Draw/instance data

- object transform when not pre-baked;
- semantic/pick handle;
- material/page selection when not per-vertex;
- tint/override state when explicitly supported;
- plane/visibility state if needed by the draw path.

## 10. Shader organization

WGSL source is organized by responsibility, for example:

```text
shaders/
  common/
    coordinates.wgsl
    hsl.wgsl
    material.wgsl
    depth_bias.wgsl
  scene/
    static.vert.wgsl
    static.frag.wgsl
    dynamic.vert.wgsl
    dynamic.frag.wgsl
  picking/
    pick.vert.wgsl
    pick.frag.wgsl
  diagnostics/
    priority.wgsl
    alpha.wgsl
    normals.wgsl
    depth.wgsl
```

Generated constants or material tables may be injected at build/runtime, but the shader preprocessing system must remain small and deterministic.

A source shader from RuneLite is reference evidence, not a file to transliterate mechanically.

## 11. HSL/color boundary

Reference mode preserves the audited OSRS/RuneLite color payload semantics.

HSL-to-RGB conversion may occur in the shader for efficiency, provided:

- packed values match the semantic/model result exactly;
- conversion is tested against the pinned reference helper/table;
- textured-face behavior follows the selected reference profile;
- enhanced color processing is separately switchable.

The renderer must not redo semantic model lighting from normals in reference mode.

## 12. Reverse-Z depth

The baseline depth target is `Depth32Float` where supported.

Reverse-Z conventions:

```text
depth clear      = 0.0
near              -> larger depth
far               -> smaller depth
depth compare     = GreaterEqual
```

Projection maps to wgpu's `0..1` NDC depth convention.

`GreaterEqual` is chosen rather than strict `Greater` to tolerate intentional equal-depth replay between compatible passes while authored priority/bias remains explicit.

Depth writes are enabled for opaque scene geometry and selectively disabled for translucent passes according to the pass contract.

## 13. Authored face bias

The renderer preserves semantic `faceBias` as its own value.

Reference profile adopts the audited RuneLite-style clip-depth adjustment as the initial parity strategy:

```text
clip_position.z += face_bias / 128.0
```

applied before perspective division and under reverse-Z, where larger bias wins.

This is a renderer decision, not an OSRS cache semantic.

The implementation must have a dedicated bias fixture because projection changes can make an apparently small formula materially different.

Enhanced profile may use a numerically improved equivalent only when it preserves required ordering and can be compared against reference mode.

## 14. Culling

Canonical raster state:

```text
front face = CCW
cull mode  = Back
```

Render extraction normalizes source triangles into that convention once.

Required validation includes:

- ordinary model triangle;
- all four loc orientations;
- mirrored model;
- terrain shape rotations;
- type-4 diagonal-decoration path.

Two-sided diagnostic rendering may disable culling, but production reference rendering must not rely on double-sided rasterization to hide winding errors.

## 15. MSAA

Renderer support target:

- 1x mandatory;
- 4x preferred when adapter/format support allows;
- other sample counts optional.

Reference screenshot fixtures default to 1x unless the fixture explicitly tests multisampling.

Enhanced editor presentation defaults to 4x when supported.

## 16. Opaque pass

Opaque static geometry is drawn from zone-compiled ranges.

Opaque dynamic geometry is drawn after static opaque geometry unless a reference ordering contract requires it to participate in an ordered packet.

Opaque pass state:

- reverse-Z depth test enabled;
- depth writes enabled;
- blending disabled;
- backface culling enabled;
- authored face bias active.

Coplanar behavior is not delegated to unspecified driver polygon offset.

## 17. Ordered face path

Any renderable requiring reference face ordering retains immutable face records and an indexable vertex source.

Per frame, CPU ordering produces an **ordered index/draw stream**, not a mutated canonical mesh.

The ordering implementation must be capable of reproducing `FACE-002` reference behavior:

1. depth bucket visible faces;
2. preserve priority queues 0..11;
3. compute `(1,2)`, `(3,4)`, `(6,8)` average thresholds;
4. interleave priority 10/11 at the reference boundaries;
5. preserve far-to-near order within required queues.

The implementation may use scratch index buffers, indirect draws, or another mechanism as long as exact crafted fixtures pass.

## 18. Static priority-sensitive content

Static compilation must not erase priority metadata.

The baseline classification is:

- ordinary opaque static faces that are reference-safe under depth+bias remain in static zone ranges;
- faces/models whose appearance depends on camera-relative priority ordering are registered as ordered renderables even if their geometry itself is static;
- transparent static content remains available to the alpha ordering system.

This hybrid avoids rebuilding all map geometry each frame while retaining exact behavior where ordering is load-bearing.

## 19. Transparency pass

Transparency is a dedicated ordered pass.

Baseline policy:

- depth test remains enabled under reverse-Z;
- depth writes disabled for conventional blended faces;
- source alpha semantics preserved;
- draw order is far-to-near according to the owning reference ordering contract;
- ordered priority behavior is resolved before batching can reorder faces;
- blend state is explicit and covered by golden fixtures.

Opaque/alpha split is an optimization after semantic classification, not a replacement for face-priority rules.

## 20. Terrain pipelines

Terrain uses the topology already produced by semantic extraction.

Two GPU input cases exist:

### Flat paint

- four semantic corner positions/heights;
- two exact triangles from `TERRAIN-002`;
- full-tile UV convention when textured.

### Shaped tile model

- exact vertices/faces from `TERRAIN-001`;
- face material/texture assignment preserved;
- tile-local UV derivation from semantic vertex X/Z where required by the reference profile.

The GPU does not regenerate terrain shape templates.

## 21. Object model pipeline

Object render meshes arrive after:

- model selection;
- mirroring;
- transform order;
- recolor/retexture;
- contouring when applicable;
- static normal reconciliation/final lighting where applicable;
- dynamic animation/morph model generation when applicable.

The renderer applies scene placement and renderer-only transforms such as camera/view projection. It does not rerun semantic model construction.

## 22. Texture animation

Texture animation is driven by an explicit deterministic render tick.

Material preparation maps decoded animation direction/speed to a UV velocity.

Shader animation uses the material UV velocity and tick. Pausing/scrubbing the editor can freeze or set the tick exactly.

Wall-clock time must not be the only animation source because reproducible golden frames require deterministic time.

## 23. Picking target

Preferred pick attachment:

```text
R32Uint
```

Each visible draw writes a renderer pick ID associated with the current extraction generation.

A side table maps that ID to:

- semantic scene handle;
- optional face/tile/subcomponent data;
- provenance required by diagnostics.

One-pixel or small-region readback uses a staging buffer asynchronously.

## 24. Diagnostic passes

Diagnostic modes reuse the same scene visibility and geometry whenever practical.

Examples:

- priority heat map consumes face priority;
- alpha view consumes face alpha;
- bias view consumes authored bias;
- depth view samples/reconstructs reverse-Z depth;
- normals view uses retained normal buffers;
- zone view uses compiled zone bounds/generation metadata.

A diagnostic shader must not require changing semantic scene values.

## 25. Pipeline cache

Renderer pipelines are keyed by explicit state such as:

- render profile;
- color/depth format;
- sample count;
- opaque/alpha/picking/diagnostic pass;
- texture page binding strategy;
- feature flags that materially change shader interfaces.

Pipeline cache keys must not include semantic asset IDs.

## 26. Capability policy

Initialization records adapter limits/features needed by the selected renderer path.

Required baseline capabilities should stay within broadly supported native wgpu features.

Optional features may improve performance but cannot be required for semantic correctness.

Examples of optional future acceleration:

- multi-draw indirect;
- indirect first-instance;
- timestamp queries;
- compute culling;
- GPU-driven ordered packet generation.

If absent, the CPU/reference path remains valid.

## 27. Validation requirements

The final renderer implementation must validate at least:

- static and dynamic shaders against the same material fixture;
- CCW/culling under ordinary, mirrored, and rotated geometry;
- reverse-Z near/far ordering;
- equal-depth + authored-bias ordering;
- priority 0..11 exact crafted outputs;
- alpha blend/order cases;
- texture page/material mapping;
- UV reconstruction cases;
- deterministic texture animation ticks;
- 1x reference screenshots and 4x enhanced presentation separately;
- picking IDs under hidden/visible/bridge-linked tiles.

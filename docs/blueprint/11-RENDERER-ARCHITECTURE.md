# RustOSRS Rust/wgpu Renderer Architecture

Status: **Checkpoint 5 normative renderer blueprint**  
Decision class: `PROJECT_DECISION`

This document defines how `osrs-render` turns a canonical semantic scene into frames without taking ownership of OSRS behavior.

The renderer is a compiler and presentation system. It is never the source of truth for loc placement, model selection, terrain topology, morphs, normal merging, lighting semantics, bridges, face metadata, or other contracts in `docs/specs/`.

## 1. Goals

The renderer must provide:

- native Rust + `wgpu` rendering suitable for an eframe/egui editor viewport;
- deterministic reference/parity rendering for verification;
- high-quality editor presentation without corrupting parity semantics;
- large-scene performance through chunk-aligned static compilation and dirty rebuilds;
- correct handling of semantic face priority, alpha, textures, UVs, authored face bias, bridges, and dynamic models;
- explicit GPU/device failure handling and recoverable resource rebuilds;
- render diagnostics as first-class product capabilities;
- reusable lower-level rendering suitable for a future non-editor OSRS application.

## 2. Non-goals

`osrs-render` does not:

- decode cache bytes;
- choose fallback models when semantic model selection returns none;
- resolve object morphs;
- perform semantic loc dispatch;
- repair bad footprints or bridge planes;
- perform OSRS cross-model normal reconciliation after extraction;
- mutate the canonical scene because a GPU optimization prefers another representation;
- depend on egui widget types.

## 3. Pipeline

The required data flow is:

```text
Canonical semantic scene
        |
        v
Render extraction snapshot
        |
        v
Render-world compiler
   |             |
   |             +--> dynamic/ordered renderables
   v
static zone artifacts
        |
        v
GPU resource cache
        |
        v
frame preparation
        |
        +--> visibility / plane / roof filters
        +--> ordered-face preparation
        +--> picking request
        |
        v
wgpu render graph
        |
        v
viewport texture
```

The semantic scene can survive complete destruction and recreation of every layer below it.

## 4. Render extraction snapshot

`osrs-render` consumes an immutable snapshot generated from semantic scene state.

A snapshot must carry enough information to render and diagnose the scene without calling back into mutable editor state during a frame.

Minimum snapshot content:

- scene generation/revision number;
- scene/world origin and explicit coordinate-space metadata;
- terrain surfaces and already-resolved topology;
- static and dynamic scene instances;
- resolved model geometry or immutable model handles;
- semantic object/instance identifiers;
- storage plane, render/height level, and linked-below relationships where required for filtering;
- face colors/HSL values required by the selected render profile;
- face textures and texture-coordinate inputs;
- face alpha/transparency;
- face priority/default priority;
- authored face bias;
- normal data retained for diagnostics or enhanced presentation when available;
- material/texture animation inputs;
- roof/visibility grouping only when represented as explicitly derived render state.

The extraction boundary is the last place semantic Rust types may be transformed into renderer-oriented structures.

## 5. Immutable generations

Every extracted renderable and zone artifact is associated with a semantic generation.

Rules:

1. a GPU artifact is valid only for the extraction generation it was compiled from;
2. a later semantic edit never mutates an old artifact in place as source truth;
3. asynchronous compilation may finish late, but stale results are discarded by generation comparison;
4. frame rendering may continue from the latest complete generation while a newer dirty zone compiles;
5. save/export never reads renderer artifacts.

This prevents editor commands, worker threads, and GPU uploads from racing semantic state.

## 6. Static versus dynamic classification

The renderer distinguishes **render stability**, not semantic importance.

### Static renderables

A renderable is eligible for zone compilation when its extracted geometry/material state does not require camera- or time-dependent CPU geometry regeneration for the current profile.

Typical static content:

- terrain;
- non-animated finalized loc models;
- finalized wall and wall-decoration models;
- static floor decorations;
- other scene geometry whose semantic model is stable.

### Dynamic/ordered renderables

A renderable remains in the per-frame/dynamic path when it requires one or more of:

- animation-frame geometry;
- current morph/preview state regeneration;
- camera-dependent reference face ordering;
- transparency ordering not safely representable by static batches;
- temporary editor preview/ghost geometry;
- selection/gizmo overlays;
- profile-specific behavior requiring dynamic reconstruction.

Static/dynamic classification must not change the semantic model used to create the renderable.

## 7. Zone compilation

The primary static compilation unit is an **8x8 tile zone**, aligned with the OSRS chunk size.

Reasons:

- it matches natural map/chunk granularity;
- edits invalidate a bounded area;
- GPU buffers remain reasonably sized;
- scene streaming can load/unload zones independently;
- plane/roof range metadata can be represented per zone;
- it mirrors a proven RuneLite strategy without adopting RuneLite's buffers as semantic truth.

Zone size is renderer policy and may be superseded by ADR if profiling demonstrates a better unit.

### Zone artifact contents

A compiled zone may contain:

- opaque static vertex/index ranges;
- alpha/ordered static geometry references;
- per-plane draw ranges;
- derived roof/visibility ranges;
- material-page usage;
- semantic object-to-range lookup for diagnostics/picking;
- bounds for frustum culling;
- generation and dirty state.

A zone artifact must never be the only copy of semantic object placement or model metadata.

## 8. Dirty propagation

Semantic changes produce typed invalidation events.

Examples:

- terrain height/shape edit -> affected zone plus neighbor border dependencies;
- loc add/remove/rotate -> owning zone and any zones affected by footprint/bounds;
- semantic normal-finalization change -> every zone containing an affected finalized model;
- material definition change -> dependent material resources and zones only when baked data changes;
- texture animation tick -> no semantic/zone rebuild;
- camera move -> no semantic/zone rebuild;
- roof visibility toggle -> no geometry rebuild when ranges already exist.

The renderer owns dependency indexes needed to map semantic invalidations to disposable artifacts.

## 9. Threading and ownership

The architecture assumes three responsibility domains:

### Semantic/editor thread

Owns document mutations and publishes immutable semantic generations.

### Extraction/compile workers

May perform CPU-heavy work such as:

- zone extraction;
- vertex/index packing;
- static range generation;
- bounds generation;
- ordered-face scratch preparation;
- texture decode/preparation if the cache/material layer has provided immutable source pixels.

Workers do not own `wgpu::Device` state as semantic truth and do not mutate the editor document.

### Render/GPU owner

Owns:

- `wgpu::Device`;
- `wgpu::Queue`;
- render pipelines;
- bind groups;
- GPU buffers/textures;
- frame encoders;
- swapchain/viewport targets;
- upload retirement/fence bookkeeping.

CPU compilation results are transferred to this owner for resource creation/update.

## 10. GPU resource lifecycle

GPU objects are cached by stable renderer keys plus generation.

Required resource classes:

- static zone vertex/index buffers;
- dynamic/ordered scratch buffers;
- material/texture pages;
- material metadata buffers;
- frame/camera uniform buffers;
- instance/draw metadata;
- depth target;
- color target;
- picking ID target;
- optional MSAA target;
- diagnostic buffers.

Resource eviction must be safe under in-flight frames. Replacing a zone creates a new resource generation and retires the prior allocation only after it is no longer referenced.

## 11. Device loss and surface failure

GPU failure is recoverable where `wgpu` permits it.

On device/resource recreation:

1. preserve the semantic scene and editor document;
2. discard GPU resources;
3. recreate pipelines and global material resources;
4. rebuild visible/required zones from the latest extraction generation;
5. surface a structured diagnostic if adapter/device capabilities changed.

A GPU reset must never require re-importing the map.

## 12. Render profiles

The renderer supports two explicit presentation profiles.

### Reference profile

Purpose: parity testing and forensic comparison.

Characteristics:

- semantic baked lighting/color is authoritative;
- no extra physically based relighting;
- deterministic texture-animation tick input;
- deterministic camera/projection inputs;
- authored face metadata preserved exactly;
- optional presentation effects disabled unless specifically part of the reference test;
- MSAA normally disabled for pixel-comparison fixtures unless the fixture specifies it.

### Enhanced editor profile

Purpose: best editor usability and visual quality.

May enable:

- MSAA;
- anisotropic filtering;
- smoother camera motion;
- optional enhanced lighting/normal visualization;
- fog/presentation improvements;
- high-quality texture sampling;
- overlays and diagnostics.

Enhanced features must not mutate semantic geometry/material state and must be switchable away when reference behavior is required.

## 13. eframe/egui boundary

`osrs-render` exposes a renderer-facing viewport API, not egui widgets.

The eventual editor integration owns:

- viewport rectangle and DPI;
- input routing;
- requesting a frame;
- presenting the resulting texture through egui/eframe;
- converting pointer positions into viewport pixel coordinates;
- initiating pick requests.

`osrs-render` owns the render target and can expose a texture/view handle through a narrow integration adapter in the application layer.

This avoids coupling reusable renderer code to editor layout.

## 14. Frame preparation

A frame consumes:

- immutable render-world generation;
- camera matrices/viewport;
- current render profile;
- current plane/roof visibility mask;
- deterministic animation tick/time state;
- diagnostic mode;
- optional pick request.

Frame preparation produces a draw plan. It does not modify semantic state.

## 15. Render graph

The baseline render graph is:

```text
1. upload/prepare transient ordered data
2. opaque terrain/static pass
3. opaque dynamic/ordered pass
4. alpha/ordered pass
5. editor overlays/diagnostics
6. optional picking pass (or shared geometry replay)
7. resolve MSAA when enabled
8. expose viewport color target
```

Exact pass splitting may be optimized later, but the observable ordering rules in the face/material specifications must remain testable.

## 16. Lighting boundary

Reference-mode object lighting is already a semantic/model-build result.

The renderer must not silently reinterpret `LIGHTING-001` using a modern directional light.

If enhanced lighting is offered:

- it is renderer policy;
- it consumes retained normals/raw material information when available;
- it must not overwrite reference HSL/color data;
- reference mode remains available without double-lighting.

## 17. Visibility and roofs

Plane filtering and roof removal are derived presentation state.

They operate on:

- semantic plane/render-level metadata;
- derived roof groups/ranges;
- linked-below relationships;
- viewport/editor visibility configuration.

They do not rewrite semantic plane identity.

## 18. Picking

Picking uses a renderer-owned ID namespace mapped back to stable semantic handles.

The preferred implementation is an off-screen integer ID target sharing the scene depth convention.

Requirements:

- semantic entity IDs are not packed directly into fragile GPU bit layouts unless a stable encoding is documented;
- renderer pick IDs are generation-scoped;
- readback is asynchronous;
- stale pick results are rejected when the scene generation or viewport request generation changed;
- hidden/filtered geometry follows the same visibility rules as the visible frame;
- diagnostics may optionally expose face/tile/subcomponent IDs.

The editor decides what selecting a returned semantic handle means.

## 19. Diagnostics

The renderer must support diagnostic views without modifying the semantic scene.

Required modes include at least:

- wireframe/topology;
- vertex and merged-normal visualization where data exists;
- face priority;
- face alpha/transparency;
- authored face bias;
- texture/material ID;
- storage plane and render level;
- zone boundaries and dirty generations;
- semantic object/pick ID;
- static versus dynamic path;
- depth visualization;
- culled/filtered reason reporting.

Diagnostics should be selectable per viewport and must retain object/region/tile provenance.

## 20. Performance principles

Optimization order is:

1. preserve semantic correctness;
2. avoid unnecessary semantic rebuilds;
3. limit extraction to dirty dependencies;
4. compile static data by zone;
5. minimize GPU allocation churn;
6. batch compatible draws;
7. use per-frame dynamic sorting only for content that needs it;
8. profile before introducing more complicated GPU-driven techniques.

GPU-driven indirect drawing, compute culling, bindless material systems, meshlets, or similar techniques are allowed future optimizations only if they preserve the contracts in this blueprint.

## 21. Required renderer tests

Before the renderer is considered implementation-complete it needs:

- semantic-scene destruction/recreation test proving renderer independence;
- zone dirty/rebuild generation tests;
- cross-zone object footprint invalidation tests;
- device/resource recreation test;
- static/dynamic classification tests;
- reference-profile deterministic frame tests;
- mirrored-model culling fixture;
- priority/alpha/bias fixtures;
- bridge/plane/linked-below visibility fixture;
- texture/UV/animation fixtures;
- picking generation/stale-result tests;
- diagnostic provenance tests.

Verification details are expanded in Checkpoint 7.

## 22. Governing ADRs

Renderer-wide project decisions are recorded separately:

- `ADR-0004-reverse-z-raster-conventions.md`
- `ADR-0005-zone-compiled-hybrid-rendering.md`
- `ADR-0006-reference-and-enhanced-render-profiles.md`

A future implementation may change these only through a superseding ADR.
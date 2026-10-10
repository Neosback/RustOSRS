# RustOSRS GPU Data and Render Pass Contracts

Status: **Normative renderer blueprint, corrected by M10 foundation audit**  
Decision class: `PROJECT_DECISION`

This document defines the renderer-facing ABI between extracted OSRS scene data and future `wgpu` realization. It does not redefine OSRS semantic truth.

## 1. Governing principles

1. GPU artifacts are disposable derivatives.
2. Semantic face/plane/material metadata survives until the owning render policy consumes it.
3. Static batching cannot erase parity or picking provenance.
4. Ordered preparation never mutates canonical meshes.
5. Every GPU resource is rebuildable from a current immutable extraction generation.
6. OSRS software/client behavior and imported RuneLite GPU behavior are distinct reference targets and must be named explicitly.

## 2. Canonical `RenderMesh`

Before GPU packing, renderer-owned CPU data must be able to express:

- integer-derived renderer-local positions;
- triangle indices;
- baked reference face colors/HSL payload;
- semantic texture ID and renderer material handle;
- UV/texture-triangle inputs;
- raw signed face alpha;
- face priority or model default priority;
- authored face bias;
- original face render type;
- semantic/renderable identity;
- suppressed-face state via baked `c == -2` semantics;
- plane/provenance needed for picking and diagnostics.

A GPU pipeline's first implementation may ignore some fields only after retaining them in CPU extraction.

## 3. Position conversion

Semantic integer coordinates remain integer through semantic construction and placement.

Renderer extraction performs explicit origin rebasing while integer-exact, then converts to floating GPU positions at the GPU packing boundary.

No hidden half-tile correction, corrective rotation, or semantic bridge adjustment is allowed in this conversion.

## 4. Vertex/data formats

Static and dynamic paths use compatible logical material/color contracts even if physical buffers differ.

Diagnostic streams remain separate so editor-only attributes do not bloat production static geometry.

Indexed rendering is the default. De-indexing is permitted only if provenance remains recoverable through side tables.

GPU-facing winding is normalized to CCW exactly once.

## 5. Face admission

Before any draw packet is emitted:

- baked face color marker `c == -2` means suppressed and the face is not drawn in Reference mode;
- flat marker `c == -1` remains a valid drawn-face state;
- suppression is semantic output, not an alpha/blend optimization;
- renderer classification cannot re-admit a suppressed triangle.

This explicitly protects the target alpha/render-type sentinel behavior already implemented by `osrs-core::lighting`.

## 6. Material table

Semantic texture IDs map through renderer-owned `MaterialHandle` values.

Material entries preserve:

- full-width texture identity;
- source/composition metadata;
- average RGB/opacity inputs;
- animation direction/speed;
- source sprite/color-transform metadata;
- sampler/reference profile identity;
- provenance.

No fixed RuneLite texture count is semantic truth.

## 7. Texture storage

The intended wgpu baseline uses paged 2D texture arrays where supported. Paging is renderer policy and must not impose a fixed semantic texture-ID range.

Adapters that cannot realize the selected path must choose an explicitly documented fallback or surface a capability diagnostic. Silent texture loss is prohibited.

## 8. Reference texture construction and sampling

The imported RuneLite renderer provides one explicit comparison profile.

Audited texture image behavior:

- imported texture images are 128x128 for that renderer snapshot;
- source RGB pixel `0` is uploaded transparent;
- nonzero source RGB is uploaded with alpha `255`;
- level-0 alpha is sampled for cutout/discard behavior;
- staged shader discards when level-0 alpha is below `1`;
- brightness is applied in shader-side color processing;
- textured model face lighting consumes the baked lightness payload rather than re-running semantic normal lighting.

Audited sampler behavior must not be summarized as simply "nearest":

```text
magnification                         = NEAREST
minification, filtering level 0      = NEAREST
minification, filtering level >= 1   = NEAREST_MIPMAP_LINEAR
imported config default level         = 1
S wrap                                = CLAMP_TO_EDGE
T wrap                                = default repeat in audited setup
```

RustOSRS Reference sampling selects and tests an explicit profile. Enhanced anisotropic/filtering behavior is separately selectable and cannot silently replace the Reference profile.

## 9. Frame/camera/draw data

Frame globals include deterministic render tick and profile/diagnostic state.

Camera globals include view transform, explicit reverse-Z projection, camera position, and scene-origin rebase.

Draw data includes semantic/pick handle, material selection, renderer transform where not pre-baked, and plane/visibility metadata required by the selected profile.

## 10. HSL/color boundary

Reference mode consumes semantic/model lighting output. It does not recompute target object lighting from normals.

HSL-to-RGB may occur on GPU only if exact packed inputs and conversion helpers are verified against the target/reference tables.

Textured-face lightness behavior and enhanced color modes remain explicit renderer-profile choices.

## 11. Reverse-Z Reference contract

ADR-0004 is authoritative.

Reference baseline:

```text
depth clear   = 0.0
near           = larger depth
far            = smaller depth
depth compare = Greater
```

The imported reference projection has no finite far plane. RustOSRS must reproduce equivalent `0..1` wgpu depth behavior rather than using `GreaterEqual` as a convenience.

## 12. Authored face bias

Reference profile preserves the audited pre-divide bias behavior:

```text
position.z += face_bias / 128.0
```

Because the reference projection is perspective/no-far, normalized-depth separation decreases with distance. Required fixtures therefore cover at least two materially different camera distances.

Hardware polygon offset is not a semantic substitute.

## 13. Opaque paths

Opaque zone-compiled static and dynamic geometry use explicit reverse-Z depth testing, depth writes, blending disabled, backface culling, and authored bias.

Static compilation does not imply that all static faces are ordinary opaque work. Priority-sensitive or alpha-sensitive static content may register with ordered systems.

## 14. Ordering reference distinction

### RustOSRS software/client Reference ordering

Priority-sensitive Reference content uses the exact `FACE-002` `Model.method5946` queue/threshold algorithm implemented in M10 CPU preparation.

### Imported RuneLite GPU behavior

The imported RuneLite renderer exposes render modes:

```text
DEFAULT
SORTED
SORTED_NO_DEPTH
UNSORTED
UNSORTED_NO_DEPTH
```

Its `ModelUploader` priority queues execute only when `prioritySort=true`; the audited plugin selects that for `SORTED_NO_DEPTH`. Ordinary dynamic upload passes `false`, static opaque upload does not universally apply the priority queues, and alpha/static content in `Zone` uses distance/depth ordering.

Therefore "RuneLite reproduces the priority algorithm" is only true for the path that actually enables priority sorting. RustOSRS documents any RuneLite-GPU comparison profile separately from the stronger software/client Reference policy.

## 15. Transparency

Transparency ordering and depth behavior are profile-owned.

For Reference work, do **not** assume the conventional modern rule that all blended geometry disables depth writes. The imported RuneLite snapshot leaves ordinary depth writes active and represents explicit no-depth work through separate render modes/ranges.

Required Reference implementation must prove:

- source alpha interpretation;
- model-level and face-level transparency interaction where applicable;
- far-to-near ordering contract;
- strict `Greater` behavior;
- depth-write behavior;
- explicit no-depth behavior if represented.

An Enhanced profile may use conventional read-only-depth transparency only as an explicit divergence.

## 16. Terrain pipelines

GPU terrain consumes already-constructed semantic topology.

Flat paint uses the exact two-triangle `TERRAIN-002` split. Shaped terrain uses exact `TERRAIN-001` vertices/faces.

The GPU does not rebuild the terrain shape templates or terrain-color builder.

Full terrain-color semantic construction remains an upstream `TERRAIN-004` implementation task, now source-verified rather than source-blocked.

## 17. Plane/roof visibility inputs

Renderer grouping may consume semantic storage plane, source plane, linked-below state, and once implemented, target `originalPlane`/`minPlane` semantics.

A RuneLite-style `maplevel` is derived visibility/settings lookup. It must not be interpreted as moving tile geometry into another semantic storage plane.

Renderer grouping must leave the semantic scene hash unchanged.

## 18. Object pipeline

Object meshes arrive after semantic model selection, mirroring, transforms, recolor/retexture, contouring, normal reconciliation, lighting, morph/animation ownership, and placement.

Renderer code applies view/projection/material/pass policy only. It does not reconstruct semantic models.

## 19. Texture animation

Animation uses deterministic render tick, never wall-clock-only time.

M10 material preparation maps decoded direction/speed to the pinned UV velocity convention. Future GPU code must reproduce those exact tick results.

## 20. Zone compilation

Primary static compilation unit is an 8x8 semantic tile zone keyed by storage plane and signed zone coordinates.

Zone work is generation-aware:

- unchanged zones may remain reusable after unrelated semantic edits;
- changed zones reject stale build tickets;
- cross-zone footprints invalidate every affected zone;
- render-origin movement cannot change semantic zone identity.

## 21. Picking

Preferred pick target remains integer ID based. Pick IDs are extraction-generation scoped and map through a side table to semantic identity plus optional face/tile/subcomponent provenance.

Stale readback must never change selection for a newer semantic generation.

## 22. Diagnostics

Priority, alpha, bias, depth, normals, plane, material, and zone diagnostics consume the same retained semantic/extraction metadata. Diagnostics do not alter canonical values.

## 23. Capability policy

Optional GPU features may improve performance but cannot become requirements for semantic correctness. Unsupported baseline renderer requirements produce explicit diagnostics rather than silent fallback that changes appearance.

## 24. Required verification before renderer parity claims

Structural tests:

- static/dynamic/ordered classification;
- suppressed-face exclusion;
- exact priority emission fixture;
- material identity and full-width texture IDs;
- exact UV cases;
- zone invalidation and stale generation;
- semantic hash unchanged by renderer grouping.

Reference raster tests:

- strict reverse-Z `Greater` near/far/equal-depth cases;
- bias at multiple distances;
- ordinary/mirrored/rotated winding;
- alpha ordering plus depth-write/no-depth cases;
- texture zero-alpha/cutout behavior;
- reference min/mag/wrap sampling profile;
- deterministic texture animation.

Visual tests:

- internal RustOSRS goldens are regression evidence;
- at least one externally captured/pinned client or RuneLite comparison artifact is required before claiming external "1:1" visual parity for a behavior.

A self-generated screenshot alone cannot prove parity with an external renderer.

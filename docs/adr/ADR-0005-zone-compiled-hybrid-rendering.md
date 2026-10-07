# ADR-0005: Zone-Compiled Hybrid Rendering

Status: **Accepted**  
Date: 2026-10-07

## Context

The editor needs large-scene performance without turning GPU buffers into source truth. Most map geometry is static for long periods, while some content remains animated, morph-dependent, transparent, priority-sensitive, or temporarily edited.

A monolithic scene buffer makes small edits expensive. A fully per-object draw model creates unnecessary CPU/GPU overhead. A fully baked model risks erasing camera-dependent priority/transparency behavior.

The audited RuneLite renderer demonstrates that chunk-aligned static compilation is practical, but RustOSRS must preserve stronger semantic/test boundaries than merely copying that implementation.

## Decision

RustOSRS uses an **8x8 tile zone** as the primary static renderer compilation unit and combines it with a dynamic/ordered renderable path.

The renderer therefore has two complementary classes:

### Zone-compiled static path

Used for geometry whose extracted appearance is stable enough to bake into zone-local GPU artifacts.

Examples:

- terrain;
- finalized static loc models;
- walls and wall decorations;
- static floor decorations;
- ordinary opaque static faces that do not require camera-dependent reference ordering.

### Dynamic/ordered path

Used for geometry requiring per-frame or camera-relative processing.

Examples:

- animated locs;
- morph/preview results that regenerate geometry;
- transparent content requiring ordered blending;
- priority-sensitive models/faces requiring `FACE-002` reference ordering;
- temporary editor previews/ghosts.

Static geometry may still register selected faces/models with the ordered path when camera-dependent ordering is required. "Static" therefore describes geometry stability, not permission to discard face metadata.

## Dirty model

Semantic edits publish invalidation events. The renderer maps them to affected zones/resources.

Dirty rebuilds:

1. start from the newest immutable semantic generation;
2. extract only affected renderer dependencies where practical;
3. compile CPU zone artifacts off-thread;
4. discard stale results if their generation is no longer current;
5. upload replacement GPU resources;
6. retire old resources safely after in-flight use ends.

Camera movement, texture animation ticks, and ordinary visibility filters do not rebuild semantic zones.

## Consequences

Positive:

- bounded rebuild cost for terrain/loc edits;
- natural compatibility with 8x8 OSRS chunks;
- simple streaming and frustum-culling bounds;
- renderer artifacts remain reconstructible;
- priority/transparency correctness is not sacrificed to static batching;
- future client-like applications can reuse the same static/dynamic split.

Costs:

- cross-zone object dependencies need explicit invalidation;
- material changes may fan out to multiple zones;
- ordered static geometry requires side metadata in addition to baked buffers;
- more lifecycle bookkeeping than a single monolithic mesh.

## Alternatives considered

### One buffer for the whole scene

Rejected because editor-local edits would trigger excessive rebuild/upload work and complicate streaming.

### Per-tile buffers

Rejected as the baseline because allocation/draw overhead would be excessive relative to the expected edit granularity.

### Per-object rendering only

Rejected as the baseline because static maps contain enough objects/terrain to make draw-call and resource-management overhead unnecessary.

### Fully GPU-driven meshlet system immediately

Deferred. It may become a later optimization, but it adds complexity before the semantic/reference renderer is proven.

## Constraints/invariants affected

- D1: semantic scene remains renderer-independent;
- D4: dirty tracking begins at semantic edits;
- E4: GPU optimizations never become source truth;
- H1/H2: zone artifacts retain semantic provenance for diagnostics.

## Follow-up work

Implementation must define:

- zone dependency index;
- cross-zone footprint invalidation;
- material-to-zone dependency tracking;
- CPU compile job format;
- stale-generation rejection;
- GPU resource retirement;
- ordered-static registration;
- zone diagnostic overlays.

Checkpoint 7 defines verification ownership.

## Supersedes

None.
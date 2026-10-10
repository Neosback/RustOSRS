# ADR-0005: Zone-Compiled Hybrid Rendering

Status: **Accepted, clarified 2026-10-10**  
Date: 2026-10-07  
Clarification provenance: `docs/implementation/REFERENCE-PROVENANCE-CORRECTION-2026-10-10.md`

## Context

The editor needs large-scene performance without turning GPU buffers into source truth. Most map geometry is static for long periods, while some content remains animated, morph-dependent, transparent, priority-sensitive, or temporarily edited.

A monolithic scene buffer makes small edits expensive. A fully per-object draw model creates unnecessary CPU/GPU overhead. A fully baked model risks erasing camera-dependent priority/transparency behavior.

The audited RuneLite renderer demonstrates that chunk-aligned static compilation is practical, but a post-M9 audit established an important distinction: RuneLite does not universally run its priority sorter. Its captured GPU dispatch enables priority sorting specifically for `RENDERMODE_SORTED_NO_DEPTH`, while static zone and ordinary depth-tested paths behave differently.

RustOSRS therefore uses RuneLite's zone architecture as renderer evidence while keeping the software-client priority algorithm as the stronger Reference-profile ordering oracle where RustOSRS classifies content as ordering-sensitive.

## Decision

RustOSRS uses an **8x8 tile zone** as the primary static renderer compilation unit and combines it with dynamic and ordered renderable paths.

### Zone-compiled static path

Used for geometry whose extracted appearance is stable enough to bake into zone-local GPU artifacts.

Examples:

- terrain;
- finalized static loc models;
- walls and wall decorations;
- static floor decorations;
- ordinary opaque static faces that are reference-safe under depth and authored bias.

### Dynamic path

Used for geometry requiring regeneration/per-frame updates but not camera-dependent face ordering.

Examples:

- animated loc geometry;
- morph-state regeneration;
- temporary preview geometry;
- profile-specific dynamic reconstruction.

### Ordered path

Used when RustOSRS Reference policy requires camera/depth-sensitive ordered preparation.

Examples:

- content requiring the `FACE-002` software-client priority oracle;
- transparency whose owning reference contract requires ordered blending/emission;
- other explicitly proven camera-relative ordering cases.

Static geometry may register selected models/faces with the ordered path even when geometry itself is static. "Static" describes geometry/resource stability, not permission to discard priority, alpha, bias, or other face metadata.

## Priority-policy clarification

RustOSRS does **not** claim that its ordered-path classifier duplicates RuneLite's `RENDERMODE_*` dispatch one-for-one.

Two different questions are tested separately:

1. Does the software-client priority algorithm produce the exact required face emission order?
2. Which RustOSRS renderables must use that algorithm in the Reference profile?

Captured RuneLite operational behavior remains useful renderer evidence, including its `SORTED_NO_DEPTH` distinction, but it does not weaken the canonical semantic requirement to preserve authored priority or the Reference profile's ability to select the exact software-client sorter when needed.

## Dirty model

Semantic edits publish invalidation events. The renderer maps them to affected zones/resources.

Dirty rebuilds:

1. start from the newest immutable semantic generation;
2. extract only affected renderer dependencies where practical;
3. compile CPU zone artifacts off-thread;
4. reject stale results when the affected zone now requires a newer generation;
5. upload replacement GPU resources;
6. retire old resources safely after in-flight use ends.

Unchanged zones may remain reusable across a newer global semantic generation when their own required generation did not change. Camera movement, texture-animation ticks, and ordinary visibility filters do not rebuild semantic zones.

## Consequences

Positive:

- bounded rebuild cost for terrain/loc edits;
- natural compatibility with 8x8 OSRS chunks;
- renderer artifacts remain reconstructible;
- priority/transparency correctness is not sacrificed to static batching;
- software-client parity and RuneLite GPU dispatch remain explicitly distinguishable.

Costs:

- cross-zone object dependencies require explicit invalidation;
- material changes may fan out to multiple zones;
- ordered static geometry needs side metadata;
- renderer classification requires reference fixtures instead of assuming RuneLite's operational categories are universal.

## Alternatives considered

### Copy RuneLite render-mode dispatch exactly

Rejected as the universal architecture rule. RuneLite's captured GPU dispatch is one renderer implementation, while RustOSRS also targets exact software-client semantics and editor use cases. A RuneLite-specific profile may reproduce those render modes explicitly, but they do not define semantic truth.

### One buffer for the whole scene

Rejected because local edits would trigger excessive rebuild/upload work.

### Per-tile buffers

Rejected because allocation/draw overhead is excessive for the expected edit granularity.

### Per-object rendering only

Rejected because static scenes contain enough geometry to make unnecessary draw/resource overhead significant.

### Fully GPU-driven meshlets immediately

Deferred until semantic/reference rendering is proven.

## Required follow-up

Implementation must define and test:

- zone dependency index;
- cross-zone footprint invalidation;
- material-to-zone dependency tracking;
- CPU compile job ownership;
- stale-generation rejection;
- GPU resource retirement;
- ordered-static registration;
- zone diagnostics;
- Reference classifier cases using the software-client FACE-002 oracle;
- optional RuneLite-profile render-mode cases including `UNSORTED`, `SORTED`, and `SORTED_NO_DEPTH` when that profile is implemented.

## Supersedes

None. The 2026-10-10 clarification narrows the earlier implication that RuneLite and RustOSRS ordered dispatch were the same thing.

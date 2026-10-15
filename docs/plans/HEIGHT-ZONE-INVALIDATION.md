# Height Zone Invalidation Plan

Status: Checkpoint H2 dependency metadata contract complete and certified. Runtime Height invalidation remains conservative region based.

Branch: `refactor/height-zone-invalidation`

Base main: `5938df7d723c55b66ad4522f79c08cf89bf0acbf`

## Goal

Narrow finalized Height edit projection work from the current conservative region-ring policy to 8x8 world zones only when the runtime can prove every affected projection zone.

Semantic ownership does not change. `EditorSession` remains authoritative and 8x8 zones remain projection/cache granularity only.

## Non-goals

This branch does not:

- change Object edit invalidation
- change Height semantic planning or history
- change terrain Paint invalidation
- narrow contextual scene assembly
- change Reference or HD visual semantics
- introduce another authoritative map model
- guess a universal model radius

## Current behavior

`crates/osrs-editor/src/app/world_runtime/invalidation.rs` uses zone invalidation only for pure terrain footprints.

Any finalized footprint containing Height vertices or Object tiles still falls back to region invalidation. For Height impacts that include terrain geometry, normals, or lighting, that fallback expands one region ring.

This policy is intentionally conservative and remains the certified runtime behavior after H2.

## Audited Height dependencies

### 1. Terrain geometry

A terrain surface consumes four height-grid corners. One changed height vertex can therefore change the geometry of adjacent terrain tiles that reference that corner.

This dependency is locally bounded.

### 2. Terrain slope lighting

`osrs-scene::terrain_build::compute_slope_light` computes a corner light from height samples one grid point away in X and Y.

Terrain surfaces then consume four corner-light values.

Therefore Height edits can change terrain presentation beyond only the tiles that directly contain the edited vertex, but the terrain-only portion remains locally bounded.

The existing radius-5 Paint rule is not relevant here because that radius comes from underlay material/color blending, not Height sampling.

### 3. Initial object placement height

`osrs-scene::placement_height::sample_placement_height` samples four height-grid values around the midpoint of the rotated object-definition footprint.

The sampled coordinates depend on:

- anchor tile
- definition `size_x`
- definition `size_y`
- orientation
- scene-edge fallback behavior

A changed height vertex can therefore alter a placed object's vertical placement even when the object's anchor is not on the edited tile.

There is no proven fixed anchor radius independent of definition dimensions.

### 4. Lit-model ground contouring

`osrs-core::contour::contour_ground_in_place` and `contour_ground_copy` calculate the model's X/Z cylinder radius from actual model vertices.

Contour sampling is based on the model's transformed world X/Z positions, not merely the definition footprint.

A fixed radius based only on the Height edit coordinate or object anchor would therefore be unsafe without a proven target-cache bound.

### 5. ModelData contouring

`osrs-core::contour::contour_model_data_copy` uses actual vertex min/max X/Z bounds.

This is a separate dependency shape from the lit-model cylinder bound and must be represented rather than approximated away.

### 6. Dynamic/morphed models

`osrs-scene::dynamic_model::resolve_dynamic_model`:

1. resolves the active morph and placement
2. selects the active model
3. optionally poses it
4. contours the private working model

The effective Height read-set can therefore depend on active definition/model state and posed geometry.

### 7. Downstream scene relationships

A Height-driven change to contoured/placed geometry can affect final model bounds and normal/lighting relationships used by scene finalization.

The invalidation boundary must include every output zone whose final geometry can change, not only the zone containing the source Height vertex.

## H1 conclusion

A single constant Height radius is not currently proven safe.

The branch must use explicit projection dependency metadata and retain the current region fallback whenever that metadata cannot prove a complete zone set.

## H2 dependency contract

H2 adds a projection-only dependency contract in:

`crates/osrs-editor/src/app/world_runtime/height_dependencies.rs`

The core mapping is:

```text
HeightVertexCoord (source plane + world x/y)
    -> deterministic set of output 8x8 world zones
```

The metadata is intentionally disposable. It is not semantic authority and it does not modify `MapDocument` / `EditorSession` ownership.

### Freshness identity

Every collection carries a `HeightDependencyStamp` containing:

- region projection epoch
- presentation generation

Lookup returns stale when either no longer matches the caller's current projection state.

### Completeness

Collections begin `Incomplete` and must be explicitly certified `Complete` by a future producer only after every Height-sensitive projection stage has contributed its read-set.

An incomplete collection is unusable for narrowing and therefore forces fallback when H3 consumes it.

### Deterministic merge and deduplication

Dependencies use ordered map/set storage so merge order does not change the resulting source-to-zone mapping and duplicate source-zone edges collapse deterministically.

Collections can merge only when producer region and freshness stamp match.

Completeness is fail-closed: merging a complete collection with an incomplete collection produces an incomplete collection.

### Producer-region boundary

A collection represents one producer region. Recording an output zone that belongs to another region is rejected rather than silently widening or misclassifying the dependency set.

This makes cross-region dependency handling explicit for H3/H4 instead of hiding it in the metadata layer.

### Runtime behavior after H2

H2 deliberately does not switch Height edits to zone invalidation.

The module is compiled into the editor, but existing Height invalidation remains on the certified conservative region policy until H3 wires accepted/current build results into the dependency contract and can prove a complete zone set.

## Required freshness rule

Dependency metadata must never survive as trusted current data across a projection state change that can alter Height dependencies.

At minimum, trust must be tied to the same current region/projection epoch and presentation generation used by stale-build rejection.

Examples that must invalidate or make dependency metadata unusable until refreshed:

- Object placement/deletion/move/rotation that changes height-sensitive scene content
- source replacement
- presentation/source generation changes where the dependency-producing worker is replaced
- rejected/stale build completion

If a Height edit arrives while dependency metadata for an affected region is absent or stale, use the existing conservative region policy.

## Context versus output

This work narrows dirty output scope only.

The world worker continues to construct the full contextual scene required for:

- terrain lighting
- normal reconciliation
- contouring
- object interactions
- Reference extraction
- 117HD procedural generation

Only final projection emission/installation is eligible for zone filtering.

## Checkpoints

### H1 - dependency audit and protocol

Complete and certified.

Deliverables:

- refreshed `DEV_HANDOFF.md`
- this safety/architecture plan
- no production behavior change

### H2 - dependency metadata types and collection contract

Complete and certified.

Certified code head:
`c59eea2cadffbc5861f6ecf8c138a9a0b6b11f99`

CI:
Run `38689828881` / #1227 - SUCCESS.

Delivered:

- canonical `HeightVertexCoord` source keys retain source plane and world tile coordinates
- deterministic source-to-zone mapping using ordered map/set storage
- producer-region validation for output zones
- projection epoch + presentation generation freshness stamps
- explicit `Incomplete` / `Complete` state
- stale/incomplete/current lookup outcomes
- fail-closed merge semantics and duplicate elimination
- tests for negative coordinates, region/zone edges, multiple zones, stale stamps, mismatched producers/stamps, known-empty complete lookups, and incomplete merges
- current Height invalidation remains region based

### H3 - runtime integration and zone planner

Not started.

Acceptance:

- current dependency metadata is installed only from accepted/current build results
- finalized Height receipts retain enough canonical coordinate identity, including source plane, to query H2 metadata correctly
- finalized Height receipt resolves to exact affected zones when metadata is complete/current
- missing or stale metadata falls back to region invalidation
- no semantic authority moves into WorldRuntime
- full contextual scene assembly remains unchanged

### H4 - adversarial integration coverage

Not started.

Acceptance includes:

- 8x8 edge
- 64x64 edge
- rotated multi-tile placement
- contoured geometry crossing zones
- dynamic/morphed case where current runtime supports deterministic fixture input
- Undo/Redo
- in-flight stale build
- dependency metadata loss/staleness fallback

### H5 - certify and merge

Not started.

Required exact-head Tier A/B/C plus focused Height invalidation coverage. After merge, certify integrated `main`, refresh `DEV_HANDOFF.md`, stop, and report before any Object invalidation branch is created.

## Safety invariant

For every Height semantic transaction, zone-scoped invalidation is permitted only when the selected zone set is a proven superset of every projection output that can change because of the touched Height vertices.

If that proof is unavailable, use the existing conservative region fallback.

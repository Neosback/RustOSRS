# Height Zone Invalidation Plan

Status: Checkpoint H1 audit complete. No runtime invalidation behavior changed.

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

Any finalized footprint containing Height vertices or Object tiles falls back to region invalidation. For Height impacts that include terrain geometry, normals, or lighting, that fallback expands one region ring.

This policy is intentionally conservative and currently certified.

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

## Proposed dependency model

The preferred direction for H2 is a projection-only Height dependency read-set.

Conceptually:

```text
height source vertex (plane, world x, world y)
    -> one or more output world zones whose final projection reads that height
```

The inverse mapping is useful at edit time because a finalized Height receipt already contains the changed Height vertices.

The dependency data is not semantic authority. It is disposable projection metadata tied to a successfully installed/current scene build.

## Required freshness rule

Dependency metadata must never survive as trusted current data across a projection state change that can alter Height dependencies.

At minimum, trust must be tied to the same current region/projection epoch used by stale-build rejection.

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

Complete.

Deliverables:

- refreshed `DEV_HANDOFF.md`
- this safety/architecture plan
- no production behavior change

### H2 - dependency metadata types and collection contract

Not started.

Acceptance:

- projection-only types represent Height source vertices and affected output zones
- deterministic merge/deduplication behavior
- explicit completeness/freshness state
- tests cover negative coordinates, zone/region edges, and multi-zone dependencies
- current Height invalidation remains region based

### H3 - runtime integration and zone planner

Not started.

Acceptance:

- current dependency metadata is installed only from accepted/current build results
- finalized Height receipt resolves to exact affected zones when metadata is complete/current
- missing or stale metadata falls back to region invalidation
- no semantic authority moves into WorldRuntime

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

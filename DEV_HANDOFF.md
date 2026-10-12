# RustOSRS Map Editor Architecture Handoff

Last updated: 2026-10-12

This is the living handoff for the RustOSRS map-editor architecture project. Update it at every meaningful checkpoint and before each branch merge.

## Project goal

Build a production-grade OSRS editor where semantic map state is authoritative, deterministic, headless-testable, and independent from UI, rendering, ECS/runtime state, and cache codecs.

## Working rules

1. One branch at a time.
2. Work in small checkpoints and stop after each checkpoint for review.
3. Do not start the next checkpoint without explicit direction.
4. Fully validate intended branch scope before merge.
5. Preserve semantic/reference parity unless a checkpoint explicitly changes behavior.
6. Keep 117HD/rendering isolated from editor-domain refactors unless an explicit adapter change is required.
7. One logical user gesture should become one logical `MapPatch` and one history entry.
8. Update this file at each meaningful checkpoint and before merge.

## Target architecture

```text
osrs-core
  Canonical semantic vocabulary: coordinates, IDs, planes, shared OSRS values.

osrs-edit
  Authoritative headless editable-map domain.
  Owns semantic state, validation, reversible patches, history, query contracts,
  change impact, and later revisions/transactions.

osrs-cache
  Cache codecs and cache-format infrastructure.

osrs-scene
  OSRS scene semantics.

osrs-world
  Cache-to-scene orchestration only. No new authoritative edit semantics.

osrs-editor
  Application/product layer: sessions, tools, input, previews, orchestration,
  adapters, and UI.

renderer / 117HD
  Disposable reactive projection/cache of authoritative semantic state.
```

`osrs-edit` depends only on `osrs-core`, enforced by the repository architecture guard.

## Core domain decisions

### Authoritative state

`MapDocument` is the authoritative editable-map state. Renderer meshes, GPU resources, ECS state, scene-worker results, and UI state are projections/caches and must never become the edit source of truth.

### Coordinates and planes

Reuse typed coordinates/planes from `osrs-core`.

- `MapPosition` = map tile + `SourcePlane`
- `HeightVertexCoord` = terrain corner/height vertex + `SourcePlane`
- Tile attributes and corner heights remain distinct semantic coordinates.
- Do not collapse source planes, storage planes, render levels, collision planes, or renderer-local coordinates into raw tuples.

### Terrain

`TerrainTile` owns tile-level underlay/overlay/flags. Height is separate.

- overlay shape: `0..=11`
- overlay rotation: `0..=3`
- terrain storage is sparse
- absent terrain = `TerrainTile::default()`
- setting terrain to default removes the sparse entry

### Heights

Height vertices are explicit. Missing height is not numeric zero.

`SetHeight` requires the vertex to exist and its current value to match the operation's `before` value.

### Objects

`ObjectPlacement` uses semantic `ObjectId`, `MapPosition`, validated loc kind, and validated orientation.

- `AddObject` requires absence
- `RemoveObject` requires presence

### Patch model

```rust
MapPatch
  private Vec<MapOp>
  private derived ChangeImpact

MapOp
  SetTerrain { coord, before, after }
  SetHeight { coord, before, after }
  AddObject { placement }
  RemoveObject { placement }
```

`MapPatch::new` derives semantic impact. `MapPatch::inverse` reverses operation order and exactly inverts every operation.

### Atomic application

`MapDocument::validate_patch` simulates touched state, including effects of earlier operations in the same patch, without mutating the real document.

`MapDocument::apply_patch` mutates only after the entire patch validates.

### Change impact

`ChangeImpact` is semantic, not renderer-specific. Current categories cover terrain geometry/material, normals, lighting, collision, locations, and bounds.

### Edit history

`EditHistory` is the headless undo/redo contract around `MapDocument` + `MapPatch`.

- one successful non-empty patch = one history entry
- undo applies the exact inverse
- redo reapplies the original
- successful new edits clear redo
- empty patches create no history
- failed commit/undo/redo leaves stacks coherent
- undo/redo return `ChangeImpact`

### Read-only query boundary

`MapQuery` currently exposes:

```rust
trait MapQuery {
    fn terrain_at(&self, coord: MapPosition) -> TerrainTile;
    fn height_at(&self, coord: HeightVertexCoord) -> Option<i32>;
    fn contains_object(&self, placement: ObjectPlacement) -> bool;
}
```

`MapDocument` implements it directly. Do not expand it speculatively.

## Merged foundation history

### PR #33: edit-domain foundation

- Branch: `refactor/editor-edit-domain-foundation`
- Final pre-merge head: `ded9c670dd2c3658785ea462cded34dcd010bd7c`
- Squash merge: `5c5eef3bde696b20ae0319867e6e84567701ebf6`
- Post-merge handoff/main: `31da7b55e45557cd9f4e581b3798310ff26b53d3`
- CI `38239181734`: Tier A/B/C PASS

Delivered `osrs-edit`, semantic values, architecture guard, `ChangeImpact`, reversible `MapPatch`, authoritative `MapDocument`, atomic validation/application, and exact inverse round trips.

### PR #34: history/query foundation

- Branch: `refactor/editor-edit-history-foundation`
- Final pre-merge head: `4847c1edf45d969223d6b9ad32be48b60f01bbf8`
- Squash merge: `cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`
- Post-merge main: `32e65e93a1d67282059098bc482aa8bcd6c10a3e`
- Post-merge CI `38259779463`: Tier A/B/C PASS

Delivered failure-safe `EditHistory` and read-only `MapQuery`.

## Current branch

Branch: `refactor/editor-path-commit-domain`

Base: validated `main` head `32e65e93a1d67282059098bc482aa8bcd6c10a3e`.

### Checkpoint 10: semantic terrain-path commit bridge

Normal terrain-path commits were moved across the semantic edit boundary before changing the generator.

- `app/path_commit.rs` defines `TerrainPathIntent` and `build_terrain_path_patch`.
- Touched legacy tile/height values seed a temporary semantic `MapDocument`.
- The complete laid path becomes one deterministic `MapPatch`.
- Duplicate tile intents fold to final terrain/height operations per coordinate.
- Shape/rotation are validated through semantic types.
- The full semantic patch validates/applies before legacy editor mutation.
- The result is projected into the current `TileEdit` world layer.
- Existing dirty-region/rebuild behavior remains unchanged.
- One laid path remains one existing undo step.

Checkpoint 10 final head:

- `7a9fc7af761046093bbf15ffa229acfa2cc828f6`
- CI `38261950847`: Tier A/B/C PASS

### Checkpoint 11: simplified true-shape overlay fitter

Direction changed intentionally: the path tool should draw the band the user asks for, not secretly solve a routing problem.

Removed from the product and source tree for now:

- Walls mode and wall/tower object emission
- obstacle avoidance and rerouting
- A*/cost-grid pathfinding
- path-network/auto-connect behavior
- wander/noise generation
- width-variation generation
- preserve-existing-overlay topology repair
- old planner/router/network/wall code and their obsolete regression/fixture files

The active `terrain_path` directory is now intentionally:

```text
terrain_path/
  fitter.rs
  mod.rs
  shapes.rs
```

### Active fitting pipeline

1. User control points are smoothed with clamped uniform Catmull-Rom.
2. Fewer than three control points remain unchanged, so two points are exactly a straight segment.
3. Curved segments use 12 samples per control segment and preserve exact endpoints.
4. Candidate tiles come from segment-local boxes expanded by half the requested width.
5. Tile centers farther than `half_width + 0.75` are rejected early.
6. Every remaining tile gets an 8x8 ideal coverage mask from exact point-to-polyline distance.
7. Candidate overlay masks come from the repository's audited `osrs_scene::ShapedTerrainSurface` geometry through `terrain_path::overlay_triangles`; shape tables/rotation formulas are not duplicated.
8. Candidate set is EMPTY first, then stored full-tile shape `0`, then stored shapes `1..=11` at rotations `0..=3`.
9. Initial selection maximizes 8x8 area agreement. Exact ties retain the earlier candidate, so EMPTY wins exact total ties.
10. If `edge_weight > 0`, up to six deterministic coordinate-descent passes add shared-edge mismatch cost against current neighboring choices.
11. Empty fits are omitted from output.
12. Fitted tiles still commit through the Checkpoint 10 semantic `MapPatch` boundary as one undo step.

### Conservative improvements over the supplied PathOverlayFitter idea

Only low-risk improvements were added:

- signed world coordinates work directly; there is no fixed `mapWidth`/`mapLength` clamp assumption
- exact existing renderer-derived OSRS shape geometry is reused instead of copying shape tables or rotation formulas
- segment-local candidate boxes avoid scanning a huge global rectangle on long diagonal paths while preserving the possible touched area
- persisted/UI `width` and `edge_weight` are finite and clamped
- deterministic `BTreeMap`/`BTreeSet` traversal is used where ordering matters

Deliberately not added:

- randomness
- annealing/global optimization
- hidden obstacle logic
- topology repair
- automatic joining to existing overlays
- new shape heuristics beyond area agreement + shared-edge agreement

The goal is to make the simple algorithm reliable before adding sophistication.

### Simplified Spline settings

`SplineSettings` now contains only:

- `width`
- `edge_weight`
- `snap_points`

Persisted values are sanitized to the same ranges the UI exposes. Old planner/wall settings are no longer retained as compatibility fields.

### Fitter regression coverage

Current focused tests cover:

- exact candidate count and EMPTY/full ordering
- two-point straight-line preservation
- Catmull-Rom endpoint preservation
- deterministic output
- stored shape range `0..=11`
- rotation range `0..=3`
- negative world coordinates
- simplified persisted-settings clamping/fallback

The migration workspace passed `cargo check --workspace` and the focused fitter tests before publishing the production tree. Normal repository Tier A/B/C validation is required on the final handoff head before this checkpoint is closed.

Pre-handoff cleanup head removing the legacy generator files:

- `f676fa59007ec86efda0c3b485a9aae805587d57`

## Current boundaries

The editor still does **not** own a persistent `MapDocument` + `EditHistory` session. Checkpoint 10 uses a touched-state semantic snapshot and projects the validated patch into the existing legacy editor/world-edit layer. That broader ownership migration remains later work.

Do not reintroduce walls or pathfinding without an explicit decision.

Do not broaden the next checkpoint into brush/height/object-tool migration, revision synchronization, renderer, or 117HD work.

## Next checkpoint after validation

Perform a focused quality/fixture pass on the new fitter only:

1. Add representative straight, diagonal, gentle-curve, sharp-curve, narrow, wide, and negative-coordinate fixtures.
2. Measure area-fit error and shared-edge disagreement deterministically.
3. Verify shape/orientation output against exact renderer geometry.
4. Check boundary behavior around tile edges/corners and widths near the lower UI bound.
5. Tune only constants with evidence. Do not add routing or heuristic complexity by default.
6. Stop and report before any new feature work.

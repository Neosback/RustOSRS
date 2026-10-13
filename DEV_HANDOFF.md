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
- old planner/router/network/wall code and obsolete regression/fixture files

The runtime `terrain_path` implementation is intentionally small:

```text
terrain_path/
  fitter.rs
  mod.rs
  shapes.rs
```

`quality.rs` is present only under `#[cfg(test)]` for the Checkpoint 12 regression harness.

#### Active fitting pipeline

1. User control points are smoothed with clamped uniform Catmull-Rom.
2. Fewer than three control points remain unchanged, so two points are exactly a straight segment.
3. Curved segments use 12 samples per control segment and preserve exact endpoints.
4. Candidate tiles come from segment-local boxes expanded by half the requested width.
5. Tile centers farther than `half_width + 0.75` are rejected early.
6. Every remaining tile gets an 8x8 ideal coverage mask from exact point-to-polyline distance.
7. Candidate overlay masks come from audited `osrs_scene::ShapedTerrainSurface` geometry through `terrain_path::overlay_triangles`; shape tables/rotation formulas are not duplicated.
8. Candidate set is EMPTY first, then stored full-tile shape `0`, then stored shapes `1..=11` at rotations `0..=3`.
9. Initial selection maximizes 8x8 area agreement. Exact ties retain the earlier candidate, so EMPTY wins exact total ties.
10. If `edge_weight > 0`, up to six deterministic coordinate-descent passes add shared-edge mismatch cost against current neighboring choices.
11. Empty fits are omitted from output.
12. Fitted tiles still commit through the Checkpoint 10 semantic `MapPatch` boundary as one undo step.

#### Conservative improvements over the supplied PathOverlayFitter idea

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

#### Simplified Spline settings

`SplineSettings` now contains only:

- `width`
- `edge_weight`
- `snap_points`

Persisted values are sanitized to the same ranges the UI exposes. Old planner/wall settings are no longer retained as compatibility fields.

Checkpoint 11 validated final head:

- `83e3bd1f68a6550a467e18ee6ee5bc588fb049fe`
- CI `38266732098`: Tier A/B/C PASS

### Checkpoint 12: deterministic fitter quality fixtures

The production fitter was intentionally left unchanged. This checkpoint added an independent black-box quality harness in `terrain_path/quality.rs` to determine whether the simple algorithm actually needed more heuristics before changing it.

Representative fixture matrix:

1. horizontal straight path
2. vertical straight path
3. 45-degree diagonal
4. shallow diagonal
5. gentle curve
6. tight multi-turn curve
7. narrow path
8. wide path
9. exact tile-corner crossing
10. negative world coordinates

The harness independently:

- rasterizes the requested band rather than calling the fitter's private scoring helpers
- samples selected coverage from renderer-derived overlay triangles
- measures 8x8 ideal-vs-selected coverage agreement
- measures opposing shared-edge strip disagreement
- compares default `edge_weight = 0.75` against independent `edge_weight = 0` fitting on the basic straight/diagonal fixtures
- runs the same fixtures twice to prove repeatability
- locks stable FNV-1a hashes over `(tile_x, tile_y, shape, orientation)` output for all ten fixtures

Quality contract currently enforced:

- ordinary straight/diagonal/gentle fixtures require at least 82% sampled coverage agreement
- tight/corner/negative fixtures require at least 80% sampled coverage agreement
- the wide fixture requires at least 86% sampled coverage agreement
- edge mismatch ceilings are 35-45% depending on fixture difficulty
- default edge relaxation must not increase shared-edge mismatches on the four basic fixtures
- every fixture output must be repeatable and match its locked golden hash

The first measurement run intentionally used zero placeholder hashes. CI `38343977544` proved all substantive checks passed and failed only the deliberately unseeded snapshot assertion. That run produced the canonical ten output hashes, which were then locked.

Checkpoint 12 implementation head with locked snapshots:

- `1260f359f196b561948ad41c7186d00f4670d5a3`
- CI `38344200202`: Tier A/B/C PASS
- Tier A architecture/format/workspace/strict Clippy: PASS
- Tier B full workspace tests including all fitter quality fixtures: PASS
- Tier C M3-M9 semantic/reference parity: PASS

Conclusion from the fixture evidence: **do not add another fitter heuristic now.** The simple deterministic algorithm meets the current representative coverage/continuity contract, and extra routing/topology/global-optimization complexity would add risk without a demonstrated failure to solve.

Any future intentional fitter change must review both quality metrics and golden-hash changes before updating the snapshots.

## Current boundaries

The editor still does **not** own a persistent `MapDocument` + `EditHistory` session. Checkpoint 10 uses a touched-state semantic snapshot and projects the validated patch into the existing legacy editor/world-edit layer. That broader ownership migration remains later work.

Do not reintroduce walls or pathfinding without an explicit decision.

Do not broaden this branch into brush/height/object-tool migration, revision synchronization, renderer, or 117HD work.

## Next checkpoint

Perform branch readiness for `refactor/editor-path-commit-domain`, not another fitter feature pass:

1. Compare the full branch against current `main`.
2. Audit the complete changed-file list and diff for scope leakage.
3. Confirm no temporary migration/helper files remain.
4. Confirm the removed legacy planner/wall stack has no surviving references.
5. Review the semantic path-commit bridge + simplified fitter + quality fixtures as one coherent PR scope.
6. Update this handoff with exact branch-readiness evidence if needed.
7. Open/update the PR, verify exact-head CI/mergeability, and merge only when the branch is clean.
8. Stop before starting the next feature branch.

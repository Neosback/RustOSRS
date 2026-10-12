# RustOSRS Map Editor Architecture Handoff

Last updated: 2026-10-12

This is the living development handoff for the RustOSRS map-editor architecture project. Update it at every meaningful checkpoint and before each branch merge.

## Project goal

Build a production-grade OSRS editor architecture where semantic map state is authoritative, deterministic, headless-testable, and independent from UI, rendering, ECS/runtime state, and cache codecs.

The current spline/path generator is intentionally **not** being redesigned yet. Its replacement comes later, after the edit-domain, history/query, view-synchronization, and persistence boundaries are stable.

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

`osrs-edit` currently depends only on `osrs-core`, enforced by the repository architecture guard.

## Core architecture decisions

### Authoritative state

`MapDocument` is the authoritative editable-map state. Renderer meshes, GPU resources, ECS state, scene-worker results, and UI state are projections/caches and must never become the edit source of truth.

### Coordinates and planes

Reuse the typed coordinate/plane vocabulary in `osrs-core`.

- `MapPosition` = source-map tile + `SourcePlane`
- `HeightVertexCoord` = terrain corner/height vertex + `SourcePlane`
- Tile attributes and terrain-corner heights remain distinct semantic coordinates.
- Do not collapse source planes, storage planes, render levels, collision planes, or local renderer coordinates into raw tuples.

### Terrain

`TerrainTile` owns tile-level underlay/overlay/flags. Height is not part of `TerrainTile`.

- overlay shape: `0..=11`
- overlay rotation: `0..=3`
- terrain storage is sparse
- an absent terrain entry means `TerrainTile::default()`
- setting terrain back to default removes the sparse entry

### Heights

Height vertices are explicit values. Missing height is not equivalent to numeric zero.

`SetHeight` therefore requires:

- the addressed vertex to exist
- the current value to equal the operation's `before` value

### Objects

`ObjectPlacement` uses semantic `ObjectId`, `MapPosition`, validated loc kind, and validated orientation.

`MapDocument` stores exact semantic placements:

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

`MapPatch::new` derives semantic change impact from operations. Callers cannot provide stale dirty metadata.

`MapPatch::inverse`:

- reverses operation order
- swaps before/after values
- converts add/remove pairs

This preserves exact transactional undo for dependent multi-operation patches.

### Atomic application

`MapDocument::validate_patch` simulates all touched state, including effects of earlier operations in the same patch, without mutating the real document.

`MapDocument::apply_patch` mutates only after the entire patch validates. Any rejected operation leaves the authoritative document unchanged.

Current deterministic validation failures include:

- terrain before-value mismatch
- missing height vertex
- height before-value mismatch
- duplicate object add
- missing object remove

### Change impact

`ChangeImpact` is semantic rather than renderer-specific. Current categories:

- terrain geometry
- terrain material
- normals
- lighting
- collision
- locations
- bounds

The mapping is intentionally conservative. Later view synchronization may translate semantic impact into tighter region/chunk rebuild work without changing patch data.

### Edit history

`EditHistory` is the headless undo/redo contract around `MapDocument` + `MapPatch`.

Rules now enforced:

- every successful non-empty `MapPatch` is exactly one undo entry
- undo applies the exact inverse patch
- redo reapplies the original patch
- successful new non-empty commits clear redo history
- empty patches create no entry and do not destroy redo history
- failed commits change neither document nor history
- failed undo/redo restores the popped history entry to its original stack
- undo/redo return `ChangeImpact`

`EditHistory` owns no UI or renderer state.

### Read-only semantic queries

`MapQuery` is the first read-only tool/planner-facing semantic query contract.

Current methods:

```rust
trait MapQuery {
    fn terrain_at(&self, coord: MapPosition) -> TerrainTile;
    fn height_at(&self, coord: HeightVertexCoord) -> Option<i32>;
    fn contains_object(&self, placement: ObjectPlacement) -> bool;
}
```

`MapDocument` implements `MapQuery` directly. The contract:

- duplicates no map state
- exposes no mutation capability
- preserves sparse terrain semantics
- preserves missing-height vs zero-height semantics
- uses exact semantic object-placement identity
- exposes semantic values rather than renderer/cache/runtime representations

Do not expand this interface speculatively. Add area/object enumeration or richer spatial indexes only when a concrete tool requires them and the semantics are clear.

### Future revisions / view synchronization

Async scene/mesh work should eventually carry document/chunk revisions. Results built against stale revisions must be discarded so old background work cannot overwrite newer editor state.

## Foundation merge

The first edit-domain foundation was merged previously.

- Branch: `refactor/editor-edit-domain-foundation`
- PR: `#33`
- Final pre-merge branch head: `ded9c670dd2c3658785ea462cded34dcd010bd7c`
- Squash merge: `5c5eef3bde696b20ae0319867e6e84567701ebf6`
- Post-merge handoff/base commit: `31da7b55e45557cd9f4e581b3798310ff26b53d3`
- Post-merge CI run `38239181734`: Tier A/B/C PASS

The foundation delivered:

- new `osrs-edit` workspace crate
- semantic map/height/object values
- architecture dependency guard
- `ChangeImpact`
- reversible `MapOp` / `MapPatch`
- authoritative `MapDocument`
- atomic patch validation/application
- exact inverse round trips
- semantic validation tests

## Current active branch

`refactor/editor-edit-history-foundation`

Created directly from validated base:

`31da7b55e45557cd9f4e581b3798310ff26b53d3`

### History work delivered

- `crates/osrs-edit/src/history.rs`
- exported `EditHistory`
- commit / undo / redo
- redo invalidation after a new edit
- failure-safe stack restoration
- empty-patch behavior
- exact document-state undo/redo tests
- composite terrain + height + object patch as one history step

Checkpoint 7 final handoff head:

- `a24258054497fda6b0655f6098bbd009b7bf6815`
- CI run `38249383659`: Tier A/B/C PASS

### Query work delivered

- `crates/osrs-edit/src/query.rs`
- exported `MapQuery`
- `MapQuery` implemented directly for `MapDocument`
- semantic terrain query
- explicit height query
- exact object-placement membership query
- tests proving sparse terrain defaults remain defaults
- tests proving missing height remains `None`
- tests proving object identity includes orientation
- no duplicate query state or mutable internals exposed

Query implementation head before this handoff update:

- `4c5dd4c5bad2f3bde6f7016ef8413a8f7e93a328`
- CI run `38251735284`: Tier A/B/C PASS

## Current `osrs-edit` files

```text
crates/osrs-edit/
  Cargo.toml
  src/
    document.rs
    history.rs
    lib.rs
    object.rs
    patch.rs
    position.rs
    query.rs
    terrain.rs
```

No renderer, egui, cache-codec, scene-worker, ECS, or runtime dependency belongs in this crate.

## Scope explicitly NOT touched on the active branch

- existing editor UI/tool wiring
- current editor runtime undo stack
- spline/path planner
- spline/path preview worker
- brush editing
- height-tool migration
- object-tool migration
- renderer invalidation
- 117HD
- persistence/export

## Validation record

### Foundation branch

- patch primitives head `4394278f69c3523b7115d76ac0b4afaeace06fb8`
- CI `38231806257`: PASS
- `MapDocument` implementation head `c94e9a71c02b62b84a74651827b3f02d10ddf96c`
- CI `38233801420`: PASS
- final foundation head `ded9c670dd2c3658785ea462cded34dcd010bd7c`
- CI `38238688037`: PASS
- merged PR `#33`

### History branch

Checkpoint 7 implementation head:

- `a84096b076e95c91ec7c52431a494374ef1ccdce`
- CI `38249012271`: PASS
- Tier A architecture/format/workspace/strict Clippy: PASS
- Tier B workspace tests including `EditHistory`: PASS
- Tier C M3-M9 semantic/reference parity: PASS

Checkpoint 7 final handoff head:

- `a24258054497fda6b0655f6098bbd009b7bf6815`
- CI `38249383659`: PASS

Checkpoint 8 query implementation head:

- `4c5dd4c5bad2f3bde6f7016ef8413a8f7e93a328`
- CI `38251735284`: PASS
- Tier A architecture/format/workspace/strict Clippy: PASS
- Tier B workspace tests including `MapQuery`: PASS
- Tier C M3-M9 semantic/reference parity: PASS

The final handoff-update head for Checkpoint 8 must also pass full CI before the checkpoint is considered closed.

## Checkpoint history

### Checkpoint 1: semantic edit-domain foundation

Created additive `osrs-edit` semantic value types without changing runtime/editor behavior.

### Checkpoint 2: architecture/workspace integration

Added dependency-boundary enforcement and workspace/lockfile integration.

### Checkpoint 3: living handoff

Established this file as the architecture/project continuity document.

### Checkpoint 4: patch primitives

Added `ChangeImpact`, reversible `MapOp`, and `MapPatch`.

### Checkpoint 5: authoritative `MapDocument`

Added authoritative semantic storage, queries, deterministic validation, and atomic whole-patch application.

### Checkpoint 6: foundation branch readiness and merge

Audited scope, removed unrelated lockfile churn, validated, opened PR `#33`, and squash-merged the foundation.

### Checkpoint 7: headless edit history

Added failure-safe `EditHistory` with one patch = one history step and exact undo/redo semantics.

### Checkpoint 8: read-only semantic query contract

Added `MapQuery` as a small tool/planner-facing read-only boundary implemented directly by `MapDocument`. It currently exposes terrain lookup, explicit height lookup, and exact object-placement membership only. No planner, editor tool, UI, renderer, or 117HD integration was added.

## Branch status / next checkpoint

The intended **history + basic query-contract scope is now feature-complete** on `refactor/editor-edit-history-foundation`.

Do **not** add editor integration or planner migration to this branch.

When explicitly directed to continue, the next checkpoint should be branch readiness only:

1. Verify the final handoff-update head passes full Tier A/B/C CI.
2. Compare the complete branch against current `main`.
3. Audit the full diff for scope leakage.
4. Confirm no accidental changes to renderer, 117HD, path planning, or existing editor runtime behavior.
5. Open/update the PR.
6. Verify mergeability and exact-head CI.
7. Merge the branch once clean.
8. Record the merge in this handoff on `main`.
9. Stop before creating the next development branch.

## Later branches, tentative order

Directional only. Do not start automatically.

1. Migrate one existing edit path through the semantic domain boundary, likely spline/path **commit only**, while leaving planner/preview behavior untouched.
2. Migrate brush/terrain/height editing.
3. Migrate object editing.
4. Add revision-based async synchronization and stale-result rejection.
5. Refine semantic change-impact/invalidation mapping.
6. Harden persistence/export and round-trip fixtures.
7. Replace/redesign the spline/path/auto-tile generator on top of the stable semantic query/patch architecture.
8. Plugin contracts only after the editor boundaries have proven stable.

## Path-generator rewrite remains deferred

The existing spline/path generator has grown too large and should eventually be replaced, but not during the current architecture checkpoints.

When that rewrite starts:

- build against `MapQuery` for reads
- emit `MapPatch` for authoritative commits
- do not mutate `EditorApp` or renderer state directly
- preserve useful existing golden/topology/geometry/seam/network/routing/determinism fixtures as successor acceptance tests where they describe desired behavior

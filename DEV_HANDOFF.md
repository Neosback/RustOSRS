# RustOSRS Map Editor Architecture Handoff

Last updated: 2026-10-12

This is the living development handoff for the RustOSRS map-editor architecture project. Update it at every meaningful checkpoint and before each branch merge.

## Project goal

Build a production-grade OSRS editor architecture where semantic map state is authoritative, deterministic, headless-testable, and independent from UI, rendering, ECS/runtime state, and cache codecs.

The spline/path direction changed on 2026-10-12: wall generation and obstacle/network pathfinding are disabled for now. The active tool is a deliberately small deterministic pipeline: clamped Catmull-Rom smoothing -> ideal band sampling -> exact OSRS overlay-shape fitting -> bounded shared-edge relaxation -> one semantic `MapPatch` commit.

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

### Current terrain-path commit bridge

Normal `PathMode::Terrain` commits now cross the semantic edit boundary without changing planner or renderer behavior.

The migration is intentionally transitional:

1. `spline_tool` still plans exactly as before and produces the existing `PathTile` sequence.
2. `apply_terrain_path` snapshots only the touched legacy tile/height values into a temporary semantic `MapDocument`.
3. `path_commit::build_terrain_path_patch` reads through `MapQuery` and produces one compact `MapPatch` for the entire laid path.
4. Duplicate tile intents are folded to one final terrain op and one final height op per semantic coordinate.
5. The complete patch is validated/applied atomically to the semantic snapshot before legacy state changes.
6. The patched semantic result is projected back into the existing world edit layer.
7. The existing stroke/history boundary still records the whole laid path as one legacy undo step.
8. Existing dirty-region/rebuild scheduling remains unchanged.

This means the `MapPatch` is now the canonical semantic description of a normal terrain-path commit, but the editor does **not** yet keep a persistent `MapDocument`/`EditHistory` session. That broader ownership migration comes later.

Wall-mode commits remain on the existing legacy `apply_path` path for now because they also emit object placements. Do not claim wall/object commit migration is complete until its semantic object adapter has been implemented and validated separately.

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

## History/query merge

The headless history/query foundation is merged to `main`.

- Branch: `refactor/editor-edit-history-foundation`
- Base: `31da7b55e45557cd9f4e581b3798310ff26b53d3`
- PR: `#34` (`refactor(editor): add headless edit history and query contracts`)
- Final pre-merge branch head: `4847c1edf45d969223d6b9ad32be48b60f01bbf8`
- Squash merge commit: `cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`
- Post-merge handoff/main head: `32e65e93a1d67282059098bc482aa8bcd6c10a3e`
- Post-merge CI run `38259779463`: Tier A/B/C PASS
- Branch comparison before PR: 7 commits ahead, 0 behind `main`
- Canonical PR diff: 4 files only (`history.rs`, `query.rs`, `lib.rs`, `DEV_HANDOFF.md`)
- No renderer, 117HD, spline/path planner, preview worker, brush/object tool, or existing editor-runtime files changed

### History work delivered

- `crates/osrs-edit/src/history.rs`
- exported `EditHistory`
- commit / undo / redo
- one non-empty `MapPatch` = one logical history entry
- redo invalidation after a successful new edit
- failure-safe stack restoration
- empty-patch behavior
- exact document-state undo/redo tests
- composite terrain + height + object patch as one history step

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

## Current active branch

`refactor/editor-path-commit-domain`

- Base: validated `main` head `32e65e93a1d67282059098bc482aa8bcd6c10a3e`
- Current implementation head before this handoff update: `3a05caebf60eae6ffd0986fadcc1d1bb313584ae`
- Scope: normal terrain-path commit boundary only
- Planner, preview worker, routing, geometry, topology, auto-tiling, renderer, and 117HD remain untouched
- Wall-mode/object path commits remain legacy and are explicitly deferred to a later checkpoint

### Terrain-path commit work delivered on this branch

- Added `osrs-edit` as an `osrs-editor` dependency.
- Added `app/path_commit.rs` as a pure semantic commit adapter.
- Added `TerrainPathIntent` for already-planned path output plus armed height/flag intent.
- Added `build_terrain_path_patch(&impl MapQuery, ...)`.
- Path commit validation now uses semantic `OverlayShape` / `OverlayRotation` invariants.
- Duplicate tile intents collapse deterministically before `MapPatch` construction.
- Semantic no-op path intent produces an empty patch.
- Armed height requires an explicit semantic height vertex.
- Normal terrain-path commits validate/apply one `MapPatch` before any legacy editor mutation.
- Patched terrain/height state is projected into the existing `TileEdit` world layer.
- Existing dirty-region scheduling and async rebuild behavior are unchanged.
- Existing legacy stroke history still receives exactly one completed path step.
- `commit_walls` still calls the legacy `apply_path` implementation.
- Added focused headless tests for channel preservation, armed height/flags, duplicate intent folding, semantic no-op behavior, and missing-height rejection.

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

## Validation record

### Foundation branch

- patch primitives head `4394278f69c3523b7115d76ac0b4afaeace06fb8`
- CI `38231806257`: PASS
- `MapDocument` implementation head `c94e9a71c02b62b84a74651827b3f02d10ddf96c`
- CI `38233801420`: PASS
- final foundation head `ded9c670dd2c3658785ea462cded34dcd010bd7c`
- CI `38238688037`: PASS
- merged PR `#33`

### History/query branch

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

Checkpoint 8 final handoff head:

- `4847c1edf45d969223d6b9ad32be48b60f01bbf8`
- push CI `38252254353`: PASS
- PR-triggered CI `38259341642`: PASS
- Tier A architecture/format/workspace/strict Clippy: PASS
- Tier B full workspace tests: PASS
- Tier C M3-M9 semantic/reference parity: PASS

PR `#34` was then squash-merged to `main` as `cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`.

### Terrain-path commit branch

Checkpoint 10 implementation head before this handoff update:

- `3a05caebf60eae6ffd0986fadcc1d1bb313584ae`
- Source formatting was run by the scoped migration helper before commit.
- Full Tier A/B/C validation must pass on the final handoff-update head before Checkpoint 10 is considered closed.

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

### Checkpoint 9: history/query branch readiness and merge

Compared the complete branch against `main`, confirmed it was 7 commits ahead and 0 behind, audited the four-file diff, opened PR `#34`, verified GitHub mergeability, required the PR-triggered exact-head CI to pass Tier A/B/C, and squash-merged the branch as `cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`.

### Checkpoint 10: normal terrain-path semantic commit bridge

Created `refactor/editor-path-commit-domain` from validated `main`. Inspected the existing `commit_spline` → `apply_path` seam and kept the planner/preview pipeline unchanged. Added a semantic terrain-path commit adapter that builds one deterministic `MapPatch` from the already-produced path tiles, validates/applies it against a touched-state `MapDocument` snapshot, and then projects the resulting values into the unchanged legacy world-edit/history layer.

Normal terrain-path commits now cross `MapQuery`/`MapPatch`; wall-mode commits remain legacy because they also emit object placements. This is a deliberate checkpoint boundary, not a claim that the whole spline tool or editor session has migrated.

## Current branch status / next checkpoint

Active branch: `refactor/editor-path-commit-domain`.

### Simplified path-generator direction

The previous terrain planner is no longer part of the active Spline-tool runtime. The current branch intentionally disables:

- Walls mode and wall/tower object emission
- obstacle avoidance and rerouting
- A*/cost-grid pathfinding
- path-network auto-connect behavior
- wander/noise/width-variation generation
- preserve-existing-overlay topology logic

The active path algorithm is intentionally small:

1. Control points are smoothed with clamped uniform Catmull-Rom, 12 samples per segment.
2. Candidate tiles are collected from segment-local boxes expanded by half the requested width. This is equivalent to considering every tile that may touch the band without scanning a huge global bounding rectangle.
3. Each candidate tile samples an 8x8 ideal coverage mask against exact point-to-polyline distance.
4. Shape candidates are sampled from `terrain_path::shape_triangles`, which itself uses audited `osrs_scene::ShapedTerrainSurface` geometry. Stored map shapes remain `0..=11`.
5. EMPTY wins exact ties; the first pass maximizes 8x8 area agreement independently.
6. Up to six deterministic coordinate-descent passes add shared-edge mismatch cost controlled by `edge_weight`.
7. Empty fits are omitted.
8. The result still commits through the semantic `MapPatch` boundary and remains one undo step.

Safe simplifications/improvements over the supplied `PathOverlayFitter` idea:

- signed world coordinates are supported directly; there is no `mapWidth/mapLength` clamp assumption
- exact renderer-derived shape triangles are reused rather than duplicating the shape tables or rotation formulas
- segment-local candidate boxes avoid pathological global-bounding-box work on long diagonals
- width and edge weight are finite/clamped at the editor boundary
- no randomness, annealing, topology repair, or additional heuristics were added

The old planner/router/wall source files remain in the repository temporarily as uncompiled historical code while this replacement proves itself. They are not wired into the active `terrain_path` module and should not be revived implicitly.

Next checkpoint after this fitter is validated should be a focused quality pass on its visual output and fixtures only. Do not reintroduce obstacle routing or walls without an explicit decision.


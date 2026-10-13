# RustOSRS Map Editor Architecture Handoff

Last updated: 2026-10-13

This is the living handoff for the RustOSRS map-editor architecture project. Update it at every meaningful checkpoint and before each branch merge.

## Project goal

Build a production-grade OSRS editor where semantic map state is authoritative, deterministic, headless-testable, and independent from UI, rendering, world-worker state, and cache codecs.

## Working rules

1. One active development branch at a time.
2. Break work into small checkpoints and stop after each checkpoint for review.
3. Do not start the next checkpoint without explicit direction.
4. A completed subsystem must pass Tier A/B/C before merge.
5. Merge completed systems back to `main`, then retire the finished branch.
6. Preserve semantic/reference parity unless a checkpoint explicitly changes behavior.
7. Keep renderer/117HD work isolated from editor-domain refactors unless an adapter change is explicitly required.
8. One logical user gesture should become one logical `MapPatch` and one history entry.
9. `MapDocument` must remain authoritative even if renderer, picker, worker, or UI caches disappear.

## Target architecture

```text
osrs-core
  Canonical semantic vocabulary: coordinates, IDs, planes, shared OSRS values.

osrs-edit
  Authoritative headless editable-map domain.
  Owns semantic state, validation, reversible patches, history, query contracts,
  and semantic change impact.

osrs-cache
  Cache codecs and cache-format infrastructure.

osrs-scene
  OSRS scene semantics.

osrs-world
  Cache-to-scene orchestration and transitional render projection.
  Must not become the authoritative edit store.

osrs-editor
  Application/product layer: persistent editor session, tools, input, previews,
  orchestration, adapters, and UI.

renderer / 117HD
  Disposable reactive projection/cache of authoritative semantic state.
```

`osrs-edit` depends only on `osrs-core`, enforced by the repository architecture guard.

## Core domain contract

### Authoritative state

`MapDocument` is the authoritative editable-map state. Renderer meshes, GPU resources, picker data, world-worker edits, and UI state are projections/caches and must never become the final edit source of truth.

### Coordinates and planes

Reuse typed coordinates and planes from `osrs-core`.

- `MapPosition` = map tile + `SourcePlane`
- `HeightVertexCoord` = terrain corner/height vertex + `SourcePlane`
- tile attributes and corner heights remain distinct semantic coordinates
- do not collapse source planes, storage planes, render levels, collision planes, or renderer-local coordinates into raw tuples

### Terrain and heights

- `TerrainTile` owns underlay/overlay/flags; height is separate
- overlay shape range is `0..=11`
- overlay rotation range is `0..=3`
- terrain storage is sparse; absent terrain is `TerrainTile::default()`
- missing height is not numeric zero

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

`MapPatch::new` derives semantic impact. `MapPatch::inverse` reverses operation order and exactly inverts each operation. `MapDocument::validate_patch` validates the complete touched state before mutation, and `MapDocument::apply_patch` applies only after full validation succeeds.

### History

`EditHistory` is failure-safe undo/redo around `MapDocument + MapPatch`.

- one successful non-empty patch = one history entry
- undo applies the exact inverse
- redo reapplies the original
- a successful new edit clears redo
- empty patches do not create history or clear redo
- failed commit/undo/redo leaves history coherent

### Read-only query boundary

`MapQuery` exposes semantic reads required by migrated planners:

```rust
trait MapQuery {
    fn terrain_at(&self, coord: MapPosition) -> TerrainTile;
    fn height_at(&self, coord: HeightVertexCoord) -> Option<i32>;
    fn contains_object(&self, placement: ObjectPlacement) -> bool;
}
```

Do not expand it speculatively.

## Merged systems

### PR #33: edit-domain foundation

Delivered `osrs-edit`, semantic values, the architecture guard, `ChangeImpact`, reversible `MapPatch`, authoritative `MapDocument`, atomic validation/application, and exact inverse round trips.

- squash merge: `5c5eef3bde696b20ae0319867e6e84567701ebf6`

### PR #34: history/query foundation

Delivered failure-safe `EditHistory` and read-only `MapQuery`.

- squash merge: `cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`

### PR #35: semantic path commit + simplified fitter

Delivered terrain-path commits through semantic `MapPatch`, deterministic Catmull-Rom + OSRS overlay-shape fitting, quality fixtures, and removal of the obsolete planner/router/network/wall/noise stack.

Do not reintroduce pathfinding, wall mode, topology repair, automatic joining, or hidden obstacle routing without an explicit product decision.

- squash merge: `cf552b734e1fdef54bf90b3b5c08211433f56a40`

### PR #36: persistent semantic editor session

Delivered the completed semantic-session subsystem:

- persistent `EditorSession` owning authoritative `MapDocument + EditHistory + revision`
- one real session owned by `EditorApp`
- canonical `MapRegionSnapshot`
- deterministic/idempotent region replacement and unload
- decoded `RegionMap` -> semantic snapshot adapter before renderer/picker/GPU extraction
- terrain-height resolution through existing terrain-load semantics
- streamed semantic hydration from decoded source data
- dirty-region protection against streaming overwrite/eviction
- explicit semantic-session reset on cache switch
- renderer, picker, GPU, and 117HD remain downstream projections

No Paint, Height, Object, or runtime undo/redo gesture was cut over in this PR.

- squash merge/current validated `main`: `ace39238f67fa5c3f31611bf60ca5c106ace1d90`
- PR: #36
- post-merge CI `38363970256`: Tier A/B/C PASS

## Current branch

Branch: `refactor/editor-paint-session`

Base: validated `main` head `ace39238f67fa5c3f31611bf60ca5c106ace1d90`.

Purpose: cut the terrain Paint tool over to the persistent semantic session in small checkpoints. Do not mix Height, Objects, renderer/117HD redesign, or unrelated editor cleanup into this branch.

## Checkpoint 17: pure semantic Paint planner

Added `crates/osrs-editor/src/app/paint_commit.rs` as a pure application-layer planner. Runtime input, world projection, renderer invalidation, and legacy history are intentionally unchanged at this checkpoint.

### Intent contract

```rust
TerrainPaintIntent {
    tile: (i32, i32),
    underlay: Option<Option<FloorUnderlayId>>,
    overlay: Option<Option<FloorOverlayId>>,
    overlay_shape: Option<(OverlayShape, OverlayRotation)>,
    height: Option<i32>,
    flags: Option<u8>,
}
```

For underlay/overlay channels:

- `None` = preserve current semantic channel
- `Some(None)` = clear channel
- `Some(Some(id))` = set channel

Shape/rotation apply only when the resulting tile actually has an overlay.

### Planner contract

`build_terrain_paint_patch(query, plane, intents)`:

- reads current authoritative state only through `MapQuery`
- produces one compact `MapPatch` for one logical Paint stroke
- preserves all unarmed semantic channels
- supports underlay set/clear
- supports overlay set/clear
- preserves existing overlay shape/rotation when replacing only overlay id
- uses canonical shape 0 / rotation 0 when adding an overlay to a tile with no prior overlay and no armed shape channel
- ignores shape/rotation after an overlay clear because no semantic overlay remains
- supports explicit Paint-height intent without treating a missing height as zero
- returns `PaintCommitError::MissingHeight` when an armed height edit targets a coordinate with no explicit semantic height
- folds repeated samples of one tile into one final terrain operation and one final height operation
- removes semantic no-ops from the final patch
- emits operations in typed-coordinate `BTreeMap` order for deterministic output
- preserves region-boundary distinctions and negative world coordinates

The planner does not mutate `EditorSession`, `MapDocument`, renderer state, worker state, or legacy edit maps itself.

### Regression coverage

Focused tests cover:

1. only armed channels change
2. overlay clear removes overlay shape/rotation semantically
3. overlay creation on an empty tile gets canonical default shape/rotation when shape is unarmed
4. semantic no-op produces an empty patch
5. repeated stroke samples collapse to final terrain/height operations
6. adjacent tiles across a 64x64 region boundary remain distinct
7. negative world coordinates remain exact
8. armed Paint-height requires an explicit semantic height

### Runtime compatibility boundary

This checkpoint deliberately does **not** change live Paint behavior yet.

Still legacy/runtime-active:

- `editing::Stroke`
- `EditorApp.edited`
- legacy `EditStep` Paint history/redo
- `record()` / `send_edits()` projection path
- current worker `TerrainEdits`
- current renderer dirty-region rebuild flow

No Height-tool, Object-tool, global undo/redo UI, renderer, cache decode, or 117HD behavior changed.

Checkpoint 17 implementation head before this handoff update:

- `cff67249c721dd4f388fb4c258261290c9832139`
- CI `38365239576`: Tier A/B/C PASS
- Tier A architecture, formatting, workspace check, strict Clippy: PASS
- Tier B full workspace tests, including all Paint-planner regressions: PASS
- Tier C M3-M9 semantic/reference parity: PASS

Final code diff at that head is intentionally narrow:

- `crates/osrs-editor/src/app.rs`: +2 module-registration lines
- `crates/osrs-editor/src/app/paint_commit.rs`: new pure planner/tests
- branch was 0 commits behind `main`

## Current architecture debt

Paint still has two representations in the application only because runtime cutover has not happened yet:

1. the new pure semantic planner contract exists
2. live Paint still mutates legacy `EditorApp.edited` and legacy history

Do **not** create a permanent synchronization model between those two. The next checkpoints must make the semantic session authoritative for Paint, then retire Paint-specific legacy authority once runtime projection and undo/redo are proven.

Height and Objects remain intentionally legacy until their own future subsystems.

## Next checkpoint

Do not begin this checkpoint without explicit direction.

### Checkpoint 18: route completed Paint strokes through `EditorSession`

Scope only the terrain Paint tool:

1. translate the completed Paint stroke into `TerrainPaintIntent` values
2. build one `MapPatch` against `self.session.document()`
3. commit that patch exactly once with `self.session.commit(...)`
4. preserve one user stroke = one semantic history entry
5. project the committed semantic result through the existing `TileEdit` / worker / dirty-region path so visible behavior remains unchanged
6. do not let the projection become a second semantic authority
7. preserve no-op behavior: an empty semantic patch must not create history or disturb redo
8. add runtime-adapter tests where practical
9. keep Height and Objects on their existing paths
10. do not cut global undo/redo over yet unless required to keep Paint state coherent; if it is required, stop and reassess the checkpoint boundary rather than broadening silently
11. run Tier A/B/C and stop for review

Only after live Paint commits authoritatively through `EditorSession` should the next checkpoint move Paint undo/redo and remove its corresponding legacy authority.

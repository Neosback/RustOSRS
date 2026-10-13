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

`MapQuery` exposes only semantic reads currently required by migrated tools:

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

Delivered:

- terrain-path commits through one semantic `MapPatch` before legacy projection
- one laid path remains one logical undo step
- deterministic Catmull-Rom + true OSRS overlay-shape fitter
- independent quality fixtures and locked deterministic outputs
- simplified Spline settings: `width`, `edge_weight`, `snap_points`
- removal of the legacy planner/router/network/wall/noise stack and obsolete fixtures

Do not reintroduce pathfinding, wall mode, topology repair, automatic joining, or hidden obstacle routing without an explicit product decision.

- merged/current `main` base: `cf552b734e1fdef54bf90b3b5c08211433f56a40`
- latest observed full `main` validation: Tier A/B/C PASS

## Current branch

Branch: `refactor/editor-session-domain`

Base: validated `main` head `cf552b734e1fdef54bf90b3b5c08211433f56a40`.

This branch establishes one persistent semantic session in the application before any editing tool is cut over from legacy authority.

### Checkpoint 14: persistent semantic editor session contract

Added `crates/osrs-editor/src/session/` with an application-layer `EditorSession` around the already-tested headless domain primitives.

```rust
EditorSession {
    document: MapDocument,
    history: EditHistory,
    revision: u64,
}

EditCommit {
    revision: u64,
    impact: ChangeImpact,
}
```

Contract established by this checkpoint:

- `MapDocument` is owned persistently by the session
- `EditHistory` remains the sole undo/redo implementation; the application layer does not duplicate its rules
- successful non-empty commit advances the session revision exactly once
- successful undo advances the session revision exactly once
- successful redo advances the session revision exactly once
- empty commits are true no-ops and preserve redo history
- undo/redo with no available entry do not advance revision
- rejected commits do not mutate the document, history, or revision
- failed undo does not advance revision or corrupt the history stack
- callers receive read-only document access through `document()`
- each successful state transition returns semantic `ChangeImpact` with its new revision

Focused regression coverage was added for all of the above behaviors.

Checkpoint 14 final handoff head:

- `a2cda76d80542725f4aafc64d197b443e2efca66`
- CI `38359876749`: Tier A/B/C PASS

### Checkpoint 15: EditorApp session ownership

`EditorApp` now owns exactly one persistent `EditorSession` for the lifetime of the application.

Ownership wiring:

- `app.rs` imports `crate::session::EditorSession`
- `EditorApp` contains a real `session: EditorSession` field
- `EditorApp::new` initializes the session deterministically with `EditorSession::default()`
- startup asserts revision zero in debug builds
- the session module is active normally from `main.rs`; the old broad module-level dead-code allowance is gone
- legacy `edited`, `height_edited`, legacy history/redo, stroke, dirty-region, and world projection fields remain untouched
- no editing gesture is routed into the session yet

The semantic document is intentionally empty at startup. Cache-loaded terrain has not been ingested into it yet. Do not fake that ownership transition by reconstructing semantic state from renderer meshes, picker data, GPU buffers, or other presentation caches.

The session mutation/query API is already regression-tested but most of it is not yet called by the normal binary. A narrow `#[cfg_attr(not(test), allow(dead_code))]` remains on the staged `EditCommit`/session API only. Tests are not suppressed. Remove that narrow allowance naturally as runtime tools begin using those methods.

No Paint, Height, Object, undo/redo UI, cache decoding, world-worker, renderer, or 117HD behavior changed in this checkpoint.

Checkpoint 15 implementation head before this handoff update:

- `398e7287c2b6acfaef0389e1c53bf0e95fc0b163`
- CI `38361076460`: Tier A/B/C PASS
- Tier A architecture, formatting, workspace check, strict Clippy: PASS
- Tier B full workspace tests: PASS
- Tier C M3-M9 semantic/reference parity: PASS

## Current architecture debt

The application now owns the semantic session, but it does not yet contain the cache-loaded map state. Legacy edit stores therefore remain the active runtime behavior and must not be deleted yet.

The migration must remove parallel authority deliberately, one source/tool at a time. Do not create a permanent synchronization model between two editable documents.

## Next checkpoint

Do **not** migrate Paint, Height, Objects, legacy undo/redo, renderer invalidation, or 117HD yet.

Next solve the semantic loaded-region ingestion boundary:

1. identify the authoritative decoded/cache-side semantic data available when a region finishes loading
2. define the smallest region snapshot/ingestion contract needed by `EditorSession`
3. preserve exact tile attributes, explicit corner heights, object placements, typed coordinates, and source-plane semantics
4. make region ingestion deterministic and idempotent
5. define region replacement/unload behavior explicitly before wiring streaming churn
6. do not derive semantic state from rendered geometry, picker caches, or GPU data
7. add focused headless tests for region ingestion/replacement semantics
8. keep current visual/editor behavior unchanged
9. run Tier A/B/C and stop for review

Only after the session can own truthful loaded semantic state should individual editing tools begin committing to it.

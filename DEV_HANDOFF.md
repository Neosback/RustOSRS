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

- merged/current `main` base before this branch: `cf552b734e1fdef54bf90b3b5c08211433f56a40`
- latest observed full `main` validation before this branch: Tier A/B/C PASS

## Current branch

Branch: `refactor/editor-session-domain`

Base: validated `main` head `cf552b734e1fdef54bf90b3b5c08211433f56a40`.

This branch completes the persistent semantic editor-session subsystem. It establishes the session contract, installs it into `EditorApp`, and hydrates it directly from decoded source-region data before any individual editing tool is cut over.

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
- each successful edit transition returns semantic `ChangeImpact` with its new revision

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

The session mutation/query API is regression-tested. A narrow `#[cfg_attr(not(test), allow(dead_code))]` remains only on staged edit APIs not yet used by runtime tools. Remove it naturally as tools cut over.

Checkpoint 15 implementation head:

- `398e7287c2b6acfaef0389e1c53bf0e95fc0b163`
- CI `38361076460`: Tier A/B/C PASS

### Checkpoint 16: semantic loaded-region ingestion

The application session now owns truthful semantic state for streamed source regions without scraping renderer output.

#### Canonical region snapshots

`osrs-edit` now exposes `MapRegionSnapshot` as the pure domain boundary for one 64x64 source region:

- exact `RegionCoord`
- sparse canonical `TerrainTile` entries
- explicit `HeightVertexCoord -> i32` values
- exact `ObjectPlacement` set
- construction rejects terrain, height, or object values assigned to the wrong region

`MapDocument::replace_region` and `MapDocument::remove_region` provide deterministic infrastructure hydration/eviction without creating user edit history. Exact repeated replacement is a true no-op; replacement removes stale values only inside the target region and preserves all other regions.

#### Decoded-source adapter

`crates/osrs-editor/src/session/region_ingest.rs` adapts `osrs_world::RegionMap` into `MapRegionSnapshot` before renderer/picker/GPU extraction participates.

The adapter:

- uses canonical typed world coordinates and `SourcePlane`
- preserves underlay, overlay id, overlay shape, overlay rotation, settings/flags, and object placements
- masks source floor ids with the same `0x7fff` semantic id mask used by scene construction
- validates overlay shape/rotation and object kind/orientation instead of clamping invalid data
- resolves final terrain heights with `osrs_scene::terrain_load::TerrainLoadGrid`, preserving default-noise, explicit-height, and lower-plane inheritance semantics
- stores the 64x64 source-owned height samples per plane; neighboring source-region boundary samples remain owned by their neighboring region rather than being invented

#### Stream integration

`RegionGeometry` now carries `Arc<RegionMap>` containing the decoded source-region state used for semantic hydration. This payload is produced before render extraction and is independent of Reference/117HD geometry.

For each current-generation `StreamEvent::Region`, `EditorApp` adapts and ingests that decoded source snapshot into its persistent session before uploading renderer projections.

Streaming lifecycle rules:

- identical clean-region rebuilds do not advance session revision
- changed clean-region snapshots replace the region deterministically and advance revision
- clean regions leaving the camera working set are evicted from the semantic document
- presentation-only rebuilds do not clear semantic session state
- a newly opened cache worker emits `Ready`, which resets the entire semantic session so data cannot leak between caches
- failed render builds do not silently destroy previously valid semantic state

#### Dirty-region protection

Once a successful semantic edit touches a region, the session marks that region dirty. Later stream snapshots or camera eviction cannot overwrite/discard that authored region state. Dirty status remains conservative across undo/redo for this subsystem; clearing/compacting dirty provenance is deferred until tool cutover and persistence semantics require it.

This prevents streaming from becoming a competing authority once semantic tools begin committing into the session.

#### Compatibility boundary

Legacy Paint, Height, Object, legacy undo/redo, `TerrainEdits`, dirty-region renderer projection, and existing world edit behavior remain active and unchanged. Transitional legacy worker edits can still appear in decoded clean-region snapshots during this migration. No editing gesture has been cut over to `EditorSession::commit` yet.

No 117HD/rendering behavior was intentionally changed. The decoded semantic source payload is orthogonal to whether the region is rendered through Reference, HD, or Reference fallback.

Checkpoint 16 implementation head before this handoff update:

- `dc7efccdb28d6d7b0bd6f1051936ba9e8f95c918`
- CI `38363291044`: Tier A/B/C PASS
- Tier A architecture, formatting, workspace check, strict Clippy: PASS
- Tier B full workspace tests, including region ingestion/replacement/dirty protection: PASS
- Tier C M3-M9 semantic/reference parity: PASS

## Persistent semantic session subsystem status

The subsystem is complete and ready to merge as one unit:

1. `EditorSession` owns authoritative `MapDocument + EditHistory + revision`.
2. `EditorApp` owns exactly one persistent session.
3. Decoded cache/world regions hydrate that session directly into canonical semantic values.
4. Region replacement and unload behavior are deterministic and idempotent.
5. Session-authored dirty regions are protected from streaming overwrite/eviction.
6. Cache switches reset semantic authority explicitly.
7. Renderer, picker, GPU, and 117HD data remain downstream projections, never semantic load sources.
8. Full Tier A/B/C is green.

After the final documentation head is validated, squash-merge this branch to `main` and retire it before beginning tool migration.

## Architecture debt after this merge

The remaining major debt is tool/runtime cutover. `EditorApp` still contains legacy editable-tile state and independent legacy undo/redo, and `osrs-world::TerrainEdits` remains a transitional projection/store.

Do not remove those structures wholesale. Cut over one complete editing system at a time so each logical gesture commits one semantic `MapPatch`, projects its result to current world/render adapters, and preserves established behavior.

## Next subsystem after merge

Start from the newly validated `main` on a fresh branch. Do not mix this into `refactor/editor-session-domain` after merge.

Recommended first tool cutover: terrain Paint.

Suggested checkpoints:

1. define/reuse a pure Paint intent/planner that reads `MapQuery` and produces one `MapPatch` per logical stroke
2. route Paint commit through the persistent `EditorSession`
3. project the resulting semantic changes through the existing world/renderer invalidation path without creating a second authority
4. preserve one stroke = one semantic history entry
5. add regression coverage for underlay, overlay, clear/no-op, repeated samples, region boundaries, and negative coordinates
6. only after Paint undo/redo uses the session successfully, remove the corresponding legacy Paint authority
7. Tier A/B/C, merge to `main`, retire that branch, then choose the next tool

Do not begin Height, Objects, renderer invalidation redesign, or 117HD work in the Paint branch unless required by a narrow adapter boundary.

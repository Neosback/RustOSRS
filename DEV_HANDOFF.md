# RustOSRS Map Editor Architecture Handoff

Last updated: 2026-10-13

This is the living handoff for the RustOSRS map-editor architecture migration. Keep it current at every meaningful checkpoint and before subsystem merges.

## Project goal

Build a production-grade OSRS editor where semantic map state is authoritative, deterministic, headless-testable, and independent from UI, rendering, world-worker state, picker state, cache codecs, and GPU resources.

Core invariant:

> If every GPU buffer, scene cache, picker cache, world-worker edit mirror, and UI widget vanished, the complete edited map would still exist correctly inside the semantic editor session.

## Working rules

1. One active development branch/system at a time.
2. Work in small checkpoints and stop after each checkpoint for review.
3. Do not begin the next checkpoint without explicit user direction.
4. A completed subsystem must pass exact-head Tier A/B/C before merge.
5. After merge, validate the exact squash commit on `main`.
6. Preserve semantic/reference parity unless a checkpoint explicitly changes behavior.
7. Keep renderer/117HD work isolated from editor-domain migration unless a narrow adapter change is required.
8. One logical user gesture should become one logical `MapPatch` and one semantic history entry.
9. `MapDocument` is authoritative; worker/render/picker/UI state is projection/cache only.
10. Do not create parallel replacement architecture crates. Use the existing workspace boundaries.

## Target architecture

```text
UI gesture
  -> tool/intent
  -> pure semantic planner
  -> MapPatch
  -> EditorSession::commit
  -> revision + ChangeImpact
  -> projection/invalidation
  -> async rebuild
  -> generation/revision check
  -> GPU replacement
```

Loading remains:

```text
cache decode
  -> semantic RegionMap / MapRegionSnapshot
  -> EditorSession / MapDocument
  -> scene construction
  -> renderer
```

Never seed semantic authority by scraping renderer meshes, picker geometry, or GPU buffers.

## Workspace mapping

Use the existing crates:

- `osrs-core`: canonical coordinates, IDs, planes, shared semantic vocabulary
- `osrs-edit`: pure/headless editable-map domain, patches, history, validation, query contracts
- `osrs-cache`: cache codecs/infrastructure
- `osrs-scene`: OSRS scene semantics
- `osrs-world`: cache-to-scene orchestration and transitional worker/render projection
- `osrs-editor`: application/session/tools/input/UI/adapters
- `osrs-render`: renderer projection
- `osrs-hd`: 117HD-related rendering work

Do not introduce parallel `domain`, `editor-core`, `editor-app`, or ECS replacement crates for this migration.

## Core domain contract

### Coordinates / values

- use typed coordinates from `osrs-core`
- `MapPosition = MapTile + SourcePlane`
- `HeightVertexCoord = MapTile corner + SourcePlane`
- `TerrainTile` owns underlay/overlay/flags; height is separate
- `OverlayShape`: `0..=11`
- `OverlayRotation`: `0..=3`
- `LocKind`: `0..=22`
- `ObjectOrientation`: `0..=3`
- missing sparse terrain = `TerrainTile::default()`
- missing height is distinct from numeric `0`
- object identity is exact `ObjectPlacement`

### Patch/history

`MapPatch` contains reversible `MapOp` values and derives `ChangeImpact` from the operations.

`MapDocument::apply_patch` validates the full touched state atomically before mutation.

`EditHistory` semantics:

- one successful non-empty patch = one semantic history entry
- undo applies the exact inverse
- redo reapplies the original
- a new successful edit clears redo
- empty patches do not create history and do not clear redo
- failed commit/undo/redo leaves state/history coherent

`MapQuery` exposes semantic reads only:

- terrain at position
- explicit height at vertex
- exact object-placement presence

## Merged architecture systems

### PR #33 — edit-domain foundation

- `osrs-edit`
- semantic values
- architecture guard
- `ChangeImpact`
- reversible `MapPatch`
- authoritative `MapDocument`
- atomic validation/application

Squash merge: `5c5eef3bde696b20ae0319867e6e84567701ebf6`

### PR #34 — history/query foundation

- failure-safe `EditHistory`
- read-only `MapQuery`

Squash merge: `cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`

### PR #35 — semantic path commit + simplified fitter

- path planning through semantic `MapPatch`
- deterministic Catmull-Rom fitting
- OSRS overlay-shape matching
- quality fixtures
- obsolete planner/router/network/wall/noise stack removed

Squash merge: `cf552b734e1fdef54bf90b3b5c08211433f56a40`

Do not reintroduce pathfinding, wall mode, topology repair, automatic joining, or hidden obstacle routing without an explicit product decision.

### PR #36 — persistent semantic editor session

- `EditorSession { MapDocument, EditHistory, revision }`
- real persistent session owned by `EditorApp`
- semantic region hydration/unload
- `MapRegionSnapshot`
- decoded `RegionMap` -> semantic snapshot adapter before render extraction
- exact terrain-height resolution through existing scene-load semantics
- dirty-region preservation across streaming churn
- session reset on cache-source switch

Squash merge / validated main: `ace39238f67fa5c3f31611bf60ca5c106ace1d90`
Post-merge CI `38363970256`: Tier A/B/C PASS

### PR #37 — Paint -> semantic session

Paint migration complete:

- pure deterministic Paint planner
- one Paint stroke -> one `EditorSession::commit`
- Paint undo/redo -> `EditorSession::undo/redo`
- worker `TileEdit` retained only as projection
- legacy Paint before/after history authority retired
- lightweight global routing marker preserves chronological mixed history
- no-op and redo invalidation behavior covered

Squash merge / validated main: `be59c01dc57e72d256ff616d1947594d939a4b5b`
Post-merge CI `38370847153`: Tier A/B/C PASS

### PR #38 — Height -> semantic session

Height migration complete:

- pure deterministic Height planner
- immutable `HeightStrokeSpec`
- ordered repeated samples preserved
- exact storage quantization after each application
- explicit lower-plane base handling; missing base is never zero
- one completed Height gesture -> one semantic commit
- deterministic selection ordering
- responsive worker preview retained as projection
- Height undo/redo -> `EditorSession::undo/redo`
- lightweight Height routing marker only; legacy Height before/after authority retired
- Paint + Height semantic history ordering covered

Squash merge / current validated `main`:
`392206ae55e35dbe26e17d664a693b21a42dc006`

Post-merge CI `38384232077`: Tier A/B/C PASS

## Semantic loading / residency details

`EditorSession` tracks loaded and dirty regions.

- clean region ingestion may replace semantic source state
- identical ingestion is a no-op
- dirty regions are not overwritten by incoming streaming snapshots
- dirty regions are not semantically evicted by camera unload
- cache-source switch resets the semantic session

`RegionGeometry` carries the decoded semantic `RegionMap` source used before scene/render extraction.

Height ingestion stores source-owned 64x64 final height samples per plane. Do not invent shared 65th boundary samples; neighboring source regions own their own boundary data.

Current transitional caveat: `WorldDefinitions::region()` can already include legacy worker edits, so region ingestion may observe transitional legacy-edited source values until all editing systems are migrated.

## Current active branch

Branch: `refactor/editor-object-session`

Base:
`392206ae55e35dbe26e17d664a693b21a42dc006`

Purpose: migrate the Object editing subsystem to the persistent semantic session.

Do not mix terrain-path history, context-menu terrain cleanup, renderer redesign, 117HD work, or unrelated editor cleanup into this branch.

## Checkpoint 23 — pure semantic Object transaction planner

Status: COMPLETE.

Implementation head:
`678a032b4ce68d2f6d675ac0fc1faca818a962ca`

CI run:
`38385929194` — Tier A/B/C PASS.

### Added

`crates/osrs-edit/src/object_edit.rs`

Exported from `osrs-edit`:

- `build_object_patch`
- `ObjectEditError`

### Planner contract

```text
build_object_patch(query, removed, added) -> Result<MapPatch, ObjectEditError>
```

The planner:

- depends only on `MapQuery`
- operates only on exact semantic `ObjectPlacement` identities
- has no renderer, picker, region-slot, `LocSpec`, worker, or UI dependency
- canonicalizes equivalent input sets into deterministic operation order
- deduplicates repeated removal/addition requests
- collapses an exact remove+add of the same placement into a semantic no-op
- validates every removal exists in the authoritative semantic document
- validates every addition is absent from the authoritative semantic document
- emits removals before additions
- represents move/turn/floor-shift naturally as one remove+add transaction
- returns one reversible `MapPatch`

Canonical ordering key:

```text
(source plane, world x, world y, object id, loc kind, orientation)
```

The plane key uses the typed `SourcePlane::index().get()` accessor.

### Regression coverage

Focused tests prove:

1. delete -> one `RemoveObject`
2. duplicate -> one `AddObject`
3. move -> remove then add in one patch
4. exact remove+add cancels to semantic no-op
5. duplicate requests collapse under set semantics
6. missing removal is rejected before patch application
7. already-present addition is rejected before patch application
8. equivalent unordered input sets produce identical canonical patches
9. turning an object changes only its exact orientation identity

### Runtime compatibility boundary

Checkpoint 23 intentionally does **not** change live Object behavior.

Still legacy at runtime:

- `object_tool::edit_objects`
- `LocSpec` as the app/worker adapter representation
- `LocEdit::Remove/Add`
- `EditStep.removed / EditStep.added`
- Object undo/redo through legacy `revert`
- object selection/reselection behavior
- drag/gizmo preview

This branch is therefore **not mergeable yet** as a completed Object subsystem.

## Next checkpoint — Checkpoint 24

Route one completed Object action through the semantic session at the commit boundary only.

Intended flow:

```text
existing selected LocSpec values
  -> exact adapter to ObjectPlacement
  -> build_object_patch(EditorSession::document, removed, added)
  -> one EditorSession::commit
  -> authoritative MapDocument
  -> project committed result through existing LocEdit worker/rebuild path
```

Checkpoint 24 scope:

- cover the existing `edit_objects` funnel used by delete, rotate, move, floor shift, duplicate, and gizmo commits
- add a strict `LocSpec <-> ObjectPlacement` adapter with typed validation
- one logical Object action -> one semantic commit/revision/history entry
- semantic no-op -> no global history entry
- preserve existing selection/reselection and worker/render behavior
- failure before projection must leave worker state unchanged
- projection failure after semantic commit must attempt semantic rollback
- add focused runtime/adapter regressions

Checkpoint 24 must **not** migrate Object undo/redo yet unless required for immediate coherence. If global history would become unsafe, stop and reassess rather than silently broadening the checkpoint.

Expected following checkpoint, not yet authorized:

- Object undo/redo -> `EditorSession::undo/redo`
- replace legacy Object before/after history with a lightweight routing marker
- retire Object legacy history authority
- then exact-head A/B/C, PR, squash merge, and post-merge main validation

## Remaining separate migration targets

After Objects are complete, still separate:

- terrain-path runtime commit/history migration from temporary semantic snapshot to persistent `EditorSession`
- context-menu `clear_overlay_at`
- remaining legacy worker/projection cleanup
- renderer invalidation/reactive projection improvements
- 117HD integration/parity work

Do not fold these into the Object subsystem branch.

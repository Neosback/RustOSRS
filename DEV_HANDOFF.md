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

## Core semantic contract

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

`MapPatch` contains reversible `MapOp` values and derives `ChangeImpact` from the operations.

`MapDocument::apply_patch` validates the complete touched state atomically before mutation.

`EditHistory` semantics:

- one successful non-empty patch = one semantic history entry
- undo applies the exact inverse
- redo reapplies the original
- a new successful edit clears semantic redo
- empty patches create no history and do not clear semantic redo
- failed commit/undo/redo leaves state/history coherent

`MapQuery` exposes semantic reads only: terrain, explicit height, and exact object-placement presence.

## Merged architecture systems

### PR #33 — edit-domain foundation

Squash merge: `5c5eef3bde696b20ae0319867e6e84567701ebf6`

- `osrs-edit`
- semantic values
- architecture guard
- `ChangeImpact`
- reversible `MapPatch`
- authoritative `MapDocument`
- atomic validation/application

### PR #34 — history/query foundation

Squash merge: `cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`

- failure-safe `EditHistory`
- read-only `MapQuery`

### PR #35 — semantic path commit + simplified fitter

Squash merge: `cf552b734e1fdef54bf90b3b5c08211433f56a40`

- path planning through semantic `MapPatch`
- deterministic Catmull-Rom fitting
- OSRS overlay-shape matching
- quality fixtures
- obsolete planner/router/network/wall/noise stack removed

Do not reintroduce pathfinding, wall mode, topology repair, automatic joining, or hidden obstacle routing without an explicit product decision.

### PR #36 — persistent semantic editor session

Squash merge / validated main: `ace39238f67fa5c3f31611bf60ca5c106ace1d90`
Post-merge CI `38363970256`: Tier A/B/C PASS.

- persistent `EditorSession { MapDocument, EditHistory, revision }`
- real session owned by `EditorApp`
- semantic region hydration/unload
- `MapRegionSnapshot`
- decoded `RegionMap` -> semantic snapshot before render extraction
- exact terrain-height resolution through scene-load semantics
- dirty-region preservation across streaming churn
- session reset on cache-source switch

### PR #37 — Paint -> semantic session

Squash merge / validated main: `be59c01dc57e72d256ff616d1947594d939a4b5b`
Post-merge CI `38370847153`: Tier A/B/C PASS.

- pure deterministic Paint planner
- one Paint stroke -> one `EditorSession::commit`
- Paint undo/redo -> `EditorSession::undo/redo`
- worker `TileEdit` projection only
- legacy Paint before/after history authority retired
- lightweight global routing marker preserves chronological mixed history

### PR #38 — Height -> semantic session

Squash merge / current validated `main`:
`392206ae55e35dbe26e17d664a693b21a42dc006`

Post-merge CI `38384232077`: Tier A/B/C PASS.

- pure deterministic Height planner
- immutable `HeightStrokeSpec`
- ordered repeated samples preserved
- exact storage quantization after each application
- explicit lower-plane base handling; missing base is never zero
- one completed Height gesture -> one semantic commit
- Height undo/redo -> `EditorSession::undo/redo`
- worker preview retained only as projection
- lightweight Height routing marker; legacy Height before/after authority retired
- Paint + Height semantic history ordering covered

## Semantic loading / residency

`EditorSession` tracks loaded and dirty regions.

- clean region ingestion may replace semantic source state
- identical ingestion is a no-op
- dirty regions are not overwritten by incoming streaming snapshots
- dirty regions are not semantically evicted by camera unload
- cache-source switch resets the semantic session

`RegionGeometry` carries the decoded semantic `RegionMap` source used before scene/render extraction.

Height ingestion stores source-owned 64x64 final height samples per plane. Do not invent shared 65th boundary samples; neighboring source regions own their own boundary data.

Transitional caveat: `WorldDefinitions::region()` can already include legacy worker edits, so region ingestion may observe transitional legacy-edited source values until all editing systems are migrated.

## Current active branch

Branch: `refactor/editor-object-session`

Base:
`392206ae55e35dbe26e17d664a693b21a42dc006`

Purpose: complete the Object editing subsystem migration to authoritative semantic state/history.

Do not mix terrain-path migration, context-menu terrain cleanup, renderer redesign, 117HD work, or unrelated editor cleanup into this branch.

## Checkpoint 23 — pure semantic Object planner

Status: COMPLETE.

Implementation head:
`678a032b4ce68d2f6d675ac0fc1faca818a962ca`

CI `38385929194`: Tier A/B/C PASS.

Final documented head:
`9c69eb2a6f807fd8139114ac9ca6cfd4119b4c3d`

CI `38386822954`: Tier A/B/C PASS.

Added `crates/osrs-edit/src/object_edit.rs` and exported:

- `build_object_patch`
- `ObjectEditError`

Planner contract:

```text
build_object_patch(query, removed, added) -> Result<MapPatch, ObjectEditError>
```

The planner:

- uses only `MapQuery` + exact `ObjectPlacement`
- is independent from renderer, picker slots, `LocSpec`, worker, and UI state
- deduplicates repeated requests
- collapses exact remove+add pairs to semantic no-op
- validates removals exist and additions are absent
- emits deterministic canonical ordering
- emits removals before additions
- represents delete/duplicate/move/turn/floor shift/multi-object actions as one reversible patch

Canonical ordering key:

```text
(source plane, world x, world y, object id, loc kind, orientation)
```

Focused tests cover delete, duplicate, move, no-op collapse, dedupe, invalid presence, canonical ordering, and orientation identity.

## Checkpoint 24 — live Object action -> semantic commit

Status: COMPLETE.

Implementation head:
`085285739f0a2090b5ccafa731ecd2c937440f74`

CI `38389419996`: Tier A/B/C PASS.

Final documented head:
`8ed687844c2bbb3cd1d554f82d26c9d9c9f695de`

CI `38389908396`: Tier A/B/C PASS.

Live Object actions now follow:

```text
keyboard / drawer / pointer drag / gizmo / floor action
  -> existing LocSpec request values
  -> strict LocSpec -> ObjectPlacement adapter
  -> build_object_patch(EditorSession::document, removed, added)
  -> one EditorSession::commit
  -> authoritative MapDocument
  -> canonical patch-derived LocEdit projection
  -> worker region rebuild
  -> pending reselect resolution
```

Covered action funnel:

- delete
- duplicate
- rotate
- tile move
- pointer drag
- floor shift
- gizmo translation
- gizmo rotation
- keyboard/drawer actions

The adapter validates source plane, loc kind, and orientation and preserves exact object id, world tile, source plane, loc kind, and orientation.

Worker `LocEdit`s are derived from the canonical semantic patch, not raw UI vectors. Duplicate requests and exact semantic no-ops therefore cannot reappear in the worker layer.

A successful Object action creates exactly one semantic history entry/revision. Rejected/no-op actions do not mutate worker state or create a global history step. Floor-shift changes `view_plane` only after a real successful commit.

Checkpoint 24 intentionally left Object undo/redo legacy, so that head was a **do-not-merge intermediate state**. Checkpoint 25 closes that temporary split-history hazard.

## Checkpoint 25 — Object undo/redo -> semantic session

Status: COMPLETE.

Implementation head:
`c3e76736e27ea623ee43c4b07d702186493c3098`

Implementation CI:
`38395996742` — Tier A/B/C PASS.

### Object history authority

Live Object actions no longer store legacy Object before/after authority in `EditStep.removed / EditStep.added`.

Instead, each migrated Object action pushes one lightweight `ObjectHistoryRoute` into the existing global chronological history stack.

`ObjectHistoryRoute` stores only canonical `LocSpec` identities needed to project the semantic state into the still-transitional worker layer:

```text
ObjectHistoryRoute {
  removed: canonical LocSpec[]
  added: canonical LocSpec[]
}
```

The route is projection metadata only. It does not contain a second `MapDocument`, does not apply semantic mutations, and does not decide undo/redo state.

### Undo flow

```text
global Object routing marker
  -> EditorSession::undo()
  -> authoritative MapDocument after inverse MapPatch
  -> validate ObjectHistoryRoute against MapDocument
  -> derive inverse LocEdit projection
  -> worker rebuild
  -> clear stale selection
  -> queue placements that now exist for reselect
  -> move global marker to redo stack
```

### Redo flow

```text
global Object routing marker
  -> EditorSession::redo()
  -> authoritative MapDocument after original MapPatch
  -> validate ObjectHistoryRoute against MapDocument
  -> derive forward LocEdit projection
  -> worker rebuild
  -> clear stale selection
  -> queue placements that now exist for reselect
  -> move global marker back to history stack
```

### Projection validation and failure safety

`build_object_history_projection` validates the entire requested worker projection against the already-transitioned authoritative document before emitting any `LocEdit` values.

For undo:

- placements being removed from the worker must already be absent semantically
- placements being restored to the worker must already be present semantically

For redo the expected identities are reversed.

A stale or misordered routing marker therefore fails before worker mutation.

If Object projection fails after a successful semantic undo/redo transition:

- undo failure attempts `EditorSession::redo()` rollback
- redo failure attempts `EditorSession::undo()` rollback
- the global routing marker is returned to its original stack
- the worker is not mutated by the failed projection validation

This matches the failure-safe routing pattern already used by Paint and Height.

### Mixed history chronology

Paint, Height, and Object now all use the same model:

```text
one successful semantic commit
  <-> one lightweight global routing marker
  <-> one EditorSession semantic history entry
```

The global stack chooses which chronological tool action the user is undoing/redoing; the semantic session decides the authoritative state transition. Object no longer bypasses the semantic history, closing the Checkpoint 24 split-history hazard.

Still-legacy terrain/path actions remain on the fallback `revert` path and do not silently mutate `EditorSession` history.

### Regression coverage added/retained

Checkpoint 25 covers:

- semantic Object undo projection
- semantic Object redo projection
- exact expected `LocEdit` ordering for both directions
- reselect identities after undo/redo
- stale Object route rejection before worker projection
- Object history marker carries no legacy tile/object authority
- existing Paint marker carries no Object route
- existing Height marker carries no Object route
- prior Object commit/no-op/canonical adapter tests remain green
- full workspace tests remain green
- M3 through M9 reference/semantic parity remains green

### Object subsystem status after Checkpoint 25

The Object editing subsystem migration is now functionally complete on this branch:

- semantic planner: complete
- live commit authority: `EditorSession`
- authoritative Object state: `MapDocument`
- undo authority: `EditorSession`
- redo authority: `EditorSession`
- worker `LocEdit`: projection only
- renderer/selection state: projection only
- legacy Object before/after history authority: retired for migrated live Object actions

The branch is now eligible for integration review/PR after the final documented-head A/B/C validation. Do not merge until explicitly authorized by the next checkpoint.

## Next boundary — not started

Object subsystem integration review:

1. verify final branch diff against current `main`
2. open the focused Object migration PR
3. exact PR-head validation/review
4. squash merge to `main`
5. validate the exact squash commit on `main`
6. retire the branch if tooling permits

Do **not** start this integration step without explicit user direction.

## Remaining separate migration targets

After Objects are merged, keep these as separate systems/checkpoints:

- terrain-path runtime commit/history migration from temporary semantic snapshot to persistent `EditorSession`
- context-menu `clear_overlay_at`
- remaining legacy worker/projection cleanup
- renderer invalidation/reactive projection improvements
- 117HD integration/parity work

Do not fold these into the Object subsystem branch.

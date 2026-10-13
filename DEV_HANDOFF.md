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
7. Keep renderer/117HD work isolated from editor-domain refactors unless a narrow adapter change is required.
8. One logical user gesture should become one logical `MapPatch` and one semantic history entry.
9. `MapDocument` remains authoritative even if renderer, picker, worker, or UI caches disappear.

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

`MapDocument` is the authoritative editable-map state. Renderer meshes, GPU resources, picker data, world-worker edits, and UI state are projections/caches, not final edit authority.

### Coordinates and terrain

- use typed coordinates and planes from `osrs-core`
- `MapPosition` = map tile + `SourcePlane`
- `HeightVertexCoord` = terrain corner + `SourcePlane`
- `TerrainTile` owns underlay/overlay/flags; height is separate
- overlay shape range is `0..=11`
- overlay rotation range is `0..=3`
- missing terrain is `TerrainTile::default()`
- missing height is not numeric zero

### Patch/history model

`MapPatch` contains reversible semantic `MapOp` values and derives `ChangeImpact` from them. `MapDocument::apply_patch` validates the full touched state before mutation.

`EditHistory` is the only intended long-term undo/redo authority:

- one successful non-empty patch = one history entry
- undo applies the exact inverse
- redo reapplies the original
- a successful new edit clears redo
- empty patches do not create history or clear redo
- failed commit/undo/redo leaves history coherent

`MapQuery` exposes only semantic reads needed by migrated planners: terrain, height, and object presence.

## Merged systems

### PR #33: edit-domain foundation

- `osrs-edit`, semantic values, architecture guard
- `ChangeImpact`, reversible `MapPatch`, authoritative `MapDocument`
- atomic validation/application and inverse round trips
- squash merge: `5c5eef3bde696b20ae0319867e6e84567701ebf6`

### PR #34: history/query foundation

- failure-safe `EditHistory`
- read-only `MapQuery`
- squash merge: `cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`

### PR #35: semantic path commit + simplified fitter

- terrain-path commits through semantic `MapPatch`
- deterministic Catmull-Rom + OSRS overlay-shape fitting
- quality fixtures
- removed obsolete planner/router/network/wall/noise stack
- squash merge: `cf552b734e1fdef54bf90b3b5c08211433f56a40`

Do not reintroduce pathfinding, wall mode, topology repair, automatic joining, or hidden obstacle routing without an explicit product decision.

### PR #36: persistent semantic editor session

Delivered the semantic-session subsystem:

- persistent `EditorSession` owning `MapDocument + EditHistory + revision`
- one real session owned by `EditorApp`
- canonical `MapRegionSnapshot`
- deterministic/idempotent region replacement and unload
- decoded `RegionMap` -> semantic snapshot adapter before renderer/picker/GPU extraction
- terrain-height resolution through existing terrain-load semantics
- streamed semantic hydration from decoded source data
- dirty-region protection against streaming overwrite/eviction
- semantic-session reset on cache switch
- renderer, picker, GPU, and 117HD remain downstream projections

No Paint, Height, Object, or runtime undo/redo gesture was cut over in PR #36.

- squash merge/current validated `main`: `ace39238f67fa5c3f31611bf60ca5c106ace1d90`
- PR: #36
- post-merge CI `38363970256`: Tier A/B/C PASS

## Current branch

Branch: `refactor/editor-paint-session`

Base: validated `main` head `ace39238f67fa5c3f31611bf60ca5c106ace1d90`.

Purpose: cut terrain Paint over to the persistent semantic session. Do not mix Height, Objects, renderer/117HD redesign, or unrelated editor cleanup into this branch.

## Checkpoint 17: pure semantic Paint planner

Added `crates/osrs-editor/src/app/paint_commit.rs`.

`TerrainPaintIntent` models independently armed underlay, overlay, overlay shape/rotation, height, and flags. `build_terrain_paint_patch(query, plane, intents)`:

- reads authoritative state through `MapQuery`
- emits a compact deterministic `MapPatch`
- preserves unarmed channels
- supports underlay/overlay set and clear
- preserves shape/rotation when replacing only overlay ID
- uses canonical shape 0 / rotation 0 for a new overlay when shape is unarmed
- treats missing height as missing, never zero
- folds repeated samples to final operations
- removes semantic no-ops
- handles region boundaries and negative coordinates exactly

Focused regression coverage exists for all of the above.

Checkpoint 17 validated head:

- `cff67249c721dd4f388fb4c258261290c9832139`
- CI `38365239576`: Tier A/B/C PASS

## Checkpoint 18: live Paint stroke -> authoritative semantic commit

Live terrain Paint now commits through the persistent `EditorSession` at stroke completion while preserving the current immediate worker/render preview during the drag.

### Runtime flow

```text
pointer drag
  -> legacy live TileEdit preview (unchanged responsiveness)
  -> completed Stroke
  -> captured Paint Brush
  -> TerrainPaintIntent values
  -> one combined MapPatch
  -> EditorSession::commit exactly once
  -> reconcile semantic result into current TileEdit/world projection
  -> existing dirty-region renderer rebuild path
```

The semantic commit, not `EditorApp.edited` or worker `TerrainEdits`, is the authoritative completed Paint transaction.

### Stroke semantics

- the exact `Brush` first used by a Paint stroke is captured in `Stroke::paint_brush`
- later UI brush changes cannot silently change the meaning of an already-open stroke
- finished Paint tile transitions are translated into typed semantic intents
- changes are grouped by `SourcePlane`, then combined into one final `MapPatch`
- one Paint stroke creates one `EditorSession::commit`, even if the adapter contains changes on more than one source plane
- armed Paint height uses the already-quantized runtime result as the semantic target
- semantic no-op patches do not create semantic history
- semantic planning or commit failure rolls the already-visible preview back through the existing projection path
- after a successful commit, the visible legacy projection is reconciled to the authoritative semantic result; this canonicalizes cases such as adding an overlay to a semantically empty tile with unarmed shape/rotation

### Compatibility boundary

This checkpoint intentionally leaves live preview/projection infrastructure in place:

- `EditorApp.edited` still caches/projectively mirrors visible tile state
- `record()` and `send_edits()` still feed current worker `TerrainEdits`
- renderer dirty-region rebuild behavior is unchanged
- the completed Paint step is still copied to legacy `EditStep` history temporarily so existing global UI state remains compatible

That legacy `EditStep` is now transitional compatibility state for Paint, not the semantic source of truth.

Global Ctrl/Cmd+Z and redo are **not cut over yet**. They still pop the legacy history stack. Therefore the branch must not merge yet: semantic Paint history and global legacy history need to be unified before Paint migration is complete.

Also intentionally unchanged:

- Height tool
- Object tool
- global undo/redo routing
- context-menu `clear_overlay_at` legacy action
- existing terrain-path transitional bridge
- cache decode/region ingestion
- renderer/117HD behavior

### Regression coverage

Added an adapter-level regression proving that one Paint stroke containing multiple semantic operations across source planes:

- produces one combined patch
- advances `EditorSession` revision once
- creates one semantic undo entry
- is fully reversed by one semantic undo

Checkpoint 18 implementation head before this handoff update:

- `0996bfcc7a3c58eb302585f83eb33900746d18d1`
- CI `38367658420`: Tier A/B/C PASS
- Tier A architecture, formatting, workspace check, strict Clippy: PASS
- Tier B full workspace tests, including the Paint runtime adapter regression: PASS
- Tier C M3-M9 semantic/reference parity: PASS

## Remaining Paint architecture debt

Paint is now semantically authoritative when a stroke completes, but two temporary compatibility structures remain:

1. live legacy projection state (`EditorApp.edited` / worker `TerrainEdits`) used to preserve current rendering behavior
2. legacy `EditStep` history used by the current global undo/redo UI

Do not turn either into a permanent second authority.

Height and Objects remain intentionally legacy until their own future subsystems.

## Next checkpoint

Do not begin without explicit direction.

### Checkpoint 19: Paint undo/redo cutover

Scope only Paint history synchronization:

1. make user-facing undo/redo for semantic Paint invoke `EditorSession::undo/redo`
2. project the resulting authoritative semantic state back through the existing `TileEdit` / worker / dirty-region path
3. preserve correct interleaving behavior with still-legacy Height/Object actions; if a safe mixed-history boundary cannot be kept narrow, stop and document the required history-router design instead of silently broadening scope
4. remove Paint's dependence on legacy `EditStep` as undo authority once semantic undo/redo is proven
5. keep live worker/render projection as a cache/adapter only
6. add regression coverage for Paint commit -> undo -> redo, no-op history, and redo invalidation after a new Paint commit
7. do not migrate Height or Objects
8. do not redesign renderer invalidation or 117HD
9. run Tier A/B/C and stop for review

Only after Paint commit + undo + redo are all semantic and the corresponding legacy Paint history authority is retired should this branch be considered for merge.
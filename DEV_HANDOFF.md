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

Live terrain Paint commits through the persistent `EditorSession` at stroke completion while preserving the current immediate worker/render preview during the drag.

```text
pointer drag
  -> legacy live TileEdit preview
  -> completed Stroke
  -> captured Paint Brush
  -> TerrainPaintIntent values
  -> one combined MapPatch
  -> EditorSession::commit exactly once
  -> reconcile semantic result into current TileEdit/world projection
  -> existing dirty-region renderer rebuild path
```

Key behavior:

- the exact `Brush` first used by a Paint stroke is captured in `Stroke::paint_brush`
- finished Paint transitions are translated into typed semantic intents
- changes are grouped by `SourcePlane` and combined into one final `MapPatch`
- one Paint stroke creates one semantic history entry even across source planes
- armed Paint height uses the already-quantized runtime result
- semantic no-op patches do not create history
- planning/commit failure rolls the preview back through the existing projection path
- successful commits reconcile visible projection from authoritative semantic state

Checkpoint 18 validated head:

- `dfee0084867b301288a5749cd6f2225a2b3abd53`
- CI `38368016985`: Tier A/B/C PASS

## Checkpoint 19: Paint undo/redo -> authoritative semantic history

User-facing Paint undo/redo now routes through `EditorSession::undo/redo`. Paint no longer stores legacy before/after tile values as undo authority.

### Mixed-history router

Height, Objects, terrain-path compatibility actions, and context-menu clear-overlay are still legacy. To preserve chronological global Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z without migrating those systems prematurely, the existing editor history stack now acts as a routing ledger:

- legacy entries still carry their existing `EditStep` before/after/object data
- semantic Paint entries carry only `SemanticPaintHistory { brush, keys }`
- the semantic Paint marker contains no legacy tile before/after state
- popping a Paint marker dispatches to `EditorSession::undo/redo`
- popping a legacy entry continues to use the existing `revert` path

This keeps one chronological user-facing history order while leaving Paint state authority exclusively in `EditorSession`.

### Semantic Paint projection

After semantic Paint undo/redo, `project_semantic_paint_history` reads the authoritative `MapDocument` and updates the existing `EditorApp.edited` / `TileEdit` / worker path only as a projection.

Projection is channel-aware:

- only channels armed by the original Paint brush are projected from semantic state
- unarmed channels remain untouched, which allows still-legacy actions on other channels to interleave without Paint overwriting them
- overlay projection includes canonical semantic shape/rotation when overlay was armed
- Paint height projection requires an explicit semantic height
- projection values are prepared before mutation so adapter failure does not leave a partially projected Paint step
- if projection fails after semantic undo/redo, the code attempts the inverse semantic operation and restores the routing marker to its original global stack

Live drag preview remains transitional infrastructure for responsiveness; it is not completed-edit authority.

### Redo/no-op semantics

- an empty Paint patch creates no semantic history and no routing marker
- one Paint commit -> one semantic undo -> one semantic redo restores the authoritative states
- a new semantic Paint commit after undo invalidates semantic redo through `EditHistory::commit`
- `can_undo` / `can_redo` remain based on the chronological global routing stacks so hidden semantic history cannot bypass still-legacy actions

### Regression coverage

Checkpoint 19 adds/keeps focused tests proving:

1. one Paint stroke is one semantic session history entry
2. semantic Paint projection follows commit -> undo -> redo
3. unarmed height survives Paint undo/redo projection unchanged
4. semantic Paint no-op creates no history
5. a new semantic Paint commit invalidates semantic redo
6. a semantic Paint routing marker carries no legacy tile/object authority

### Deliberately unchanged

- Height remains legacy
- Objects remain legacy
- terrain-path runtime bridge remains transitional
- context-menu `clear_overlay_at` remains legacy
- `EditorApp.edited` and worker `TerrainEdits` remain live projection/cache infrastructure
- renderer invalidation and 117HD are unchanged

Checkpoint 19 implementation head before this handoff update:

- `7cb50b1f040796c2b7d733429d76aa88a7029be7`
- CI `38370104124`: Tier A/B/C PASS
- Tier A architecture, formatting, workspace check, strict Clippy: PASS
- Tier B full workspace tests including semantic Paint history/projection regressions: PASS
- Tier C M3-M9 semantic/reference parity: PASS

## Paint subsystem status

The terrain Paint tool migration is now complete for the intended branch scope:

- planning is semantic
- completed Paint commits are semantic
- Paint undo is semantic
- Paint redo is semantic
- legacy Paint `EditStep` before/after tile authority is retired
- live worker/render structures are projection only

The global history stack still exists because non-Paint tools remain legacy, but Paint contributes only a lightweight routing marker for chronological interleaving. Do not expand that marker into a second Paint state store.

This branch is now eligible for merge after the exact documentation head passes Tier A/B/C.

## Next subsystem

Do not begin without explicit direction and do not continue it on this completed Paint branch.

After this branch is merged, start a fresh branch from validated `main`. The next editor-domain migration should be chosen as its own subsystem; Height is the natural next terrain candidate because it still owns legacy height mutation/history, but no Height work is authorized by this handoff alone.

# RustOSRS Map Editor Architecture Handoff

Last updated: 2026-10-13

This is the living handoff for the RustOSRS map-editor architecture migration. Keep it current at meaningful checkpoints and before subsystem merges.

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
11. Prefer modules below 1,000 LOC. Split by reason to change before a module becomes a god file.
12. `osrs-editor` Rust modules have a hard CI ceiling of 1,600 lines and 64 KiB so whole-file tooling remains comfortably safe.

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

- typed coordinates come from `osrs-core`
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
- decoded `RegionMap` -> semantic snapshot before render extraction
- exact terrain-height resolution through scene-load semantics
- dirty-region preservation across streaming churn
- session reset on cache-source switch

### PR #37 — Paint -> semantic session

Squash merge / validated main: `be59c01dc57e72d256ff616d1947594d939a4b5b`
Post-merge CI `38370847153`: Tier A/B/C PASS.

- one Paint stroke -> one semantic commit/history entry
- Paint undo/redo -> `EditorSession`
- worker `TileEdit` projection only
- legacy Paint before/after authority retired

### PR #38 — Height -> semantic session

Squash merge / current validated `main`:
`392206ae55e35dbe26e17d664a693b21a42dc006`

Post-merge CI `38384232077`: Tier A/B/C PASS.

- deterministic Height planner and immutable stroke spec
- exact storage quantization and lower-plane handling
- one completed Height gesture -> one semantic commit
- Height undo/redo -> `EditorSession`
- worker preview projection only
- legacy Height before/after authority retired

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

Purpose: complete and integrate the Object editing subsystem migration to authoritative semantic state/history.

Do not mix terrain-path migration, context-menu terrain cleanup, renderer redesign, 117HD work, or unrelated UI cleanup into this branch.

## Checkpoint 23 — pure semantic Object planner

Status: COMPLETE.

Implementation head: `678a032b4ce68d2f6d675ac0fc1faca818a962ca`
CI `38385929194`: Tier A/B/C PASS.

Final documented head: `9c69eb2a6f807fd8139114ac9ca6cfd4119b4c3d`
CI `38386822954`: Tier A/B/C PASS.

Added `osrs-edit::build_object_patch` and `ObjectEditError`.

Planner properties:

- `MapQuery` + exact `ObjectPlacement` only
- no UI/renderer/picker/worker dependency
- deterministic canonical ordering
- repeated requests deduplicated
- exact remove+add collapses to no-op
- removals must exist; additions must be absent
- removals precede additions
- delete/duplicate/move/turn/floor shift/multi-object actions are one reversible patch

## Checkpoint 24 — live Object action -> semantic commit

Status: COMPLETE.

Implementation head: `085285739f0a2090b5ccafa731ecd2c937440f74`
CI `38389419996`: Tier A/B/C PASS.

Final documented head: `8ed687844c2bbb3cd1d554f82d26c9d9c9f695de`
CI `38389908396`: Tier A/B/C PASS.

All current Object action sources converge on one semantic transaction funnel:

```text
Object action
  -> strict LocSpec -> ObjectPlacement adapter
  -> build_object_patch(EditorSession::document, ...)
  -> one EditorSession::commit
  -> authoritative MapDocument
  -> canonical LocEdit projection
  -> worker rebuild/reselect
```

The strict adapter validates plane, loc kind, and orientation before mutation. Canonical patch operations drive worker projection, so duplicate requests and semantic no-ops cannot leak into the worker layer.

Checkpoint 24 intentionally left Object undo/redo legacy and was a do-not-merge intermediate state.

## Checkpoint 25 — Object undo/redo -> semantic session

Status: COMPLETE.

Implementation head: `c3e76736e27ea623ee43c4b07d702186493c3098`
Implementation CI `38395996742`: Tier A/B/C PASS.

Final documented head before integration hardening:
`a04a58987225be737ba12c409b4f157513d9cb41`
CI `38396388862`: Tier A/B/C PASS.

Live Object actions no longer use `EditStep.removed / EditStep.added` as Object undo authority. Each action stores one lightweight `ObjectHistoryRoute` containing only canonical `LocSpec` identities needed for downstream projection.

Undo/redo model:

```text
global chronological route
  -> EditorSession::undo / redo
  -> authoritative MapDocument
  -> validate route against resulting semantic state
  -> derive LocEdit projection
  -> worker rebuild/reselect
```

If projection validation fails after semantic history moves, the inverse semantic operation is attempted and the global marker returns to its original stack. Stale/misordered routes therefore fail before worker mutation.

Paint, Height, and Object now share the same one-commit / one-route / one-semantic-history chronology.

## Checkpoint 26 — Object integration hardening / anti-god-module guard

Status: IMPLEMENTATION COMPLETE; PR creation pending final documented-head validation.

Implementation head:
`d1a5c154d45711999fea1181278fa7a348227d3f`

Implementation CI:
`38406565544` — Tier A/B/C PASS.

### `editing.rs` responsibility split

The branch had grown `crates/osrs-editor/src/app/editing.rs` to roughly 1,830 LOC / 68 KiB. It was still below GitHub's absolute file API limits, but whole-file connector edits were becoming unnecessarily fragile and the module mixed too many reasons to change.

Checkpoint 26 splits it by responsibility:

```text
app/editing.rs
  - stroke construction/input
  - Paint/Height semantic commit bridge
  - legacy terrain-path bridge
  - tile worker projection
  - dirty-region rebuild requests

app/editing/history.rs
  - EditStep / semantic routing markers
  - global chronological history stack
  - Paint/Height/Object semantic undo/redo routing
  - Object LocEdit projection dispatch

app/editing/tests.rs
  - editing/history regression coverage
```

`editing.rs` is now roughly 950 LOC, below the project's <1,000 LOC soft design target.

No Object/Paint/Height semantic behavior changed in the split. Tier B and M3-M9 parity remained green.

### API-safe editor module guard

`scripts/check_architecture.py` now scans every `crates/osrs-editor/src/**/*.rs` file.

Hard CI limits:

- maximum 1,600 lines per Rust module
- maximum 64 KiB per Rust module

These are intentionally much stricter than GitHub's absolute API maximum. They are a repository maintainability/tooling ceiling so files are split before whole-file editing becomes fragile.

Soft design target:

- prefer <1,000 LOC
- typical focused modules should be substantially smaller
- split by reason to change, not arbitrary line chunks

The architecture-guard regression suite now verifies:

- the current editor tree satisfies the size policy
- line-cap violations fail
- byte-cap violations fail

Existing unrelated UI debt such as `settings_window.rs` remains below the hard ceiling and is deliberately not refactored on the Object branch. Future cleanup should be a separate focused checkpoint.

### Checkpoint 26 integration status

At implementation head `d1a5c154...`:

- architecture boundaries: PASS
- editor module-size guard: PASS
- rustfmt: PASS
- workspace compile: PASS
- strict Clippy: PASS
- Tier B workspace tests: PASS
- Tier C M3-M9 semantic/reference parity: PASS

The Object subsystem remains functionally complete and is now structurally safer to maintain through GitHub/API tooling.

## Next boundary — Object integration PR

After this documented head passes exact A/B/C:

1. verify branch is still based cleanly on current `main`
2. open the focused Object migration PR
3. stop for review before merge
4. on explicit continuation: verify PR head, squash merge, validate exact squash commit on `main`, and retire the branch if tooling permits

Do not merge during Checkpoint 26.

## Remaining separate migration targets

After Objects are merged, keep these as separate systems/checkpoints:

- terrain-path runtime commit/history migration from temporary semantic snapshot to persistent `EditorSession`
- context-menu `clear_overlay_at`
- remaining legacy worker/projection cleanup
- renderer invalidation/reactive projection improvements
- 117HD integration/parity work
- unrelated large UI-module cleanup such as `settings_window.rs`

Do not fold these into the Object subsystem branch.

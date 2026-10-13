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
5. After merge, validate the exact integrated commit on `main`.
6. Preserve semantic/reference parity unless a checkpoint explicitly changes behavior.
7. Keep renderer/117HD work isolated from editor-domain migration unless a narrow adapter change is required.
8. One logical user gesture should become one logical `MapPatch` and one semantic history entry.
9. `MapDocument` is authoritative; worker/render/picker/UI state is projection/cache only.
10. Use the existing workspace boundaries; do not create parallel replacement architecture crates.
11. Prefer actively edited modules below ~1,000 LOC and split by reason to change.
12. Every Rust module under `crates/osrs-editor/src` has a hard CI ceiling of 1,600 lines and 64 KiB.

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

- `osrs-core`: typed coordinates, IDs, planes, shared semantic vocabulary
- `osrs-edit`: pure/headless editable domain, patches, history, validation, queries
- `osrs-cache`: cache codecs/infrastructure
- `osrs-scene`: OSRS scene semantics
- `osrs-world`: cache/scene orchestration and transitional worker projection
- `osrs-editor`: application/session/tools/input/UI/adapters
- `osrs-render`: renderer projection
- `osrs-hd`: 117HD rendering work

## Core semantic contract

- typed coordinates come from `osrs-core`
- `MapPosition = MapTile + SourcePlane`
- `HeightVertexCoord = MapTile corner + SourcePlane`
- `TerrainTile` owns underlay/overlay/flags; height is separate
- missing sparse terrain = `TerrainTile::default()`
- missing height is distinct from numeric `0`
- object identity is exact `ObjectPlacement`
- `MapPatch` contains reversible `MapOp`s and derives `ChangeImpact`
- `MapDocument::apply_patch` validates the complete touched state atomically before mutation
- one successful non-empty patch = one semantic history entry
- empty patches create no history and do not clear redo
- failed commit/undo/redo leaves state/history coherent

## Merged architecture systems

### PR #33 — edit-domain foundation
Squash merge: `5c5eef3bde696b20ae0319867e6e84567701ebf6`

### PR #34 — history/query foundation
Squash merge: `cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`

### PR #35 — semantic path planner + simplified fitter
Squash merge: `cf552b734e1fdef54bf90b3b5c08211433f56a40`

- deterministic Catmull-Rom fitting
- OSRS overlay-shape matching
- pure terrain-path `MapPatch` planner
- obsolete pathfinding/router/network/wall/noise stack removed

Do not reintroduce pathfinding, wall mode, topology repair, automatic joining, or hidden obstacle routing without an explicit product decision.

### PR #36 — persistent semantic editor session
Squash merge: `ace39238f67fa5c3f31611bf60ca5c106ace1d90`
Post-merge CI `38363970256`: Tier A/B/C PASS.

- persistent `EditorSession { MapDocument, EditHistory, revision }`
- semantic region hydration/unload
- dirty-region preservation across streaming churn
- cache-source reset boundary

### PR #37 — Paint -> semantic session
Squash merge: `be59c01dc57e72d256ff616d1947594d939a4b5b`
Post-merge CI `38370847153`: Tier A/B/C PASS.

- Paint commit + undo/redo authoritative through `EditorSession`
- worker `TileEdit` is projection only

### PR #38 — Height -> semantic session
Squash merge: `392206ae55e35dbe26e17d664a693b21a42dc006`
Post-merge CI `38384232077`: Tier A/B/C PASS.

- Height commit + undo/redo authoritative through `EditorSession`
- exact storage quantization and explicit lower-plane semantics
- worker preview remains projection only

### PR #39 — Object -> semantic session + editor module hardening
Merged by user as a regular merge commit:
`50960fddbcc6851dea285b4b90c51b806fd70215`

Post-merge CI `38410554574`: Tier A/B/C PASS, including M3-M9.
The former `refactor/editor-object-session` branch is retired.

Object status:

- pure deterministic Object planner
- all live Object actions commit once through `EditorSession`
- Object undo/redo use `EditorSession`
- worker `LocEdit` is projection only
- stale projection routes validate against authoritative `MapDocument`
- Paint/Height/Object share one chronological global-routing / semantic-history order

Module hardening merged with PR #39:

- old `app/editing.rs` (~1,830 LOC / ~68 KiB on the Object branch) was split by responsibility
- `editing/history.rs` owns chronological semantic history routing/projection
- `editing/tests.rs` owns editing/history regressions
- hard CI module guard: 1,600 lines and 64 KiB
- soft design target: <1,000 LOC where practical

## Semantic loading / residency

`EditorSession` tracks loaded and dirty regions.

- clean region ingestion may replace semantic source state
- identical ingestion is a no-op
- dirty regions are not overwritten by incoming streaming snapshots
- dirty regions are not semantically evicted by camera unload
- cache-source switch resets the semantic session

Height ingestion stores source-owned 64x64 final height samples per plane. Do not invent shared 65th boundary samples; neighboring source regions own their own boundary data.

Transitional caveat: `WorldDefinitions::region()` can include legacy worker edits until all editing systems are migrated.

## Current active branch

Branch: `refactor/editor-path-session`

Base / validated `main`:
`50960fddbcc6851dea285b4b90c51b806fd70215`

Purpose: migrate live terrain-path commit/history from the temporary semantic snapshot bridge to the persistent `EditorSession` without mixing context-menu, renderer, or 117HD work.

## Checkpoint 27 — live terrain path -> persistent semantic commit

Status: IMPLEMENTATION COMPLETE; **DO NOT MERGE YET**.

Implementation head:
`2245f2dc027123362e1d38869981e56b4fdc1aba`

Implementation CI:
`38412603434` — Tier A/B/C PASS, including M3-M9.

### Runtime authority

Before this checkpoint, `apply_terrain_path` built a temporary `MapDocument`, applied the semantic path patch there, then copied the result back into legacy tile state. The persistent `EditorSession` did not receive the path transaction.

Checkpoint 27 changes the live path commit boundary to:

```text
fitted PathTile[]
  -> TerrainPathIntent[]
  -> build_terrain_path_patch(EditorSession::document, ...)
  -> projection preflight on a disposable scratch clone
  -> one EditorSession::commit
  -> authoritative MapDocument
  -> existing TileEdit worker projection
```

The scratch `MapDocument` clone is projection preflight only. It receives the exact already-built patch solely to prove that the authoritative result can be represented by the transitional worker adapter before the real session is mutated. It is never editor authority and is discarded before worker mutation.

### Height semantics

When the path brush has Height armed:

- plane 0 uses storage base `0`
- upper planes require the actual semantic lower-plane height
- a missing lower-plane base rejects the path instead of silently substituting zero
- the pure path planner still requires an explicit existing semantic height vertex on the edited plane
- storage quantization remains consistent with existing cache representation

### Failure/no-op behavior

Before `EditorSession::commit`, the adapter preflights the complete worker projection and rejects if:

- a required projection tile is unavailable
- semantic terrain IDs cannot fit the transitional worker representation
- a required semantic height is missing
- the semantic patch cannot produce a representable worker change

An empty semantic path patch creates no semantic history entry.

A successful non-empty path operation creates exactly one persistent semantic commit/revision, then mirrors the final semantic result into the existing `edited` / `height_edited` / `TileEdit` worker projection.

### Anti-god/module split

Path runtime responsibility moved out of `app/editing.rs` into:

`crates/osrs-editor/src/app/editing/path.rs`

This removes roughly another 166 lines from the editing shell instead of adding new path logic back into a growing central module. `editing/path.rs` is a focused ~290-line runtime adapter/test module and remains far below the repository hard module-size ceiling.

Existing Paint/Height/input/rebuild behavior remains in `editing.rs`; path fitting remains in `terrain_path` / `spline_tool`; pure semantic path planning remains in `app/path_commit.rs`.

### Regression coverage

Checkpoint 27 adds/retains coverage that:

- upper-plane path Height requires a real semantic lower-plane base
- one non-empty path patch is one persistent `EditorSession` revision/history entry
- full workspace tests remain green
- M3-M9 semantic/reference parity remains green
- architecture/module-size guard remains green

### IMPORTANT temporary history hazard

Path **commit** is now authoritative in `EditorSession`, but global path **undo/redo is still legacy `EditStep.changes`** in this checkpoint.

Therefore this branch is intentionally not mergeable yet. A path commit followed by legacy path undo can visually/worker-revert the path while leaving the path transaction on top of semantic history, causing the same split-history hazard previously closed for Paint, Height, and Objects.

Do not merge Checkpoint 27 by itself.

## Next checkpoint — not started

Checkpoint 28: terrain-path undo/redo -> semantic session.

Narrow target:

1. replace path legacy before/after history authority with lightweight projection-routing metadata
2. route path undo through `EditorSession::undo()`
3. route path redo through `EditorSession::redo()`
4. validate authoritative post-transition terrain/height state before worker projection
5. roll semantic history back if projection preparation fails
6. preserve mixed Paint + Height + Object + Path chronological ordering
7. remove migrated path dependence on legacy `EditStep.changes`
8. exact-head Tier A/B/C
9. only after the full Path subsystem is green should a PR/merge be considered

Do not start Checkpoint 28 without explicit user direction.

## Remaining separate migration targets

After Path is complete and merged:

- context-menu `clear_overlay_at`
- remaining legacy worker/projection cleanup
- renderer invalidation/reactive projection improvements
- 117HD integration/parity work
- unrelated large UI-module cleanup such as `settings_window.rs`

Keep these separate from the Path branch.

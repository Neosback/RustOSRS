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
5. Merge completed systems back to `main`, then retire the finished branch when tooling permits.
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

- squash merge: `ace39238f67fa5c3f31611bf60ca5c106ace1d90`
- PR: #36
- post-merge CI `38363970256`: Tier A/B/C PASS

### PR #37: terrain Paint -> semantic session

Completed the Paint migration:

- pure deterministic Paint planner
- one completed Paint stroke -> one `EditorSession::commit`
- Paint undo/redo -> `EditorSession::undo/redo`
- live `TileEdit` / worker state retained only as projection for responsiveness
- Paint legacy before/after `EditStep` state retired as undo authority
- global history stack retains only a lightweight Paint routing marker so chronological interleaving with still-legacy actions is preserved
- semantic no-op and redo invalidation behavior covered by regression tests

- squash merge/current validated `main`: `be59c01dc57e72d256ff616d1947594d939a4b5b`
- PR: #37
- post-merge CI `38370847153`: Tier A/B/C PASS

The merged Paint feature branch still exists because the connected GitHub tooling does not expose safe branch deletion. It is safe to remove manually.

## Current branch

Branch: `refactor/editor-height-session`

Base: validated `main` head `be59c01dc57e72d256ff616d1947594d939a4b5b`.

Purpose: migrate the dedicated Height tool to the persistent semantic session. Do not mix Objects, terrain-path migration, context-menu terrain actions, renderer/117HD redesign, or unrelated editor cleanup into this branch.

## Checkpoint 20: pure semantic Height stroke planner

Added `crates/osrs-editor/src/heights/commit.rs` and registered it beneath the existing `heights` module. Runtime Height behavior is intentionally unchanged in this checkpoint.

### Captured stroke meaning

`HeightStrokeSpec` captures the immutable semantic meaning of one Height stroke:

- `SourcePlane`
- `HeightTool`
- brush radius
- brush shape
- flatten anchor
- deterministic stroke seed

This is deliberately one value rather than a wide planner argument list. It also establishes the contract needed for the runtime cutover: once a Height stroke starts, later UI preference changes must not silently alter the operation that eventually commits.

### Planner contract

`build_terrain_height_patch(query, spec, centers)` produces one deterministic `MapPatch` from ordered Height-tool samples.

The planner:

- reads authoritative height state only through `MapQuery`
- addresses terrain corners with typed `MapTile + SourcePlane -> HeightVertexCoord`
- preserves the order of stroke samples
- allows repeated centers because a held sculpt brush intentionally applies repeatedly
- makes later samples see the storage-quantized result of earlier samples
- evaluates each individual brush application against a stable pre-application snapshot, matching the current live tool
- uses the existing pure Raise / Lower / Smooth / Flatten / Roughen arithmetic
- preserves deterministic Roughen behavior through the captured stroke seed
- skips missing target heights instead of inventing numeric zero
- folds every touched corner into one final `SetHeight { before, after }`
- removes final semantic no-ops
- emits operations in deterministic coordinate order through `BTreeMap`
- preserves negative world coordinates exactly

### Exact OSRS height-storage quantization

The planner uses the existing `encode_height` / `decode_height` rules after each application, not only at final commit. This is important because subsequent repeated Height samples read the already-quantized result in the current runtime.

Plane handling is explicit:

- plane 0 uses cache base height `0`
- planes 1..=3 encode relative to the explicit height on the plane directly below
- a missing lower-plane base returns `HeightCommitError::MissingBaseHeight`
- missing lower-plane data is **not** treated as height zero

The last rule is an intentional semantic hardening over the legacy `EditorApp::base_height`, which currently falls back to zero when the lower tile cannot be found. It preserves the domain invariant that a missing semantic height and numeric zero are different states.

### Regression coverage

Focused tests prove:

1. repeated samples on one corner fold into one final semantic `SetHeight`
2. a missing target height is skipped rather than treated as zero
3. an upper-plane edit requires its explicit lower-plane base
4. upper-plane quantization uses the real lower-plane base and preserves the OSRS stored-byte-1 quirk
5. storage quantization can correctly collapse a Height application into a semantic no-op
6. negative world coordinates remain exact
7. final patch operation order is deterministic even when sample order is not coordinate order

Checkpoint 20 implementation head before this handoff update:

- `ea326104fdce233b5bc476e447e580f04523a328`
- CI `38372440515`: Tier A/B/C PASS
- Tier A architecture, formatting, workspace check, strict Clippy: PASS
- Tier B full workspace tests including Height-planner regressions: PASS
- Tier C M3-M9 semantic/reference parity: PASS

### Deliberately unchanged

- `sculpt_at` still previews/mutates through legacy `EditorApp.edited` and worker `TileEdit`
- `height_on_selection` still uses the legacy runtime path
- completed Height strokes still enter legacy `EditStep` history
- Height undo/redo is still legacy
- `height_edited` remains active projection/storage-adapter state
- Objects remain legacy
- terrain-path runtime bridge remains transitional
- context-menu `clear_overlay_at` remains legacy
- renderer invalidation and 117HD are unchanged

## Height subsystem status

Checkpoint 20 establishes the semantic Height planning boundary only. The Height subsystem is **not complete and must not be merged yet**.

The next runtime migration must keep the current responsive preview as projection while making the completed Height transaction authoritative in `EditorSession`.

## Next checkpoint

Do not begin without explicit direction.

### Checkpoint 21: live Height stroke -> authoritative semantic commit

Narrow scope:

1. capture one immutable `HeightStrokeSpec` when a Height stroke begins/first applies
2. record the ordered Height application centers, including repeated held-brush samples
3. make completed Height strokes plan one `MapPatch` from the authoritative session and call `EditorSession::commit` exactly once
4. preserve current live `TileEdit` / worker preview during the stroke
5. reconcile successful semantic heights back through the existing projection path
6. roll back the visible preview if semantic planning/commit fails
7. make selection-based Height sequencing deterministic before semantic planning
8. keep global Height undo/redo on the legacy routing path for this checkpoint
9. do not migrate Objects, path, context-menu terrain actions, renderer invalidation, or 117HD
10. run Tier A/B/C and stop for review

Only after Height commit is semantic should Height undo/redo be migrated in a separate checkpoint.
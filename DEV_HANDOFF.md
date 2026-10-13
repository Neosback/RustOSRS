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

PR: #38 — `Migrate Height editing to semantic EditorSession`

Purpose: migrate the dedicated Height tool to the persistent semantic session. Do not mix Objects, terrain-path migration, context-menu terrain actions, renderer/117HD redesign, or unrelated editor cleanup into this branch.

## Checkpoint 20: pure semantic Height stroke planner

Added `crates/osrs-editor/src/heights/commit.rs` and registered it beneath the existing `heights` module. Runtime Height behavior was intentionally unchanged in this checkpoint.

### Captured stroke meaning

`HeightStrokeSpec` captures the immutable semantic meaning of one Height stroke:

- `SourcePlane`
- `HeightTool`
- brush radius
- brush shape
- flatten anchor
- deterministic stroke seed

This is deliberately one value rather than a wide planner argument list. It also establishes the runtime contract: once a Height stroke starts, later UI preference changes must not silently alter the operation that eventually commits.

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

The planner uses the existing `encode_height` / `decode_height` rules after each application, not only at final commit. Subsequent repeated Height samples therefore read the already-quantized result exactly as the current runtime does.

Plane handling is explicit:

- plane 0 uses cache base height `0`
- planes 1..=3 encode relative to the explicit height on the plane directly below
- a missing lower-plane base returns `HeightCommitError::MissingBaseHeight`
- missing lower-plane data is **not** treated as height zero

The last rule intentionally hardens the legacy `EditorApp::base_height`, which falls back to zero when the lower tile cannot be found. It preserves the domain invariant that missing semantic height and numeric zero are distinct states.

### Regression coverage

Focused tests prove:

1. repeated samples on one corner fold into one final semantic `SetHeight`
2. a missing target height is skipped rather than treated as zero
3. an upper-plane edit requires its explicit lower-plane base
4. upper-plane quantization uses the real lower-plane base and preserves the OSRS stored-byte-1 quirk
5. storage quantization can correctly collapse a Height application into a semantic no-op
6. negative world coordinates remain exact
7. final patch operation order is deterministic even when sample order is not coordinate order

Checkpoint 20 final documented head:

- `0671dda23f9328e129b30fe8e809d01afb17797d`
- CI `38372828941`: Tier A/B/C PASS

## Checkpoint 21: live Height stroke -> authoritative semantic commit

Completed Height gestures now commit through the persistent `EditorSession` while the existing worker/render path remains the immediate preview/projection layer.

```text
pointer drag / selection Height action
  -> captured HeightStrokeSpec
  -> ordered Height sample centers
  -> existing live TileEdit preview
  -> release / completed gesture
  -> build_terrain_height_patch(EditorSession::document, spec, centers)
  -> one EditorSession::commit
  -> authoritative semantic heights
  -> reconcile current TileEdit/world projection
```

### Immutable live-stroke semantics

The first actual Height application captures one `HeightStrokeSpec`. The live preview then uses `HeightStrokeSpec::apply_sample`, the same captured arithmetic entry point used by semantic replay.

This means changing Height settings while a drag is already open cannot change the semantic meaning of the transaction at release.

`Stroke` records:

- `height_spec: Option<HeightStrokeSpec>`
- ordered `height_centers: Vec<MapTile>`

Repeated centers are retained intentionally because holding the sculpt brush on one tile applies again after `SCULPT_REPEAT`.

### One gesture -> one semantic commit

At `end_stroke`:

- the exact ordered samples are replayed against `EditorSession::document()`
- one `MapPatch` is built for the complete Height gesture
- `EditorSession::commit` is called exactly once
- one successful non-empty Height gesture therefore creates one semantic history entry and one session revision advance
- a semantic no-op creates no semantic history entry and no global legacy entry

The existing live `TileEdit` preview still updates during the drag for responsiveness. It is not the completed Height state authority after commit.

### Projection and failure handling

Successful semantic commits are reconciled back into the legacy `TileValues` / `TileEdit` / worker projection by reading authoritative heights from the session document.

Projection preparation happens before mutation so a projection error cannot leave half of a Height stroke reconciled.

Failure handling is conservative:

- if a preview-touched semantic height is missing, the visible preview is reverted and nothing is committed
- planner errors such as a missing upper-plane base revert the visible preview
- semantic commit rejection reverts the visible preview
- if projection fails after commit, the editor attempts an immediate semantic undo and reverts the visible preview
- missing semantic heights are never converted to numeric zero

`height_edited` remains active because the current worker/cache-format adapter still needs to know when to emit an encoded height byte. It is projection/storage-adapter state, not completed semantic authority.

### Selection-based Height

`height_on_selection` uses the same captured `HeightStrokeSpec` and semantic commit boundary as pointer sculpting.

Selected coordinates are sorted before application. This removes the old `HashSet` iteration nondeterminism so operations whose result can depend on application order replay identically and deterministically.

The existing selection behavior remains otherwise intact:

- radius 0
- square footprint
- flat falloff
- first coordinate in the deterministic selection sequence supplies the automatic Flatten anchor
- one selection action remains one logical Height transaction

Checkpoint 21 implementation head before its handoff update:

- `d26608f9282ce83488bb6bf9f94e4c10f4b82adf`
- CI `38380243232`: Tier A/B/C PASS

Checkpoint 21 stopped intentionally before Height undo/redo. That temporary history gap is closed by Checkpoint 22 below.

## Checkpoint 22: Height undo/redo -> authoritative semantic history

Height history now follows the same authority model as Paint: the global mixed-history stack decides **when** a Height transaction is undone or redone, while `EditorSession::history` exclusively decides **what semantic state** is restored.

### Lightweight Height routing marker

Added:

```text
SemanticHeightHistory { keys: Vec<TileKey> }
```

A completed semantic Height transaction now leaves only this lightweight marker in the global `EditStep` stack.

The marker contains no before/after heights and no duplicate semantic state. `EditStep::changes` is empty for semantic Height markers, so legacy `revert` can no longer act as Height undo authority.

### Authoritative undo and redo

When global history reaches a Height marker:

```text
Ctrl/Cmd+Z
  -> global marker selects the chronological Height action
  -> EditorSession::undo()
  -> MapDocument restores exact inverse semantic height state
  -> projection adapter reads MapDocument
  -> EditorApp.edited / TileEdit / worker mirror the result
```

Redo is symmetric through `EditorSession::redo()`.

This closes the Checkpoint 21 failure mode where visible Height state could move backward through legacy history while the semantic session still retained the Height patch on top.

Paint and Height now share one semantic subsequence in `EditorSession::history`, and their global routing markers preserve chronological ordering around still-legacy actions.

### Height projection contract

`project_height_values` changes only the height channel of the current `TileValues` projection. It preserves:

- underlay
- overlay
- overlay shape
- overlay rotation
- flags

`project_semantic_height_history` prepares every projected value before mutating the worker-facing state, so a missing tile or missing semantic height cannot produce a partially applied undo/redo projection.

`height_edited` remains a transitional cache-format/worker projection flag so encoded `TileEdit.height` bytes continue to be emitted. It is not semantic history or completed Height authority.

### Failure-safe routing

Height undo:

- `EditorSession::undo()` must return one semantic edit
- projection failure attempts `EditorSession::redo()` to restore the semantic transaction
- the global marker is returned to the undo stack when the operation cannot complete coherently

Height redo mirrors this behavior:

- `EditorSession::redo()`
- projection failure attempts `EditorSession::undo()`
- the global marker is returned to the redo stack on failure

An unexpected `Ok(None)` is treated as a history desynchronization and does not advance the global routing stack.

### Regression coverage

Checkpoint 22 adds focused coverage proving:

1. semantic Height projection follows commit -> undo -> redo exactly
2. Height projection preserves every non-height terrain channel
3. Paint followed by Height shares one semantic history order
4. undo first removes Height while preserving Paint
5. the next undo removes Paint
6. redo restores Paint before Height
7. a semantic Height global marker carries no legacy tile before/after authority
8. the existing Paint marker remains unaffected and explicitly carries no Height marker

Checkpoint 22 implementation head before this handoff update:

- `cf93a569654124ff09c6265d0341d31880dd2754`
- CI `38383558840`: Tier A/B/C PASS
- Tier A architecture, formatting, workspace check, strict Clippy: PASS
- Tier B full workspace tests including Height history/projection regressions: PASS
- Tier C M3-M9 semantic/reference parity: PASS

## Height subsystem status

The dedicated Height tool migration is now **complete** for this architecture phase:

- semantic Height planner: complete
- immutable live-stroke meaning: complete
- one gesture -> one semantic commit: complete
- deterministic selection sequencing: complete
- live preview retained only as projection: complete
- Height undo -> `EditorSession::undo`: complete
- Height redo -> `EditorSession::redo`: complete
- legacy Height before/after undo authority: retired
- Paint + Height semantic history ordering: covered

PR #38 is the integration PR for this completed subsystem.

### Deliberately still transitional / out of scope

- `EditorApp.edited` remains worker/render projection state
- `height_edited` remains cache-format/worker projection state
- Objects remain legacy
- terrain-path runtime history remains transitional
- context-menu `clear_overlay_at` remains legacy
- renderer invalidation and 117HD are unchanged

These are separate migration systems and must not be folded into the completed Height branch.

## Next work after PR #38

Do not begin another subsystem without explicit direction.

After PR #38 is merged and post-merge `main` CI is green, choose one separate next migration target rather than broadening this branch. Likely candidates are Objects or the remaining terrain/path/context-menu legacy bridges, but that choice is a new checkpoint/product decision.

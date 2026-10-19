# Semantic authority audit

Status: active cleanup baseline

This document classifies state by authority before further refactoring. It is not a replacement for `ARCHITECTURE.md`; it records the concrete implementation boundaries that currently enforce that architecture and the remaining transitional debt.

## Canonical authority chain

```text
MapDocument
  ↑ wrapped by WorkspaceDocument (map + authored scenario)
  ↑ wrapped by EditorDocument (workspace + authored appearance)
  ↑ owned by EditorSession (history + revisions + dirty-region policy)
  ↑ owned by EditRuntime (application transaction gateway)
  ↑ coordinated by EditorApp
```

### Map state

`osrs-edit::MapDocument` is the only authoritative editable terrain/height/object state.

Its storage is private. Interactive writes enter through `MapPatch`; `apply_patch` validates the complete patch before applying any operation. Loading and streaming have explicit history-free infrastructure boundaries instead of pretending to be user edits.

Legal low-level mutation boundaries:

- `MapDocument::from_parts`: initial semantic hydration.
- `MapDocument::apply_patch`: validated semantic patch application, normally reached through history/session policy.
- `MapDocument::replace_region`: streamed clean-region hydration.
- `MapDocument::remove_region`: streamed clean-region eviction, with session policy deciding whether eviction is legal.

No renderer, worker, picker, UI, preview, or cache object is allowed to become a second map authority.

### Authored workspace state

`WorkspaceDocument` composes `MapDocument` with authored `ScenarioDocument`. It does not copy map state. `WorkspacePatch` routes map and scenario patches through the same generic `PatchHistory` implementation.

`EditorDocument` composes that workspace with authored `ThemePack`. `EditorPatch` routes workspace and appearance edits through the same generic history implementation. Appearance intentionally advances authored revision without changing map/collision revision.

These are composition layers around the canonical map, not competing map models.

### History and session policy

`PatchHistory<D>` is the canonical reversible history implementation. Map, workspace, and editor history are specializations of that same implementation rather than independent undo engines.

`EditorSession` owns:

- `EditorDocument`
- `PatchHistory<EditorDocument>`
- total semantic revision
- authored edit revision
- loaded-region residency
- dirty-region protection

A checkpointed edit changes semantic authority first. Finalization clears the old redo branch only after downstream projection succeeds. Projection failure restores document/history/revisions/dirty-region ownership through the exact inverse transaction rather than cloning the full map.

Legal session lifecycle exceptions to interactive history:

- `ingest_region` and `unload_region`: streaming clean semantic state.
- `apply_untracked`: opening/applying a project diff. It is authored state, marks regions dirty, and advances edit revision, but intentionally creates no interactive undo entry.
- `load_appearance`: project loading of already validated authored appearance.
- `reset`: backing-source replacement.

These exceptions should remain narrow and named. New feature edits must not use them to bypass normal history.

### Application transaction gateway

`EditRuntime` privately owns `EditorSession` and is the application-level semantic gateway.

For interactive map edits the required flow is:

```text
intent
  -> pure semantic planner
  -> MapPatch
  -> EditRuntime::execute / execute_named
  -> EditorSession checkpointed commit
  -> read-only post-commit MapDocument passed to projection planning
  -> finalize on projection success
     OR exact semantic restore on projection failure
  -> EditReceipt
  -> world/render invalidation
```

Undo and redo use the same checkpoint/finalize/restore policy through `EditRuntime::replay`.

Scenario and appearance use equivalent `EditRuntime` gateway methods. Transaction lifecycle primitives remain private to the runtime so callers cannot finalize or restore checkpoints independently.

## Non-authoritative state

The following categories are disposable or reconstructible and must never be queried as semantic truth when a canonical semantic query exists:

- world worker mirrors and streaming projections
- render/GPU resources
- picker geometry and contextual scene snapshots
- selection and hover state
- UI/widget state
- diagnostic traces
- previews and ghosts
- tile/object projection adapters

Deleting any of these may lose presentation/cache work but must not lose an authored map edit.

## Transitional terrain adapter

`TerrainController` is the largest remaining authority-shaped compatibility surface.

It currently owns:

- `edited`: transitional tile values used by the worker/picker projection
- `height_edited`: projection markers for explicit heights
- active `Stroke` preview state
- stroke counter/seed state
- chronological application history markers
- redo markers

Its source explicitly states that committed before/after authority belongs to `EditorSession` and that this controller is removable when workers consume semantic snapshots and session history supplies sufficient projection footprints.

### What is already semantic

The following paths already plan/commit through `EditRuntime` and keep only projection-routing metadata in the mixed history stack:

- Paint
- Height
- terrain Path / spline
- tile Paste
- Objects
- Prefabs / generated Buildings through composite routing
- NPC Scenario
- Appearance

Paint and Height have no legacy commit fallback: a stroke without a semantic planner is reverted instead of recording separate tile-history authority.

### Remaining mixed-history debt

`editing/history.rs::EditStep` still has two kinds of data:

1. semantic routing markers (`semantic_paint`, `semantic_height`, `semantic_object`, `semantic_path`, `semantic_scenario`, `semantic_appearance`), and
2. legacy before/after payloads (`changes`, `removed`, `added`).

`Route::of(step) == None` is the remaining legacy replay path. That path directly replays the stored worker tile/object values instead of replaying semantic history.

This is the primary CP2 migration target. Do not delete it until every remaining producer is identified and migrated. Do not add a new producer to it.

The chronological `TerrainController.history` / `redo_stack` are also transitional. For semantic entries they do not own before/after values, but they still duplicate chronological position so the editor can route session undo/redo to the correct projection adapter. A mismatch between this marker stack and `EditorSession` history is therefore a correctness risk even though the semantic document remains canonical.

## EditorApp classification

`EditorApp` is currently a composition root with named owners for project/session/terrain interaction/world/render/UI/settings/diagnostics/plugins/palettes/NPC/audio/player/prefabs/names/assets/HD/object placement/export/script state.

The important distinction is:

- `session: EditRuntime` is semantic authority.
- `terrain: TerrainController` is compatibility projection/history routing.
- `world: WorldRuntime` is scene/worker projection orchestration.
- `renderer: RenderRuntime` is GPU presentation.
- other runtimes own product/UI/asset state, not map truth.

The existing `editor_app_surface.json` ratchet should continue preventing casual growth of direct `EditorApp` behavior/state.

## Cleanup priorities

### A1. Eliminate legacy `EditStep` payload producers

Inventory every constructor/write of `EditStep.changes`, `EditStep.removed`, and `EditStep.added` that can enter `TerrainController.history` after a completed action.

For each remaining action:

1. find or create the pure semantic planner in the existing owning crate,
2. commit through `EditRuntime`,
3. plan projection from the authoritative post-commit document inside the gateway boundary,
4. retain only the minimum projection-routing footprint,
5. add exact commit/cancel/undo/redo coverage,
6. remove that action's legacy payload route.

Do this one feature at a time.

### A2. Replace mixed chronological markers with session-owned replay descriptors

After A1 reaches zero legacy payload producers, move the projection footprint needed for replay closer to semantic history/receipts so the application no longer needs a second chronological marker stack.

Do not expose history internals merely to achieve this. Prefer an opaque replay descriptor/footprint returned by the semantic boundary.

### A3. Retire `TerrainController.edited` as a semantic-looking tile mirror

Workers should eventually consume revisioned semantic snapshots/projections directly. Until then, `edited` remains presentation/worker compatibility state and planners must continue reading `EditRuntime::document()` rather than this map.

### A4. Preserve narrow lifecycle exceptions

Project load, source hydration, region ingest/eviction, and appearance load are legitimate history-free boundaries. Keep them explicit and do not turn `apply_untracked` into a general escape hatch.

## Guard candidates

Add these only after their current legitimate call sites are inventoried:

- reject new legacy `EditStep` before/after payload producers
- ratchet the count of legacy history routes downward
- reject new application call sites that mutate `MapDocument` directly instead of using `EditRuntime`/session policy
- reject new `TerrainController` fields that look like committed semantic authority
- keep `EditRuntime.session` private
- keep projection closures read-only over authoritative semantic state
- keep `EditorApp` field/method surface ratcheted

The guard should ratchet existing debt down. It must not freeze legitimate hydration/project boundaries or require a full Rust build to detect source-level ownership regressions.

## Exit condition for CP2

CP2 is complete when:

- every completed user map edit is authoritative in `EditorSession`
- no completed action relies on legacy `EditStep.changes/removed/added` for undo/redo truth
- the remaining chronological routing metadata has an explicit removal path
- cheap architecture tooling rejects new competing authority
- no OSRS/rendering behavior changed as a side effect of the cleanup

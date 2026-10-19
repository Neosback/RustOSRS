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

These exceptions remain narrow and named. New feature edits must not use them to bypass normal history.

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

`TerrainController` remains the largest authority-shaped compatibility surface.

It currently owns:

- `edited`: transitional tile values used by the worker/picker projection
- `height_edited`: projection markers for explicit heights
- active `Stroke` preview state
- stroke counter/seed state
- chronological semantic application markers
- redo markers

Its source explicitly states that committed before/after authority belongs to `EditorSession` and that this controller is removable when workers consume semantic snapshots and semantic replay supplies sufficient projection footprints.

### Active stroke preview is no longer history

CP2 separated open Paint/Height gesture state from completed history routing.

`Stroke` now owns its own transient `changes: Vec<StrokeChange>` preview buffer. It no longer embeds or constructs an `EditStep`. User cancellation restores that preview and its explicit-height markers; failed semantic commit restores the same visible tile values that the old preview rollback path restored.

This removes a misleading coupling where an in-progress preview looked like a completed history entry.

### Completed product edits are semantic

The following paths plan/commit through `EditRuntime` and retain only projection-routing metadata in the application marker stack:

- Paint
- Height
- terrain Path / spline
- tile Paste
- Objects
- Prefabs / generated Buildings through composite routing
- NPC Scenario
- Appearance

Paint and Height have no legacy commit fallback: a stroke without a semantic planner is reverted instead of recording separate tile-history authority.

### Legacy runtime replay has been removed

Product `EditStep` values no longer carry tile/object before/after payloads. The runtime `revert(EditStep, ...)` fallback is gone.

`Route::of(step) == None` is now treated as an invalid/out-of-sync semantic marker: undo or redo returns the marker to its stack, reports the problem, and changes no state. It does not replay worker values as authority.

The remaining `changes`, `removed`, and `added` names exist only under `cfg(test)` in `EditStep`, solely so three existing contract tests can prove semantic constructors do not populate the retired payload categories. Product builds cannot access or consume those fields. CP2C2 should rewrite those tests around positive routing contracts and delete the test-only fields entirely.

### Remaining chronological-marker debt

`TerrainController.history` / `redo_stack` are still transitional application-level chronological markers.

They do not own semantic before/after state, but they duplicate chronological position so the editor can select the correct projection adapter while `EditorSession` performs authoritative undo/redo. A mismatch between the marker stack and semantic session history is therefore a correctness risk even though `MapDocument` remains canonical.

The history router already mitigates this by replaying through `EditRuntime::replay`, preparing projection inside the gateway boundary, finalizing only after projection succeeds, and restoring the marker when semantic replay fails.

The long-term removal path is to make semantic history/replay return an opaque descriptor or projection footprint sufficient for application replay, so the editor does not need a second chronological stack.

## EditorApp classification

`EditorApp` is currently a composition root with named owners for project/session/terrain interaction/world/render/UI/settings/diagnostics/plugins/palettes/NPC/audio/player/prefabs/names/assets/HD/object placement/export/script state.

The important distinction is:

- `session: EditRuntime` is semantic authority.
- `terrain: TerrainController` is compatibility projection/history routing.
- `world: WorldRuntime` is scene/worker projection orchestration.
- `renderer: RenderRuntime` is GPU presentation.
- other runtimes own product/UI/asset state, not map truth.

The existing `editor_app_surface.json` ratchet should continue preventing casual growth of direct `EditorApp` behavior/state.

## Enforced CP2 ratchets

`python3 scripts/check_authority.py` is a cheap source-level gate. It currently enforces two rules without compiling Rust:

1. checkpoint lifecycle APIs may appear only inside `EditorSession` internals or `EditRuntime`, and
2. legacy `EditStep` payload references may not escape or grow beyond the temporary CP2 test scaffold.

The legacy-history baseline is deliberately a ceiling, not a required count. Debt may decrease without widening the baseline. Any new hit in another file or growth inside the temporary files fails the gate.

When CP2C2 deletes the test-only scaffold, remove the baseline entries rather than preserving an allowance of zero-valued dead architecture.

## Cleanup priorities

### A1. Remove final test-only legacy payload scaffold

Rewrite the three `EditStep` tests to assert the positive semantic routing contract directly, then delete the `cfg(test)` `changes`, `removed`, and `added` fields and reduce/remove the legacy-history baseline.

This is source/test cleanup only; runtime product authority is already semantic.

### A2. Replace the second chronological marker stack

Move the projection footprint needed for replay closer to semantic history/receipts so the application no longer needs a second chronological marker stack.

Do not expose mutable history internals merely to achieve this. Prefer an opaque replay descriptor/footprint returned by the semantic boundary. Preserve one logical semantic history and one authoritative ordering.

### A3. Retire `TerrainController.edited` as a semantic-looking tile mirror

Workers should eventually consume revisioned semantic snapshots/projections directly. Until then, `edited` remains presentation/worker compatibility state and planners must continue reading `EditRuntime::document()` rather than this map.

### A4. Preserve narrow lifecycle exceptions

Project load, source hydration, region ingest/eviction, and appearance load are legitimate history-free boundaries. Keep them explicit and do not turn `apply_untracked` into a general escape hatch.

## Next guard candidates

Add these only after their current legitimate call sites are inventoried:

- reject direct application mutation of `MapDocument` outside `EditRuntime`/session policy
- reject new `TerrainController` fields that look like committed semantic authority
- keep `EditRuntime.session` private
- keep projection closures read-only over authoritative semantic state
- keep `EditorApp` field/method surface ratcheted

The guard should ratchet existing debt down. It must not freeze legitimate hydration/project boundaries or require a full Rust build to detect source-level ownership regressions.

## Exit condition for CP2

CP2 is complete when:

- every completed user map edit is authoritative in `EditorSession`
- no product action relies on legacy `EditStep` before/after payloads for undo/redo truth
- active preview data is separate from completed history routing
- the remaining chronological routing metadata has an explicit removal path
- cheap architecture tooling rejects new competing authority
- the test-only retired payload scaffold is removed
- no OSRS/rendering behavior changed as a side effect of the cleanup

All runtime authority conditions above are now satisfied. The remaining CP2 work is removal of the test-only scaffold plus the longer-lived chronological-marker and worker-preview adapters identified for subsequent checkpoints.

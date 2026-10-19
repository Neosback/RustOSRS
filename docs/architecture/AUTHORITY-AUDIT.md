# Semantic authority audit

Status: active cleanup baseline

This document classifies current state by authority. It does not replace `ARCHITECTURE.md`; it records the concrete implementation boundaries that currently enforce that architecture and the transitional debt still being removed on `refactor/architecture-ai-foundation`.

## Canonical authored authority chain

```text
MapDocument
  ^ wrapped by WorkspaceDocument (map + authored scenario)
  ^ wrapped by EditorDocument (workspace + authored appearance)
  ^ owned by EditorSession (history + revisions + residency/dirty policy)
  ^ owned by EditRuntime (application transaction gateway)
  ^ coordinated by EditorApp
```

This hierarchy must be read carefully:

- `MapDocument` is the sole authoritative terrain/height/object-placement state.
- `ScenarioDocument` is separate authored scenario state.
- `ThemePack` is separate authored appearance state.
- `WorkspaceDocument` and `EditorDocument` compose those authored domains without copying map authority.
- `EditorSession` owns one authoritative chronological history over the composed authored document.
- `EditRuntime` owns the application transaction lifecycle.

No renderer, worker, picker, UI, preview, cache, diagnostic structure, or MCP/automation adapter may become a second authored authority.

## Map state

`osrs-edit::MapDocument` storage is private. Interactive writes enter through reversible `MapPatch` operations. `apply_patch` validates the complete patch before applying operations.

Legal low-level map mutation boundaries remain narrow and named:

- `MapDocument::from_parts`: initial semantic hydration.
- `MapDocument::apply_patch`: validated semantic patch application under session/history policy.
- `MapDocument::replace_region`: streamed clean-region hydration.
- `MapDocument::remove_region`: streamed clean-region eviction, subject to session dirty-region policy.

Project/source loading is not disguised as interactive history.

## Authored workspace state

`WorkspaceDocument` composes `MapDocument` with `ScenarioDocument`. `WorkspacePatch` routes map and scenario changes through the generic history implementation.

`EditorDocument` composes the workspace with `ThemePack`. `EditorPatch` routes workspace and appearance changes through the same chronological semantic history.

These wrappers are composition layers, not competing map models.

## History and session policy

`PatchHistory<D>` is the canonical reversible history implementation. Map, workspace, and editor history use that generic mechanism rather than independent undo engines.

`EditorSession` owns:

- `EditorDocument`
- `PatchHistory<EditorDocument>`
- total semantic revision
- authored edit revision
- loaded-region residency
- dirty-region protection

Checkpointed transactions change semantic authority first. Finalization clears an old redo branch only after downstream projection preparation succeeds. Projection failure restores document/history/revisions/dirty-region ownership through the exact inverse transaction rather than cloning the whole map.

Legal history-free session boundaries are intentionally narrow:

- `ingest_region` and `unload_region`: streaming clean semantic state
- `apply_untracked`: applying a project diff without interactive undo history
- `load_appearance`: loading already-authored appearance state
- `reset`: replacing the backing source

New interactive features must not use these as escape hatches.

## Application transaction gateway

`EditRuntime` privately owns `EditorSession` and is the application semantic gateway.

Interactive map edits follow:

```text
intent
  -> pure planner
  -> MapPatch
  -> EditRuntime::execute / execute_named
  -> checkpointed EditorSession commit
  -> read-only post-commit MapDocument passed to projection preparation
  -> finalize on projection success
     OR exact semantic restore on projection failure
  -> EditCommit / EditReceipt
  -> ChangeImpact + touched semantic coordinates
  -> world/render invalidation
```

Undo/redo uses the same checkpoint/finalize/restore policy through `EditRuntime::replay`.

Scenario and appearance use equivalent runtime gateway methods. Transaction lifecycle primitives remain private to the runtime so callers cannot finalize or restore checkpoints independently.

## Existing semantic invalidation vocabulary

`osrs-edit::ChangeImpact` already exists and is derived from semantic operations. It intentionally describes semantic effects rather than renderer-specific dirty flags.

Current categories include terrain geometry/material, normals, lighting, collision, locations, bounds, and scenario impact.

Do not create a second invalidation enum merely to satisfy a future architecture diagram. CP3/CP4 should extend or translate this existing vocabulary only when real projection consumers prove a missing semantic category or insufficient coordinate precision.

## Non-authoritative state

The following is disposable or reconstructible and must never be queried as authored truth when a canonical semantic query exists:

- world worker mirrors and streaming projections
- scene/render/GPU resources
- picker geometry and contextual scene snapshots
- selection and hover state
- UI/widget state
- diagnostic traces
- previews and ghosts
- tile/object compatibility projection adapters

Deleting any of these may lose presentation/cache work but must not lose authored state.

## TerrainController transitional state

`TerrainController` remains the largest semantic-looking compatibility surface.

It currently owns transitional concerns including:

- `edited`: tile values used by worker/picker projection compatibility
- `height_edited`: projection markers for explicit heights
- active `Stroke` preview state
- stroke seed/counter state
- a parallel chronological semantic-routing marker stack

Planners must read authoritative state through `EditRuntime::document()` rather than `TerrainController.edited`.

### Active stroke preview is separate from history - complete

`Stroke` owns transient `changes: Vec<StrokeChange>` preview data directly. It no longer embeds or constructs an `EditStep`.

User cancellation restores preview values and explicit-height markers. Failed semantic commit restores the same visible preview values through a stroke-specific rollback path.

### Legacy before/after history payloads are removed - complete

Product `EditStep` values no longer carry tile/object `changes`, `removed`, or `added` payloads. The old runtime `revert(EditStep, ...)` fallback is gone.

The temporary test-only retired payload scaffold has also been removed. Positive semantic routing tests now prove constructor behavior without retaining dead fields.

This means completed product edits are semantically authoritative in `EditorSession`; application markers no longer duplicate before/after truth.

## Semantic replay descriptor migration

The remaining correctness risk is duplicate chronology, not duplicate semantic payload authority.

`TerrainController.history` and `redo_stack` still have to stay aligned with `PatchHistory<EditorDocument>` so application replay can choose a projection adapter. That is the next debt to remove.

The branch now contains the required semantic foundation:

### Replay R1 - complete

`osrs-edit::MapReplayFootprint` is derived from the exact `MapPatch` and records:

- semantic terrain channels by map position
- changed height vertices
- exact removed object placements
- exact added object placements

The footprint is independent of workers, renderer, UI, and `TileValues`.

### Replay R2 - complete

`EditorSession` derives immutable Map / Scenario / Appearance replay descriptors from the exact next authoritative history patch without exposing raw `PatchHistory` or mutable history internals.

### Replay R3A - complete

The editor contains a generic map replay projector that:

- captures only transitional tile values named by the semantic footprint
- updates only semantic terrain channels present in the footprint
- updates only recorded height vertices
- validates object mutation polarity against authoritative post-replay state
- prepares a complete projection before any worker mutation is applied

`EditRuntime` exposes the immutable replay descriptor and derives its own diagnostic/touch footprint from it.

### Replay R3B - next

The active undo/redo router still uses `EditStep`/tool-specific `Route` values to capture and prepare projection even though the semantic descriptor and generic projector now exist.

The next checkpoint should wire active replay directly to `EditRuntime::replay_descriptor()` and the generic projector, then remove the ignored legacy replay-footprint argument.

This checkpoint must preserve:

- exact semantic rollback on projection preparation failure
- mutation of worker/UI mirrors only after runtime finalization
- object reselection behavior
- Scenario and Appearance replay
- existing Paint/Height/Path/Object/composite undo/redo semantics

### Replay R4 - after R3B

Once the generic route is proven:

- remove `TerrainController.history` / `redo_stack`
- remove `EditStep` and `Semantic*History` routing types
- make undo/redo availability query semantic authority through `EditRuntime`
- retain any user-facing labels only as non-authoritative metadata if still worth keeping

Exactly one chronological history should remain.

### Replay R5 - final CP2 ratchets

After the parallel stack is gone:

- remove obsolete history allowances from architecture tooling
- add a cheap guard against a second application history stack
- continue guarding `EditRuntime.session` privacy and checkpoint lifecycle ownership
- update registry/design references
- add reproducible architecture metrics only where the metric can be interpreted reliably

## EditorApp classification

`EditorApp` remains the composition root. The important ownership distinctions are:

- `session: EditRuntime` - authored semantic gateway
- `terrain: TerrainController` - preview/projection compatibility state, not authored truth
- `world: WorldRuntime` - scene/worker projection orchestration
- `renderer: RenderRuntime` - GPU presentation
- other runtimes - product/UI/asset state under their own ownership boundaries

The existing `editor_app_surface.json` ratchet should continue preventing casual growth of direct `EditorApp` behavior/state.

## Current cheap authority tooling

`python3 scripts/check_authority.py` provides source-level protection without compiling Rust.

The gate should continue to enforce only rules that can be identified reliably from source. Baselines are ceilings, not required debt counts. When debt is removed, delete the allowance rather than preserving a zero-value compatibility concept.

## Guard candidates after chronology cleanup

Add only after legitimate call sites are inventoried:

- reject direct interactive `MapDocument` mutation outside session/runtime policy
- reject new `TerrainController` fields that look like committed semantic authority
- keep `EditRuntime.session` private
- keep projection closures read-only over authoritative semantic state
- keep `EditorApp` field/method surface ratcheted
- reject reintroduction of application-level history stacks parallel to `PatchHistory`

Guards must not freeze legitimate hydration, streaming, or project-load boundaries.

## CP2 exit condition

CP2 is complete when:

- every completed user map edit is authoritative in `EditorSession`
- no product action relies on legacy before/after application payloads for undo/redo truth
- active preview data is separate from completed chronology
- semantic replay projection derives from the authoritative history patch
- no second application chronological stack must stay aligned with `PatchHistory`
- cheap architecture tooling rejects new competing authority/history
- no OSRS/rendering behavior changed as a side effect of the cleanup

The first three conditions and the semantic replay foundations are complete. Active work begins at Replay R3B.

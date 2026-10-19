# Semantic authority audit

Status: **post-CP2 merged baseline**

This document classifies the current authored-state and history authority boundaries after the architecture/AI foundation CP2 stopping point merged to `main`.

It does not replace `ARCHITECTURE.md`. `ARCHITECTURE.md` remains the repository ownership contract, while `docs/architecture/RUSTOSRS-TECHNICAL-ARCHITECTURE-SPEC.md` describes the preferred long-term technical direction.

## Canonical authored authority chain

```text
MapDocument
  ^ wrapped by WorkspaceDocument (map + authored scenario)
  ^ wrapped by EditorDocument (workspace + authored appearance)
  ^ owned by EditorSession (history + revisions + residency/dirty policy)
  ^ owned by EditRuntime (application transaction gateway)
  ^ coordinated by EditorApp
```

This hierarchy is authoritative:

- `MapDocument` is the sole authoritative terrain/height/object-placement state.
- `ScenarioDocument` is separate authored scenario state.
- `ThemePack` is separate authored appearance state.
- `WorkspaceDocument` and `EditorDocument` compose authored domains without copying map authority.
- `EditorSession` owns one authoritative chronological history over the composed authored document.
- `EditRuntime` owns the application transaction lifecycle.

No renderer, worker, picker, UI, preview, cache, diagnostic structure, procedural generator, automation adapter, or future MCP adapter may become a second authored authority.

## Map state

`osrs-edit::MapDocument` storage is private. Interactive writes enter through reversible `MapPatch` operations. `apply_patch` validates the complete patch before applying operations.

Legal low-level map mutation boundaries remain narrow and named:

- `MapDocument::from_parts`: initial semantic hydration;
- `MapDocument::apply_patch`: validated semantic patch application under session/history policy;
- `MapDocument::replace_region`: streamed clean-region hydration;
- `MapDocument::remove_region`: streamed clean-region eviction, subject to session dirty-region policy.

Project/source loading is not disguised as interactive history.

## Authored workspace state

`WorkspaceDocument` composes `MapDocument` with `ScenarioDocument`. `WorkspacePatch` routes map and scenario changes through the generic history implementation.

`EditorDocument` composes the workspace with `ThemePack`. `EditorPatch` routes workspace and appearance changes through the same chronological semantic history.

These wrappers are composition layers, not competing map models.

## History and session policy

`PatchHistory<D>` is the canonical reversible history implementation. Map, workspace, and editor history use that generic mechanism rather than independent undo engines.

`EditorSession` owns:

- `EditorDocument`;
- `PatchHistory<EditorDocument>`;
- total semantic revision;
- authored edit revision;
- loaded-region residency;
- dirty-region protection.

Checkpointed transactions change semantic authority first. Finalization clears an old redo branch only after downstream projection preparation succeeds. Projection failure restores document/history/revisions/dirty-region ownership through the exact inverse transaction rather than cloning the whole map.

Legal history-free session boundaries remain intentionally narrow:

- `ingest_region` and `unload_region`: streaming clean semantic state;
- `apply_untracked`: applying a project diff without interactive undo history;
- `load_appearance`: loading already-authored appearance state;
- `reset`: replacing the backing source.

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

The legacy compatibility replay-footprint argument has been removed. Replay footprint/diagnostic touches are sourced from the authoritative semantic replay descriptor.

## Existing semantic invalidation vocabulary

`osrs-edit::ChangeImpact` already exists and is derived from semantic operations. It intentionally describes semantic effects rather than renderer-specific dirty flags.

Current categories include terrain geometry/material, normals, lighting, collision, locations, bounds, and scenario impact.

Do not create a second invalidation enum merely to satisfy a future architecture diagram. Future projection work should extend or translate this existing vocabulary only when real consumers prove a missing semantic category or insufficient coordinate precision.

## Non-authoritative state

The following is disposable or reconstructible and must never be queried as authored truth when a canonical semantic query exists:

- world worker mirrors and streaming projections;
- scene/render/GPU resources;
- picker geometry and contextual scene snapshots;
- selection and hover state;
- UI/widget state;
- diagnostic traces;
- previews and ghosts;
- tile/object compatibility projection adapters;
- generated render packets and caches.

Deleting any of these may lose presentation/cache work but must not lose authored state.

## TerrainController state

`TerrainController` remains a semantic-looking compatibility/projection surface, but it does not own completed chronology.

Its remaining concerns include transitional projection/preview state such as:

- edited tile values used by worker/picker projection compatibility;
- explicit-height projection markers;
- active `Stroke` preview state;
- stroke seed/counter state.

Planners must read authoritative state through `EditRuntime::document()` rather than projection compatibility values.

`Stroke` owns transient preview changes directly. User cancellation and failed commit rollback restore preview state without creating or consulting a second history mechanism.

## Semantic replay architecture

The semantic replay migration is complete through the CP2 stopping point.

### Replay R1: semantic replay footprint

`osrs-edit::MapReplayFootprint` is derived from the exact `MapPatch` and records semantic terrain channels, changed height vertices, exact removed object placements, and exact added object placements.

It is independent of workers, renderer, UI, and legacy `TileValues` authority.

### Replay R2: authoritative descriptors

`EditorSession` derives immutable Map / Scenario / Appearance replay descriptors from the exact next authoritative history patch without exposing raw mutable `PatchHistory` internals.

### Replay R3: generic projection

Undo/redo projection is driven directly by the authoritative replay descriptor and the generic map replay projector.

The projector prepares only the semantic channels/vertices/object polarity named by the replay footprint, validates against real post-replay semantic state, and prepares projection changes before worker/UI mirrors are mutated.

### Replay R4: one chronology

`TerrainController.history`, `redo_stack`, tool-shaped `EditStep` payloads, `Semantic*History` types, marker-routing stacks, and tool-specific replay adapters are removed.

There is exactly one stored chronological history:

```text
EditorSession -> PatchHistory<EditorDocument>
```

Undo/redo availability and ordering come from semantic authority.

### Replay R5: ratchets

Architecture tooling now treats reintroduction of the retired application history API/stack as a regression.

The architecture dashboard and source-level checks provide cheap evidence before Rust compilation.

## EditorApp classification

`EditorApp` remains the composition root. The important ownership distinctions are:

- `session: EditRuntime`: authored semantic gateway;
- `terrain: TerrainController`: preview/projection compatibility state, not authored truth or chronology;
- `world: WorldRuntime`: scene/worker projection orchestration;
- `renderer: RenderRuntime`: GPU presentation;
- other runtimes: product/UI/asset state under their own ownership boundaries.

The existing `editor_app_surface.json` ratchet should continue preventing casual growth of direct `EditorApp` behavior/state.

## Current cheap authority tooling

`python3 scripts/check_authority.py` protects authority rules without compiling Rust.

The current quick gate includes checks for:

- semantic checkpoint gateway ownership;
- legacy history authority regression;
- application history authority regression.

Architecture baselines are ceilings, not required debt counts. When debt is removed, delete the allowance rather than preserving a zero-value compatibility concept.

The reproducible architecture dashboard should continue reporting measurable ownership/debt signals that are stable enough to interpret.

## Guard direction from this baseline

Add guards only after legitimate call sites are inventoried. High-value future protections include:

- reject direct interactive `MapDocument` mutation outside session/runtime policy;
- reject new `TerrainController` fields that look like committed semantic authority;
- keep `EditRuntime.session` private;
- keep projection closures read-only over authoritative semantic state;
- keep `EditorApp` field/method surface ratcheted;
- reject any reintroduction of application-level history stacks parallel to `PatchHistory`;
- reject renderer/worker/picker state used as authored query truth.

Guards must not freeze legitimate hydration, streaming, project-load, export, or derived-projection boundaries.

## CP2 exit condition

CP2 is complete on `main` because:

- every completed user map edit is authoritative in `EditorSession`;
- no product action relies on legacy before/after application payloads for undo/redo truth;
- active preview data is separate from completed chronology;
- semantic replay projection derives from the authoritative history patch;
- there is no second application chronological stack aligned with `PatchHistory`;
- the retired tool-specific replay marker/route layer is gone;
- replay footprint ownership is semantic rather than caller-supplied compatibility metadata;
- architecture tooling rejects competing application history authority;
- the cleanup did not intentionally change OSRS/rendering semantics.

The next architecture work should focus on **projection precision, immutable/revisioned world-to-render boundaries, parity hardening, measurement, diagnostics, and automation**, as described in `docs/architecture/RUSTOSRS-TECHNICAL-ARCHITECTURE-SPEC.md` and future explicitly authorized plans.

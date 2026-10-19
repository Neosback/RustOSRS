# Semantic replay projection design

Status: accepted cleanup design for the next CP2/CP3 migration

## Problem

`EditorSession` owns the one authoritative semantic history, but `TerrainController` still keeps a second chronological `history` / `redo_stack` of `EditStep` routing markers. Those markers no longer contain authoritative before/after state, but they must stay perfectly aligned with `PatchHistory<EditorDocument>` so undo/redo can choose the correct transitional worker projection.

That is unnecessary duplicate chronology and a correctness risk.

The remaining marker variants are tool-shaped:

- Paint: brush + tile keys
- Height: tile keys
- Path: tile keys + whether Height participates
- Object: removed/added placements
- Composite: path footprint + object route
- Scenario
- Appearance

The semantic history already contains the exact patch that is about to undo or redo. `PatchHistory` exposes `next_undo()` / `next_redo()` to its owner. The application should derive projection requirements from that patch instead of maintaining another chronological stack.

## Decision

Derive a **semantic replay projection descriptor from the authoritative history patch**.

Do not add feature/tool provenance to `MapPatch`. A map transaction should remain about what changed, not which UI produced it.

Do not expose mutable history stacks or raw history patches to `EditorApp`.

The intended flow becomes:

```text
Ctrl/Cmd+Z or redo
  -> EditRuntime asks EditorSession for the next replay descriptor
  -> descriptor is derived from the exact next EditorPatch
  -> application captures only disposable pre-replay projection values named by descriptor
  -> EditRuntime performs checkpointed undo/redo
  -> projection is prepared from authoritative post-replay semantic state
  -> finalize on projection success, otherwise exact rollback
  -> apply prepared projection mechanically
```

There is still one semantic history and one chronological ordering.

## Map replay footprint

A `MapPatch` contains enough semantic information to derive replay projection needs without knowing its originating tool:

```text
MapOp::SetTerrain { coord, before, after }
MapOp::SetHeight { coord, before, after }
MapOp::AddObject { placement }
MapOp::RemoveObject { placement }
```

Derive a compact, immutable footprint with these concepts:

```rust
TerrainReplayChange {
    coord: MapPosition,
    channels: TerrainChannels,
}

TerrainChannels:
    UNDERLAY
    OVERLAY   // overlay id + shape + rotation as one semantic channel
    FLAGS

MapReplayFootprint {
    terrain: Vec<TerrainReplayChange>,
    heights: Vec<HeightVertexCoord>,
    removed_objects: Vec<ObjectPlacement>,
    added_objects: Vec<ObjectPlacement>,
}
```

Names are illustrative until implementation; ownership and behavior are the important part.

### Channel derivation

For each `SetTerrain` operation:

- `before.underlay != after.underlay` -> `UNDERLAY`
- `before.overlay != after.overlay` -> `OVERLAY`
- `before.flags != after.flags` -> `FLAGS`

For each non-no-op `SetHeight`, record its `HeightVertexCoord`.

For object operations, record exact canonical `ObjectPlacement` identity and mutation polarity.

For replay, Undo uses the inverse mutation polarity and Redo uses the original polarity. The set of terrain/height channels touched is direction-independent.

Coalesce repeated coordinates deterministically so projection reads/writes each semantic coordinate once even if a valid dependent patch contains multiple operations for it.

## Why this replaces tool-specific routes

### Paint

Current Paint replay needs a `Brush` only to know which preview channels to overwrite. The patch already proves which semantic channels changed. A generic terrain projector can therefore update exactly those channels from the authoritative post-replay `MapQuery`.

### Height

`SetHeight` already identifies the exact corner vertex. The projection can update Height and mark the existing explicit-height compatibility set without a `SemanticHeightHistory` marker.

### Path / Paste

Paths and pasted terrain are ordinary terrain/height operations. Their replay projection is the same channel-driven projector as Paint and Height.

### Objects

`AddObject` and `RemoveObject` carry exact canonical placement identity. The descriptor can provide the object mutations directly to the existing object-history projection logic or its generalized successor.

### Prefabs / generated buildings

A composite patch naturally yields terrain/height and object footprint data in one descriptor. No separate `Composite` route is needed.

### Scenario / Appearance

`EditorPatch` and `WorkspacePatch` already distinguish these authored domains structurally:

```text
EditorPatch::Appearance
EditorPatch::Workspace(WorkspacePatch::Scenario)
EditorPatch::Workspace(WorkspacePatch::Map)
```

They can therefore produce typed replay kinds without a parallel application marker.

## Ownership

### `osrs-edit`

Own only reusable semantic map footprint derivation from `MapPatch` if the abstraction is useful outside the editor. It must contain no worker, GPU, UI, `TileValues`, or renderer concepts.

### `EditorSession`

Own derivation of the next **editor-level replay descriptor** from its private `PatchHistory<EditorDocument>` because only the session should inspect the application-composed `EditorPatch` chronology.

The session may expose an immutable descriptor, not the mutable history stack or patch reference.

### `EditRuntime`

Own the checkpointed replay transaction. Callers should receive/use the descriptor through the runtime boundary and never coordinate raw `EditorSession` history directly.

### `osrs-editor` projection adapter

Translate the semantic descriptor to temporary worker projection values while `TerrainController.edited` and object worker mirrors still exist. This adapter is disposable and must not become semantic authority.

## Generic terrain replay projection

Replace Paint/Height/Path-specific post-replay projectors with one semantic-channel projector:

```text
input:
  current transitional TileValues
  authoritative MapQuery
  TerrainReplayChange + optional Height vertex

output:
  reconciled TileValues
```

Rules:

- update underlay only when `UNDERLAY` is present
- update overlay id/shape/rotation only when `OVERLAY` is present
- update flags only when `FLAGS` is present
- update height only for recorded height vertices
- preserve all unrelated preview channels

This preserves the important behavior currently achieved by `project_paint_values`, `project_height_values`, and `project_semantic_path_values` without carrying tool provenance through history.

## Transaction safety

Projection preparation remains inside the existing `EditRuntime` checkpoint boundary.

Do not change this ordering:

```text
semantic replay
  -> prepare complete projection from post-replay MapDocument
      failure -> exact semantic rollback
      success -> finalize semantic transaction
  -> mechanically apply prepared worker/UI projection
```

No worker mutation may happen before projection planning has succeeded.

## Migration checkpoints

### R1: semantic footprint primitive

- add map-patch footprint derivation
- no editor behavior changes
- focused `osrs-edit` unit tests only

Acceptance tests:

- terrain channel differences are exact
- overlay ID/shape/rotation behave as one overlay channel
- no-op operations create no footprint entries
- repeated coordinates coalesce deterministically
- inverse patch preserves terrain/height coordinate footprint
- object mutation polarity reverses correctly under inverse

### R2: editor replay descriptor

- derive Map / Scenario / Appearance descriptor from `EditorSession`'s exact next undo/redo patch
- keep history patch itself private
- no worker projection migration yet

Acceptance tests:

- next undo/redo descriptor tracks the exact semantic history head
- failed replay leaves descriptor/history ordering unchanged
- commit after undo invalidates redo only after finalization as today

### R3: generic map replay projector

- add channel-driven terrain/height projection
- adapt exact object mutations
- preserve current projection failure rollback semantics
- compare against existing Paint/Height/Path/Object/Composite replay tests

### R4: remove application chronological marker stack

- route undo/redo exclusively from the runtime/session descriptor
- delete `EditStep`, `SemanticPaintHistory`, `SemanticHeightHistory`, `SemanticPathHistory`, and object/composite routing markers when no longer referenced
- delete `TerrainController.history` / `redo_stack`
- make `can_undo` / `can_redo` query semantic authority through `EditRuntime`

### R5: lower architecture debt and guards

- remove the temporary legacy-history baseline entirely
- add a cheap guard against a second application history stack or new `EditStep`-style authority
- update the authority audit and feature registry references

## Non-goals

- no generic command hierarchy
- no second semantic history abstraction
- no feature/tool name stored in `MapPatch`
- no renderer-specific dirty flags in `osrs-edit`
- no exposure of mutable `PatchHistory` internals to the app
- no rewrite of the proven checkpoint/finalize/rollback mechanism
- no projection optimization mixed into this migration

## Why this direction is preferred

It reduces three currently separate concepts:

```text
semantic patch
+ tool-specific history marker
+ tool-specific replay projector
```

toward:

```text
semantic patch
-> derived semantic replay footprint
-> generic projection adapter
```

The patch remains the single source of truth for what changed, while the application retains only disposable projection state. This is more DRY at the semantic level without pretending that renderer/worker projections should disappear.

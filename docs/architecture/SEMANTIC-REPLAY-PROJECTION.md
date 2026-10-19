# Semantic replay projection design

Status: accepted design, partially implemented

Current branch: `refactor/architecture-ai-foundation`

## Problem

`EditorSession` owns the one authoritative semantic history, but `TerrainController` still keeps a second chronological `history` / `redo_stack` of routing markers. Those markers no longer contain authoritative before/after state, but they must stay aligned with `PatchHistory<EditorDocument>` so undo/redo can choose a worker/UI projection adapter.

That duplicate chronology is now the remaining replay-architecture problem.

## Decision

Derive replay projection requirements from the exact authoritative history patch.

Do not add feature/tool provenance to `MapPatch`. A semantic transaction records what changed, not which UI produced it.

Do not expose mutable history stacks or raw history patches to `EditorApp`.

The target flow is:

```text
Undo / Redo
  -> EditRuntime asks EditorSession for the next replay descriptor
  -> descriptor is derived from the exact next EditorPatch
  -> application captures only disposable pre-replay projection state named by descriptor
  -> EditRuntime performs checkpointed semantic replay
  -> projection is prepared from authoritative post-replay semantic state
  -> finalize on projection success
     OR exact rollback on projection failure
  -> prepared projection is applied mechanically
```

There is one semantic history and one chronological ordering.

## Implemented semantic footprint

`osrs-edit::MapReplayFootprint` is the canonical semantic replay footprint for map patches.

It records:

```text
MapReplayFootprint
  terrain: MapPosition + TerrainChannels
  heights: HeightVertexCoord
  removed_objects: ObjectPlacement
  added_objects: ObjectPlacement
```

`TerrainChannels` currently contains:

- `UNDERLAY`
- `OVERLAY`, where overlay ID/shape/rotation intentionally form one semantic channel
- `FLAGS`

The footprint is deterministic, coalesces repeated terrain/height coordinates, ignores no-op value operations, and preserves/reverses object mutation polarity correctly through patch inversion.

It contains no worker, UI, renderer, GPU, `TileValues`, or tool provenance.

## Implemented editor replay descriptor

`EditorSession` derives immutable replay descriptors from the exact semantic history head:

```text
EditReplayDescriptor::Map(MapReplayFootprint)
EditReplayDescriptor::Scenario
EditReplayDescriptor::Appearance
```

Undo derives the map footprint from the inverse patch so object mutation polarity matches the actual replay direction. Redo derives it from the original patch.

The raw `PatchHistory` and `EditorPatch` remain private to session ownership.

`EditRuntime::replay_descriptor()` exposes the immutable descriptor through the application gateway.

## Implemented generic map replay projector

The editor now has a generic channel-driven map replay adapter.

It captures only transitional tile values named by the semantic footprint, then prepares the complete worker/UI projection from authoritative post-replay state.

Rules:

- update underlay only when `UNDERLAY` is present
- update overlay ID/shape/rotation only when `OVERLAY` is present
- update flags only when `FLAGS` is present
- update height only for recorded height vertices
- preserve unrelated preview channels
- validate removed objects are absent after replay
- validate added objects are present after replay
- convert exact semantic object placement mutations into the existing worker projection form

Missing transitional tiles or unrepresentable semantic values fail projection preparation before any worker mutation is returned.

## Transaction safety

The ordering remains non-negotiable:

```text
semantic replay
  -> prepare complete projection from post-replay authored state
      failure -> exact semantic rollback
      success -> finalize semantic transaction
  -> mechanically apply prepared worker/UI projection
```

No worker mutation may happen before projection preparation and runtime finalization succeed.

## Why tool-specific replay routes can disappear

### Paint

The semantic patch already identifies exactly which terrain channels changed. Replay no longer needs the original `Brush` to determine which channels should be refreshed.

### Height

`SetHeight` identifies exact changed vertices. Replay does not need a Height-specific chronological marker.

### Path and tile Paste

These are ordinary terrain/height semantic operations. Their replay projection follows the same channel/height footprint.

### Objects

`AddObject` and `RemoveObject` carry exact canonical placement identity and replay polarity.

### Prefabs and generated buildings

A composite semantic patch naturally contains terrain/height and object operations in one footprint.

### Scenario and Appearance

`EditorPatch` structure already distinguishes these authored domains without a parallel tool marker.

## Ownership

### `osrs-edit`

Own reusable semantic footprint derivation from `MapPatch` only. No worker, renderer, UI, or compatibility projection types belong here.

### `EditorSession`

Own derivation of the next application-level replay descriptor from private semantic chronology.

### `EditRuntime`

Own replay descriptor access, checkpointed undo/redo, finalization, exact rollback, receipt/touch accounting, and diagnostics observation.

### `osrs-editor` replay adapter

Translate semantic replay descriptors into disposable worker/UI projection while transitional mirrors still exist. This layer must never become semantic authority.

## Migration status

### R1 - semantic footprint primitive - COMPLETE

Implemented in `osrs-edit` with focused tests for:

- exact terrain channel differences
- overlay identity/shape/rotation as one semantic channel
- no-op omission
- deterministic coordinate coalescing
- inverse terrain/height footprint stability
- inverse object mutation polarity

### R2 - editor replay descriptor - COMPLETE

Implemented in `EditorSession` with tests proving:

- next undo/redo descriptor follows the exact authoritative history head
- rollback restores descriptor ordering
- Scenario and Appearance are structurally typed descriptors

### R3A - generic map replay projector foundation - COMPLETE

Implemented and tested, including:

- named-channel-only terrain projection
- independent overlay and height projection
- semantic object mutation projection
- failure before worker mutation when required transitional data is missing

`EditRuntime` also derives its replay diagnostic/touch footprint from the semantic descriptor.

### R3B - wire descriptor into active undo/redo routing - NEXT

The current active router still chooses tool-specific `Route` values from `EditStep` and uses tool-specific replay preparation. `EditRuntime::replay` still accepts an ignored compatibility footprint parameter.

Next checkpoint:

- capture replay input from `EditRuntime::replay_descriptor()`
- use the generic map replay projector for Map descriptors
- handle Scenario/Appearance directly from descriptor kind
- remove the ignored compatibility replay-footprint argument
- preserve notices and object reselection as non-authoritative UX behavior
- preserve exact rollback and post-finalization-only worker mutation

Acceptance:

- replay projection requirements come exclusively from the semantic descriptor
- no tool-specific map replay planner is required
- existing Paint/Height/Path/Object/composite replay behavior remains covered

### R4A - remove duplicate chronological marker stacks

After R3B is proven:

- remove `TerrainController.history`
- remove `TerrainController.redo_stack`
- make undo/redo availability come from `EditRuntime`
- remove `EditStep` and `Semantic*History` types when unreferenced

User-facing labels may survive only as optional non-authoritative metadata if still justified. They must not require a second history stack.

### R4B - remove obsolete tool-specific replay-only helpers

Delete replay-specific Paint/Height/Path/Object projection code that has no remaining live-preview or commit-projection caller.

Do not delete shared helpers still required for live gesture preview or commit reconciliation.

### R5 - lower architecture debt and tighten guards

- remove obsolete legacy/history baselines
- add a cheap guard against reintroducing parallel application chronology
- keep checkpoint lifecycle and session privacy enforcement
- update authority/feature documentation
- add reproducible architecture metrics where useful

## Non-goals

- no generic command hierarchy
- no second semantic history abstraction
- no tool name/provenance stored in `MapPatch`
- no renderer-specific dirty flags in `osrs-edit`
- no exposure of mutable `PatchHistory` internals
- no rewrite of checkpoint/finalize/rollback
- no replay optimization mixed into chronology cleanup
- no assumption that every presentation cache must disappear

## End state

The migration reduces:

```text
semantic patch
+ tool-specific history marker
+ tool-specific replay projector
```

to:

```text
semantic patch
-> derived semantic replay descriptor
-> generic disposable projection adapter
```

The semantic patch remains the source of truth for what changed. Projection state remains disposable. Only one chronological history remains.

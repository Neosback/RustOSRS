# Semantic replay projection design

Status: accepted design, implemented through the CP2 stopping point on `main`.

Reconciled against `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd` on 2026-10-19.
See [AUTHORITY-AUDIT.md](AUTHORITY-AUDIT.md) for the current authority classification.

## Resolved problem

The former `TerrainController.history` / `redo_stack` duplicated the chronology owned by
`PatchHistory<EditorDocument>`. Those routing stacks and tool-shaped history markers are removed.
The remaining controller owns preview/projection compatibility state only.

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

### R3B - active descriptor-driven replay - COMPLETE

The active router reads `EditRuntime::replay_descriptor()`, uses the generic map replay projector,
and handles Scenario/Appearance by descriptor kind. The ignored compatibility footprint argument
is removed. Projection preparation uses post-replay authority before finalization and worker mutation.

### R4A/R4B - duplicate chronology and obsolete routes - COMPLETE

`TerrainController.history`, `redo_stack`, `EditStep`, `Semantic*History` payloads and tool-specific
replay routing are removed. Undo/redo availability comes from `EditRuntime`. Helpers still used for
live preview and commit reconciliation remain projection adapters, not chronology.

### R5 - authority ratchets - COMPLETE

`check_authority.py` rejects escaped checkpoint lifecycle and reintroduced application history.
The architecture guard and application-surface ratchet remain in force; removed allowances must not
be restored. See [the technical audit](TECHNICAL-ARCHITECTURE-AUDIT.md) for subsequent projection work.

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

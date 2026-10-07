# RustOSRS Editor Document, Commands, History, and Persistence

Status: **Checkpoint 6 normative editor blueprint**  
Decision class: `PROJECT_DECISION`

This document defines how `osrs-editor` owns project/document state, semantic mutations, undo/redo, autosave, recovery, and save/export boundaries.

## 1. Core rule

All persistent edits execute through semantic commands or transactions.

The renderer, panels, inspectors, gizmos, and tools never patch GPU state as the authoritative result of an edit.

## 2. Document model

An open project has three distinct representations:

### Source baseline

The immutable source identity the project was opened against:

- cache/revision profile;
- cache fingerprint;
- source region/map identities;
- baseline decoded definitions/assets.

### Working semantic document

The current authoritative edited state.

It may be represented internally as a baseline plus normalized patches, materialized region data, or a hybrid, but its public command API exposes semantic operations rather than raw byte offsets.

### Export artifact

Bytes/files produced from a validated semantic document for a selected target/export format.

Export is a derived operation. Exported bytes are not the live project state.

## 3. Stable semantic identity

Commands operate on stable identifiers, not array slots or GPU handles.

Recommended identity categories:

- `RegionId`;
- `WorldTile` / semantic tile key;
- `LocInstanceId`;
- `DefinitionId`;
- project-owned annotation IDs.

If source formats do not contain an intrinsic unique loc ID, the editor assigns a project/session-stable identity derived from source region + original record identity and persists remapping when necessary.

Identity must survive reordering, zone rebuilds, panel refreshes, and most non-destructive edits.

## 4. Command contract

Conceptually:

```rust
trait EditorCommand {
    fn label(&self) -> CommandLabel;
    fn validate(&self, doc: &SemanticDocument) -> Result<(), CommandError>;
    fn apply(&mut self, doc: &mut SemanticDocument) -> Result<ChangeSet, CommandError>;
    fn revert(&mut self, doc: &mut SemanticDocument) -> Result<ChangeSet, CommandError>;
}
```

Exact Rust signatures may differ, but every command must support deterministic application/reversal or store sufficient before-state to reverse exactly.

`ChangeSet` describes semantic consequences such as:

- changed regions;
- changed tiles;
- changed loc instances;
- scene rematerialization requirements;
- renderer dirty zones;
- inspector/search invalidation;
- export-dirty regions;
- problem revalidation scope.

The command itself does not manipulate GPU buffers.

## 5. Transaction contract

A user gesture can produce many low-level mutations but should normally enter history as one transaction.

Examples:

- drag a loc from one tile to another;
- paint a terrain brush stroke across 30 tiles;
- rotate a multi-selection;
- paste a group of locs;
- change four corner heights through one sculpt gesture.

A transaction:

1. captures its starting semantic state or reversible deltas;
2. allows preview/interactive updates;
3. validates the final result;
4. commits one history entry on pointer/key completion;
5. cancels cleanly back to the exact starting state.

A failed commit must not leave half-applied semantic state.

## 6. Interactive edit preview

During a drag/brush gesture, the editor may choose one of two safe patterns:

### Reversible live transaction

Semantic working state is updated continuously, but the transaction owns the full inverse and can roll back exactly.

### Overlay preview

The authoritative document remains unchanged while the tool renders a placement/transform ghost. A single semantic command is applied on commit.

Use overlay preview whenever the target semantic operation is discrete and cheap, such as placing a loc or choosing orientation.

Use reversible live transactions when users need immediate scene feedback for continuous terrain edits.

## 7. Command categories

Initial command families should include:

### Loc commands

- place loc;
- delete loc;
- move loc by semantic tile delta;
- set loc orientation;
- replace loc definition while preserving compatible placement fields;
- duplicate loc;
- batch delete/move/orient compatible selections.

### Terrain commands

- set corner height(s);
- sculpt height delta;
- flatten to height;
- set underlay;
- set overlay;
- set terrain shape;
- set terrain rotation;
- set supported tile flags/settings;
- batch operations over selected rectangles/brush masks.

### Project/editor metadata commands

Only when metadata should participate in project undo, for example named annotations. Camera and normal panel state should not normally enter semantic history.

## 8. Semantic-safe transform policy

The editor must not imply that OSRS locs support arbitrary continuous transforms when the export semantics do not.

Persistent loc transformation is restricted to representable semantic operations such as:

- whole-tile anchor changes;
- orientation values supported by the loc format;
- definition/type changes supported by the target profile.

A free-translation/rotation gizmo may be used as an interaction aid, but commit must snap/convert to a valid semantic operation or remain an editor-only annotation/preview that is explicitly non-exportable.

The user must never believe an arbitrary transform was saved to the OSRS map when it cannot be represented.

## 9. Terrain transaction granularity

Terrain editing must record exact before/after values for every affected semantic field.

A brush stroke should generally be one history entry even when it crosses zone or region boundaries.

The resulting `ChangeSet` may dirty multiple 8x8 renderer zones and multiple source regions.

If an operation requires unavailable neighbor context, validation blocks or explicitly limits the operation rather than silently treating missing neighbors as zeros/defaults.

## 10. History model

History contains ordered committed transactions.

Required state:

- undo stack;
- redo stack;
- current document generation;
- saved checkpoint generation/hash;
- autosaved checkpoint generation/hash;
- human-readable command labels;
- affected-region summary.

A new committed command after an undo clears the redo branch unless a future branching-history feature is explicitly designed.

## 11. Command coalescing

History may coalesce repeated compatible commands when doing so matches user intent.

Examples:

- repeated arrow-key nudges of one selection;
- repeated numeric edits while one field retains focus;
- continuous terrain brush samples in one pointer stroke.

Coalescing must have explicit boundaries such as:

- pointer/key release;
- focus change;
- selection change;
- active tool change;
- time threshold only as a secondary heuristic.

Do not coalesce edits across different semantic targets merely because they occurred close in time.

## 12. Undo/redo invariants

Undo/redo must restore:

- exact semantic field values;
- loc existence and identity;
- region ownership;
- semantic generation;
- corresponding dirty/rematerialization signals.

Undo does not need to restore:

- GPU allocation addresses;
- renderer compile timing;
- pick IDs;
- hover state;
- camera unless camera history is a future explicit feature.

After undo/redo, derived scene/render state may rebuild asynchronously from the restored semantics.

## 13. Dirty state

Distinguish:

- `project_dirty`: current semantic project differs from last explicit project save;
- `export_dirty`: one or more source/export targets differ from the last successful export/write;
- `autosave_pending`: semantic generation has not yet been captured by autosave;
- `renderer_dirty`: derived rendering needs recompilation;
- `problems_dirty`: validation must be rerun.

These flags are related but not interchangeable.

A render-zone rebuild must never make the project appear saved.

## 14. Project file

A RustOSRS project file is a versioned editor artifact, not a replacement OSRS cache format.

Recommended logical contents:

```text
project_schema_version
project_id
created_with
last_saved_with
target_profile
source_cache_fingerprint
source_references
workspace_regions
semantic_edit_payload / working map payload
project_annotations
export_configuration
optional camera bookmarks
```

User-global UI layout/theme/shortcut preferences should normally live outside the project.

## 15. Project serialization

Requirements:

- explicit schema version;
- forward migration path;
- reject unsupported future schema versions with a useful error;
- deterministic serialization where practical for testing/diffability;
- no serialized GPU handles;
- no dependence on transient egui IDs;
- semantic IDs and target revision retained;
- source-cache fingerprint retained so changed source data is detectable.

The specific serialization format is implementation policy to finalize before code lands. Human-readable formats are useful for manifests, while large terrain/edit payloads may justify a structured binary companion. The persistence API must not make file-format choice part of semantic APIs.

## 16. Atomic project save

A normal save must use an atomic replacement strategy where supported:

1. serialize a self-consistent project generation;
2. write to a temporary sibling file;
3. flush/sync according to platform policy;
4. replace/rename the target;
5. only then mark that generation explicitly saved.

A failed save leaves the previous valid project intact and keeps the document dirty.

## 17. Autosave

Autosave protects work but is not equivalent to explicit save.

Requirements:

- generation-tagged;
- never blocks the UI thread on expensive serialization/I/O;
- writes a consistent immutable document snapshot or journal segment;
- stale autosave completions cannot overwrite newer autosaves;
- autosave failure is surfaced in the status/problem UI;
- explicit save does not delete the only recovery copy until the explicit save is confirmed valid.

A sensible initial policy is event-driven plus periodic debounce, for example after meaningful edits settle and at a bounded recurring interval. Exact cadence belongs to implementation settings, not semantics.

## 18. Recovery

At startup/open, if recovery data is newer than the last explicit save, offer a clear recovery path.

Recovery information should include:

- project path/identity;
- explicit-save generation/time;
- recovery generation/time;
- source-cache fingerprint compatibility;
- validation status when available.

Recovery opens into a new working session first. The original explicit project file is not overwritten until the user explicitly saves.

## 19. Journal option

For large projects, an append-only command journal may supplement periodic full autosave snapshots.

Journal entries must be:

- versioned;
- checksummed/framed so partial tail writes are detectable;
- tied to a base snapshot generation;
- deterministic/replayable;
- compactable after a successful full snapshot.

The journal must record semantic commands/deltas, not mouse events or GPU changes.

## 20. Save versus export

### Save project

Persists RustOSRS editor state and working semantics.

### Export/write target map data

Validates and encodes the current semantic document into target OSRS/cache/map formats.

These are separate commands and separate dirty states.

Closing the editor after saving the project may still warn that map export is stale when that distinction matters to the workflow.

## 21. Export validation

Before export, validate at least:

- target-profile compatibility;
- region ownership;
- semantic field representability;
- loc type/orientation/model constraints where applicable;
- terrain data encodability;
- unresolved revision-sensitive blockers;
- missing source assets/definitions;
- editor-only unsupported transforms/annotations;
- source-cache fingerprint policy.

Export must fail with contextual errors rather than silently dropping unsupported state.

## 22. Export transaction

Export should operate from an immutable semantic snapshot tagged with project generation.

If editing continues during export:

- the export may finish for its captured generation;
- UI reports which generation was exported;
- `export_dirty` remains true if the live document has advanced.

This avoids freezing editing while maintaining correctness.

## 23. Source cache changes

When reopening a project against a source cache whose fingerprint differs:

1. do not silently accept equivalence;
2. show changed-source status;
3. validate target/revision compatibility;
4. offer an explicit rebase/relink workflow later;
5. preserve the old source identity in project provenance.

A different cache may change definitions/models/textures even if region IDs are identical.

## 24. Background work and generations

Any asynchronous task that reads document data receives an immutable snapshot/generation.

Examples:

- scene materialization;
- zone compilation;
- thumbnail generation;
- validation;
- autosave;
- export.

Completion handlers compare generations and either install the result, mark it historical, or discard it.

Workers never hold a long-lived mutable borrow/lock over the live editor document.

## 25. Concurrency model

Preferred architecture:

- UI thread owns immediate editor/session state;
- semantic document mutations serialize through the command dispatcher;
- worker pool handles pure snapshot-based jobs;
- results return through typed messages/events;
- renderer consumes generation-tagged extraction artifacts;
- save/export workers operate on captured document snapshots.

Avoid a design where every panel locks one global `Arc<RwLock<Everything>>` throughout rendering. Fine-grained immutable snapshots and message passing are easier to reason about and test.

## 26. Failure handling

A command failure must include enough context to diagnose:

- command type;
- semantic IDs;
- region/tile/plane;
- target profile;
- owning validation/spec when known.

A failed command must either apply completely or not apply.

Autosave/save/export errors never silently clear dirty state.

## 27. Required tests

Implementation must eventually include:

- apply/revert equality tests for every command;
- randomized command/undo sequences returning to exact initial document state;
- transaction cancel tests;
- history coalescing boundary tests;
- cross-region brush undo tests;
- stale worker result rejection tests;
- atomic-save failure tests;
- autosave generation ordering tests;
- recovery from truncated/corrupt journal tail;
- project schema migration tests;
- save-vs-export dirty-state tests;
- source-cache fingerprint mismatch tests.

## 28. Acceptance conditions

The document/history system is correct when:

1. every persistent UI edit maps to a semantic command/transaction;
2. undo/redo is exact and independent of renderer state;
3. cancelled gestures restore exact pre-gesture semantics;
4. project save and target export are distinct and correctly tracked;
5. autosave/recovery cannot overwrite newer work with stale generations;
6. asynchronous work cannot commit against the wrong semantic generation;
7. unsupported state blocks export instead of being silently lost;
8. a complete renderer reset has no effect on document/history correctness.

# RustOSRS Map Editor Architecture Handoff

Last updated: 2026-10-14

`ARCHITECTURE.md` is the authoritative architecture contract. This handoff records current implementation state, exact SHAs, completed checkpoints, transitional hazards, validation, and the next allowed boundary.

## Project goal

Build a production-grade OSRS editor where semantic map state is authoritative, deterministic, headless-testable, and independent from UI, rendering, world-worker state, picker state, cache codecs, and GPU resources.

Core invariant:

> If every GPU buffer, scene cache, picker cache, world-worker edit mirror, and UI widget vanished, the complete edited map would still exist correctly inside the semantic editor session.

## Mandatory working rules

1. Read `ARCHITECTURE.md` and this handoff before coding.
2. One active development branch/system at a time.
3. Work in small checkpoints and stop after each checkpoint for review.
4. Do not begin the next checkpoint without explicit user direction.
5. A completed subsystem must pass exact-head Tier A/B/C as required before merge.
6. After merge, validate the exact integrated `main` commit.
7. Preserve semantic/reference parity unless a checkpoint explicitly changes behavior.
8. Keep renderer/117HD work isolated from editor-domain migration unless a narrow adapter change is required.
9. One logical user gesture should become at most one logical `MapPatch` and one semantic history entry.
10. `MapDocument` is authoritative. Worker/render/picker/UI state is projection/cache only.
11. Use existing workspace boundaries. Do not create parallel replacement architecture crates.
12. New or fully refactored production Rust files have a normal hard cap of 500 lines and 32 KiB.
13. Dedicated test modules have a hard cap of 700 lines and 40 KiB.
14. Existing oversized production modules are listed in `scripts/architecture_debt.json` at exact measured baselines and may never grow beyond those baselines.
15. Once an allowlisted file falls within the normal cap, its exception must be removed permanently.
16. Exact SHA matters. A green run certifies only the SHA it tested.

## Target data flow

```text
COMMAND
UI/input -> typed intent -> pure planner -> MapPatch -> EditorSession::commit

QUERY
UI/tool/read model -> read-only semantic query -> MapDocument

PROJECTION
semantic commit -> revision + ChangeImpact + touched area
                -> projection invalidation
                -> async rebuild
                -> revision/generation check
                -> worker/render/GPU replacement
```

Loading remains:

```text
cache decode
  -> semantic RegionMap / MapRegionSnapshot
  -> EditorSession / MapDocument
  -> scene construction
  -> renderer
```

Never seed semantic authority by scraping renderer meshes, picker geometry, worker edit mirrors, or GPU buffers.

## Workspace ownership summary

- `osrs-core`: typed coordinates, IDs, planes, shared semantic vocabulary
- `osrs-edit`: pure/headless editable domain, patches, history, validation, queries
- `osrs-cache`: cache codecs/infrastructure
- `osrs-scene`: OSRS scene semantics
- `osrs-hd`: reusable 117HD data/settings/material/environment semantics
- `osrs-render`: WGPU projection only
- `osrs-world`: streaming/orchestration and transitional projection workers
- `osrs-reference`: fixtures/parity infrastructure, never a production dependency
- `osrs-editor`: product composition root, UI, tools/controllers, application adapters

## Core semantic contract already established

- canonical typed coordinates come from `osrs-core`
- `MapPosition = MapTile + SourcePlane`
- `HeightVertexCoord = MapTile corner + SourcePlane`
- `TerrainTile` owns underlay/overlay/flags; height is separate
- missing sparse terrain = `TerrainTile::default()`
- missing height is distinct from numeric `0`
- object identity is exact `ObjectPlacement`
- `MapPatch` contains reversible `MapOp`s and derives `ChangeImpact`
- `MapDocument::apply_patch` validates the complete touched state atomically before mutation
- one successful non-empty patch = one semantic history entry
- empty patches create no history and do not clear redo
- failed commit/undo/redo leaves state/history coherent

## Merged architecture systems

### PR #33: edit-domain foundation

Squash merge:
`5c5eef3bde696b20ae0319867e6e84567701ebf6`

### PR #34: history/query foundation

Squash merge:
`cf7001a9d0d7af847cc2ad8568a4701a7dc7f1eb`

### PR #35: semantic path planner + simplified fitter

Squash merge:
`cf552b734e1fdef54bf90b3b5c08211433f56a40`

Established:

- deterministic Catmull-Rom fitting
- OSRS overlay-shape matching
- pure terrain-path `MapPatch` planner
- obsolete pathfinding/router/network/wall/noise stack removed

Do not reintroduce pathfinding, wall mode, topology repair, automatic joining, or hidden obstacle routing without an explicit product decision.

### PR #36: persistent semantic editor session

Squash merge:
`ace39238f67fa5c3f31611bf60ca5c106ace1d90`

Post-merge CI `38363970256`: Tier A/B/C PASS.

Established:

- persistent `EditorSession { MapDocument, EditHistory, revision }`
- semantic region hydration/unload
- dirty-region preservation across streaming churn
- cache-source reset boundary

### PR #37: Paint -> semantic session

Squash merge:
`be59c01dc57e72d256ff616d1947594d939a4b5b`

Post-merge CI `38370847153`: Tier A/B/C PASS.

Established:

- Paint commit + undo/redo authoritative through `EditorSession`
- worker `TileEdit` is projection only

### PR #38: Height -> semantic session

Squash merge:
`392206ae55e35dbe26e17d664a693b21a42dc006`

Post-merge CI `38384232077`: Tier A/B/C PASS.

Established:

- Height commit + undo/redo authoritative through `EditorSession`
- exact storage quantization and explicit lower-plane semantics
- worker preview remains projection only

### PR #39: Object -> semantic session + editor module hardening

Regular merge commit:
`50960fddbcc6851dea285b4b90c51b806fd70215`

Post-merge CI `38410554574`: Tier A/B/C PASS, including M3-M9.

Established:

- pure deterministic Object planner
- live Object actions commit once through `EditorSession`
- Object undo/redo use `EditorSession`
- worker `LocEdit` is projection only
- stale projection routes validate against authoritative `MapDocument`
- Paint, Height, and Object share one chronological semantic-history routing order
- old `app/editing.rs` responsibility was split into focused history/tests modules

### PR #40: Path -> persistent semantic session

Final pre-merge branch head:
`6a66758bf901a3c8af919dffc1434b84a0735c23`

Exact branch CI:
`38416867837` - Tier A/B/C PASS, including M3-M9.

Regular merge commit on `main`:
`a125f8e237f4a1594fe585df5a0dd7a2e2011da6`

Exact post-merge CI:
`38422182517` - Tier A/B/C PASS, including M3-M9.

Path authority now follows:

```text
fitted PathTile[]
  -> TerrainPathIntent[]
  -> build_terrain_path_patch(EditorSession::document, ...)
  -> projection preflight on disposable scratch state
  -> one EditorSession::commit
  -> authoritative MapDocument
  -> transitional worker TileEdit projection
```

Path undo/redo follows:

```text
global chronological route
  -> EditorSession::undo / redo
  -> authoritative MapDocument
  -> complete projection preflight
  -> worker/render mirror update
```

Path no longer stores terrain/height before/after values as history authority. Its global route stores only lightweight semantic projection metadata:

```text
SemanticPathHistory {
  keys: Vec<TileKey>,
  include_height: bool,
}
```

Paint, Height, Object, and Path now share the same semantic-history model:

```text
one user transaction
  -> one semantic commit
  -> one lightweight global route
  -> one EditorSession history entry
  -> projection-only worker mirror
```

The former Path split-history hazard is closed.

### PR #41: Architecture Contract v2 + debt ratchet

Final branch head:
`e38a3b5035c6ebd6ea65adb899fc31a89c45ebea`

Exact branch CI:
`38427434417` - Tier A/B/C PASS, including M3-M9.

Regular merge commit on `main`:
`bd21b72555cbe75dd1fbb8b70df1f7cd5b90ad41`

Exact post-merge CI:
`38428015008` - Tier A/B/C PASS, including M3-M9.

Established:

- root `ARCHITECTURE.md` is the authoritative Architecture Contract v2
- production Rust module normal cap is 500 lines / 32 KiB
- dedicated test-module cap is 700 lines / 40 KiB
- 13 pre-existing oversized production modules are frozen at measured debt baselines
- stale, orphaned, malformed, or growing debt entries fail the architecture guard
- architecture guard self-tests run in Tier A

The former `architecture/editor-contract-v2` branch could not be deleted through the currently available repository tooling, so its continued remote existence must not be interpreted as an active development branch.

## Semantic loading and residency

`EditorSession` tracks loaded and dirty regions.

- clean region ingestion may replace semantic source state
- identical ingestion is a no-op
- dirty regions are not overwritten by incoming streaming snapshots
- dirty regions are not semantically evicted by camera unload
- cache-source switch resets the semantic session

Height ingestion stores source-owned 64x64 final height samples per plane. Do not invent shared 65th boundary samples. Neighboring source regions own their boundary data.

Transitional caveat: `WorldDefinitions::region()` can still include legacy worker edits until remaining editing/projection routes are migrated.

## Completed: runtime ownership and architecture debt work

`RenderRuntime` (`app/render_runtime.rs`) owns the eframe WGPU render state, the sole `SceneRenderer`, viewport targets, HD GPU install, and frame submission.

`WorldRuntime` (`app/world_runtime.rs`) owns the disposable world-side projection state: the `RegionStreamer` and `AnimationWorker`, presentation generation/HD flag, resident/in-flight/dirty region sets, animation schedule, worker-readiness flag, and mirroring of tile/loc edits to the worker. `EditorSession` remains authoritative.

`InteractionRuntime` (`app/interaction_runtime.rs`) owns camera, view plane, hover/context picks, selection, marquee, spline state, tool peek, object drag, and gizmo state. It remains field ownership only; behavior encapsulation is still pending.

Every file on the former architecture-debt allowlist is now within the 500-line / 32 KiB cap and `scripts/architecture_debt.json` is empty. The guard still rejects new or growing exceptions.

### Phase 4 closed: context-menu overlay clear is semantic

`clear_overlay_at` now commits through the same Paint planner as every other stroke. The legacy terrain-mutation fallback is gone, so the invariant "every terrain mutation enters through `EditorSession`" is enforced in code.

### Phase 8 started: map-stream encoders (`osrs-cache::encode`)

`encode_terrain` / `encode_locations` are strict inverses of `decode_terrain` / `decode_locations`, with typed `MapEncodeError`s for unrepresentable values (no silent truncation). Measured against every decodable square of the pinned build-241 cache:

- locations: 2936 / 2936 byte-exact
- terrain: 2935 / 2936 byte-exact; the one exception carries client-discarded data that the semantic decoder does not retain
- every re-encoded terrain stream decodes back to identical retained semantics
- repacking all five files of every real group is byte-exact: 2936 / 2936

### Phase 8: export of edited regions

`session::export` diffs the authoritative document against pristine source semantics, rewrites only changed terrain/location streams, preserves untouched group files, and verifies the result by decoding. The source cache is never modified.

Not done: writing into `.dat2`/`.idx`, reference-table updates, and full loadable-cache repack/version/CRC bookkeeping.

## Current checkpoint: project/source lifecycle hardening

Active branch: `fix/project-source-lifecycle`

Base `main`:
`fcb80c17a00d33421fc446cc68cb78efaa1ae3ec`

Certified implementation head before this handoff update:
`2ddc6192f491f67a4bfd3919c60dc99391223505`

Exact CI:
`38524410640` - Tier A/B/C PASS, including architecture guard, formatting, workspace compile, strict Clippy, WebAssembly portability, workspace tests, and M3-M9 semantic/reference parity.

Lifecycle contract established on this branch:

```text
current session
  -> source/project transition request
  -> if dirty: Save / Discard / Cancel
  -> transition permitted
  -> reset source-bound project + semantic state
  -> load/restart pristine source
  -> apply only the new project's diffs
```

- cache switches and GUI project opens now use one lifecycle guard
- a source/project replacement is blocked while an asynchronous project save is in flight
- Save-before-transition waits for the save to finish; failed/disconnected saves reopen the transition prompt
- cache replacement clears source-bound project state and resets the authoritative `EditorSession`
- Open Project is replacement, not implicit merge: it resets the current semantic session and restarts the current cache from its pristine source before applying the opened project's diffs
- scripted project open refuses to replace unsaved work and refuses to run while a save is active
- pending project diffs are retained until semantic application and all synchronous projection steps succeed, so failures remain retryable
- project projection retries derive work from the stored `RegionDiff`, not only from a now-empty second semantic patch
- obsolete `EditorSession::is_loaded` and `clear_history` APIs were removed because project replacement now resets the whole session instead of pretending history-only replacement is sufficient
- regression tests prove source-bound project reset, replacement dropping old semantic edits/pending diffs, failed project-region application retaining its pending diff, and dirty transition staging without replacement

Known lifecycle caveats intentionally not solved on this branch:

- semantic rollback after projection failure is still not revision-neutral; the next focused architecture branch should solve exact transaction rollback rather than extending this lifecycle branch
- if an opened project's pending region applies while a save snapshot is already in flight, the saved file still contains that pending diff, but the later `edit_revision` can conservatively leave the session marked dirty and require another save; this is safe but can be redundant
- projects remain native-only; the WebAssembly project module is a stub, while browser cache/source replacement still resets the semantic session

### Projects: File > Open / Save project (Ctrl/Cmd+S)

A project file (`rustosrs-project-v1` JSON, `session/project_file.rs`) stores only per-region `RegionDiff`s plus the bundled cache profile id. Decoding validates semantic values and rejects unknown formats/out-of-range values rather than clamping them.

- Save (`app/project.rs`) runs on a worker thread, reads pristine source squares, diffs a document snapshot, merges still-unapplied project diffs, and writes atomically using a temporary file + rename.
- Open validates the file against the currently open cache, then routes through the lifecycle guard. Opening a project means replacement: the old semantic session/history/project state is discarded, the current source is restarted from pristine cache data, and only the newly opened project's diffs are queued.
- Project diffs apply through `EditorSession::apply_untracked` without becoming undo steps. The region remains queued until semantic application and synchronous tile/object projection succeed.
- Closing the application with unsaved edits still uses Save and quit / Quit without saving / Cancel. A close request supersedes a pending source/project transition choice.
- Export continues to share the same `RegionDiff` representation.

### Place new objects

The Objects tool can add cache objects through the same semantic Object planner and `EditorSession` authority used for move/duplicate/delete. Placement remains one semantic commit and one undo step.

### Copy / paste tiles

Tile copy/paste records terrain channels and heights relative to the copied selection and pastes through one semantic `MapPatch`/history step. Objects are intentionally not copied by the tile clipboard.

### Phase 7 gate: WebAssembly portability

The Tier A CI gate runs the WebAssembly portability checks and is green on the current lifecycle branch. Native-only project/export behavior remains cfg-gated with browser stubs where required.

## Remaining migration roadmap

### Phase 2: decompose EditorApp state ownership

Completed ownership extractions:

1. `RenderRuntime` - done
2. `WorldRuntime` - done
3. `InteractionRuntime` - field ownership done; behavior encapsulation pending

Remaining focused ownership work:

4. `ProjectRuntime`
5. `UiRuntime`
6. `DiagnosticsRuntime`

Do not combine runtimes into one large refactor. Behavior remains unchanged during pure ownership extraction.

### Transaction rollback before central gateway

Before routing more edits through one gateway, make failed semantic-commit + projection sequences exactly revision-neutral. Required invariant after failed projection: document, history, revision, edit revision, and redo state are exactly what they were before the attempted transaction.

### Phase 3: central edit gateway

- one application gateway for edit intents/patches
- Paint, Height, Object, Path, and context-menu edits converge on one semantic commit-result path
- remove transitional split routing when safe
- keep `MapPatch` and `EditorSession` as semantic authority

### Phase 5: projection/invalidation service

- centralize semantic receipt/change impact -> invalidation
- remove tool-specific dirty-region poking
- revision/generation-check every async install
- add 8x8 projection granularity only after profiling and edge-rule tests justify it

### Phase 8: cache writing/round-trip guarantees

- add small deterministic checked-in write/repack fixtures that do not require a real cache environment
- retain the real-cache full-sweep tests as extended validation
- complete transactional loadable-cache repack only after fixture guarantees are strong

### Phase 9: renderer/projection optimization

- profile first
- introduce finer zone/chunk rebuilds only where measured
- semantic ownership remains unchanged

### Phase 10: continue 117HD parity

Resume on separate focused branches once editor authority/projection boundaries are stable enough to prevent coupling.

## Next boundary

Finish this branch through review/PR/merge only after explicit user direction. After it is merged and the new exact `main` is certified, re-read main before choosing the next branch. The current provisional next architecture branch is `refactor/edit-transaction-rollback`; central edit-gateway and projection-invalidation work should remain later, separate branches.

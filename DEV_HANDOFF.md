# RustOSRS Map Editor Architecture Handoff

Last updated: 2026-10-13

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

## Current active checkpoint: Phase 2.1 RenderRuntime ownership extraction

Branch:
`refactor/editor-render-runtime`

Certified base `main`:
`bd21b72555cbe75dd1fbb8b70df1f7cd5b90ad41`

Base CI:
`38428015008` - Tier A/B/C PASS.

Validated implementation head before final handoff bookkeeping:
`89392a6c28eb645a9a787ac8e72d7db946c4813b`

Exact implementation CI:
`38430771972` - Tier A/B/C PASS, including M3-M9 semantic/reference parity.

### Ownership extracted

`EditorApp` no longer directly owns the eframe WGPU render state, raw `SceneRenderer`, or viewport render targets.

`RenderRuntime` now owns:

- `egui_wgpu::RenderState`
- the sole `SceneRenderer`
- viewport color texture and texture view
- viewport depth texture and texture view
- egui viewport texture registration/update
- render-origin reads and rebasing
- region upload/removal and texture installation adapters
- overlay-line installation
- HD lighting, shadow settings, parameters, material GPU installation, and shadow-memory metrics
- viewport command-encoder creation
- scene frame encoding
- queue submission
- `finish_frame()` / GPU scene timing
- submitted-work completion callback registration
- resident GPU-zone and vertex-memory metrics

Existing UI, streaming, selection, gizmo, plugin, settings, and HD modules reach rendering through explicit `RenderRuntime` methods. There is no broad `Deref` escape hatch back to `SceneRenderer`.

The viewport camera and semantic/editor state remain outside `RenderRuntime`, as required. This checkpoint does not change scene semantics, rendering algorithms, 117HD behavior, cache decoding, edit authority, streaming policy, or user-visible product behavior.

### Validated diff before handoff bookkeeping

Compared with certified base `bd21b72555cbe75dd1fbb8b70df1f7cd5b90ad41`:

- branch status: 8 commits ahead, 0 behind
- `crates/osrs-editor/src/app.rs`: +25 / -55
- `crates/osrs-editor/src/app/render_runtime.rs`: +198 / -0
- `crates/osrs-editor/src/app/streaming.rs`: +1 / -1
- `crates/osrs-editor/src/app/viewport.rs`: +1 / -63

No `osrs-render`, `osrs-world`, `osrs-edit`, `osrs-cache`, `osrs-scene`, `osrs-hd`, or `osrs-reference` source file is changed by the implementation checkpoint.

### Validation history

Two intermediate CI runs failed before semantic tests because the extraction deliberately exposed narrow mechanical issues:

- `38430309846`: formatting-only failure in the new runtime module
- `38430582735`: compile failure because the explicit runtime facade initially omitted `hd_shadow_map_bytes()` used by the settings UI

Both were corrected without changing architecture or behavior. The next exact head `89392a6c28eb645a9a787ac8e72d7db946c4813b` passed Tier A/B/C completely.

This handoff bookkeeping commit changes documentation only. Its resulting branch SHA must receive exact-head Tier A/B/C before the Phase 2.1 PR is opened and the checkpoint is reported complete.

## Remaining migration roadmap

### Phase 2: decompose EditorApp state ownership

Extract one coherent runtime at a time:

1. `RenderRuntime` - current Phase 2.1 checkpoint
2. `WorldRuntime` - next, not started
3. `InteractionRuntime`
4. `ProjectRuntime`
5. `UiRuntime`
6. `DiagnosticsRuntime`

Do not combine runtimes into one large refactor. Behavior remains unchanged during pure ownership extraction.

### Phase 3: central edit gateway

- one application gateway for edit intents/patches
- Paint, Height, Object, Path, and context-menu edits converge on one semantic commit-result path
- remove transitional split routing when safe
- keep `MapPatch` and `EditorSession` as semantic authority

### Phase 4: context-menu semantic migration

- migrate `clear_overlay_at`
- prove all semantic map mutations enter through the transaction boundary

### Phase 5: projection/invalidation service

- centralize semantic receipt/change impact -> invalidation
- remove tool-specific dirty-region poking
- revision/generation-check every async install
- add 8x8 projection granularity only after profiling and edge-rule tests justify it

### Phase 6: large UI debt

Refactor allowlisted UI/application modules one responsibility at a time. Do not merely move broad `impl EditorApp` blocks.

### Phase 7: platform ports + WebAssembly gate

- concentrate native/web differences
- add WASM compile gate
- harden browser storage/project abstractions

### Phase 8: cache writing/round-trip guarantees

- encoder guarantee definitions
- semantic and byte-exact roundtrip tests where promised
- untouched archive preservation
- transactional save/repack design

### Phase 9: renderer/projection optimization

- profile first
- introduce finer zone/chunk rebuilds only where measured
- semantic ownership remains unchanged

### Phase 10: continue 117HD parity

Resume on separate focused branches once editor authority/projection boundaries are stable enough to prevent coupling.

## Next boundary after Phase 2.1

The next planned subsystem is **Phase 2.2: WorldRuntime ownership extraction**.

It is NOT started on the current branch.

Do not begin it until:

1. the final Phase 2.1 branch head passes exact Tier A/B/C
2. the Phase 2.1 diff is verified as RenderRuntime ownership work plus handoff documentation only
3. a focused Phase 2.1 PR is opened
4. the user explicitly authorizes its merge
5. the real integrated `main` SHA is read and validated
6. the Phase 2.1 branch is retired if tooling permits
7. the user explicitly says to continue to Phase 2.2

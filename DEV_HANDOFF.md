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

The former `refactor/editor-object-session` branch is retired.

Established:

- pure deterministic Object planner
- live Object actions commit once through `EditorSession`
- Object undo/redo use `EditorSession`
- worker `LocEdit` is projection only
- stale projection routes validate against authoritative `MapDocument`
- Paint, Height, and Object share one chronological semantic-history routing order
- old `app/editing.rs` responsibility was split into focused history/tests modules

The 1,600-line / 64-KiB guard introduced during this checkpoint was an interim hardening step. Phase 1 replaces it with the Architecture Contract v2 ratchet.

### PR #40: Path -> persistent semantic session

PR:
`#40 refactor(editor): migrate terrain path editing to semantic session`

Final pre-merge branch head:
`6a66758bf901a3c8af919dffc1434b84a0735c23`

Exact branch CI:
`38416867837` - Tier A/B/C PASS, including M3-M9.

Regular merge commit on `main`:
`a125f8e237f4a1594fe585df5a0dd7a2e2011da6`

Exact post-merge CI:
`38422182517` - Tier A/B/C PASS, including M3-M9.

The former `refactor/editor-path-session` branch is retired.

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

## Semantic loading and residency

`EditorSession` tracks loaded and dirty regions.

- clean region ingestion may replace semantic source state
- identical ingestion is a no-op
- dirty regions are not overwritten by incoming streaming snapshots
- dirty regions are not semantically evicted by camera unload
- cache-source switch resets the semantic session

Height ingestion stores source-owned 64x64 final height samples per plane. Do not invent shared 65th boundary samples. Neighboring source regions own their boundary data.

Transitional caveat: `WorldDefinitions::region()` can still include legacy worker edits until remaining editing/projection routes are migrated.

## Current active checkpoint: Phase 1 architecture enforcement

Branch:
`architecture/editor-contract-v2`

Validated base `main`:
`a125f8e237f4a1594fe585df5a0dd7a2e2011da6`

Purpose:

- add `ARCHITECTURE.md` as the authoritative Architecture Contract v2
- replace the interim 1,600-line / 64-KiB editor module guard
- enforce 500-line / 32-KiB normal production limits
- enforce 700-line / 40-KiB dedicated-test limits
- freeze existing oversized production modules at exact measured baselines
- make stale/orphan/malformed debt entries fail CI
- self-test the debt ratchet
- reconcile this handoff with the real merged Path state

Explicitly out of scope:

- `EditorApp` runtime extraction
- context-menu semantic migration
- generic projection/invalidation redesign
- renderer refactor
- UI feature decomposition
- platform/WASM refactor
- cache save/repack work
- 117HD work

### Debt measurement checkpoint

Temporary measurement head:
`3400e9704911f664e2cb882144eade44f607a062`

CI run:
`38425580831`

This run intentionally lowered the normal production cap before adding exceptions so CI would report the complete live debt set. It found exactly 13 oversized production modules:

| Module | Baseline lines | Baseline bytes |
| --- | ---: | ---: |
| `app/context_menu.rs` | 647 | 31,661 |
| `app/drawer.rs` | 951 | 37,023 |
| `app/editing.rs` | 792 | 28,638 |
| `app/editing/history.rs` | 641 | 25,633 |
| `app/object_tool.rs` | 550 | 20,164 |
| `app/settings_window.rs` | 1,569 | 51,638 |
| `app/spline_tool.rs` | 580 | 21,409 |
| `app/workspace.rs` | 755 | 28,664 |
| `app/worldmap.rs` | 619 | 23,663 |
| `app.rs` | 738 | 29,602 |
| `paint.rs` | 583 | 17,591 |
| `pick.rs` | 716 | 24,812 |
| `settings.rs` | 592 | 22,909 |

The measured baselines live in `scripts/architecture_debt.json`.

Ratchet rules now enforced by `scripts/check_architecture.py`:

1. A new/non-allowlisted production module fails above 500 lines or 32 KiB.
2. A dedicated test module fails above 700 lines or 40 KiB.
3. An allowlisted module may not exceed either its recorded line or byte baseline.
4. An allowlisted file that falls within the normal cap fails until its stale exception is removed.
5. An allowlist entry with no matching file fails.
6. Malformed debt configuration fails.
7. Existing workspace dependency-boundary rules remain enforced.

Self-tests in `scripts/test_check_architecture.py` cover the normal caps, test caps, debt no-growth, stale exception removal, orphan detection, malformed entries, and existing dependency rules.

Current Phase 1 implementation commits before final handoff bookkeeping:

- `3400e9704911f664e2cb882144eade44f607a062` - measure live module debt
- `b795cacdd435c93519c599ea2fa383af36e0136b` - record measured debt baselines
- `3648bec3e310a16936a3391baa8e70fbc987dea7` - enforce editor debt ratchet
- `79d4bdfed3a1c62aacaf620dfba8c8408780952c` - add debt-ratchet self-tests
- `54fbaee2576eaba3309a5f1595f86df2d5bdef0a` - add Architecture Contract v2
- `0eb135ff55b4bf58a334608fa5802d2136850023` - reconcile handoff and complete pre-PR documentation

### Phase 1 validation and PR

Validated implementation/documentation head before this final handoff bookkeeping commit:
`0eb135ff55b4bf58a334608fa5802d2136850023`

Exact CI:
`38426658112` - Tier A/B/C PASS, including M3-M9 semantic/reference parity.

Focused integration PR:
`#41 architecture(editor): adopt contract v2 and debt ratchet`

The PR diff contains exactly these five files:

- `ARCHITECTURE.md`
- `DEV_HANDOFF.md`
- `scripts/architecture_debt.json`
- `scripts/check_architecture.py`
- `scripts/test_check_architecture.py`

No runtime editor, renderer, world, cache codec, or 117HD production source file is changed by this checkpoint.

This final handoff bookkeeping commit changes documentation only. Its resulting branch SHA must receive exact-head CI before Phase 1 is reported complete. After that validation, PR metadata may be updated with the final SHA and CI run because PR metadata does not alter the branch commit.

## Remaining migration roadmap

After Phase 1 is merged and exact integrated `main` is green, continue only after explicit user direction.

### Phase 2: decompose EditorApp state ownership

Extract one coherent runtime at a time, in this order unless live evidence justifies a deliberate architecture checkpoint change:

1. `RenderRuntime`
2. `WorldRuntime`
3. `InteractionRuntime`
4. `ProjectRuntime`
5. `UiRuntime`
6. `DiagnosticsRuntime`

Do not combine these into one large refactor. Behavior should remain unchanged during pure ownership extraction.

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

## Next boundary after Phase 1

The next planned subsystem is **Phase 2.1: RenderRuntime ownership extraction**.

It is NOT started on the current branch.

Do not begin it until:

1. Phase 1 exact branch head passes required CI.
2. Phase 1 diff is verified architecture/docs/tests only.
3. A focused Phase 1 PR is opened.
4. The user explicitly authorizes the merge.
5. The real integrated `main` SHA is read and validated.
6. The Phase 1 branch is retired if tooling permits.
7. The user explicitly says to continue to the next checkpoint.

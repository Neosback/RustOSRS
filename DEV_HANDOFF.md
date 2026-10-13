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

## Completed: Phase 2.1 RenderRuntime ownership extraction

Merged as PR #42 (`adb80da2`). `RenderRuntime` (`app/render_runtime.rs`) owns the eframe WGPU render state, the sole `SceneRenderer`, viewport targets, HD GPU install, and frame submission. No behavior change.

## Current checkpoint: Phase 2.2 WorldRuntime ownership extraction

Branch: `refactor/editor-world-runtime` (cut from `main` `adb80da2`).

`WorldRuntime` (`app/world_runtime.rs`) now owns the disposable world-side projection state that previously lived directly on `EditorApp`:

- the `RegionStreamer` and `AnimationWorker`
- presentation generation / HD flag (stale-build rejection via `is_current_generation`)
- resident (`loaded`), `in_flight`, and edit-`dirty` region sets, with the stream-window policy (`wanted_regions`, `stale_regions`) as pure, unit-tested functions
- the animation schedule (smoothing, frozen, cycle, centre, dirty)
- worker-readiness flag
- mirroring of tile/loc edits to the worker (`mirror_tile_edits`, `mirror_loc_edits`) - still projection only; `EditorSession` stays authoritative

`EditorApp` keeps orchestration of stream events (`app/streaming.rs`): it asks `WorldRuntime` for bookkeeping and applies the render/inspection/semantic-session consequences itself. The write-only `last_build_ms` field was deleted.

Validation (local; no CI access from this environment): `cargo fmt`, `cargo clippy --workspace --all-targets --all-features -D warnings`, `cargo test -p osrs-editor`, architecture guard, and a 25 s release smoke run against the pinned b241 cache (9 regions streamed, animation upload live, ~60 fps).

Debt baselines tightened: `app.rs` 738 -> 665 lines, `app/editing.rs` 792 -> 670, `app/editing/history.rs` 641 -> 632.

### Phase 4 closed: context-menu overlay clear is semantic

`clear_overlay_at` previously fell through `end_stroke` to the legacy `push_step(stroke.step)` path, so it was the last terrain mutation outside `EditorSession`. It now arms `overlay_clear_brush()` (overlay channel `Fill::Clear`, nothing else) and commits through the same Paint planner as every other stroke. The legacy fallback in `end_stroke` is removed: a stroke without a Paint brush or Height spec is reverted with a notice (and trips a `debug_assert!`), so the invariant "every terrain mutation enters through `EditorSession`" is enforced in code, not just by convention. Pure Paint/Height planning and projection helpers moved to `app/editing/plan.rs`. Regression tests cover the clear commit/undo and the no-op case.

### Phase 6: architecture debt retired

Every file on the debt allowlist is now within the 500-line / 32 KiB cap and `scripts/architecture_debt.json` is empty (the guard still rejects any growth or new exception). Pure moves, no behavior change, verified by tests, the architecture guard, and release smoke runs/screenshots:

- `settings_window.rs` (1569) -> `settings_window/{mod,catalog,rows,row_widgets,pages,tests}.rs`
- `drawer.rs` (951) -> `drawer/{mod,palettes,channels,tabs}.rs`
- `workspace.rs` (755) -> `workspace/{mod,dock,tools}.rs`
- `context_menu.rs` (647) -> `context_menu/{mod,editor_menu,game_menu,effects}.rs`
- `worldmap.rs`, `spline_tool.rs`, `object_tool.rs` -> `canvas`/`key`, `input`/`overlay`, `drawer` submodules
- `editing/history.rs` -> `history/routing.rs` (undo/redo); `editing.rs` -> `commit.rs` + `actions.rs`
- `app.rs` -> `init.rs` (`EditorApp::new`) + `input.rs`
- inline test modules of `settings.rs`, `paint.rs`, `pick.rs` -> `*/tests.rs`; `pick/describe.rs`

Known remaining smell (move-only, not yet fixed): `routing.rs` holds ~190-line `undo` and `redo`, `commit.rs::end_stroke` ~175 lines, `row_widgets.rs::draw_row` ~365 lines. These exceed the 100-line function guideline and are the next real refactor targets. Submodules use `use super::*;`, matching the existing `app/streaming.rs` style.

### Phase 2.3: InteractionRuntime (field ownership)

`InteractionRuntime` (`app/interaction_runtime.rs`) now owns camera, view plane, hover/context picks, selection (objects, tiles, pending reselect), marquee, spline state, tool peek, object drag, and gizmo state. It was first tried before the size debt was retired and rejected because the ~300 call-site renames pushed frozen files over their baselines; with the debt gone it landed as a field-ownership move only. It is still a plain-fields struct: encapsulating selection behind methods is the follow-up that would make it more than a rename. `EditorApp` is down to 58 direct fields (target: roughly 8-12 subsystem fields).

### Phase 8 started: map-stream encoders (`osrs-cache::encode`)

`encode_terrain` / `encode_locations` are strict inverses of `decode_terrain` / `decode_locations`, with typed `MapEncodeError`s for unrepresentable values (no silent truncation). Measured against every decodable square of the pinned build-241 cache with `cargo run --release -p osrs-reference --example map_encode_roundtrip -- $RUSTOSRS_TARGET_CACHE_DIR`:

- locations: 2936 / 2936 byte-exact
- terrain: 2935 / 2936 byte-exact; the one exception carries the client-discarded second tile block (flags bit 0), which the decoder does not retain, so the encoder cannot reproduce it
- 1 square is undecodable without XTEA keys (the profile has none)
- every re-encoded terrain stream decodes back to identical tiles

Canonical form: per tile `overlay, settings, underlay, height`, then a trailing flags byte `0` (build-241 files always end with it). Map groups hold five files (0 terrain, 1 locations, 2..=4 other); `pack_group`/`split_group`/`encode_container`/`decode_container` reproduce them, and repacking all five files of every real group is byte-exact (2936 / 2936). Every real map container uses gzip.

### Phase 8: export of edited regions (File > Export edited regions...)

- `session::export::export_region(original terrain, original locations, MapDocument)` diffs the authoritative document against the semantic baseline of the same decoded square and patches only differences into the original tiles/locations: channels (underlay, overlay id+shape+rotation, flags; raw id bit 15 preserved when the semantic id is unchanged), heights (re-stored plane by plane because explicit heights are relative to the plane below; unstorable heights are an error, never silently rounded), and objects. It returns `None` for an unedited square (untouched archives are never rewritten) and `RegionNotLoaded` when the document holds no state for the square (instead of reading as a mass deletion).
- `osrs-world::SourceSquares` reads pristine squares (no worker edit layer) plus all group files.
- `app/export.rs` runs the export on a worker thread over a document snapshot and writes `5/<group>.dat` (gzip container of the original group with files 0 and 1 replaced, files 2..=4 preserved) and `manifest.json` (profile, cache fingerprint, per-region change counts). The source cache is never modified. Native only; the menu entry is a no-op on wasm.
- Validation: unit tests plus real-cache tests (skipped without `RUSTOSRS_TARGET_CACHE_DIR`): every sampled real square exports as `None` when unedited, real edits survive export and reload, and the full file-writing path decodes back to the edit with files 2..=4 byte-identical.

Not done: writing into `.dat2`/`.idx` and updating the reference table (a loadable repacked cache), project save/load, version/CRC bookkeeping in exported containers. Placement note: `session::export` lives in `osrs-editor` because it needs `adapt_region_map`; move it down with that adapter if another consumer appears.

## Remaining migration roadmap

### Phase 2: decompose EditorApp state ownership

Extract one coherent runtime at a time:

1. `RenderRuntime` - done (PR #42)
2. `WorldRuntime` - done (this branch)
3. `InteractionRuntime` - done (field ownership; behavior encapsulation pending)
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

## Next boundary

Next candidates: encapsulate selection behind `InteractionRuntime` methods, `ProjectRuntime`/`UiRuntime` field extractions, break up the long functions listed above, Phase 3 central edit gateway, Phase 5 projection service, Phase 7 WASM gate.

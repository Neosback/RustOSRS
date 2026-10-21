# Editor feature map

The executable navigation registry is [`scripts/editor_features.json`](../../scripts/editor_features.json). The architecture guard verifies every reference, requires the four behavior-test references for each mutating tool, and compares feature entries with the actual `Tool` enum. This page explains the flow; the registry is the precise function/test index.

```text
UI/input → controller intent → pure MapQuery planner → MapPatch
         → EditRuntime execute/replay → EditReceipt → world invalidation → renderer
```

| Feature | Controller | Headless planner | Gateway / projection |
|---|---|---|---|
| Select | `app/interaction_runtime.rs`, `app/selection.rs` | No semantic mutation | Selection/marquee transitions |
| Paint | `app/editing.rs` | `osrs-edit/src/paint.rs` | `app/editing/commit.rs` |
| Height | `app/editing.rs` | `osrs-edit/src/height/commit.rs` | `app/editing/commit.rs` |
| Spline / Path | `app/spline_tool/input.rs`, `app/interaction_runtime/spline.rs` (fit in `terrain_path/fit.rs`) | `osrs-edit/src/path.rs`, `osrs-edit/src/path_height.rs` | `app/editing/path.rs` |
| Objects | `app/object_tool.rs`, `app/gizmo.rs` | `osrs-edit/src/object_edit.rs` | `app/object_tool/commit.rs` |
| NPC scenarios (rail tab, not a `Tool`) | `app/npc_runtime.rs`, `app/npc_panel/` | `osrs-edit/src/scenario.rs`, `osrs-playtest` | `EditRuntime::execute_scenario`, chronological `WorkspaceHistory` |
| Appearance (View tab, not a `Tool`) | `app/theme_studio/`, UI drafts in `UiRuntime` | `osrs-hd::theme::ThemePatch` | `EditRuntime::execute_appearance`, same generic chronological history |
| Prefabs (rail tab, not a `Tool`) | `app/prefab_panel/` (library, placement gesture and ghost), `app/prefab_runtime.rs` (library and files) | `osrs-edit/src/prefab/` | `app/editing/prefab_commit.rs`, composite history route (terrain keys plus object route) |
| Buildings (Prefabs panel Build page) | `app/prefab_panel/build.rs`, `app/prefab_runtime/build.rs` | `osrs-edit` `building_*` generator through `BuildingKit` (`building_kit.rs`) | same prefab commit and composite route |
| Building capture | `app/prefab_panel/capture/` and `capture.rs` | `osrs-scene::building_census::description`, `osrs-edit::capture_building_prefab` | explicit room/plane/piece review, source/revision-stamped acceptance |
| Prefab Studio | `app/prefab_studio/`, `workspace.rs` (library / Builder / WFC) | `capture_prefab_over`, `plan_place_exact` | synthetic map source, isolated baseline then normal tool history; sparse save |
| WFC prefab floors | `app/prefab_studio/catalog.rs` | `osrs-edit::plan_wfc_floors` | authored scene gateway, undo and sparse save; never the Lab mirror |
| Procedural Lab | `app/prefab_studio/lab/`, `osrs-lab` JSON-lines | existing edit planners, WFC, foundation fitting | parked author runtime; Lab history; read-only scene projection |
| Workshop = the Studio window with tabs Scene / Names / Train / Paths / Floor patterns / Practice room (not a `Tool`) | `app/prefab_studio/` (window, header tabs, Scene and Practice room), `app/workshop.rs`, `workshop/{strip,buildings,teach,table}.rs`, `workshop/train/` (job thread, model list, Try it, `live.rs` trained styles on the Build page) | none: library, name book and model files only; objects named through `osrs-edit::name_book` (`osrs-core::object_names`); training and building in `osrs-lab` (`train_model`, `build_with_model`) | no document edit; "Place" and "Open in scene editor" reuse the Prefabs commit and Studio routes; deletes move files to `.trash` |
| Paths tab (Workshop) | `app/workshop/paths*` (areas, teach, try, lay), `osrs-lab::paths` | `osrs-lab` path model, router and checks over a `PathLesson` | laying on the map reuses `EditorApp::apply_terrain_path` (one undoable path transaction); lessons and models are files only |
| WFC lens (Workshop tab) | `app/workshop/wfc.rs` | `osrs-edit::wfc_lens` (`look`, `refloor_task`, `sample`, `trace`), `solve_wfc_observed` | no document edit (view and test only) |
| Object names | `app/names.rs`, `app/names/custom.rs`, `app/inspector/naming.rs` | `osrs-core::object_names` (precedence), `osrs-scene::building_census::guess` (suggestions) | settings-adjacent `names/objects.names.json`, written atomically; no map change |
| Tile paste | `app/editing/paste.rs`, `app/tile_clipboard.rs` | Paint planner | Existing Path history projection |

Application paths are relative to `crates/osrs-editor/src`. `osrs-edit` paths are relative to `crates`.

The Spline gesture freezes available semantic region membership, origin and map dimensions at
its first point. Production fitting uses the source Java integer-map iteration order. Committed
heights retain single-unit precision through semantic history, projects and absolute worker
projections; target export is the explicit lossy boundary. Laying the overlay and optional height
smoothing are two source-defined operations and two undo steps. See
[the tile/path study](../research/TILE-APPEARANCE-AND-PATHS.md) and the pinned oracle under
`scripts/path_oracle/` for the source contract and differential cases.

## Runtime ownership

| Owner | State and behavior | Must not own |
|---|---|---|
| `EditRuntime` | Semantic session, transaction checkpoint/finalize/rollback, edit receipts | UI widgets or GPU resources |
| `InteractionRuntime` | Camera, selection, hover, tile clipboard, marquee/tool/object/gizmo gestures, spline preview worker | Committed map mutations |
| `ProjectRuntime` | Source identity/profile/error, pending diffs, saved revision, native save and replacement policy | Authored map values |
| `WorldRuntime` | Streaming, rebuild scheduling, dependency proofs, CPU picking/outline projections | Semantic history |
| `RenderRuntime` | GPU resources, upload and viewport submission | Editable document state |
| `UiRuntime` | Windows, searches, transient notices, modal policy; Script UI returns an action | Domain mutation |
| `DiagnosticsRuntime` | Timing/smoke hooks and bounded observations | Commit/worker acceptance decisions |
| `SettingsRuntime` | Preferences, keymap and persistence cadence | Tool gestures |
| `PluginRuntime` | Built-in plugin callbacks and overlay contributions | Mutable application/document access |
| `PaletteRuntime` | Floor palette job, texture previews and their lifetime | Terrain edits |
| `NpcRuntime` | NPC catalog, immutable assets, playtest simulation, identity-stamped acceptance of worker results, entity-layer geometry requests | Authored scenario state (lives in `EditorSession`), GPU resources, terrain |

Existing feature controllers (`PlaceState`, export, script, HD and asset services) remain dedicated owners. `EditorApp` coordinates these through explicit fields; it is not a mutable service locator.

## Temporary adapter: TerrainController

`TerrainController` owns the existing terrain preview map, stored-height override markers, active brush stroke and stroke seed/counter. It owns no completed chronology. Canonical committed before/after data lives exclusively in `EditorSession`.

Its callers are the terrain/clipboard/history application adapters. Other features use lifecycle methods such as cancel/reset. Switching tools cancels and restores the active preview; it does not commit a second history entry or consume redo.

Session history already supplies semantic replay footprints, and descriptor-driven replay has removed the routing stacks. Remove the remaining preview/projection adapter when workers consume semantic snapshots directly and source-height overrides have a proven replacement. Until then, preserve exact preview cancellation and source-height override behavior; never use it as planner authority.

The small `app/paint_commit.rs`, `app/path_commit.rs`, and editor `heights.rs` modules retain existing application import paths while translating UI DTOs or re-exporting canonical domain types. They contain no duplicate semantic planner. Their removal condition is migration of remaining application imports to the canonical types; wire/project formats are unchanged.

## Debugging edits and rebuilds

The tile inspector's **Collision and blockers** section reads immutable contextual snapshots from
accepted world builds. It explains walking/projectile wall bits, footprint/floor blockers and their
original object/terrain source planes, including neighbouring wall anchors and invisible placements.
Dirty/pending or obsolete producer epochs show an updating state. Missing land/definitions are
explicit; this is not a full route/reach or actor-size test. Ownership and oracle commands:
[`docs/specs/collision.md`](../specs/collision.md). The scene builder retains clipping independently
of renderable admission; the UI never derives it from picker triangles or edits a collision grid.

The tile inspector's **Edit and rebuild trace** shows retained source-plane edit observations, semantic revision transitions, invalidation reasons and FIFO build identities/outcomes. The 256-event ring is enabled in debug builds; `RUSTOSRS_TRACE_EDITS=1` enables it explicitly and `=0` disables it. Release builds default to disabled and event construction is lazy. Source replacement clears retained observations.

Build observations include source generation, request ID, semantic revision at scheduling, projection epoch and presentation generation. “Ticket accepted” records the FIFO freshness decision, not a claim that GPU presentation has completed. History-free source hydration and project-diff application are not interactive tool transactions and do not produce an interactive gesture event. Object tile observations describe placement anchors; invalidation events describe affected producer regions.

## Projection lifecycle contracts

[Active projection contracts](PROJECTION-CONTRACTS.md) inventory the scene/worker, render, picker,
collision, proof, animation, actor and preview lifetimes. Visual caches may retain the last accepted
picture while rebuilding; navigation may not trust stale collision evidence. Dirty work remains
required until its current completion, and membership changes discard old in-flight animation output.
Actor completions retire only their issued token/stamp and publish after the frame reconciles current
settings and revisions. Hide revokes publication while the physical FIFO request drains.
Current Empty/Failed region results use the same whole-region cleanup as semantic rejection, with
unavailable residency suppressing retries. They preserve semantic documents and pending project work;
stale terminal results cannot remove newer projections or overwrite notices.
See the inventory for source identities, failure policies, regression tests and unresolved boundaries.

## Adding a feature

Follow an existing tool's registered flow. Keep the planner deterministic and headless, preview state transient, cancellation reversible, and a completed gesture a single gateway transaction. Test atomic failure, no-op/redo preservation and exact undo/redo. Reuse the generic receipt/invalidation flow.

Extend the registry with real tests and run its guard. New behavior belongs on its controller/runtime; the application method ratchet prevents casually adding another broad `impl EditorApp`. A deliberate interface change must be reviewed alongside its owning architecture decision, not hidden in a baseline update.

Registry entries default to `kind: tool`, which must match the `Tool` enum exactly. A `kind: workspace`
entry registers an authored feature outside that enum and still requires the same planner, gateway
and four real behavior tests. It cannot relabel an existing tool to evade completeness checks.

Authored appearance is owned by `EditorSession`; `HdRuntime` retains only resolved lighting and
environment transition state. The current renderer accepts global/area environment rules and WGSL
hooks; image-material/terrain/object selectors are rejected until their projections are implemented.
See `docs/plans/THEME-STUDIO.md` for the explicit incomplete boundary.

Building relocation defaults to median support fitting with a two-tile blend. Missing support,
stepped source foundations and unverified bridge/overlay sites fail explicitly. Existing structures
cannot be changed through height fitting without a reviewed removal. The replacement modal lists
the exact removals and rejects stale map/source stamps. Rectangle capture remains a separate flow.

Studio relationship overlays project persisted masks, wall arms, outside anchors, door approaches
and inferred links; their explanation panel exposes evidence. Object edits invalidate inferred
occurrence links; human labels survive. Metadata edits participate in dirty navigation.
See [local training commands](../../scripts/lab/README.md) for source-stamped exports, integer role
models, observed WFC catalogs, held-out groups and the distinction between curriculum and evaluation.

The Builder / Prefab Studio entry and library double-click open a separate native workspace window.
Map placement remains an explicit button. The stage queues `use_virtual_map` before any region
requests. `WorldDefinitions` supplies flat four-plane source maps with no source locations, while
models, textures and definitions still come from the selected cache. Normal edits replay over this
immutable baseline through the existing FIFO worker and transactional gateway; the stage floor is
excluded from saved prefab content. No real cache map square is used, including contextual reads.

Building capture defaults to a reviewed sparse footprint. Its role filters independently control
furnishings, decoration, roofs and stairs; boundary anchors never expand terrain ownership. Legacy
rectangular capture remains available by unchecking the outside-ground exclusion. Room and plane
membership still require review rather than treating a bounding rectangle as building ownership.

The detached window uses `prefab_studio/window.rs`; the world retains its own frame and input.
`PrefabRuntime::replace_if_unchanged` protects a newer library edit from a stale detached save.
`osrs-lab::reconstruction_task` constructs the saved-building curriculum with resolved dimensions
and geographic/family grouping. `lab/runner.rs` runs the existing Python demonstration baseline,
writes local task/actions/metrics artifacts and checks tool replay before exposing it to the UI.
`script/lab.rs` restricts automation to host controls while the Lab owns edits; normal editor
commands cannot write the Lab projection. Teacher data and fitted-policy claims stay distinct.

## Authored snapshots

[Authored snapshots](../specs/authored-snapshot.md) are a separate registered read-only operation on
EditorSession's existing map/scenario/appearance composition. File → Save authored snapshot and
`project snapshot PATH` freeze one bounded native export on ProjectRuntime. The headless
`--verify-authored-snapshot FILE` checker validates all domain identities and a canonical round trip
without installing into a live session. Current content is captured; source, availability, pending
project restoration, history and clocks remain separate envelope inputs.

## Building semantic reproduction

The registered `buildings` operation also owns explicit frozen reproduction in
[prefab_runtime/build/reproduction.rs](../../crates/osrs-editor/src/app/prefab_runtime/build/reproduction.rs).
The native Build panel and `prefab reproduce PATH [failure=SEED]` capture an admitted candidate or
current rejected attempt; a native
`--reproduce-building FILE` branch runs the existing planner and EditRuntime before creating any UI.
[The contract](../specs/building-reproduction.md) records full forms, kit, canonical footprint facts,
signed map snapshot, source/build context and expected digests. This is semantic verification of
successful candidates. [Rejected-attempt reproduction](../specs/building-failure-reproduction.md)
uses a separate closed failure format and proves exact rejection with an unchanged map and zero
operations. Each failed seed can be captured without replacing the selected success. Neither
contract grants raw-patch automation, render reproduction or crash recovery.

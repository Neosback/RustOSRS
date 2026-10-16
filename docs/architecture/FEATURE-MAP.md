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

`TerrainController` owns the existing terrain preview map, stored-height override markers, active brush stroke and chronological history **routing metadata**. Canonical committed before/after data lives exclusively in `EditorSession`.

Its callers are the terrain/clipboard/history application adapters. Other features use lifecycle methods such as cancel/reset. Switching tools cancels and restores the active preview; it does not commit a second history entry or consume redo.

Remove the adapter when world workers consume semantic snapshots directly and session history supplies sufficient projection footprints for every replay. Until then, preserve its exact source-height override and history-routing behavior; never use it as planner authority.

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

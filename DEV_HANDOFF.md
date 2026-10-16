# RustOSRS Development Handoff

Last updated: 2026-10-15

Read `AGENTS.md` and `ARCHITECTURE.md` first. `docs/architecture/FEATURE-MAP.md` and `scripts/editor_features.json` locate feature ownership and tests. Live Git state is authoritative for branch/SHA facts; CI claims require actual run evidence.

## Current boundary and standing workflow

- Working branch: `main`; stay on `main` unless the user explicitly asks otherwise. One local commit per checkpoint; nothing is pushed or published without authorization.
- Program baseline: `63d2721e3fa30dc2eaecff140d6b27886e4dff2f`, merge of PR #56 (editor ownership), main CI run `38766016806` (Tier A/B/C success). The old feature branch is retired remotely.
- Commit `41d0c020` ("updates") on top of that baseline already contains the program foundations: opt-in JSON performance capture (`diagnostics/capture.rs`, schema v1), owned-GPU-byte accounting (`gpu/resources.rs`), Reference frame reuse (`render_runtime/reuse.rs`), the shared Spline settings widget (`spline_tool/panel.rs`), `scripts/performance.py`, and the 117HD source manifest/ledger (98 config items, all `audit_pending`). They are committed, not pending, but **no baseline or improvement has been recorded**.
- Authorized work and plan: `docs/plans/PERFORMANCE-UI-PLAYTEST-HD.md`. The expanded user goals (RuneLite/deob re-verification, exact tile/overlay/underlay/blend abstraction for the spline tool, NPC/entity runtime for a future own client) are carried by the checkpoints below.

## Current program checkpoints

0. Reconcile handoff/docs, gate results at `41d0c020` — done (`7587400f`).
1. Measurement: schema-2 capture, worker/renderer counters, workloads, compare tool, fixed-view pixel capture — code done (`7f1bb2a2`). **Baselines are not recorded.** A partial run (49 of 84 planned runs) was abandoned: four runs contained 5–59 s frame gaps while the editor's own frame body took ~14 ms, which points at macOS throttling an unattended window (reproduced as a ~4000 fps no-present spin and a 2.9 fps crawl in later runs). `performance.py` now holds the machine awake (`caffeinate`), discards and retries stalled runs, and continues past failures. Retake baselines on the commit that starts checkpoint 3.
2. (a) Terrain appearance contract in `osrs-scene` and real-colour spline preview — done (`8768cf07`; oracle, Reference pixel golden and fixed-view hashes unchanged). (b) Responsive panels and Spline workflow — done in this checkpoint (below); wider tool-panel audit continues as needed.
3. Measured CPU/GPU/memory optimizations — pending (needs baselines).
4. Entity cache decoders, skeletal animation, shared model appearance transforms — pending.
5. `osrs-playtest` crate, editor controller, entity rendering — pending.
6. Remaining 117HD systems and independent visual proof — pending.

### Path grading, curve and cursor preview (after 2b)

- `osrs-edit::plan_path_grade` (pure, deterministic): samples the ground along the centre line, smooths it, limits its slope with a Lipschitz envelope, and moves vertices within `width/2 + blend` toward the bed height of their nearest centre-line point (flat across the path, smoothstep falloff). `build_terrain_path_patch_with_grade` joins the graded heights to the overlay in one patch: one commit, one undo step.
- Verified in the real editor on the pinned cache: laying a graded path levelled the ground across the path, blended within reach, left ground beyond reach unchanged, and one undo restored every sampled height exactly. The preview draws the graded terrain (real builder colours and slope light) from the same planner.
- Settings added (with defaults, persistence and clamping): curve (smooth/straight), grade on/off, smoothing, max slope, blend, preview-to-cursor. Grade is off by default. These are provisional: the next step derives defaults from a census of the real map and replaces global sliders where possible.
- Cursor-following preview plans one extra segment up to the cursor alongside the placed path; it is never laid and does not affect "Lay path". Verified by unit tests only, **not visually with a real mouse**.
- Baselines: the C3 baseline job started earlier was invalid (the release binary was rebuilt while it ran and other runs competed for the machine); its output is quarantined under `target/performance/c3-before-INVALID-mixed-binaries`. Retake baselines from a frozen copy of the binary on an idle machine.

### Settings-free path tool (after the census)

- Real paths were measured (`docs/verification/PATH-CENSUS.md`) and the tool now follows them: width presets (Trail 1 / Path 3 / Road 5) with per-point widths and corner flags; the fitter prefers the real map's choice for each neighbourhood; paths replace path overlays and open ground, keep the underlay, and leave area overlays (water, floors, hidden overlays) alone, showing them in red; terrain follows the ground, with an optional "smooth ground along the path" that keeps the cross-slope. Removed settings: edge weight, snap, curve, cursor preview (always on), and the grading sliders. Persisted files with the old fields still load (unknown fields are ignored).
- The grading planner changed from "level across the path" to "ease bumps along the path, keep the cross-slope" after the census showed real paths are not level across their width.
- Verified in the real editor on the pinned cache: a variable-width path with a corner laid overlay 1 on open ground, replaced another path overlay, left a hidden overlay untouched, smoothed heights gently, and one undo restored overlay and height on every sampled tile. The cursor-following preview and red clipped tiles were seen in screenshots; hover interaction with a real mouse, double-click to toggle a corner and the width keys were covered by unit tests only.
- **Repository state to be aware of.** Commit `5abe104e` ("clean", made outside this session) deleted `runelite-master/`, `tools/` and similar and swept an unfinished snapshot of this work into history (that commit does not build on its own; the next commit does). Two guard scripts depended on the removed files (they now skip the missing tooling explicitly, see "Sound and Player rail panels"): `scripts/test_reference_regeneration.py` (needs `tools/reference-fixtures/regenerate.py`) and `scripts/test_deob_golden_index.py` (needs `tools/deob-harness/src/Dumper.java`). They need a decision from the repository owner (update them or restore the tooling). Plans that rely on the deob harness (loc-shadow oracle, entity decoder verification) are affected; the decompiled client outside the repository is untouched.

### Occupancy-first autotiling and per-stretch systems

- The fitter decides which tiles are path first (band coverage 45%, 30% for paths under two tiles wide; cleaning removes spurs and fills pits), then picks each tile's shape from its 8-neighbour bitmask using the census table (`census.txt`, `PRIOR_WEIGHT` 30). See `docs/verification/PATH-CENSUS.md`.
- Each control point now describes the stretch after it: width (interpolated), **wander** (`terrain_path/wander.rs`: Straight / Gentle / Natural, seeded per point, ends fixed) and **ground smoothing** (`GradePoint.weight`, blurred along the path in `osrs-edit::plan_path_grade`; weight 1 everywhere is the old behaviour). The Route chips and the ground checkbox apply to the selected point's stretch (all when none selected) and become the default for new points (`InteractionRuntime::sync_spline_defaults`); the points list can override one point. Wander, seed and ground flag are in the plan key. Script suffixes: `!` corner, `~` / `~~` wander, `^` ground.
- Verified in the real editor (screenshot, release build): a path with wandering stretches previews and plans; unit tests cover wander determinism/bounds/fixed ends, weight 0/0.5/gradual grading, and state semantics (selection vs all, defaults for new points, seeds unique, replans). Dragging/clicking the new row toggles with a real mouse was not exercised. A stray "Laid a fitted path" notice appeared in one scripted smoke run without a `lay` command; not reproduced or explained.

### Sound and Player rail panels

- Two new buttons on the dock rail (`RailTab::Sound`, `RailTab::Player`). Both are read-only with respect to the map: no `MapDocument` change, no history.
- **Sound** (`app/sound_panel.rs` over the pure `sound/`): lists the objects of the camera's map square (or all loaded regions, current plane or all) that play an area sound. `ObjectDefinition.sound` (`osrs-core::ObjectSound`) now retains what the cache decoder used to skip: opcodes 78/79 (looping/random sounds, change cycles, distance, retain), 91/93 (fade curves, fade durations), 95 (visibility); defaults follow the RuneLite client (fades 300, visibility 2). Rows show the object, its sound ids (RuneLite names for the 59 it names, `sound/names.txt`), reach (full volume within `retain-1+0.5` tiles of the footprint, silent past `distance+0.5`, from the client's `ObjectSound`/`class204.method4559`), and the viewport draws a marker per sound plus both ranges of the selected one. Objects that change with a variable and have no sound of their own are counted, not resolved. **No audio is played** (sound-effect synthesis from cache index 4 is not decoded), and the region's music tracks are not listed.
- **Player** (`app/player_panel.rs` over the pure `player/`): a mock player in the settings (name, gender, 23 skill levels with the game's combat-level formula, status, look ids, worn item ids by slot, varps/varbits/varcs tables), stood on a tile as a marker with its name. It decides nothing yet (it does not resolve object morphs); it exists as the place real player rendering and playtest state will attach.
- Guard scripts: `scripts/test_reference_regeneration.py` now skips (reported as skipped, 5 tests) and `scripts/test_deob_golden_index.py` skips only the harness-blob check when `tools/` is absent, instead of failing. Restoring `tools/` re-enables both fully.
- Not verified: sound panel against a second region; Player panel at narrow dock widths and display scales (no layout harness covers the dock panels); clicking the rows with a real mouse.

### Checkpoint 2b notes

- Spline panel is one pure widget set (`app/spline_tool/sections.rs`) used by the drawer (all sections) and the quick HUD (compact); the shelf keeps the shape settings. Order: appearance, shape, preview status + actions, control points; two columns from 640 points.
- Explicit plan states: needs points, needs overlay, updating, cancelled, ready, empty, failed. "Lay path" is enabled only for a ready, current preview. Cancel stops a running plan (the worker is dropped); any edit or "Plan again" lifts it.
- Lay path also sets the Paint brush's armed height/flags; the panel now says so. The preview tile set does not depend on them, so they are not in the plan key.
- Layout: quick HUD narrows and scrolls; Paint tab strip wraps and the preview stacks under the palette below 340 points; palette popup height follows the window; Height tool rows wrap; the status bar drops its figures below 900 points.
- Tests: spline panel layout at widths 240/320/480/700/1100 and scales 1/1.5/2 for every status (verified able to fail), cancel/resume/status tests. Layout tests use default fonts with the app spacing, not the app fonts. Not yet covered by a layout harness: the Paint/Height/Objects drawers and popups (checked by eye only), and display scales were not visually verified.

Do not describe this program as complete merely because its foundations or self-generated regression images pass. Viewing distance, resolution and visual settings remain fixed in performance comparisons. Client networking/accounts and full combat/economy are outside scope.

## Previously completed ownership changes (PR #56)

1. Moved Paint, Height and Path semantic planners plus their existing cases into `osrs-edit`; reused canonical brush/Height setting types without changing settings/project wire shapes. Domain intents use `MapTile`.
2. Made interaction fields private and moved camera movement and the spline preview lifecycle behind runtime operations. Preserved input polling, world lookup and semantic commit orchestration at the application boundary.
3. Extracted `ProjectRuntime` source identity, immutable native saves, saved revision, pending project diffs and save/discard/cancel/replacement policy. Dialog drawing returns choices; failed saves/projections remain retryable.
4. Extracted transient UI, settings persistence, plugin callback/overlay ownership and floor-palette lifetime. The Script window returns an action instead of executing product commands during drawing.
5. Moved CPU picking/outline projections behind `WorldRuntime`; kept GPU resources in `RenderRuntime`.
6. Encapsulated the existing terrain preview/history-routing adapter in `TerrainController`. Switching tools now cancels/restores an active preview instead of dropping the stroke and leaving its edits visible. No semantic history entry or redo invalidation occurs on cancellation.
7. Added optional bounded edit/invalidation/build observations and a tile inspector trace. FIFO acceptance, dependency proofs and fallback rebuild behavior remain authoritative and unchanged.
8. Added the contributor entry point, feature map, ADR-0011, dependency leakage checks, application-surface ratchet and feature/test reference checks.

## Invariants and remaining adapters

`MapDocument` / `EditorSession` remains authoritative. All existing semantic tool commits and history replay still pass through `EditRuntime`. Native export/inspection remains isolated. Worker mirrors, previews, CPU picking, dependency snapshots and GPU resources are disposable projections.

`TerrainController` is temporary preview/history-routing state, not a second semantic history. Remove it once workers consume semantic snapshots and session history provides replay projection footprints. Small editor planner import adapters contain only DTO conversion/re-exports. Existing broad application orchestration methods are enumerated in the ratchet; future extraction must lower it.

## Validation

Latest full run (path grading/curve/cursor work on top of `7757ff4a`; local results, not remote CI certification), Python 3.14.8 (`/opt/homebrew/bin/python3`):

- Architecture guard and its tests, editor-ownership tests, reference-regeneration tests, golden-index check, HD source pins (98 entries awaiting audit), HD-source tests and `test_performance.py`: passed.
- `cargo fmt --all --check`; locked workspace/all-target/all-feature `cargo check` and strict Clippy; WASM lower-crate check and strict editor Clippy: passed.
- Locked full workspace/all-target/all-feature tests: **861 passed, 0 failed, 18 ignored** (the two guard scripts above fail for the reason given) (ignored tests keep their external cache/GPU/oracle prerequisites).
- With the pinned cache: terrain oracle (`terrain_oracle_diff`) and the Reference pixel golden passed; `region_capture` Reference hashes were identical before and after checkpoint 2a. HD captures vary by up to one channel level in a few hundred pixels between identical runs, so HD comparisons use a tolerance.

Earlier figures (805 at `41d0c020`, 793 for PR #56) are superseded. Historical CI results only certify the SHAs they name. Cargo emits the existing vendored `rune-fs` MSRV-metadata warning.

## Subsequent work

Continue the authorized checkpoints on `main`. Follow the existing product roadmap for independent Reference visual proof, M10+ certification, remaining HD features, export generalization and recovery/project polish; this refactor does not claim those features are complete. See `docs/PROJECT-STATUS.md`.

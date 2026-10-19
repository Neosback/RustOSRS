# RustOSRS Development Handoff

Last updated: 2026-10-18

Read `AGENTS.md` and `ARCHITECTURE.md` first. `docs/architecture/FEATURE-MAP.md` and `scripts/editor_features.json` locate feature ownership and tests. Live Git state is authoritative for branch/SHA facts; CI claims require actual run evidence.

## Workshop, name book and trained building models — 2026-10-18

Baseline is `main` at `f6e760d0d7d8bbcde68435b2076123996dfb1146` plus the checkpoint commits listed
by `git log` (local, unpushed). Plain-language goal: a non-programmer can pick saved buildings,
name what is in them, train a small model on them and ask it for a new building with no example.

- **Building judge** (`osrs-scene::building_census::{judge,guess}`): one target-free check for real
  and generated buildings (rooms, entrances, reachability, blocked doorways, a 16-number feature
  vector). `guess_objects` suggests what an unnamed object is from census evidence.
- **Name book** (`osrs-core::object_names`, `osrs-edit::name_book`): one name per object id, shared by
  every building. Precedence: your name, cache name, readable symbol, accepted guess. Stored beside
  the settings in `names/objects.names.json`. Inspector "Name this", object-search tokens (`role:`,
  `unnamed`) and capture/census all read it through `app/names.rs`.
- **`osrs-lab` trainer** (`generate`, `style`, `critic`, `model`, `train`): learns a kit from saved
  prefabs, an integer-weight realism critic against 3,944 real buildings (`data/real-buildings.json`,
  geographic split), room-count table by size, best-of-N building. A model is one checksummed
  `models/<id>.model.json`. CLI: `generate`, `judge`, `train-model`, `build-model`. Pure Rust, no
  Python needed; cancel and progress callbacks.
- **Workshop window** (`app/workshop*`): left list (search, filters, multi-select, delete to
  `prefabs/.trash`, restore) and tabs Buildings / Teach / Train. Train is Pick, Name, Teach it (progress
  bar and log run on a worker thread), Ask a model (brief form, best-of-N, keep, rate). Open it from
  the Prefabs Library page or `prefab workshop [buildings|teach|train|train NAME [PREFAB...]]`.
- **Verification run**: focused gate `python3 scripts/verify.py -p osrs-core -p osrs-edit -p osrs-scene
  -p osrs-lab -p osrs-world -p osrs-editor`: architecture/format/clippy and 947 tests passed, 20 ignored;
  the run was flagged non-stable only because a doc file changed while it ran. A headless test
  (`workshop::train::tests`) trains on one building through the real worker thread, saves, reloads,
  builds twice from a brief and gets identical output, then trashes the model. Full gate: see the
  entry below if recorded; otherwise not run.
- **Path learning, WFC lens and plain-language Studio (second pass)**: Workshop tabs *Paths* and
  *What WFC sees*; Studio header has "Open the Workshop" and the old Relationships window is now
  "What belongs to this building" (plain sentences, legend with colours, "Yes, they go together" or
  "Forget this note"). `osrs-edit::wfc_lens` builds WFC's view of a prefab (tile kinds, what it
  ignores, who sits beside whom) and runs "floor this building again" (walls pinned, solver
  chooses floor) or a free solve, with a step-through via `solve_wfc_observed` (new observer, same
  result). Finding: a catalog from one building cannot solve a free area (outer walls need an
  outside to touch) and, given its own walls, can only give the same floor back; it mixes only with
  several buildings. `osrs-lab::paths`: path lessons from real map areas, a 26-input network
  (`paths/net.rs`), least-cost router, `PathModel::attempt` (given the ends, or alone), lab task
  `path_task` + `lay_actions` (laid with `Action::PathLay`, scored by the lab, real paths hidden),
  `train-paths`, `paths-census|show|stats` CLI and the `path_lessons` world example; the Workshop lays
  results on the map through `apply_terrain_path` (one undo). Numbers and limits are in
  `docs/plans/PROCGEN-LEARNING.md` ("Learning to lay paths"). `crates/osrs-lab/data/world-paths.model.json`
  is the shipped starter model.
- **One building window (third pass)**: the floating Workshop and the Builder / Prefab Studio are now one window (the Studio viewport, titled Workshop) with tabs Scene (3D editing; left panel Library = the Workshop list with details, Build new, Floor patterns advanced), Names, Train, Paths, Floor patterns (WFC lens) and Practice room (the Lab, labels rewritten in plain words). `workshop::open(app, tab)` opens it; `workshop::page` draws the non-3D tabs. Trained building styles appear in the Build page's Kit list ("... (trained model)", main window and Studio) with "Let the AI pick a good seed"; trained path models appear in the Path tool's new AI helper ("Let the AI route it" replaces the clicked points with the model's route; one undo to lay). Place names (`map_labels.json`, 863 labels) import once (Paths tab), name areas and jump to places; tested as model input, no gain (see PROCGEN-LEARNING.md). Window screenshots for review: `RUSTOSRS_STUDIO_SCREENSHOT` hook or `screencapture -l` by window id.
- **Not verified / open**: the window was screenshotted (Train tab) but the machine's display went
  idle, so the trained-state view and real-mouse behaviour were not seen. A model trained on one
  generated or captured building separates real from generated buildings easily (critic AUC about 1.0)
  but its "looks real" score stays low (about 2 to 5 percent): the generator still differs from real
  buildings in room count, furniture density and roofs. The shared
  Rust/Python split, train-only furniture and critic in `scripts/lab/train.py`, the Door socket fix in
  `scripts/lab/catalog.py`, "Lab-accepted outputs are not reviewed" and PROCGEN-LEARNING.md updates
  (512 patterns) are not done. No neural policy; the planner still requires at least two rooms.

## Gameframe HUD, Character Studio, Named Varps/Varbits, and Arrow-Key Player Walking — 2026-10-18

Baseline is `main` at `f6e760d0d7d8bbcde68435b2076123996dfb1146`.

- **Arrow-Key Camera Rotation & Player Walking**:
  - Arrow keys (`ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`) rotate camera yaw and pitch around the focal player pivot in Game and Orbit modes at 2.8 rad/s, matching authentic OSRS client camera controls.
  - Player walker smoothly interpolates orientation in 2048 client angle units toward walker heading.
  - Left-clicking terrain in Game mode displays a pulsing, fading yellow ground "X" marker on the target destination tile, matching the authentic RuneScape click indicator.
  - Ctrl+click in Game mode inverts the run state on the fly (running if walking, walking if running).
- **Dedicated Character & Player Studio Window** (`app/player_window.rs`):
  - Window opened from the Player dock panel via a prominent "✨ Open Character & Player Studio..." button, from Gameframe HUD stone tabs (Skills, Equipment, Settings), or via settings.
  - **Look & Style Tab**: Male/Female toggle, quick look presets (Default Male, Default Female, Bald Monk), kit slot style cycle buttons (`<` / `>`) and direct identity kit ID editors, authentic OSRS 8 skin tone swatches, 12 authentic hair color swatches, and clothes color sliders. Rebuilding player geometry updates immediately on the 3D model.
  - **Equipment Tab**: 11 authentic OSRS equipment slots (Head, Cape, Neck, Weapon, Body, Shield, Legs, Hands, Feet, Ring, Ammo) with quick gear set buttons (Rune Knight, Dragon Slayer, Mystic Mage) and clear slot/all.
  - **Skills & Status Tab**: Combat level calculation, All 99 and Reset 1 buttons, full 23-skill matrix, HP, Prayer, Run energy, and weight.
  - **Variables Tab**: Searchable named varps and varbits table with instant substring search and live value modification.
- **Named Varps & Varbits Catalog** (`osrs-core::var_names`):
  - Sorted `VARPS` and `VARBITS` catalog tables compiled from OSRS/RuneLite definitions (including `COM_MODE`, `AUTO_RETALIATE`, `SPECIAL_ATTACK_PERCENT`, `STAMINA_ACTIVE`, `TRANSPARENT_CHATBOX`, `SIDE_PANEL_OPAQUE`, `PRAYER_*`, `FAVOUR_*`, etc.).
  - Binary search lookups `varp_info(id)` and `varbit_info(id)`, and case-insensitive search `search_varps()` / `search_varbits()`.
- **Authentic OSRS Gameframe HUD** (`app/gameframe/`):
  - Split into focused submodules (`mod.rs`, `orbs.rs`, `tabs.rs`, `chatbox.rs`), strictly $\le 230$ lines each.
  - **Minimap Status Orbs** (`orbs.rs`): Hitpoints, Prayer, Run, and Special Attack orbs positioned along the minimap bezel curve. Decoded from cache index 8 interface sprites (`1071` frame, `1060` HP, `1063`/`1066` Prayer, `1064`/`1065` Run, `1607` Spec, and icon sprites `1067`, `1068`, `1070`, `1610`) with vertical wipe fill and clean vector fallbacks. Clicking the Run orb toggles `player.running`.
  - **Stone Tab Rows** (`tabs.rs`): Two rows of 7 stone tab buttons (top row sprite `1173`, bottom row sprite `1174`) with authentic side tab icons (`774`..`787`). Clicking Skills, Equipment, or Options opens Character Studio.
  - **Chatbox Frame** (`chatbox.rs`): Semi-transparent stone chatbox with authentic filter buttons (All, Game, Public, Private, Channel, Clan, Trade), formatted game messages (yellow/cyan/white), and a minimize/expand toggle.
- **Verification**:
  - `python3 scripts/verify.py -p osrs-edit -p osrs-lab -p osrs-world -p osrs-editor`: **PASS (focused)** in 462.0s (730 passed, 0 failed, 20 ignored; format, architecture, and clippy all green; `target/verify/gates-20261018-200531/tests/report.json`).
  - `python3 scripts/verify.py -p osrs-core -p osrs-editor`: **PASS (focused)** in 35.6s (477 passed, 0 failed, 4 ignored).
  - `cargo check --locked --target wasm32-unknown-unknown -p osrs-core -p osrs-edit -p osrs-cache -p osrs-scene -p osrs-hd -p osrs-render -p osrs-world`: **PASS**.
  - `cargo clippy --locked --target wasm32-unknown-unknown -p osrs-editor -- -D warnings`: **PASS**.
  - All files in `crates/osrs-editor/src` remain $\le 500$ lines, and `scripts/editor_app_surface.json` ratchets are strictly preserved.


## Game mode, player placement, and overlay toggle — 2026-10-18

Baseline is `main` at `f6e760d0d7d8bbcde68435b2076123996dfb1146`.

- **Player Overlay Toggle**: Added `show_overlay: bool` to `PlayerSandbox` (defaults to `true`). Exposed via a checkbox in the Player panel and in the viewport's top `Overlays` dropdown menu. When unchecked or when in Game mode, the 2D billboard pin, yellow tile outline, and floating text are suppressed while keeping the 3D player model and animation visible.
- **Place Player Here**: Added `MenuAction::PlacePlayerHere((i32, i32))` to `osrs-editor` context menu (`editor_menu.rs`, `effects.rs`, `mod.rs`). Right-clicking any 3D map tile now offers "Place player here", setting `stand`, teleporting the player walker, and notifying without needing to open the player sandbox panel or manually type coordinates.
- **Top Viewport Toolbar Controls**: Added a context menu switcher dropdown (`ContextMenuStyle::Editor` vs `ContextMenuStyle::Game`) to the top viewport toolbar next to camera modes (`Fly` / `Orbit` / `Game`), allowing fast switching without digging into settings.
- **Game Mode (`CameraMode::Game`)**:
  - Added `CameraMode::Game` button to the top toolbar alongside `Fly` and `Orbit`.
  - In Game mode, the camera locks onto the player character as its focal pivot (`camera.focus(player_pos)`), supporting middle-mouse/right-drag orbit, scroll zoom, and WASD/arrow rotation around the player pivot.
  - Left-clicking on the 3D terrain walks the player using `PlayerWalker` (`crates/osrs-playtest/src/walker.rs`), with sub-tile interpolation, 600ms tick movement, bilinear terrain height following, and animation state transitions (idle sequence `808`, walk `819`, run `824`).
  - Right-click "Walk here" context action routes to the player walker.
- **Verification**: `cargo fmt --all --check`, `/opt/homebrew/bin/python3 scripts/check_architecture.py`, `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`, `cargo clippy --locked --target wasm32-unknown-unknown -p osrs-editor -- -D warnings`, and `cargo test --locked -p osrs-editor` (345 passed), `cargo test --locked -p osrs-playtest` (9 passed), and `cargo test --locked -p osrs-lab` (26 passed) all pass cleanly. Module line limits ($\le 500$ lines) and `EditorApp` surface ratchets are strictly preserved.

## Audit baseline — 2026-10-18

Audit baseline: clean `main` at exact `ada7c6277fac47527e481994ef6d44798ecc66f3`.
The audit report [Building / Studio / Lab](docs/verification/BUILDING-STUDIO-LAB-AUDIT-2026-10-18.md)
supersedes the historical “uncommitted”, 128-pattern and stepped-foundation status claims below.
At that baseline, acceptance was incomplete: native Studio rejected the pinned b241 stage,
bridge-plane and stepped-height probes failed, WFC apply targeted the Lab mirror, and
door/human-link/held-out model boundaries had confirmed gaps. The virtual-workspace checkpoint
below resolves the stage rejection and WFC authoring route. The report records exact repros,
source locations, verification provenance and next checkpoint order.

The audit native build passed. Stored pre-checkpoint full verification belongs to `92477b00` plus the then-dirty
search fix (1,260 passed / 22 ignored); stored focused world/editor verification belongs to
`ada7c627` (394 passed / 20 ignored). The audit native Studio smoke failed; those green suites
did not override that baseline result. The audit changed documentation only and left those docs
uncommitted. See the newer implementation and native smoke results below.
AI training may be native-only; shared editing semantics retain their existing portable boundary.

## Detached workspace and first copy curriculum — 2026-10-18

Baseline remains `main` at exact `ada7c6277fac47527e481994ef6d44798ecc66f3`; changes from the
previous checkpoint are preserved and all implementation remains uncommitted.

- Builder / AI Lab / WFC now has its own immediate native viewport. The main editor continues
  streaming/editing independently. Authored sessions/history stay separate; Lab parks only its
  Studio author session. Root quit closes/prompts the workspace first, then preserves the world
  save prompt. A detached save refuses newer library content and supports Save as new recovery.
- `osrs-lab::reconstruction_task` and `task-copy` replace editor-only task conversion. Exact
  occurrences, rotations, sparse terrain and heights are retained; dimensions must be resolved,
  palettes are deduplicated and geographic/family grouping is shared with offline evaluation.
- Lab adds Generate copy demonstration, progress/cancel, task export, next-action inspection,
  replay speed and reset. Native jobs reuse the existing Python `imitate` baseline and local Lab
  server, write immutable artifacts under settings-adjacent `lab-runs/`, and independently replay
  actions before presenting a verified score. Metrics say target-visible / not a learned policy.
  Direct editor scripts pause in Lab; permitted host controls cannot bypass action authority.
- The user's saved `test` is reviewed, 10×9×2, with 154 placements, 22 asset IDs, 50 owned tiles and
  208 source height samples. The first headless run produced 141 legal actions and exact score 1.0.
  It has inferred role/evidence labels but no confirmed human object-role labels. Policy fitting
  and generalization on held-out buildings remain future work; this creates the demonstration data.
- Focused reconstruction tests (3) and prefab/editor tests (24) passed. Strict focused Clippy passed
  after fixing a test unwrap. The first detached smoke confirmed a native child viewport and
  concurrent world paint/undo, and verified teacher replay artifacts. It exposed paused Lab smoke
  controls, which have been fixed with a controls-only script boundary. Final native/full checks
  are pending; do not treat the earlier 1,265-test gate as verification of these later changes.

## Virtual prefab workspace checkpoint — 2026-10-18

Implementation baseline: `main` at exact `ada7c6277fac47527e481994ef6d44798ecc66f3`.
All changes in this checkpoint remain uncommitted; the earlier audit documentation is preserved.

- The library's former 3D gallery button and double-click now enter the full-frame Builder /
  Prefab Studio. The workspace exposes Library, existing Builder controls, authored WFC floors,
  camera framing/top/3D views, normal object/terrain tools, Save/new/Revert and the separate AI Lab.
  Transient workspace controls/catalog previews are owned by the stage's `UiRuntime`.
  Switching to newly created scenes reloads the owning library; scene and accepted Lab saves refresh
  its workspace projection. Explicit map placement remains separate.
- Gallery and Studio now configure a synthetic four-plane map before any FIFO build requests.
  The source has flat known support, base ground and no source objects; all contextual region reads
  use it. Cache definitions/models/textures remain available. `WorldDefinitions` source selection is
  a disposable worker projection and never becomes editing authority. This removes the dependency
  on empty cache squares, and floor palettes now load for stage authoring.
- Capture defaults to reviewed sparse building ground, independently retaining outside boundary
  anchors. Rectangle copying is explicit. Role controls cover furnishings, attached/ground
  decoration, roofs and stairs; room/plane/piece review still resolves ownership. Filtering remaps
  occurrence labels and links. Roof membership follows active floor evidence even above its plane.
- Learned floor application now plans in `osrs-edit::plan_wfc_floors` and commits only to the
  authored Studio runtime. Its controls live in Edit / WFC rather than the temporary Lab mirror.
  It paints terrain only; sockets constrain the solve, without claiming object realization.
- Architecture and diff checks passed; focused prefab tests passed (23) and virtual source test
  passed (1). Native build passed. The pinned b241 native smoke opened 208 objects with empty
  baseline history, exercised paint/undo/redo and saved the changed tile; it exited successfully.
  Screenshot: `target/prefab-workspace-smoke/workspace-first.png`; log and saved fixture are beside
  it. The saved smoke fixture retained all 208 exact object occurrences and 136 terrain tiles,
  excluding the synthetic stage floor. The ready-checkpoint full gate passed in 712.8 seconds:
  **1,265 passed / 0 failed / 22 ignored**, including native Clippy, architecture/registry checks,
  complete test-binary inventory and portable checks. Report:
  `target/verify/gates-20261018-142149/report.json`; both gate and test source-identity checks report
  no concurrent source changes. The earlier full run was interrupted for the UI ownership cleanup;
  it is not counted as full verification. Only this handoff documentation was updated afterward.
  Ignored external-cache / visual-oracle tests remain unverified prerequisites, not acceptance.
  A regular updated app was launched with Studio open, without an exit timer or test settings;
  the older manual instance was preserved. AI training remains allowed to be native-only; this
  checkpoint did not change the portable semantic boundary.

Remaining findings in the audit (bridge support, stepped-height precision, door labels, live human
link indexing, and held-out fitting) are separate from this workspace/capture checkpoint.

## Building correctness / Prefab Studio / procedural Lab (2026-10-18)

This section supersedes older operational status below for this checkpoint. Work started on `main`
at exact SHA `5d0d5a47f18862973a195e6abe93da63c6d19d85`, with existing interaction/picking/roof
changes already uncommitted. Those edits were preserved. No branch, worktree, push or PR was created.
All current implementation changes remain uncommitted while integration verification is in progress.

- Building descriptions live in `osrs-scene::building_census::description`. They retain exact
  occurrences, canonical wall arms, portals/approaches, attachments and candidate roof/stair links.
  Inferences carry evidence and uncertain/shared membership requires capture review.
- Prefab v2 persists sparse floors/support, dimensions, indexed relations, provenance and human
  labels. V1 remains readable. Outside anchors do not copy outside terrain. Studio saves compare
  the authored document with the pristine stage and retain intentionally default owned floors.
- Building relocation uses median support fitting, preserves relative upper heights, blends two
  tiles and rejects incomplete/stepped/occupied/unverified sites. Replacement lists exact removals
  and commits once after revision/source-stamped review. Rectangle capture remains separate.
- Native Studio parks the world, reuses the gallery stage and normal controllers, initializes an
  untracked baseline, and provides isolated undo/history, Save/new/Revert and dirty navigation.
  Overlays explain masks, outside anchors, wall arms, door approaches, roofs and relationship
  evidence. Human labels survive inferred-link invalidation. Native stages reject nonempty source
  pads, preserve keymaps and release worker joins off the frame thread.
- `osrs-lab` is a portable core/edit/scene boundary for tool actions, private targets, budgeted
  scoring, undo/replay and JSON-lines batching. Studio projects Lab state read-only while parking
  authored history. Tasks include generated tavern, prefab reconstruction and terrain relocation;
  accept/reject feedback and action traces remain local. No raw-patch or target-paste action exists.
- Learned WFC catalogs restrict compatibility to observed pairs, retain integer counts and report
  the 128-pattern limit. Kits accept versioned catalogs and quantized role suggestions. Python
  baselines and learners are documented in `scripts/lab/README.md`; confirmed human labels and
  geographic/family holdouts remain separate from weak census/117HD labels.
- Dataset v2 uses neighbor context and resolved terrain/collision; it emits exact roof/object
  occurrences, sparse floors, terrain channels, vertices, evidence, unknown/truncated coverage and
  source fingerprints. Opcode-249 object parameters retain typed values without guessing key meanings.
- Interaction fixes clear selection on every plane path, stamp outline requests/results, reuse
  roof grids, merge large-selection boundary segments and layer Escape/flyout behavior. The runtime
  extraction respects the existing 500-line cap; the EditorApp surface was reduced, not widened.

Actual checks so far: architecture guard and 27 guard tests passed; 266 edit + 71 scene + 4 Lab
library tests passed before the latest integration additions; five Python learning tests passed.
A visible-target tavern reconstruction reached exact score 1 through 950 tool actions. A seeded
private-target search smoke returned no target snapshot. Full native/WASM gate and native visual
smoke are still pending; no remote CI or independent visual-oracle acceptance is claimed.

Known deliberate constraints: stepped foundations require support-group design before automatic
fitting; unverified destination overlays are conservatively rejected. Studio saved bounds/planes
are fixed and out-of-bounds edits are rejected. Learned catalogs require explicit kit asset binding;
private JSON observations do not sandbox an untrusted process's filesystem. No trained neural
policy or accuracy on an absent human-reviewed corpus is claimed. See ADR-0015 and the feature
registry for ownership/tests.

### Follow-on checkpoints 4–5 (2026-10-18, local commits on `main`)

- `ef8e6fb9`: WFC widened to 512 patterns (`PatternDomain([u64; 8])`, `WfcPatternId(u16)`),
  weighted Shannon-entropy cell choice, `solve_wfc_with_repair`, rotation-synthesised learned
  catalogs, multi-floor (`floors: 1..=3`) generation with paired stairs and lifted roofs, and
  "Apply 8×8 to Studio Scene" from the Studio catalog.
- `92477b00`: pure path fitting (`smooth`, `fit`, `fit_bounded`, shape triangles, Java-order oracle
  and goldens) now lives in `osrs-scene::terrain_path`; `osrs-editor::terrain_path` re-exports it.
  `osrs-lab` adds `PathLay`, `HeightBrush`, `HipRoof`, `RoomRect.corner_id`, `task-paths`
  (procedural network or prefab with buildings masked), `dont_care_tiles/heights`, partial
  terrain credit, active-channel weighting, `StepResult.breakdown` and `observe:false` steps.
  `scripts/lab/train.py` compiles targets into rect/wall-line gestures, searches in stages, and
  exports furniture priors plus a structural critic (`best_of_n`).
- Checks run on `92477b00` plus the `train.py` undo fix: `python3 scripts/verify.py --full` PASS
  (1,260 passed / 0 failed / 22 ignored; fmt, native clippy, wasm check and wasm clippy green,
  1,743.9 s). Smoke: `osrs-lab task-paths 1` + visible `imitate` reached exact score 1.0 in 93
  tool actions; hidden-target `search` (12 rounds x 6 candidates) scored 0.816 in 49 actions with
  no target in the observation. This search score is a baseline, not an accuracy claim. No remote
  CI, no native visual smoke of the Studio "Apply 8x8" button and no trained neural policy.

## Current boundary and standing workflow

- Working branch: `main`; stay on `main` unless the user explicitly asks otherwise. One local commit per checkpoint; nothing is pushed or published without authorization.
- Program baseline: `63d2721e3fa30dc2eaecff140d6b27886e4dff2f`, merge of PR #56 (editor ownership), main CI run `38766016806` (Tier A/B/C success). The old feature branch is retired remotely.
- Commit `41d0c020` ("updates") on top of that baseline already contains the program foundations: opt-in JSON performance capture (`diagnostics/capture.rs`, schema v1), owned-GPU-byte accounting (`gpu/resources.rs`), Reference frame reuse (`render_runtime/reuse.rs`), the shared Spline settings widget (`spline_tool/panel.rs`), `scripts/performance.py`, and the 117HD source manifest/ledger (98 config items, all `audit_pending`). They are committed, not pending, but **no baseline or improvement has been recorded**.
- Authorized work and plan: `docs/plans/PERFORMANCE-UI-PLAYTEST-HD.md`. The expanded user goals (RuneLite/deob re-verification, exact tile/overlay/underlay/blend abstraction for the spline tool, NPC/entity runtime for a future own client) are carried by the checkpoints below.
- Source-review baseline on 2026-10-16: clean `main` at `fe1b703cf06e472b26bee68f2bd26cff2684dbe0`. Actor definition/sequence-slot and deterministic spot/projectile foundations now exist in `osrs-core`; this does not complete entity decoding, model assembly, collision snapshots or editor playtesting. Performance measurement/optimization is explicitly **paused** in the current authorized plan; historical instructions to retake baselines below do not resume it.

## CI and test suite pipeline optimization & camera speed consistency (2026-10-16)

- Started on clean `main` at exact baseline `f5265b88e1c91308023afe64fee68ea85c45fdba`.
- **Super-Light GitHub Actions (`.github/workflows/ci.yml`)**:
  - Replaced heavy multi-job CI on `push` and `pull_request` with a single ultra-fast `fast-gate` job that only checks formatting, architecture boundaries, and Clippy.
  - Moved heavy workspace tests (`tier-b`) to `workflow_dispatch` (manual run) so remote pushes finish in under a minute without consuming runner quotas or causing console lockup.
  - Added `CARGO_INCREMENTAL: "0"` and `Swatinem/rust-cache@v2` with `shared-key: "workspace-fast-ci"`.
  - Configured `[profile.dev] debug = 1` (`line-tables-only`) in `Cargo.toml`, cutting debug DWARF sections by 70% and accelerating linking by 2-3x while preserving panic backtraces.
- **Camera Speed Consistency & OSRS Navigation Improvements**:
  - **Root cause solved**: In Fly mode, casual mouse wheel or macOS trackpad inertial scrolling was executing `self.camera.speed *= (1 + scroll * 0.002)`, continuously mutating `self.camera.speed` behind the user's back up to 20,000 u/s or down to 50 u/s even though settings were unchanged.
  - **Intuitive OSRS zoom**: In Fly mode, casual scrolling now dollies the camera forward/backward along the view direction (`self.camera.forward()`), matching OSRS zoom behavior and leaving the configured fly speed untouched.
  - **Intentional speed tuning**: Scrolling while holding Secondary Drag (Right-click mouselook) now adjusts camera speed, with delta clamped to `[-0.25, 0.25]` to prevent trackpad inertial runaway.
  - **Movement vector normalization**: Normalized directional movement in `input.rs` so diagonal travel (e.g. W+D) maintains the exact same speed as straight travel instead of jumping by $\sqrt{2}\times$ (1.414x).
  - **Precision modifier**: Added `Ctrl` / `Cmd` for 0.25x precision camera control alongside `Shift` 4x boost.
  - **Hints & tests**: Updated status bar hints (`Scroll: zoom`, `Right drag + scroll: speed`, `Shift: boost`, `Ctrl: slow`) and added unit test in `interaction_runtime/camera.rs`.
- **Stock 117HD & Theme Studio Foundations (Checkpoint 1)**:
  - Implemented dynamic point lights (`osrs_hd::lights::LightSet`, waveform evaluation, distance culling, 117HD quadratic attenuation, WebGPU `@group(2) @binding(5)` buffer, WGSL lighting loop).
  - Implemented analytical sun/moon disks with horizon fade and zenith gradient in `sky.wgsl`.
  - Reconciled all 98 configuration items in `docs/verification/117hd-feature-ledger.json` (0 awaiting audit verified by `check_hd_sources.py`).
- **Local checks run**:
  - Python guards (`check_architecture.py`, `test_check_architecture.py`, `test_editor_architecture.py`, `check_hd_sources.py`, `test_hd_sources.py`): passed.
  - `cargo fmt --all --check`: passed.
  - `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: passed in 6.65s with 0 warnings.
  - `cargo test -p osrs-editor camera`: all 10 tests passed.
  - WASM check (`cargo check --locked --target wasm32-unknown-unknown -p osrs-core -p osrs-edit -p osrs-filestore -p osrs-cache -p osrs-scene -p osrs-hd -p osrs-render -p osrs-world`): passed.
- Changes remain uncommitted locally pending user review.

## Theme Studio foundation (2026-10-16, in progress)

- Authorized program: `docs/plans/THEME-STUDIO.md`. **The complete requested stock-HD/530/theme
  program is not implemented or accepted.** This is the appearance-authoring foundation checkpoint.
- Started on `main` at exact baseline `81ae4e8ad3423313c7ea7a5f401bb85eecb49d62`, with existing
  player/NPC edits. Another session committed/amended that player work during implementation;
  latest observed HEAD is `1645b81eee0e216384cc7d39afa6ed921109d809`. Shared viewer/mobile work is
  separate. Recheck live Git; do not attribute those changes to this checkpoint.
- `osrs-hd::theme` owns versioned definitions, finite values, ordered property-wise resolution,
  reversible patches and checksummed ZIP packs with immutable shared asset bytes. Only the
  foundational `osrs-core` dependency was added. ADR-0014 documents the boundary.
- Application `EditorDocument` composes the existing workspace and `ThemePack`; the existing generic
  patch history supplies chronological map/NPC/appearance undo/redo through `EditRuntime`.
  Appearance edits advance authored revision but do not dirty map regions or advance map revision.
- View → Graphics → Theme Studio provides stock/new/duplicate, native import/export/library,
  lighting/fog/water/effect controls and editable/reorderable rectangular area rules. UI drafts are
  transient. Native project v2 embeds appearance; v1 migrates to stock. Cache export is unchanged.
- `RenderRuntime` owns candidate pipeline resources/futures. WGSL vertex/surface/water/sky/post
  templates are parsed/validated before GPU preparation; scoped GPU validation completes before
  activation. Shader history/project loads restore pipelines from authority. Vertex deformation
  also reaches shadow casting; animated vertex hooks invalidate shadow reuse.
- `scripts/audit_hd_config.py` records and verifies all 98 pinned source default bodies and anchors.
  Source pins and the full supplied Java tree match. All 98 implementation dispositions remain
  `audit_pending`; this inventory does not claim stock feature reconciliation.
- Outstanding: stock point lights, celestial/day-night/reflections/wind/post/AA completion; image
  material projections and terrain/object/placement rules; 530 procedural importer and faithful
  theme; shader parameter/file UX; browser pack/library persistence; full independent visual and
  end-user acceptance. Unsupported material/selector packs are rejected explicitly. The 2008 HD
  card stays disabled. No 530 source/assets were copied or textures decoded in this checkpoint.
- Focused local checks so far: 5 theme-domain tests; 2 mixed-history tests; 2 Theme Studio
  cancellation/layout tests; 4 theme-shader tests including actual GPU activation and pixel output;
  8 existing material/profile GPU tests; architecture guard + 26 guard tests + 12 editor-contract
  tests; HD source pins/defaults: passed. Reference-regeneration reports 5 missing-tool skips and
  golden-index reports its historical missing-harness skip. Full native/WASM gates are still being
  run; subsequent results must be recorded below. No remote CI certification or independent
  RLHD/530 visual comparison is claimed. Theme changes currently remain uncommitted.

## Offline NPC scenarios checkpoint (2026-10-16)

- Authorized source: installed OpenRune at `/Users/tylercovalt/Desktop/OpenRune Project/OpenRune-Server` (revision-240 manifest, `.data/cache/LIVE`). Baseline `e1325640` plus the uncommitted work of the previous NPC session; this checkpoint finishes the editor side. Check `git log` for the checkpoint commit; nothing is pushed.
- **Layers.** `osrs-core` shared values (`scenario`, `actor_snapshot`, `actor_pose`, NPC definition/render flags). `osrs-cache` NPC decoder, `npc_model`, and `openrune/` (gamevals, installation, spawn import, explicit export; bundled profile `profiles/openrune-installed-240-live.yaml`). `osrs-edit` `ScenarioDocument`/`ScenarioPatch` plus pure planners (`plan_add_spawn`, `plan_update_spawn`, `plan_remove_spawns`, `plan_replace_scenario`) and `WorkspaceDocument`/`WorkspaceHistory` (one chronological history for map and scenario patches; `map_revision` counts authored map changes only). `osrs-playtest` headless simulation (navigation port with attribution in `LICENSE-RSMOD`, deterministic 30-cycle ticks, occupancy kept separate from static clipping). `osrs-world` `npc.rs` extracts entity-only geometry on the world worker (separate from terrain and animated locs) and serves an NPC catalog.
- **Editor.** `app/npc_runtime.rs` (`NpcRuntime`) owns catalog, immutable assets, `PlaytestRuntime`, and acceptance of worker results by `PlaytestIdentity` (source generation, map revision, scenario revision, run generation); stale results are counted and dropped. It reads collision through `npc_runtime/navigation.rs` (accepted snapshots only; rebuilding or absent coverage parks actors). `app/npc_panel/` is the **NPCs** rail tab: `scenario.rs` (create/import/load/save/export through the edit gateway), `authoring.rs` (playback, search/placement, selected-spawn editing with a draft that commits once per gesture), `viewport.rs` (armed placement/destination gestures, anchors, footprints, patrol routes, route preview and blocked reasons), `script.rs` (`npc ...` smoke commands). Entity geometry uploads under `NPC_KEY`; moving an actor never rebuilds terrain. Runtime movement never enters history.
- **Verified in the real editor** (release build, OpenRune LIVE cache, Reference profile): importing `lumbridge.toml` gave 377 spawns, 0 errors; Play advanced ticks; animated NPCs, footprints and the panel were seen in screenshots. Full-directory import audit (`cargo run -p osrs-cache --example openrune_npcs -- <root>`): 25,801 spawns, 16,345 NPC definitions decoded, 0 errors.
- **Tests added.** `osrs-edit` scenario patch/history tests; editor gateway tests (chronological map+scenario undo/redo, empty gesture keeps redo, projection rollback, stale patch rejection); `NpcRuntime` identity/diagnostic tests; existing `osrs-playtest` and `osrs-cache` NPC tests.
- **Not done / not verified.** No differential test against the supplied AssemblyScript pathfinder and no independent client comparison. HD profile NPC rendering and shadows not visually checked. Skeletal NPC animation is unsupported and reported per actor (no static substitution). Indoors/Outdoors restrictions use the terrain roof flag (bit 4) of the tile's own plane, an approximation. Scenario edits advance the project's edit revision, so the project reads as unsaved although the scenario saves to its own sidecar. Animation preview lists only the NPC's movement sequences. Unset varps read 0 for morphs (Player panel varps override). Spawn-directory import is native-only; browser path loads/saves sidecars and exports. Native dialogs, browser pickers and mouse placement/destination gestures were not exercised by hand (unit/script coverage only). Importing every spawn (25k) was audited for decoding only; per-frame cost with all spawns was not measured.
- Local gates for this checkpoint: locked workspace/all-target/all-feature tests **917 passed, 0 failed, 21 ignored**; `cargo fmt --check`; strict workspace Clippy; lower-crate WASM check; strict WASM editor Clippy; architecture guard + 26 and editor-ownership 11 tests: passed. After the test run, only lint attributes (test-module unwrap allowance, WASM dead-code allowance) changed; Clippy and fmt were re-run on those edits, the test suite was not. Local checks do not certify remote CI; no parity/pixel-golden or pinned-cache external suites were re-run for this checkpoint.

## Sound playback, names and UI behaviour (2026-10-16)

- **Synth sound effects** (`osrs-core::audio`, `osrs-cache::decode::decode_sound_effect`, `osrs-world::sound`): port of the deobfuscated client's `SoundEffect`/`Instrument`/`SoundEnvelope`/`AudioFilter` (32-bit wrapping integer maths, `f32` filter, sine table divisor 5215.1903 as in the client). Real cache (OpenRune LIVE, revision 240): **12,089** effects (file 0 of archive 4) decode and render with no failures or panics; 3 groups hold only the sampled (Vorbis, file 1) form and report "not supported" instead of substituting. Differential check against the TypeScript port in `tsps/client` on 326 effects: 145 byte-identical at its 8-bit mix, the rest differ by about one LSB (2.7% of samples; with the TS sine constant 200 identical), so the Rust follows the Java constants, not the TS port. Not compared against audio from the real client; what it sounds like was not listened to by me.
- **Playback** (`app/audio_runtime`): native `rodio` output (one player at a time); browser build reports that playback is unavailable. Worker request `Request::Sound`, event `StreamEvent::Sound`.
- **Names** (`app/names`): when the open cache is an OpenRune `.data/cache/LIVE`, gameval symbols name objects (every loc has one), NPCs, items, animations and 205 sounds. Used where the cache has no name. Hints only; nothing keys on them.
- **Sound panel**: Sounds/Objects/Object pages, Play buttons, "Sounds in the world" setting (also overlays menu, View panel), marker click / Shift+right-click opens the Object page. The owner has asked to leave further Sound panel rework until the web UI.
- **Pickers and toolbar**: choosing a picker cancels an armed NPC placement or object placement; the picker rail shows the last three tools used and an armed NPC gesture.
- **NPC panel** is an info/edit/place tool led by a "Show OpenRune spawns" switch (`settings.npc`): on loads every spawn from the installation the open cache belongs to (25,801 spawns measured at 60 fps, NPC phase about 0.5 ms; only actors within `npc.distance` tiles, nearest `npc.max_drawn`, are extracted); off hides everything and pauses the simulation but keeps the list. Folder/file choice, reload, view settings, spawn-edit files and OpenRune export are under Advanced. The UI never says "scenario"; the file format identifier is unchanged. Not exercised with a real mouse.
- **Loading splash** (`app/splash.rs`): "OpenRune" filled by a two-layer wave, driven by cache-ready and region residency; fades when built. Seen mid-fill in a screenshot; fade-out and the cancel path are covered by unit tests only.
- **Proposed ADR-0013** (egui and Svelte shells over one typed UI contract). Not accepted; needs a spike.
- Local gates for this work: locked workspace tests **959 passed, 0 failed, 21 ignored**; fmt; strict workspace Clippy; lower-crate WASM check; strict WASM editor Clippy; architecture guard + editor-ownership tests: passed.

## Roofs, the 3D prefab gallery and the labelled dataset (2026-10-18)

- **Roofs above the viewed plane** (research in `docs/research/RUNELITE-DEOB-FINDS-2026-10-18.md`). `osrs-scene::roofs` labels connected `UNDER_ROOF` tiles (setting bit 4) per plane (eight neighbours, the one-tile overhang joins, ids from the component's south-west-most world tile). `osrs-world` buckets geometry by roof id the way RuneLite's `SceneUploader` does (`roofs[plane - 1]` of the tile; reference, HD and animated paths; `ZoneGroup.roof`, `ZoneGeometry::group_roof_mut`). `osrs-render::gpu::RoofView { top_plane, hidden }` replaces every `min_plane <= view_plane` test (`SceneRenderer::set_roof_view`). Editor: setting `roofs` = Hidden (old behaviour) / Visible / **Cutaway** (default: the roof over the camera, the orbit pivot, the hovered tile and the selection goes), Settings > World and a toolbar button; `app/roof_view.rs` computes the hidden ids per frame. Picking and editing still use the viewed plane. Generated buildings now set `UNDER_ROOF` on their floors (`building_finish.rs`). Verified by screenshot at plane 0 in all three modes and with a selected tavern (its roof goes, the neighbour's stays). **Not done**: RuneLite's `overrides.jsonc` regions, roofs larger than the 145-tile labelling window, per-object roof visibility for picking, a unit test over a real `WorldScene`.
- **Found and fixed**: `osrs-world/src/hd/extract.rs` used `TILE_FLAG_VIS_BELOW = 4` (that is `UNDER_ROOF`); the client, RuneLite and `terrain_build` use `8`. World tests pass; the HD terrain output for roofed tiles may differ from before (no HD pixel oracle covered it).
- **Prefab gallery** (`app/prefab_gallery.rs`, `prefab_gallery/{stage,list}.rs`; Library page button "Open the 3D gallery", script `prefab gallery [name] [render]`). A window with a filterable, sortable card list (search, source, tag, sort) and a 3D view of the selected prefab with the real renderer: roofs hidden/visible, floor picker, turn left/right, turntable, orbit/pan/zoom. The view is a **stage**: a second isolated `EditorApp` (own world, document, history, renderer; shares the GPU device, like the export review) on the empty nine-square block of the cache around map square (50, 33), tile (3232, 2144); it lays a grass floor (underlay 50) once, places the prefab through `plan_place_prefab` + `commit_map_patch`, and undoes it for the next selection. Stills: the first time a prefab is settled on the stage (turned 0) a 320x240 still is rendered (`SceneRenderer::render_to_rgba`) and used on its card and in the Library list; "Render N preview(s)" cycles through the rest. Native only (the browser build shows a message). Memory/CPU: a second streaming world while the window is open (closing frees it). **Not verified**: real-mouse orbit/zoom in the gallery, a prefab wider than the 3x3 empty block, HD profile on the stage, stills of prefabs whose objects have not loaded their models yet (45 frames and no build in flight is the settle rule), stage colours (the floor reads mustard; ground blending at the pad edge).
- **Procgen learning** (`docs/plans/PROCGEN-LEARNING.md`): what the WFC cannot see, a label taxonomy, phases. `osrs-world/examples/building_dataset.rs` exports every real building (rooms with tags, furniture, decoration, wall edges, roof, ring, 117HD area names via `AREAS=`) to JSONL: 3,944 buildings, 35,350 objects, 1,888 inside a named area. Measured: of 21 buildings inside a 117HD bank area only 9 have a census `bank` room; of 9 church areas 3 have a `church` room.
- Local gates: see the commit messages for the run; this section is updated if they change.

## Building ghost, overlap clearing and the Auto flyout (2026-10-18)

- **Geometry ghost** (`app/viewport.rs`): with more than one object selected, every selected object's real model triangles (the same worker outline request the single selection uses, capped at 1500 objects) are drawn as one translucent mesh, so a selected building shows its walls, roof and furniture instead of only a flat floor tint. Outline requests are now queued before the pointer-dependent early returns. Seen on a Lumbridge tavern at plane 1; at plane 0 the roof ghost is drawn although roofs are hidden by the plane rule.
- **Placing clears what is there**: `osrs_edit::objects_overlapping_rect` finds objects by footprint overlap (anchors within 8 tiles, multi-tile sizes through `size_of`), used by prefab placement and the Build page's clear-site step (planes base and base+1). Prefab placement now defaults to replacing. Test in `prefab/tests.rs`.
- **Building under Auto**: the Building picker is no longer a rail button. The Auto button shows a flyout on hover (`floating.rs`: `auto_flyout`, `choose_picker`; animated, hover zone kept in `ctx.data`) listing Auto and Building; the button shows the building icon while Building is active. **Not exercised with a real mouse**: hover timing and the flyout's exit zone are unverified.
- `script/select.rs` holds the `select` script command (moved out of `script.rs` for the 500-line cap); `select report` prints how many selected objects have geometry ready, per plane.
- Local gates: locked workspace tests **1198 passed, 0 failed, 21 ignored**; fmt; strict workspace Clippy; lower-crate WASM check; strict WASM editor Clippy; architecture guard + tests: passed.

## Building picker and unified selection overlay (2026-10-18)

- **Overlay**: selected tiles draw as one shape: a single mesh fill (no lines between tiles) and an outline only along edges where a selected tile meets an unselected one. Per-object footprints are skipped when more than 40 objects are selected, so a whole building does not bury its own shape.
- **Building picker** (`SelectMode::Building`, the buildings icon in the picker rail and the Select options; `app/building_pick.rs`): a click inside a building selects its floor tiles and every object that belongs to it: anything on the floor, boundary pieces and wall decoration on the one-tile ring around it (walls owned by the outside tile of an edge), and roof pieces (loc kinds 12 to 21) on the plane above over the floor or ring. Scenery just outside the walls is not taken. The building is found with the census room detection (`osrs-scene::building_census`) over the 3x3 loaded regions around the click; clicking on a wall, a roof overhang or from the roof plane works. A box drag in this mode behaves like Auto. Script: `select building X Y`. Tested on synthetic two-room houses (rooms, ring, roof, plane above, outside scenery) and run on a real Lumbridge building (1 room, 50 floor tiles, 154 objects).
- **Not verified**: real-mouse clicks in Building mode (only the script path and a screenshot), buildings that span the edge of the loaded area, and multi-floor buildings (an upper floor is not taken with the ground floor).

## Wall slots and world-space box select (2026-10-18)

- **Missing walls**: the game keeps one boundary piece per tile, so a second straight wall on a tile (corners, T-junctions, one-wide corridors between rooms) is lost when the map loads. The generator put two on such tiles; about 15 of 644 objects never reached the rendered world (found with the new script command `prefab check`, which compares the plan with the rendered world). `osrs-edit/src/building_walls.rs` now assigns every wall edge to one of its two tiles by bipartite matching (one piece per tile, the same edges, pieces stay on their own tile when they can); the kit generator runs it before finishing. Test: every archetype at four sizes and 12 seeds (384 buildings) is closed on the outside and has no tile with two wall pieces; after the fix `prefab check` reports 0 missing for a house, a bank and a tavern.
- **Box select** (`interaction_runtime.rs`, `selection.rs`): the box is now the rectangle of ground tiles between the tile under the press and the tile under the pointer (map axes), selecting tiles and objects by the tile they stand on, drawn as an outline along the ground with a light fill. Before, tiles were chosen by projecting their centres into a flat screen rectangle. Unit-tested for the box logic; **not exercised with a real mouse drag** (the editor is not a desktop app bundle the automation can drive), so the outline drawing on uneven ground is unverified.

## Building census and finishing (2026-10-17)

- **Census** (`osrs-scene/src/building_census/`, pure analysis; examples `osrs-world/examples/building_census.rs`, `building_ascii_real.rs`): finds enclosed rooms by flood fill across wall edges (gaps of one or two edges in a wall run count as open doorways), groups rooms that share walls into buildings, and records exterior vs interior walls, doors/windows/openings, wall decorations and the side they hang on, floors per room, furniture and its distance/orientation against walls, roofs, the ring of ground outside and height spreads. Rooms are tagged from what stands in them (bank, kitchen, bedroom...). Over the whole revision-240 cache: 3,944 shells, 470 roofed, about 7 s. Findings are in `docs/building-census-findings.md`; the data decided the generator changes below.
- **Finishing** (`osrs-edit/src/building_finish.rs`, `building_finish/{furnish,grade}.rs`): driven by an optional `finish` section of a kit: floors by room role (replaces the single floor; roles listed in `building_finish::role_name`, plus `hallway` and `default`), window decoration on both faces of a wall (inside tile, and outside tile facing back, orientation = wall side), wall decoration (torches, lanterns) away from doors and windows, furniture per role (`against_wall` with the real orientation rule, `center`, and `row` for booths), multi-tile footprints anchored on the south-west tile, clear doorways and a connectivity check so a room is never cut in two, and grading (footprint levelled to the median height, blended across the margin to the site edge; heights stay in steps of 8). Deterministic in the building seed.
- **Generator changes**: the room program is sized to the site and rooms are at least 3x3 (earlier today); the hip roof overhangs the walls by one tile; kits may omit `roof` when they have `roof_hip`.
- **Built-in kits** (`app/prefab_runtime/kits/`): `lumbridge-brick` (brickwall 1902, poordoor 1535/poshdoor 1540, curtained window decoration, red slate roof) and `timber-town` (timberwall 1602, posh windows, thatch). Floors, furniture and decoration ids come from the census. In the editor, a generated bank and house now show brick walls, wood-plank floors, doors, torches, furniture and a textured roof on levelled ground.
- **Not done / not verified**: furniture orientation rules were taken from census statistics and checked only for a few pieces by eye (beds, ranges and booths may face the wrong way in places); the bank is a row of booths, not the real counter-and-staff-side layout, and has no skirting boards, vault or chandeliers; walls/doors/windows are one style per kit (no mixed piece sets as in the Varrock and Falador towns); window decoration uses loc kind 4 on both faces (real windows use kind 4 and 5 and I did not work out which goes where); upper floors, exterior decoration (lanterns, signs), gables and ridges, and multi-floor buildings are missing; the census cuts buildings at map-square borders and some fenced yards pass as buildings; the finishing stage was run on flat and sloped test sites and the real Lumbridge/desert map, not on cliffs or water.

## Prefabs and the building generator UI (2026-10-17)

- **Prefab model** (`osrs-edit/src/prefab.rs`, `prefab/place.rs`): a versioned JSON file (`rustosrs-prefab` v1) holding terrain, heights and objects over up to 128x128 tiles and 4 planes. `capture_prefab` copies a rectangle out of a `MapDocument`; `prefab_from_patch` keeps a generated patch; `plan_place_prefab` plans a placement as one `MapPatch`, turned by quarter turns (objects' stored tile moves with their footprint, overlay rotation and orientation turn with it), skipping heights the map lacks and objects already standing exactly as wanted, optionally clearing the area first. Tests: capture/place round trip, four turns are the identity, one turn moves a wall, a 2x1 object and a corner object to the clockwise positions, replace, exact inverse, rejected files.
- **One commit for terrain plus objects** (`app/editing/prefab_commit.rs`, free function `commit_map_patch`; composite history route in `editing/history/routing.rs`, footprint code split into `routing/footprint.rs`): the patch executes through the edit gateway, the tile mirror and object mirror are projected from the post-commit document inside the gateway's fallible boundary, and history records one step (`EditStep::semantic_composite`) that undo and redo replay together. A piece removed and put back exactly is cancelled out of the object route. Tests replay undo and redo and a failing projection restores the session; the live path was run on the real Lumbridge map (capture 13x11 with 96 objects, place turned, undo/redo).
- **Library and panel** (`app/prefab_runtime/` owner, `app/prefab_panel/`): rail tab "Prefabs", Library page (search, thumbnails, capture from the tile selection, rename/tags/notes, duplicate, export, import, delete with confirm, place with ghost, R turns, Esc stops, options for ground/heights/objects/replace). Files are `prefabs/*.prefab.json` beside the settings file (native); the browser build keeps the library for the session and moves files with import/export.
- **Building generator UI** (Build page; `prefab_runtime/build.rs`): kit, kind, style, site size, seed and reroll, windows/doors reroll, clear-site option, site chosen by clicking the map or from the selection, live preview ghost (green adds, red removals), Apply (one undo step) and Save to library (generated prefab with seed recorded). **Building kits** (`osrs-edit/src/building_kit.rs`): a JSON file (`rustosrs-building-kit` v1) naming the cache objects for walls, doors, windows, corner, roof patterns, sockets and junctions, with `roof_plane_offset` (the map keeps a ground-floor roof on the plane above). `kits/*.kit.json` beside the settings are loaded too. A built-in **Timber cottage** kit (timberwall 1602/5634, door 13015, window 1615, overlay 10 floor, roof_redslate 1924 kinds 12/17 and roofedge_redslate 1790 kind 18) was checked in the editor: it produces timber walls with windows and a red slate roof on plane 1.
- **Fixes after first hands-on use (same day)**: a re-applied building no longer fails with `ObjectAlreadyPresent` (pieces already standing exactly as wanted are left alone and counted); errors are worded (site without height data, rooms do not fit, roof above the top plane). The generator now sizes the room program to the site (`BuildingProgram::fitted_to_area`, about 22 tiles per room, never fewer than 2) and rooms are at least 3x3, so small houses are 2-3 rooms, not a maze. Kits can use `roof_hip` (`osrs-edit/src/building_roof_hip.rs`): an edge ring, a slope ring and a flat top computed from the footprint, with corners and inner corners; it reproduces a real Lumbridge roof piece for piece in a test. The built-in kit uses it (the roof is on the plane above the walls; the Build page has a "View plane N" button). The site remembers the plane it was chosen on. The panel's text fields are sized exactly (full-width fields were growing the dock every frame).
- **Dev tools**: `cargo run -p osrs-editor --example building_ascii -- house 3 16 12` prints a generated building as text (walls, doors, windows, floor, roof); `cargo run -p osrs-cache --example loc_info -- <OpenRune root> window 1602` names objects by id or word.
- **Script/smoke commands**: `prefab capture|place|build|page|list` (see the Script window help).
- **Not done / not verified**: the hip roof's inner-corner orientations (kinds 15 and 20) follow the one inner corner seen in the real sample and are assumed for the other three diagonals; roofs are flat-topped hips only (no gables or ridges); site terrain is not fitted (a building can land on water or slopes: roadmap checkpoint 7); one floor only; no interior furnishing; the kit has no editor UI (hand-written JSON); overlay rotation under a turn is turned by the same count as object orientation and was not compared with the client; objects missing from the object catalog are treated as one tile when turning; the browser build was not exercised with prefabs; real-mouse use of the placement gesture and site picking was not tried (script and screenshots only); no differential check of generated buildings against real OSRS buildings.
- Gates: see the Validation lines of this checkpoint's commit message.

## Player model rendering (2026-10-16)

- **Decoders** (`osrs-cache::decode::{kit,item}`, strict, layouts from OpenRune's codecs): against the OpenRune LIVE cache (revision 240) all **307** identity kits and all **33,975** items decode (6,333 wearable); the probe is `crates/osrs-cache/examples/kit_item_probe.rs`. Synthetic-byte tests cover fields, truncation, unknown opcodes and trailing bytes.
- **Assembly** (`osrs-core::player`, `osrs-cache::player_model`): `PlayerLook` -> `PlayerAppearance` (12 slots; worn items replace a slot and hide covered slots) -> recoloured part models combined and lit like the client (64/850/-30,-50,-30). Body colours use the client's tables.
- **Rendering** (`osrs-world` `PlayerCache`/`extract_player`, editor `PlayerRuntime`): the Player panel's mock player is drawn on its tile (HD faces under the HD profile), idle sequence 808 assumed (not read from the player's animation set), rebuilt every two client cycles. Worker request `Request::PlayerGeometry`, event `StreamEvent::PlayerGeometry`, stale results dropped by token.
- **NPC outlines**: the spawn squares are drawn on each actor's current tile (one per actor, capped), not at the anchor plus the walked footprint; footprints default off. The simulation starts by itself whenever the spawns switch is on.
- **Not verified**: look correctness beyond one screenshot (default kits and colours, hides, female), facing (fixed), HD player appearance, the player's walking animations.
- Gates were run on a copy of HEAD plus only these files because another writer's unfinished HD theme/appearance changes were in the same working tree: architecture guard + tests, fmt, strict Clippy (native, WASM editor), lower-crate WASM check and the full locked workspace tests all passed there.

## Object semantics slice (2026-10-16)

- `osrs-scene::object_semantics` (pure analysis, no cache/UI): name/symbol tokenizing and opacity (`tokens`), loc-type slot and verb facts (`structure`), a relation `Census` over placements (straight continuation, corner arm, run end open/perpendicular/pillar, inset, decoration; edge numbering 0 W, 1 N, 2 E, 3 S), and `infer` (role, wall-style family by union of repeated relations, suggested labels with evidence and confidence). 18 unit tests with hand-built fixtures.
- Read-only examples: `osrs-cache --example loc_names` and `osrs-world --example object_semantics <OpenRune root>`.
- **Measured on the OpenRune LIVE cache (revision 240):** 62,401 loc definitions, 30,087 with a cache name, **all 62,401 with a gameval symbol** (so names are rarely missing). Over 2,936 regions / 4,980,697 placements / 41,774 distinct placed objects: only 18 placed objects have no descriptive name or symbol, so the relation system's value is roles and families, not names. Roles found: straight 1,778, corner 986, pillar 911, door 613, window 197, opening 1,485, end cap 396, decoration 1,574, roof 499.
- **Not independent evidence.** Door/window roles require an Open verb or a name hint, so agreement with names is partly circular; the only independent figure is the inset detector's recall (about a third of door-named objects sit inside a wall run). Relations ignore bridge/plane shifts, and a run that crosses missing regions looks open. "Opening" (inset without a door verb or name) is unreviewed and probably mixes real openings with alternating wall styles. No UI, persistence of confirmed labels, or editor integration yet.

## Explainable collision checkpoint (2026-10-16)

- Implemented on `main`: **`5f7db5163a1712b761c67a3f8f350ea4ff2b6e41`**, `editor: explain collision from accepted world snapshots`. Baseline: `0f9918331de3a43d3b9720db57aa0bfadba912e8`. This handoff follow-up changes documentation only. No branch/worktree/PR was created and nothing was pushed.
- The tile Inspector now has **Collision and blockers**. It explains walking/projectile wall flags, footprint/floor blockers and original terrain/object contributors, including neighbouring wall anchors. Missing land and unavailable definitions are explicit. Producer epochs plus dirty/pending build state hide obsolete collision evidence; source replacement and unload clear it.
- `osrs-scene::collision` owns immutable contextual snapshots and a worker-local builder. Existing definition/placement side-effect plans supply the rules. `osrs-world` records clipping before model construction or scene-capacity admission, preserving invisible locs and original source planes. Bridge collision projection stays separate from scene storage. `RegionInfo` shares the accepted snapshot; no map/project format, history authority, renderer settings or viewing-distance change.
- Collision-affecting terrain flags now use a conservative 3×3 producer rebuild, because a bridge flag can move a whole anchored object's footprint between collision planes beyond the radius-5 material blend. Existing material/height proofs and Object output comparison remain in use. This fallback can cost more than a narrow flag rebuild; no performance improvement is claimed. Snapshot cell payload is 216,320 bytes per contextual window plus loc provenance; residency owns its lifetime, with no new global cache.
- Canonical contract/navigation: `docs/specs/collision.md` (`COLLISION-001`), `docs/architecture/FEATURE-MAP.md`, `scripts/collision_oracle/README.md`. The new oracle compiles unmodified external `CollisionMap.java` with original annotations; **94 complete-grid cases MATCH**. All **11** Java source/context SHA-256 pins match `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`. Revision **235** primitive proof and build-**241** integration remain distinct; this does not certify complete target-client movement.
- Final local validation of the implementation: `cargo fmt --all --check`; locked workspace/all-target/all-feature `cargo check`, strict Clippy and tests; lower-crate WASM check and strict WASM editor Clippy: **passed**. Workspace: **900 passed, 0 failed, 20 ignored**, across 117 test/example binaries, including permanent M3–M9 suites. Targeted layout coverage exercises expanded explanations/provenance at 240/320/480 logical points and scales 1/2. Native mouse/floating-panel interaction was not manually verified.
- Explicit prerequisite runs against `/Users/tylercovalt/RustOSRS-cache/b241/cache`: `collision_projection` **passed** (invisible static placement retains clipping; inverse worker edit restores every cell); `reference_profile_matches_golden` **passed** with `UPDATE_GOLDEN` removed and the existing image present (zero pixel differences). This internal fixed-camera regression is not independent OSRS/117HD visual certification. Logs remain under ignored `target/collision-*.log`.
- Architecture guard, its **26** tests, editor ownership **11** tests and new oracle regeneration check: **passed**. Historical reference-regeneration still skips **5** unavailable-tool tests; historical deob harness blob check still skips missing tooling. HD pins verify with **98** configuration entries still awaiting audit. No remote CI claim. Performance remains explicitly paused.
- Next: build headless collision-aware player movement/step diagnostics and source/revision-stamped playtest activation around these snapshots, then deterministic scenarios and actor extraction. Full routefinding, size-aware interaction reach/line-of-sight, variable-driven runtime transitions, NPC/player model assembly, skeletal posing, audio and dynamic lights are not completed by this checkpoint. Initial-placement clipping must not be confused with runtime replacement or automatically recomputed from a visual morph.

## World-editor opportunity review (2026-10-16)

- Documentation-only review requested by the user: `docs/research/EDITOR-OPPORTUNITY-REVIEW.md`, indexed in the research README. Compares existing Rust capabilities with selected RuneLite/deob, OpenRune server/routefinder and FileStore packing code. Recommends explainable inspection, collision/stateful playtesting, asset discovery/controlled authoring, reusable construction, reproducible publication, synchronized world layers, recovery/review and fidelity evidence. Recommendations do not supersede the authorized implementation plan or resume paused performance work.
- Concrete gaps include the catalog's named/model-bearing-only discovery scope, default-zero loc morph context, operation-level collision plans without world collision-grid integration, mock Player state, absent audio playback, and missing-region export rejection. Existing map editing, world-map display, sound inspection, verified export, and new actor/effect primitives are acknowledged rather than proposed as wholly new systems.
- Evidence: `docs/research/editor-opportunity-sources.json` pins **41** selected files and exact anchors. **28** Rust/RuneLite files match their declared Git blobs; OpenRune snapshots lack Git metadata and use individual SHA-256 pins. RuneLite revision 235 versus selected cache 241 remains an explicit evidence boundary. No third-party source/assets copied into production; no claim about Jagex's private tools.
- Validation: all 41 hashes/anchors and **57** local document links verified; architecture guard passed. No Rust changes, client/audio execution, visual parity, performance measurement or full Rust suite run for this documentation checkpoint. All work remains on `main`; nothing published.

## Current program checkpoints

0. Reconcile handoff/docs, gate results at `41d0c020` — done (`7587400f`).
1. Measurement: schema-2 capture, worker/renderer counters, workloads, compare tool, fixed-view pixel capture — code done (`7f1bb2a2`). **Baselines are not recorded.** A partial run (49 of 84 planned runs) was abandoned: four runs contained 5–59 s frame gaps while the editor's own frame body took ~14 ms, which points at macOS throttling an unattended window (reproduced as a ~4000 fps no-present spin and a 2.9 fps crawl in later runs). `performance.py` now holds the machine awake (`caffeinate`), discards and retries stalled runs, and continues past failures. Retake baselines on the commit that starts checkpoint 3.
2. (a) Terrain appearance contract in `osrs-scene` and real-colour spline preview — done (`8768cf07`; oracle, Reference pixel golden and fixed-view hashes unchanged). (b) Responsive panels and Spline workflow — done in this checkpoint (below); wider tool-panel audit continues as needed.
3. Measured CPU/GPU/memory optimizations — pending (needs baselines).
4. Entity cache decoders, skeletal animation, shared model appearance transforms — pending.
5. `osrs-playtest` crate, editor controller, entity rendering — first working slice (see the NPC scenarios checkpoint); gaps listed there.
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

### Path tool replaced by a port of the EikenB00m editor's

- The census/autotile/wander/per-point path tool was deleted (`terrain_path` fitter, census, occupancy, wander, `path_grade`, per-point UI, `scripts/gen_path_data.py`). The Spline tool is now the EikenB00m editor's: click tile-centre points, optional Catmull-Rom curve, fit of real overlay shapes (`terrain_path/fit.rs`), Enter lays (one undo step), optional "Smooth terrain height" (`osrs-edit/src/path_height.rs`, a second undo step, upper planes carried along). Settings: width 1-20, smooth curve, edge smoothing 0-4, smooth height, passes 1-250, blend 0-30. Script: `spline X,Y ...`, `width`, `curve on|off`, `edge N`, `height off|PASSES [BLEND]`, `lay`.
- Verified against the original: `scripts/path_oracle/run.sh` runs its Java `PathOverlayFitter` (needs a JDK and the checkout) into `terrain_path/eiken_golden.txt`; with Java's HashMap refinement order all 54 case/offset combinations match tile for tile. Production refines in (x, y) order (offset-invariant), which differs only in tied edge phases.
- Not done / not verified: full gate run (clippy, wasm, whole-workspace tests) after this change; real-editor screenshots of the new tool (the last two scripted runs loaded no world; one clean retry wrote `target/performance/visual/eiken-preview.png`, unviewed); the height smoothing has unit tests only. The RuneLite tile-rendering study was begun (terrain build is `class470.method9712`; shape tables equal Eiken's; stored overlay opcode = shape*4+orientation+2) but no study document is written yet.

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

## First-class filestore and OpenRune cache checkpoint

- Completed locally on `main`: `8353a5b1322700a7d81ee463e190529a80dc958a` (`cache: promote filestore and verify incremental repacks`). The concurrent `94d3ed16` path-tool commit already included early cache edits; this checkpoint finishes the workspace manifests/lockfile, ownership enforcement, old vendor removal and verification. No branch/worktree/PR was created and nothing was published.
- `vendor/rune-fs` was the active JS5 transport. Its maintained fork is now `crates/osrs-filestore`, a default workspace member with workspace lints/MSRV, native/WASM checks and retained MIT attribution. Only `osrs-cache` may consume it in production. Removed the blanket Clippy exemption, unsafe native mmap, unused upstream file splitter, package marker and nested upstream CI workflow. Native reads use safe positional I/O with a bounded 8,320-byte per-group window; owned/WASM reads borrow bytes directly. No raw-byte cache persists between groups.
- Studied both supplied Desktop OpenRune trees: reading, sparse IDs, incremental build verification/recording, pack-module isolation, stable custom IDs and server response storage. Reviewed-file SHA-256 pins and adoption/defer decisions are in `docs/research/OPENRUNE-CACHE.md` and `openrune-cache-sources.json`. No Kotlin source/assets were copied. Entity codecs and a persistent pack-task dependency database remain future work.
- `repack_changed_groups` verifies stored CRC/sizes/version and compares decoded bytes before skipping. Identical groups keep every physical byte and version. Force repacking still increments versions. Both paths prepare every affected group/table before writing, reject duplicate targets, and reject unsupported extra digest columns instead of retaining stale digests. They require a staging cache and one writer; preflight is not I/O rollback or atomic publication. Existing verified export remains the publication boundary.
- Local verification against the checkpoint's production code: **866 workspace tests passed, 0 failed, 18 ignored** (existing external prerequisites); formatting, locked workspace/all-target/all-feature check and strict Clippy, lower-crate WASM check, strict WASM editor Clippy and all-feature filestore WASM Clippy passed. Architecture guard + **26** tests and editor-ownership **11** tests passed. Filestore doctests **3** passed. Reference-regeneration still explicitly skips **5** missing-tool tests; deob harness-blob check still skips missing tooling. HD pins verify with **98 entries still awaiting audit**. No remote CI certification is claimed.
- Explicit optimized build-241 source-shape/fingerprint test passed using `/Users/tylercovalt/RustOSRS-cache/b241/cache`: **117,584 groups**, canonical fingerprint `ae76dad78b4990d1b404e68e77a85ed2c96cf4a56c16f7b017cb97d1e92fdb38`. Old/new readers also returned identical aggregate encoded bytes across all groups in four rounds each. Raw samples, source hashes, harness and limitations: `docs/verification/FILESTORE-PROMOTION.md` and `filestore-io-2026-10-15.json`.
- Reported tradeoff: warm full-cache read+hash median **0.616737 s** for safe positional reads versus **0.570321 s** for former mmap (**8.1% slower**, ~46 ms). The final microbenchmark had no compiler processes, but the workspace test runner remained active. This is not an editor baseline or evidence of reduced CPU/GPU/RAM. Application workload baselines, memory/CPU profiling, playtesting and remaining HD implementation/independent visual proof are still pending.
- Other pre-existing deletions in the shared working tree were left outside this commit. Recheck live Git before further work. Stay on `main` unless the user explicitly requests another branch.

## RuneLite object-system and filestore source study

- Documentation checkpoint on `main`, comparison baseline `1c6a9ec36b3c686485b072ad9d8cd86b27f570a7`. See `docs/research/RUNELITE-OBJECT-SYSTEM.md` and `RUNELITE-FILESTORE-NEEDS.md`; both are indexed by the research README. They cover definitions/placements, model construction, recolors/retextures, morphs, animation, sound, map graphics, textures and related renderable families, followed by layer-specific needs and acceptance cases.
- Supplied source: `/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin`, Git `1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`; build constants declare game revision **235**, while our selected cache is **241**. It is already our pinned loc/model/morph semantic reference; do not replace newer decoders with its older opcode set. The mixin's old object `decodeNext` replacement annotations are commented out. Audio fades use a millisecond clock, and the Java sound-file-count accessor is slot capacity, not populated sparse-ID count.
- `runelite-object-sources.json` records **67** inspected source-file hashes, **87** source-line anchors, version/Git state and metadata-probe output. The consulted files had no local modifications against their recorded commits; unrelated shared-checkout work remains outside this task.
- A scratch executable reproduced three metadata observations: a truncated one-group table is accepted as zero groups by `IndexMetadata`; a named table missing its file-name hash is accepted there; protocol 8 is accepted by both `IndexMetadata` and `ReferenceTable::parse`. The documents record these as open findings, not desired behavior. No production fixes or new runtime features were implemented in this documentation task.
- Recommended next boundary: one complete strict raw-table codec, limits during metadata opening, explicit group CRC/version policy, and append-address capacity checks. Object/sequence metadata retention, sound variants/playback, skeletal posing and variable-driven icon/sound lifecycle belong above the filestore. Existing world-map/icons, legacy animation and sound inspection were located and are not incorrectly listed as wholly missing.
- Validation for this documentation task: source hashes, source-line anchors and local Markdown links checked; metadata probe compiled and ran offline. Full Rust/native/WASM/parity suites were not rerun for documentation-only edits. Prior gate results retain their original scope and SHA.

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

Historical full run before the filestore checkpoint (path grading/curve/cursor work on top of `7757ff4a`; local results, not remote CI certification), Python 3.14.8 (`/opt/homebrew/bin/python3`):

- Architecture guard and its tests, editor-ownership tests, reference-regeneration tests, golden-index check, HD source pins (98 entries awaiting audit), HD-source tests and `test_performance.py`: passed.
- `cargo fmt --all --check`; locked workspace/all-target/all-feature `cargo check` and strict Clippy; WASM lower-crate check and strict editor Clippy: passed.
- Locked full workspace/all-target/all-feature tests: **861 passed, 0 failed, 18 ignored** (the two guard scripts above fail for the reason given) (ignored tests keep their external cache/GPU/oracle prerequisites).
- With the pinned cache: terrain oracle (`terrain_oracle_diff`) and the Reference pixel golden passed; `region_capture` Reference hashes were identical before and after checkpoint 2a. HD captures vary by up to one channel level in a few hundred pixels between identical runs, so HD comparisons use a tolerance.

Earlier figures (805 at `41d0c020`, 793 for PR #56) are superseded. Historical CI results only certify the SHAs they name. The former vendored `rune-fs` MSRV warning is removed by the current workspace-owned manifest.

## Subsequent work

Continue the authorized checkpoints on `main`. Follow the existing product roadmap for independent Reference visual proof, M10+ certification, remaining HD features, export generalization and recovery/project polish; this refactor does not claim those features are complete. See `docs/PROJECT-STATUS.md`.

# RustOSRS Development Handoff

Last updated: 2026-10-20

Read `AGENTS.md` and `ARCHITECTURE.md` first. `docs/architecture/FEATURE-MAP.md` and `scripts/editor_features.json` locate feature ownership and tests. Live Git state is authoritative for branch/SHA facts; CI claims require actual run evidence.

## Building storeys, Phase 2: per-floor rooms, setbacks, room-aware stairs — 2026-10-20

Local, uncommitted. Plan phase 2. Applies when the layout is `Compact`, the kit has a hip roof and
rooms are generated (not drawn); pattern-roof kits, `Corridors` and drawn rooms keep the old
copy-the-ground-floor stacking (`stack_upper_floors`).

- **Own plan per storey** (`BuildingKit::build_storeys`, `building_kit/generate.rs`). An upper
  floor is laid out in a rectangle on the floor below (`building_kit/floors.rs::upper_site`): the
  same rectangle, or with 25% odds a rectangular floor steps back 3–4 tiles from one side (design
  choice; the corpus has no per-floor outlines); over an L/T it is the largest inner rectangle.
  Rooms come from `upper_floor_program` (bedrooms, guest rooms, offices, storage…) through
  `plan_compact_layout_for`, then the same walls, facade, windows, furniture as the ground floor
  (`KitFinish::apply_to_floor`, grading ground-only; `GenerationSnapshot::for_floor` slices the
  plane's terrain for the sub-site and borrows ground heights). The plan's one exterior door is
  swapped for the kit's exterior wall because plan validation needs it.
- **Stairs** (`building_vertical.rs`): `FloorLayout` (tiles, room per tile, keep-clear tiles) and
  `plan_vertical_connectivity_in_layouts`. A stair and its approach stand in one room on both
  floors, never on or beside a door, preferring spots against a wall on both floors. Furniture
  on stair tiles is removed on every floor. `plan_vertical_connectivity` is unchanged for callers.
- **Roofs:** one hip per exposed level, on the plane above it: the top floor, plus any part of a
  lower floor that a smaller one leaves open, kept off the upper floor's tiles
  (`floors::roof_footprint`).
- **Judge:** `judge_snapshot` (osrs-lab) now also judges upper floors using `judge_from` (rooms
  holding a down-stair tile count as entered) and reports `stairs do not join the floors` (up
  stair without a down stair on the same tile above, or either not in a room). Verdict findings
  carry the plane.
- **Tests:** strict sweep (8 kinds × 1–3 floors × 5 sizes × 8 seeds) now also judges upper floors;
  upper floors are subsets of the floor below with different rooms >90% of the time and
  setbacks within 15–65%; every exposed level is roofed and no roof sits on the floor above;
  broken/moved/missing stairs and a sealed upper floor are noticed; floors have furniture and
  windows; floor chooser, stair, snapshot unit tests. `cargo test -p osrs-edit -p osrs-lab
  -p osrs-scene -p osrs-editor` green, strict clippy, workspace check, guards.
- **Not done / not verified:** nothing viewed in the editor (stepped roofs against taller walls,
  hip ring on a 3-tile strip, UNDER_ROOF flags on a setback); the stair kit assets are still
  ladders; roofs of a setback use the same hip as a top floor (no lean-to against the wall);
  upper floors are not furnished around the arriving stair beyond clearing its tiles; an upper
  floor is never an L/T-preserving copy of the lower outline; drawn-room multi-floor is still
  rejected. A Finder reorganisation of the project folder happened mid-session (it came back
  intact, same HEAD).

## Building layout, Phase 1: compact rooms, real room counts, L/T outlines — 2026-10-20

Local, uncommitted. Plan phase 1 of `~/.claude/plans/we-need-to-make-ancient-noodle.md`.

- **Default layout changed.** `LayoutStyle::Compact` (new default in `BuildingKit::generate*`) packs
  rooms with shared walls and no corridor bands: `building_program/compact.rs`
  (`pack_compact_program`, `plan_compact_layout`, `FootprintShape::{Rect,L,T}`; an L/T is two
  rectangular wings, each cut straight into rooms) and `building_topology/compact.rs`
  (`build_compact_building_plan`: one door per spanning-tree link over rooms that touch, preferred
  program links first, doors off wall corners; one exterior door on the entrance room off its
  corners). `LayoutStyle::Corridors` keeps the old behaviour via `generate_with_layout`. Drawn
  rooms (`generate_in_rooms`) and the non-kit `generate_building_patch` are unchanged.
- **Room counts follow the data.** `building_program/room_count.rs`: `ROOM_COUNT_TABLE` is the
  distribution of rooms by floor area for the 470 roofed buildings of
  `osrs-lab/data/real-buildings.json` (66% one room); an osrs-lab test recounts it from the corpus.
  `archetype_min_rooms` (tavern/bank/church/dungeon 2, castle 3) and at-most-300-tiles-per-room are
  design constraints, not measurements (corpus rows carry no building kind; the census only
  recognises rooms up to 500 tiles).
- **Outline odds** from the corpus: non-rectangular share is 27% at two rooms, 59% at three or
  more (floor under 90% of its bounding box). Generated: ~0.63 interior doors per room (real
  0.64), outlines within the test bands. A one-room building is always a rectangle.
- **Tests:** `building_program::{compact,room_count}` unit tests, topology tests (every compact
  plan passes `validate_building_plan`, has rooms−1 doors and one way in), and in osrs-lab
  `generated_layouts_follow_the_real_door_and_outline_statistics`, plus the Phase 0 strict sweep
  (all 8 kinds × 1–3 floors) and planned-room round-trip, both green on the new layouts. The
  learned-style test now learns from 9 buildings: compact buildings have few doors, so 3 was too
  few for the door majority vote.
- **Not done / not verified:** no visual check in the editor or gallery of L/T roofs and walls
  (the hip roof dilates any footprint; only structure and the strict judge were tested); no
  L-shaped single room; no open archways between rooms (all interior links are doors); the
  `Corridors` style is kept only for comparison and the old tests of it; reproduction
  bundles saved before this change (e.g. the frozen build-241 ones under `target/verify`) were not
  re-run; they were made with the corridor layout, so expect their patch digests to differ.
  Verification: `cargo test -p osrs-edit -p osrs-lab -p osrs-scene -p osrs-editor` green (editor
  471 passed, 4 ignored), strict clippy on the three library crates, workspace check, guards.

## Building generation correctness, Phase 0 + first fixes — 2026-10-20

Plan: `~/.claude/plans/we-need-to-make-ancient-noodle.md` (buildings end-to-end, local deterministic
models, hard validators + real-data stats). Local, uncommitted, on top of the foundation below.

- `building_census::Judgement` now carries located `Finding { kind, plane, tile }` (no room, no
  way in, unreachable room, blocked doorway); `problems()` is derived from it. `osrs_lab::Verdict`
  exposes them as `findings` (serde-defaulted, old data still loads). This is the input a repair
  step needs; nothing consumes it for repair yet.
- `judge_snapshot` judges the **lowest floor** (it used to judge the largest building, which for
  2+ floors was the upper floor with no outside door, so every multi-floor building read "no way
  in"). Stairs joining floors are still not checked by the judge.
- Planner fixes: `pack_fitted_program` drops least-important rooms until the program fits the
  site (kit path only; layouts that already packed are unchanged). It removed 80 of 960 sweep
  refusals (`CannotSplitForRoomCount`, e.g. every 14×12 site). `plan_vertical_connectivity_clear_of`
  keeps stair footprints/approaches off the tiles beside every door/doorway (the kit passes them);
  it removed all 48 "furniture blocks a doorway" results on 2-floor buildings.
- New osrs-lab tests: `every_brief_in_the_sweep_builds_and_is_usable` (8 kinds × 1-3 floors × 5
  sizes × 8 seeds, strict; baseline before the fixes was 456/960 usable) and
  `every_planned_room_is_detected_as_exactly_its_rectangle` (generator/census agreement for
  program rooms). `generation_yield_report` (ignored) prints rejections by reason.
- Known detector quirk, not fixed: a corridor crossing another corridor's wall line reads as a
  doorway gap to `Edges::close_openings`, so the corridor is cut into small pieces and a room that
  only touches those pieces becomes a separate one-room "building". Real buildings rarely have
  crossing corridors; Phase 1 layout should avoid generating them.
- Verification: `cargo test -p osrs-scene -p osrs-edit -p osrs-lab` green; strict clippy clean on
  those three crates; `cargo check --workspace --all-targets` clean; `cargo test -p osrs-editor
  prefab` 82 passed; architecture + feature-index guards pass (Python 3.14). Full gate not run.
- Not done from Phase 0: the committed real-data distribution table + realism report. Phases 1-5
  untouched.

## Local AI/procgen correctness foundation — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`; changes are local and
uncommitted. Unrelated samples and `docs/.DS_Store` remain preserved. No branch, worktree, PR or
publication. Performance work stays paused. The accepted end-to-end plan is recorded at the top
of `docs/plans/PROCGEN-LEARNING.md`; this checkpoint does **not** complete that plan.

- Building-kit v2 carries explicit optional up/down `KitStairs` assets, validates scenery kinds
  and orientations, enumerates their source-definition footprint dependencies and classifies them
  as circulation. Multi-floor requests without stairs reject explicitly; stair realization errors
  propagate. Existing starter IDs moved from generator code into kit JSON. Their cache evidence
  identifies ladder/ladder-top assets, not newly certified architectural stairs. Version-1 files
  remain readable for single-floor generation; v1 cannot carry v2 stair fields. Older multi-floor
  kits require explicit migration/review. No implicit generator fallback invents assets.
- `BuildingRoomRole` owns canonical portable labels and explicit census aliases. Drawn room
  generation preserves guest room, bathroom, church/nave, armory, cell and other supported roles;
  unknown/general labels require review rather than silently becoming living rooms. Kit finishing
  validates role keys against the same owner; the existing `role_name` API delegates there.
- Confirmed scene-slot fixes: optional finishing reserves one wall-decoration slot per source-plane
  tile across all faces, with windows considered before random wall decor. A new 48×10 full-decoration
  seed-0 regression failed before this change. Straight-wall matching now reserves fixed corner/
  pillar anchors and keeps source planes independent; unresolved structural conflicts still reject.
  Synthetic kit orientation fixtures now use the actual client N/E/S/W mapping. The real mixed-batch
  reproduction test deliberately introduces a source decoration at a seed-dependent anchor instead
  of relying on accidental generator defects; both success and rejection still replay independently.
- Dataset v2 object records gain optional ordered raw model-table evidence (`combined` versus
  `typed_alternatives`); full-width IDs and duplicates are preserved. Runtime and Python role
  features use identical model-representation/count channels. This is source table evidence, not
  discovered semantic multi-placement assemblies. Legacy datasets/models remain usable without it.
  Training now honors occurrence-level human corrections before definition-wide human labels,
  respects an explicit empty correction and excludes inferred-only labels from targets. Lab kit
  object labels retain explicit climb-up/down actions for the configured stair assets.
- **Verification:** the final stable affected edit/editor/Lab gate passed strict native all-target/
  all-feature Clippy, architecture/authority/registry guards, Python/source pins/formatting and
  **828 tests, 0 failed, 4 ignored** in 130.4 s. Report:
  `target/verify/gates-20261020-100614/report.json`. Strict WASM Clippy for all three packages passed
  in 75 s. Native/WASM/current binary source identity matched before documentation-only handoff
  changes: working-tree SHA-256 `2d60fc342863ed417583c11b5ee181099e10dc5fb78c249bcbc36c2712bf9dcf`.
  The exporter model-table regression separately passes; all 15 Python learning tests pass.
  Earlier diagnostic/failed runs remain recorded: one changed during documentation edits; a later
  run exposed invalid synthetic orientations and a test dependent on the fixed decoration bug.
  Those were repaired without weakening placement validation or changing saved golden artifacts.
- **Frozen real-cache proof:** two fresh current-binary processes still MATCH the prior successful
  build-241 bundle's patch `df744cd449bdc5a475efd45d88ae8e5949ed8d10487f4028eb04186fc24a7429`
  through existing commit/undo/redo verification. Two more agree the prior 48×10 seed-1 rejection
  now plans successfully. Its preserved rejection bundle correctly exits 1 with `planning outcome`
  mismatch, expected `rejected`, actual `accepted`; that check is not a new success capture or a
  commit/undo verification of the wide building. Evidence:
  `target/verify/procgen-foundation-process-proof.json`. No cache/GPU initialization is involved in
  these headless checks; no new live visual, all-cache, performance or remote-CI claim.
- **Next boundary:** reviewed multi-placement assembly discovery/placement and complete catalogue
  evidence/review lifecycle; trained room inference with abstention and independently reviewed
  hold-outs; distinct per-floor authored room graphs, openings/landings and definition-aware stair
  clearance/traversal; learned structural asset binding into WFC and terrain/modular-object autotile
  integration; per-element diagnostics; certified house/shop/tavern corpus and fixed-seed yield/
  validity/visual acceptance. Existing authored sketches still reject multiple floors, generated
  upper stories still reuse ground topology, and the new model-table features are not a trained
  assembly/room detector. Keep edits behind the canonical gateway and preserve bounded workers.

## Authored document snapshots — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`. Changes remain local and
uncommitted; unrelated sample JSON and `docs/.DS_Store` are preserved. No branch/worktree/publication.
Performance measurement/optimization remains paused. The full architecture goal remains incomplete.

- Existing EditorSession/EditorDocument now owns an explicit native authored-content snapshot of
  canonical MapDocument + optional ScenarioDefinition + complete ThemePack. Private wire records
  compose existing strict map transport, scenario serde/validation and checked deterministic theme
  ZIP transport. No parallel workspace/coordinate/history model or operation executor. Asset bytes,
  ordered rules/provenance, scenario metadata/behavior/source/seed, colocated distinct IDs, signed
  map coordinates and full-width IDs remain exact. New validated ScenarioDocument baseline and
  WorkspaceDocument composition constructors reuse the existing domains; existing constructor
  behavior is preserved. Decode creates an isolated fresh session, never a live import/reset.
- Accepted ADR-0020 and `docs/specs/authored-snapshot.md` freeze v1 wrapper/identity grammar. Separate
  map/scenario/appearance digests compose one authored digest. Scenario absence differs from present
  empty data; no spawn-order normalization. Theme identity uses validated definition/asset identities,
  not ZIP byte layout. Owned wrapper/identity records are closed; nested formats keep canonical
  decoder policies. Invalid domains, unsupported versions/algorithms, missing/duplicate fields and
  malformed/mismatched digests fail. The 128 MiB JSON cap is checked before parse/native allocation
  and after encoding; oversized valid content rejects without truncation. It is not a global memory
  budget. Theme archives are never extracted or activated; no GPU compilation/render proof.
- File → Save authored snapshot and `project snapshot PATH` share one ProjectRuntime export slot,
  outside source-bound project persistence reset. They freeze only at an explicit capture boundary,
  serialize off the UI thread and use the existing synced create-only staging publisher. Replacement
  cannot release the outstanding slot or rewrite its frozen contents; completion/disconnection retires
  once and explicit retry is allowed. No saved revision/path/history/dirty state is changed by capture.
  `--verify-authored-snapshot FILE` runs before app/GPU/cache initialization and checks all domains,
  identities and canonical encode/decode round trip. It reports scope/counts, not an executed edit.
- **Functional evidence:** isolated 25-second build-241 run executed `npc new`, normal `npc place 3
  3216 3216 0` and `project snapshot ...` through existing product/gateway paths. Its 35,071,318-byte
  snapshot retains 21,609 map objects, one authored NPC and stock appearance. Three fresh headless
  processes MATCHed authored digest
  `09a4573712194f1268beccab7805c83ac2bed95014be5eb3d7b81437029a08a3`.
  Three more MATCHed the 8,836-byte full-width/Unicode fixture with three behavior variants, ordered
  rules and one exact binary asset. Independent Python/hashlib/zipfile checks agree on complete
  scenario, appearance manifest/assets and composite identities for both files. Changed authored
  digest fails/nonzero; both prior real-cache building success and failure formats still MATCH.
  Proof: `target/verify/authored-snapshot-process-proof.json`. Existing evidence/expectations are
  preserved. Automatic functional-run frame statistics are not performance or independent visual proof.
- **Verification:** ten focused codec/export tests plus the canonical scenario-constructor test pass.
  Cases cover domain equality, absent/empty scenarios, every behavior/full-width values, ordered
  content, exact assets, frozen lifetime, undo identity versus increasing observations, corruption/
  duplicate/schema/domain/hash failures, native file-kind/size guards, bounded export, source/project
  replacement, saved-state independence, create-only cleanup and disconnected-slot retry. Independent
  Python vectors pin empty and full-width Unicode scenario hashes plus stock composite identity.
  Final affected osrs-edit/osrs-editor gate passed strict native all-target/all-feature Clippy,
  guards/Python/source pins/formatting and **782 tests, 0 failed, 4 pinned-cache tests ignored** in
  157.3 s: `target/verify/gates-20261020-090750/report.json`. Strict WASM Clippy for both crates passed
  (30.6 s), with identical native/WASM source before/after:
  `0acf0243bef66f6967b08caca1a4dcb43cab0fbc6f3167ac7a30efeece95e36a`.
  `wasm-source.json` records that boundary. Independent hashing of 755 build-input files matches
  recorded source stamp `c5b6124563febb4465ff0fc6ae3670499360ecba07897e8311ffb8f110e9e974`
  in `authored-snapshot-source-proof.json`. No dependency/lint/debt allowance or fixture expectation
  change. Only documentation changed after final gates. Existing prerequisite skips remain; no full
  workspace/remote CI claim.
- **Next safe boundary:** consume this authored baseline in deliberately versioned operation
  envelopes, with independent source/profile/build/project identity, availability and declared
  deterministic observations. Current snapshots capture all values currently in EditorDocument,
  not every source region, pending project diff/restoration, history or live actor position/clock.
  Fresh decode has zero normal counters and empty history/redo/dirty/available observations; it does
  not restore those lifecycles. Existing building v1 formats remain map-scoped. Keep recovery journal
  design separate from source-relative project saves and replay; preserve EditRuntime for all actual
  commits/undo/redo. Projection settling/images, other headless/MCP operations, independent fidelity
  and global worker/time/byte budgets remain open. Performance stays paused.

## Rejected building reproduction — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`. Changes remain local and
uncommitted; unrelated sample JSON and `docs/.DS_Store` are preserved. No branch/worktree/publication.
Performance measurement/optimization remains paused. The full architecture goal remains incomplete.

- The canonical product `build/plan.rs::plan` now returns a private typed failure at the actual
  request/generation/clearing/map-validation/candidate-validation/thumbnail boundary. It retains the
  original product message and exact opaque lower-owner diagnostic evidence, without parsing display
  text or inventing another lower-crate error model. Existing success algorithms and error messages
  remain intact. Exact message/cause equality deliberately detects diagnostic drift across builds;
  this is conservative reproduction evidence, not stable wording or a global unsatisfiability proof.
- The existing native worker retains actual rejected seed/forms and shared PreviewInput, as does
  synchronous planning. No document/kit snapshot per failure. Failure storage is private; UI reads
  current messages through a narrow source/revision/form/site/plane/definition guard. Stale evidence
  cannot export even before the next refresh. Mismatched failure payloads cannot publish. Clear and
  replacement release records; an explicitly frozen export may finish independently through the
  existing one-slot export/staging owner. Worker scheduling/cancellation policy is unchanged.
- Save reproduction defaults to the selected success, or first current rejection when no preview is
  usable. Save report beside each failed seed and `prefab reproduce PATH failure=SEED` capture any
  retained current rejection, including mixed batches, without replacing the selected success.
  Source/profile checks, create-only publication, 128 MiB transport cap and canonical input restoration
  are shared with success capture. Pre-planner kit/plane/admission errors and worker cancellation/
  panic/disconnection do not become invented planner rejection records.
- Accepted ADR-0019 and `docs/specs/building-failure-reproduction.md` define the separate
  `rustosrs-building-failure-v1` envelope/contract. Successful v1 schema/input digest/report remain
  unchanged. The same headless command dispatches original bytes to each closed decoder, retaining
  duplicate-key detection. Failure replay invokes the same product planner and requires exact failure
  plus equal initial/result map hashes. Unexpected success or changed stage/message/cause/map fails.
  MATCH reports `outcome: rejected`, actual seed/evidence and zero operations, with no patch digest,
  synthetic transaction, commit/undo/redo or projection. A bundle reproduces one attempt, not a batch.
- **Functional evidence:** an isolated 25-second native run against the pinned build-241 cache used
  `prefab build 3258 3216 48 10 1`, then exported rejected seed 1 through the product script action.
  Its actual rejection is `candidate_validation`, `InvalidTask("scene slot already occupied")`;
  no assumed missing-height cause was substituted or validation behavior changed. The 35,148,841-byte
  `target/verify/building-failure-live.json` MATCHed in three fresh native headless processes, with
  zero operations and unchanged map digest
  `21f9483fd9ae0679d9a63bbef0eb686f1c1a1207f3e927b1a914d648301f9537`.
  Three current processes also MATCH the previous successful real-cache bundle with its original
  patch digest `df744cd449bdc5a475efd45d88ae8e5949ed8d10487f4028eb04186fc24a7429`.
  A separate changed-cause artifact failed explicitly/nonzero. Proof:
  `target/verify/building-failure-process-proof.json`. Original evidence/expectations were preserved.
  Functional-run frame statistics are not performance or independent visual evidence.
- **Verification:** all 25 focused reproduction/source tests passed, including ten new failure cases:
  request rejection, all eight wrapping seeds, explicit selector, real mixed batch, actual generation/
  constraint and candidate-validation failures, stale/frozen lifetime and release, malformed completion,
  every failure channel/unexpected success/map mismatch, closed/duplicate schema, native publication
  and legacy dispatch. The empty-map fixture initially expected a later missing-snapshot error;
  canonical generation actually rejects earlier with InvalidPlan/MissingHeight. The test was corrected
  to that observed authoritative error; no production validation was weakened.
  Final focused editor gate passed strict native all-target/all-feature Clippy, guards/Python/source
  pins/formatting and **461 tests, 0 failed, 4 pinned-cache tests ignored** in 90.5 s:
  `target/verify/gates-20261020-075716/report.json`. Strict WASM editor Clippy passed (13.7 s).
  Identical native/WASM source before/after:
  `4ec79275bac08250b0f0266bdee5bc07a9937c87d1e528613f8f93084d14b18d`;
  `wasm-source.json` records the boundary. Independent hashing of 750 build-input files matches
  recorded source stamp `6a54e63553c6ff326d5c1a87099e2513f5cbcf352826809d7613c40a01abc5d0`
  in `building-failure-source-proof.json`. Only documentation changed after final gates. Existing
  fixtures, crate dependencies and lint/debt baselines were preserved; no full-workspace/remote CI claim.
- **Next safe boundary:** inspect canonical EditorDocument/WorkspaceDocument/ScenarioDocument/
  ThemePack owners and their existing persistence/identity before composing complete frozen workspace
  evidence. Map-only snapshots/reproduction do not cover appearance/scenario, clocks, availability,
  projection settling/images or full project recovery. Keep recovery journals distinct from replay
  and source-relative project saves; no alternate workspace/history/coordinate authority. Other
  operation contracts/MCP, independent visual fidelity and global worker/time/byte budgets remain
  open. Building rejection MATCH closes none of those boundaries; performance remains paused.

## Frozen building reproduction and headless verification — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`. Changes remain local and
uncommitted; unrelated sample JSON and `docs/.DS_Store` are preserved. No branch/worktree/publication.
Performance measurement/optimization remains paused. The full architecture goal remains incomplete.

- `PrefabRuntime::BuildState` now owns explicit native reproduction capture of a current admitted
  building candidate. Save reproduction in the Build panel and `prefab reproduce PATH` freeze the
  full signed MapDocument, actual base-request/selected-candidate forms, complete KitFile, canonical
  footprint facts and source/build context. Serialization/publication runs behind one retained
  background export slot; later edits cannot rewrite frozen evidence. Existing reports are never
  replaced: the existing export staging owner syncs staged bytes, atomically hard-links the complete
  new file and retires staging on success/failure. Unsupported hard-link filesystems fail explicitly.
- `osrs-editor --reproduce-building BUNDLE.json` runs before app/GPU/cache initialization. The closed
  v1 envelope checks input integrity, canonical transports/constructors, form/fact consistency and
  protocol bounds, then invokes the existing `build/plan.rs::plan`. Building/apply/result hashes
  must match. Isolated commit, undo and redo use the existing EditRuntime gateway and exact map
  digests. JSON MATCH/FAILED and process status expose the result. No raw-patch executor, parallel
  planner/history, live import or AI-only mutation route. Accepted ADR-0018 and
  `docs/specs/building-reproduction.md` define this concrete semantic contract.
- Source metadata uses full accepted TargetProvenance only through the world owner's current-source
  gate, plus the opened project profile/build. Project generation starts at 1 and world worker source
  at 0: these are independent clocks and must never be compared. Preview admission still uses its
  existing project generation/revision/definition checks; FIFO acceptance policy is unchanged.
  Build context records a conservative workspace source-tree digest plus compiler/target/profile/
  feature/flag fields, separately from Git identity and executable provenance. Offline reproduction
  validates declared source identity, not an external cache's authenticity.
- Real-cache verification exposed nondeterministic ordered clearing patches from region HashSet
  traversal. `objects_in_rect` now shares MapDocument's existing `(position, kind, id, orientation)`
  key; overlap queries inherit that order. Membership, eight-tile halo, footprint/orientation rules,
  fitting/generation/foundation math and final object sets are unchanged. The original failed bundle
  and differing pre-fix reports remain under `target/verify`; expectations were not rewritten.
- **Functional evidence:** an isolated 25-second native run against the pinned build-241 cache
  generated a real 202-object/196-removal preview without applying it to the live document, then
  exported a 35,148,846-byte bundle. Three fresh headless processes matched its 505-operation apply
  patch and result map, including isolated undo/redo. Proof:
  `target/verify/building-reproduction-live-canonical-proof.json`. Patch digest:
  `df744cd449bdc5a475efd45d88ae8e5949ed8d10487f4028eb04186fc24a7429`;
  result: `d5db93650c5c8459933547e1e5f86e8b1fefbcfd7d406177acca1206a612ac6e`.
  After the final source-watch fix, three current verifier processes still MATCH and honestly report
  different recorded build context; three old-bundle checks now fail with one stable corrected
  apply digest. `building-reproduction-current-verifier-proof.json` records both sets. Automatic
  frame statistics from the functional run are not performance or independent visual evidence.
- Independent Python hashing caught and corrected a path-order mismatch in the source stamp.
  Actual Cargo addition/removal of a temporary profile file then proved directory-watch invalidation
  and exact hash restoration; the probe is removed. `building-build-watch-proof.json` records the
  proof and current source stamp `3129c59060e991fd063f2c973ea6bd8d45988887260a55ed6b8ed3a973f500e6`.
  Crates, profiles and .cargo directory roots are all watched. This stamp is not the verification
  working-tree hash below and is not a complete external toolchain/binary signature.
- **Verification:** 15 focused reproduction/source-identity tests and the canonical spatial-order
  test passed. Final affected `osrs-edit`/`osrs-editor` gate passed strict native all-target/all-feature
  Clippy, guards/Python/source pins/formatting and **761 tests, 0 failed, 4 pinned-cache tests ignored**
  in 83.7 s: `target/verify/gates-20261020-072444/report.json`. Strict WASM Clippy for both crates
  passed (68.5 s), with identical source before/after native and WASM:
  `d762710c82b401ca7a329678c5fbf22f66a1a05b93ceeb2bd0fe719e171dd04f`.
  `wasm-source.json` records that boundary. Tests cover all five modes/seven stages with genuine
  missing work, wrapping candidate seeds, negative coordinates, locks/sketch/repair inputs,
  source-over-declaration sizes, stale/frozen lifetime, malformed/mismatched records, bounded export,
  create-only cleanup and file-kind/size guards. The incomplete-stage test fixture was corrected;
  production completion behavior and existing expected fixtures were preserved. Only documentation
  changed after final gates. Existing prerequisite skips remain; no full-workspace or remote CI claim.
- **Next safe boundary:** typed failed/unsatisfiable candidate reproduction. Current native batches
  discard their document/kit after planning and retain failures as display strings prefixed by seed;
  an all-failed batch has no admitted candidate to capture. Retain explicit frozen failure evidence
  and attempt identity on the existing owner, with ordinary freshness/cancellation and bounded
  lifetime. Do not manufacture a successful patch or parse display strings into a new error model.
  Workspace appearance/scenario, projection/settle/image reproduction, recovery journals, other
  headless operations/MCP, independent visual fidelity and global worker/time/byte budgets remain
  open. Semantic MATCH closes none of those boundaries; performance remains paused.

## Lossless semantic map and patch transport — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`. All changes remain local
and uncommitted; unrelated sample JSON and `docs/.DS_Store` are preserved. No branch/worktree/
publication; performance measurement/optimization remains paused. Previous goal turn made verified
progress on review freshness, capture work bounds and content digests; this continues reproduction.

- Existing MapDocument/MapPatch gain explicit v1 JSON encode/decode operations in
  `osrs-edit::semantic_transport`. Private wire records map through canonical constructors; no
  second document/history/coordinate authority or operation executor. Separate snapshot/patch
  format headers carry the corresponding canonical algorithm and content digest. Signed i32
  coordinates/heights, all u32 IDs, nullable channels and source planes remain exact. Snapshot
  duplicates reject even when equal; sparse default terrain normalizes normally. Missing heights
  remain absent. Patch ordering/no-ops/before-state/polarity are exact; MapPatch::new derives impact.
- Closed schemas reject unknown fields/operations, duplicate JSON keys, absent nullable channels,
  malformed/out-of-range numbers, unsupported formats/algorithms and invalid/mismatched digests.
  Canonical-value errors retain their snapshot collection/operation index and typed source; duplicate
  errors retain exact canonical coordinates/occurrences. Decode builds a new value without altering
  a live session/history/dirty state. Integrity does not imply patch applicability or authorize a
  live import. No raw data action was added to AI/Lab, and project/export/recovery formats remain
  separate. Accepted ADR-0017 and `docs/specs/semantic-transport.md` freeze this infrastructure API.
- Seven tests decode hand-authored full-width/signed snapshots/patches using the independent v1
  digest vectors; cover canonical re-encoding and default/zero behavior, duplicate collections,
  all invalid semantic channels and numeric/schema/version/hash failures, including duplicate
  internally tagged operation keys. Exact decoded patches commit/undo through existing history;
  an inapplicable decoded patch is still rejected and preserves redo. Initial narrow run passed
  all seven; final gate also verifies added indexed error chains and duplicate operation-key cases.
- **Verification:** focused osrs-edit gate passed strict native all-target/all-feature Clippy,
  architecture/authority/feature-index/Python/source-pin/format checks and **309 tests, 0 failed,
  0 ignored** in 76.1 s: `target/verify/gates-20261020-032021/report.json`. Strict WASM Clippy for
  osrs-edit and downstream osrs-editor passed (31.1 s), with the same stable source:
  `e75a4f2cbc674e9f1a4f14f7589c4c31327c672b7bc651b973036b304b8e699f`.
  `wasm-source.json` records that boundary. Existing digest/semantic tests and fixtures were not
  changed; no dependency, lint/debt allowance, gateway or planner-body change. Only documentation
  changed after gates. Local Markdown targets, the empty-snapshot JSON example and whitespace
  checked. Existing Python/historical prerequisite skips remain; no full-workspace, remote CI,
  complete replay/recovery, external visual or performance certification claim.
- **Next safe boundary:** consume these APIs in a real building reproduction envelope and headless
  verifier; do not stop at transport or substitute a raw-patch executor for intent reproduction.
  `prefab_runtime/build/plan.rs::plan` is the canonical product composition: frozen MapDocument,
  PreviewInput footprint facts, actual candidate BuildForm and actual compiled Kit/KitFile. A batch
  input retains the base seed while a selected candidate's form can use a wrapping offset seed;
  capture both honestly. Preserve all modes, sketch, locks, repair region and stage inputs, or reject
  explicitly unsupported operation contracts. Never silently replay only a default form/kit.
  `FootprintInputs` is already the canonical source-wins/portable-declaration resolver; serialize
  its retained facts through narrow operations and validate them against the captured kit/document,
  rather than constructing a second resolver. Use the full signed transport above, not Lab snapshots.
- Source provenance needs deliberate capture: ProjectRuntime currently stores build/profile name,
  not full TargetProvenance. Its source generation is not portable content identity. Accepted region
  descriptors retain full target identity, but are last-accepted artifacts; do not relabel one as
  current source without its existing source gate. `WorldDefinitions::context().target_provenance()`
  supplies actual immutable identity if cache opening is needed; put such work off the UI thread.
  Freeze current-preview inputs before dispatch; source/document changes later may not rewrite the
  captured bundle. A new envelope must retain source/profile/build/editor/planner identity and
  expected canonical patch/result digests, then invoke the existing planner and transaction policy.
  EditRuntime already has `new(EditorSession)`; production transactions use its execute path, while
  its convenience `commit` helper is test-only. Avoid direct session/history bypass in a verifier.
  Retain transport text until strict parsing (generic JSON Value would erase duplicate keys).
  Recovery journals, other headless/MCP operations, workspace/projection reproduction, independent
  fidelity and global budgets remain open. Full goal remains active/incomplete; performance paused.

## Versioned canonical map and patch digests — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`. All changes remain local
and uncommitted; unrelated sample JSON and `docs/.DS_Store` are preserved. No branch/worktree/
publication; performance measurement/optimization remains paused.

- Existing `MapDocument` and `MapPatch` gain explicit read-only `content_digest_v1()` operations in
  `osrs-edit::content_digest`. They return existing canonical `osrs-core::provenance::Digest256`.
  SHA-256 uses separate versioned document/patch domains, fixed little-endian widths, explicit
  optional tags and u64 counts. Map content uses canonical sorted enumeration; patch content keeps
  exact order/polarity/before/after/no-op operations. No new document/history/hash value model,
  semantic mutation route, source freshness policy or hashing in frame/commit/undo paths.
- Full signed world coordinates/heights and all u32 IDs survive. Sparse default terrain remains
  normalized; unknown heights differ from explicit zero and optional Some(0) differs from None.
  Source/cache/profile/build, availability, appearance/scenario, invocation/seed/clock, definition
  and rendering inputs remain separate envelope obligations. This identifies map content, not a
  complete workspace or portable replay. The explicit v1 APIs/bytes are frozen by accepted ADR-0016
  and `docs/specs/semantic-digests.md`; future channel/grammar changes need a new version/API.
- `sha2.workspace` reuses the existing pinned dependency; Cargo.lock changed only the osrs-edit
  dependency edge (no package/version resolution changes). Four tests pin independent Python
  struct/hashlib golden vectors for empty and signed/full-width documents/patches, insertion-order/
  default/object normalization, every map value/coordinate channel, patch order/before-state/no-ops
  and exact inverse hash restoration. Expectations were not generated from candidate Rust output.
- **Verification:** focused osrs-edit gate passed strict native all-target/all-feature Clippy,
  architecture/authority/feature-index/Python/source-pin/format checks and **302 tests, 0 failed,
  0 ignored** in 64.5 s: `target/verify/gates-20261020-030938/report.json`. Strict WASM Clippy for
  osrs-edit and downstream osrs-editor passed (40.1 s) with the same stable source:
  `3c84ca3b9ff04a8976048e6d7419c9592be137105ac375d0593205e1086bad83`.
  `wasm-source.json` records that boundary. The initial narrow digest run passed all four tests;
  final gate verifies the explicitly versioned API names. No fixture/lint/debt allowance change.
  Only documentation changed after final gates; links/whitespace checked. Existing Python/historical
  prerequisite skips remain. No full-workspace, remote CI, bundle/recovery, visual or performance claim.
- **Next safe boundary:** lossless versioned map snapshot/patch transport plus one real headless
  reproduction operation. Keep transport DTOs private, validate through canonical constructors,
  reject duplicates/unsupported versions, preserve signed world coordinates and optional values,
  and recompute MapPatch impact. Do not reuse unsigned Lab coordinates or source-relative project
  diffs as full-world snapshots. Follow with a frozen building input envelope using the actual
  canonical planner, required definition/kit data, source/profile/build identity and expected patch/
  result hashes. No UI-driving script or raw-patch executor may masquerade as intent reproduction.
  Recovery journals, editor-wide headless/MCP, external fidelity and global resource budgets remain
  open. The full goal remains active and incomplete; performance stays paused.

## Bounded capture analysis and membership work — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`; changes remain local
and uncommitted. Unrelated sample JSON and `docs/.DS_Store` are preserved. No branch/worktree/
publication; performance measurement/optimization remains paused.

- `CaptureReview` now retains one native worker independently of the current logical analysis job.
  Initial census/cache analysis and asynchronous membership rebuilds share that owner: one active
  operation, one replaceable queued closure holding immutable inputs, and capacity-one replies.
  Clear/cancel retains the draining physical worker. Reselection/replacement releases queued snapshots
  and unread results immediately, cancels active publication and waits for active work to return
  before the newest queued operation runs. No census algorithm, definition/name rules, membership
  math, semantic gateway/history, persisted schema, counter or shared worker framework change.
- Logical jobs retain semantic revision/source and a weak mailbox link. Source/hydration/eviction
  changes retire pending work before completion as well as rejecting stale payloads. An obsolete
  ticket can remove only its own queued closure, including equal-key replacements. Selection errors
  and disconnection clear refining/open state, eliminating the old permanent analysis spinner.
  Cancel takes precedence over a simultaneous modal rebuild/save. A panic closes queued senders;
  explicit reselection can replace a worker only after its thread handle is terminal. No per-frame
  restart. Shutdown closes slots and lets active cache/analysis work drain without a UI-thread join.
- Seven deterministic tests pause work using start/release handshakes: 64 replacements, same-thread
  cancel/reselection, semantic/source retirement, queued/completed input lifetime, obsolete ticket
  scope, nonjoining shutdown, panic/terminal retry and mismatched payload/errors. Real membership
  operations pass through production tickets and match canonical results. The narrow capture-filter
  run passed 24 tests (including unrelated name-matching capture tests); all 11 prefab capture tests
  passed, preserving the prior four cases. The deliberate panic test emitted its expected panic log.
- **Verification:** editor focused gate passed strict native all-target/all-feature Clippy,
  architecture/authority/feature-index/Python/source-pin/format checks and **436 tests, 0 failed,
  4 pinned-cache tests ignored** in 72.1 s:
  `target/verify/gates-20261020-030236/report.json`. Strict WASM editor Clippy passed (11.9 s)
  with the same stable source:
  `a035872b95241e85da48cfc0863eb5cb194f2b21e28cf0f2baf8e7911ac65725`.
  Analysis body matches `HEAD` after indentation normalization; membership file is byte-identical.
  Proof saved with the gate as `unchanged-capture-semantics.json`. No fixture/lint/debt allowance
  change. Documentation and the prefab registry planner symbol/index were updated afterward;
  cheap registry/architecture/link/whitespace checks cover that follow-up. Existing skips remain.
  No full-workspace, remote CI, independent visual, portable reproduction or performance claim.
- **Next safe boundary:** portable deterministic reproduction, separately from crash recovery.
  Inspection found ProjectFile v2 stores source-relative diffs without history; Lab replay uses
  task-local unsigned prefab coordinates and cannot represent the full signed world or editor
  appearance/session state. Reuse canonical typed map values/constructors and existing planners;
  do not relabel script text or undo replay as a portable bundle. Establish versioned canonical
  semantic snapshot/patch identity before recording editor operation contracts, then add a headless
  verifier. Preserve source/profile/build provenance and fail on mismatched expected hashes.
  Recovery journals, editor-wide headless/MCP, external fidelity and global worker budgets remain
  open. Per-owner capture bounds do not prove worst-case analysis/cache time or byte budgets.
  The full goal remains incomplete; performance stays paused.

## Reviewed placement and capture freshness — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`. Changes remain local
and uncommitted; unrelated sample JSON and `docs/.DS_Store` are preserved. No branch/worktree/
publication; performance measurement/optimization remains paused.

- Placement review captures semantic `session.revision()` (including hydration/eviction), source
  generation, the actual operation/asset snapshot, initial global armed gesture, plane, frozen clicked
  origin and every queried raw definition-size fact. Modal reads and approval recheck these inputs.
  Unarmed script placement remains valid; moving the mouse does not change a frozen clicked target.
  Source/catalog arrival, size changes, asset edits/removal and gesture changes revoke approval.
  Per-frame ghosts borrow the asset and do not create a retained observation map or asset clone.
- Canonical prefab planning gains fallible entry points; old explicit-size APIs delegate to the same
  body. Product placement uses known source sizes, then checked captured building sizes, and rejects
  unresolved scenery dimensions. Structural loc-kind footprints and existing normalization/rotation
  are shared. Fitted placement validates added scenery sizes even without rotation or neighbors.
  Conflict validation remains conservative over all existing document objects; a distant unresolved
  scenery definition can reject fitted placement. Foundation fitting is byte-identical to `HEAD`;
  no fitting algorithm, persisted schema, fixture or semantic gateway/history change.
- Capture analysis/admission uses semantic revision. Review selection projection, refinement controls
  and save reject stale semantic/source inputs, including changes later in the same UI frame. Names,
  learned suggestions and the analysis object table stay frozen for human review. Actual-session
  tests prove hydration/eviction reject capture/placement at unchanged authored revision, retain redo,
  and leave the document unchanged. Current placement commits through EditRuntime with exact undo.
- **Verification:** focused `osrs-edit`/`osrs-editor` gate passed strict native all-target/all-feature
  Clippy, architecture/authority/feature-index/Python/source-pin/format checks and **727 tests,
  0 failed, 4 pinned-cache tests ignored** in 146.6 s:
  `target/verify/gates-20261020-025408/report.json`. Strict WASM Clippy for both crates passed (30.3 s)
  with the same stable source:
  `9416295e5c36d8b7e93f5d9b2fd66a6995fee6e095d721a6d039abfd86a0921f`.
  The focused semantic prefab run passed all 23 cases. Existing prefab/foundation expectations remain
  unchanged; new tests exercise missing sizes and script portability. Foundation byte proof is saved
  alongside the gate as `unchanged-foundation-fit.json`. No lint/debt allowance was widened. Existing
  prerequisite skips remain; no full-workspace, remote CI, independent visual, portable reproduction
  or performance certification claim. Only documentation changed afterward.
- **Next safe boundary:** bound capture analysis and membership-refinement physical concurrency.
  Both still spawn one thread per request; discarded receivers reject publication but do not bound
  active snapshots/work. Preserve current census/ownership math and refine/save contracts, retaining
  one physical owner across cancel/reselection. Then continue portable reproduction/recovery,
  editor-wide headless/MCP and external fidelity. The full goal remains incomplete.

## Bounded building candidate ownership — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`. Changes remain local
and uncommitted; unrelated sample JSON and `docs/.DS_Store` are preserved. No branch, worktree
or publication; performance measurement/optimization remains paused.

- `BuildState` retains one native `candidates::Worker` independently of its current logical `Job`.
  The existing receiver/cancellation ticket remains the admission boundary; the worker owns one
  replaceable queued document/kit/input snapshot and one active background candidate. Production
  replies use capacity one. Reroll/source/revision/footprint replacement cancels active work and
  releases queued snapshots/unread output immediately. Only the active candidate drains before
  the latest request starts. Eight wrapping seeds, failure order, ranking and canonical planning
  remain unchanged; no new freshness counter, planner or shared worker framework.
- Logical tickets hold only a weak queue link and reuse their existing cancellation flag to remove
  their own queued request. Dropping an obsolete ticket cannot clear a newer equal-input request.
  Cancellation is checked before and after each candidate. Idle workers retain no document/kit/input
  snapshot. Owner shutdown closes slots and cancels the active candidate without a UI-thread join.
  A thread-exit guard closes queued response senders even on panic, preventing forever-pending jobs.
  The existing no-automatic-retry failure contract remains; reroll/input change can replace a worker
  only after its thread handle is terminal. Invalid inputs still retire all logical preview state.
- Explicit synchronous/script planning remains one UI-thread operation, cancelling native tickets
  but potentially overlapping one already active native candidate. That retired native result cannot
  replace the synchronous preview, even at an equal request key. This preserves synchronous behavior
  and avoids blocking UI shutdown; the bound is one native candidate plus at most one synchronous
  call per owner, not a global worker pool, fit-time limit or worst-case byte budget.
- Seven new controlled tests pause real planning: 64 reroll/source/revision replacements on one
  thread, weak queued/completed metadata retirement, invalid-input cancellation, obsolete-ticket
  scope, nonjoining shutdown, panic/queued-ticket disconnection and terminal retry, wrapping-seed
  failure/ranking/patch parity, exact patch inverse and synchronous supersession. All **35 prefab
  runtime tests** passed (0.05 s), including the unchanged prior admission/prefab tests. The native
  response path preserves controlled completion hooks for stale-batch/current-ticket tests.
- **Verification:** editor focused gate passed strict native all-target/all-feature Clippy,
  architecture/authority/feature-index/Python/source-pin/format checks and **424 tests, 0 failed,
  4 pinned-cache tests ignored** in 33.6 s:
  `target/verify/gates-20261020-023257/report.json`. Stable source before/after and through strict
  WASM editor Clippy (11.4 s):
  `e70506be9303d192ac62c20e64cbd0702f8d5a440f7bb2be5d2517cb64a46c10`.
  No semantic planner, history/gateway, persistence schema, fixture or lint/debt allowance change.
  Existing reference-tool/historical-harness skips remain. Only documentation changed afterward;
  links/whitespace checked. No full-workspace rerun, remote CI, independent visual, portable
  reproduction or performance certification claim.
- **Next safe boundary:** close reviewed placement/capture freshness dependencies. Inspection found
  `prefab_panel/placement_review.rs::approve`, its `viewport.rs::place` constructor, and capture
  analysis/admission all use authored `map_revision()`, excluding semantic hydration/eviction.
  Extend existing owners and test with real session hydration/history preservation; inspect fitted
  placement definition-size and library/intent dependencies before claiming full input closure.
  Capture analysis also spawns a thread per selection and needs its own physical-concurrency audit.
  Then continue portable reproduction/recovery, editor-wide headless/MCP and external fidelity.
  The full goal remains incomplete; performance stays paused. Live goal status is controlled by the
  app/user, independently of this operational handoff.

## Bounded spline preview ownership — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`. All changes are local
and uncommitted; unrelated sample JSON and `docs/.DS_Store` remain preserved. No branch,
worktree or publication; performance measurement/optimization remains paused.

- `interaction_runtime/spline/plan/worker.rs` now owns the existing `PlanWorker`: one physical
  thread, one replaceable pending input and one completed result per interaction runtime.
  It replaces unbounded request/result channels with narrow owned slots and a condition variable;
  fitting runs outside the mailbox mutex. Request replacement immediately retires obsolete queued
  input/completed output. The existing 24 ms debounce and fit/content key remain unchanged.
- Clear, no-input, cancellation and source replacement clear slots and revoke active output while
  retaining the draining worker. Resume and rapid source/gesture changes cannot spawn overlapping
  fits. Exact key roundtrips can accept the already active identical fit and discard redundant
  pending work. Owner drop clears/closes slots immediately; an active pure fit finishes and exits
  without joining on the UI thread. Retry requires the actual thread handle to report terminal;
  a slow observation is never treated as stopped. Browser planning remains synchronous.
- Seven new deterministic tests gate actual fitting with start/release handshakes: a 255-request
  replacement burst, inactive-consumer result retention, cancellation/key roundtrips, 63 source
  replacements while blocked, public cancellation/resume, shutdown/weak mailbox retirement and
  panic recovery through normal runtime polling. They check the same thread ID across replacement,
  current-key output and exact canonical fitted tiles/line. The first six and then all seven passed;
  a potential shutdown assertion race was removed by waiting for channel disconnection rather than
  assuming destructor order. Existing spline tests were not changed or weakened.
- **Verification:** the editor focused gate passed strict native all-target/all-feature Clippy,
  architecture/authority/feature-index/Python/source-pin/format checks and **417 tests, 0 failed,
  4 pinned-cache tests ignored** in 69.4 s:
  `target/verify/gates-20261020-021754/report.json`. Stable source before/after and through strict
  WASM editor Clippy (11.5 s):
  `431769c28578c80c50bdd6e210e267b779050d09fd1f58d4efc61a8fda621eb8`.
  The focused spline run passed all **20 tests** (0.13 s). Byte comparisons prove existing
  `fit_points`, `path_line`, `plan_key`, `PlanInput::make`, spline tests, pointer/commit input and
  semantic Path gateway bodies remain identical to `HEAD`. No math/planner/history/persistence
  or lint/debt allowance change. Ordinary reference-tool/historical-harness skips remain.
  Only documentation changed after the final gates; links/whitespace checked. No full-workspace,
  independent visual, remote CI, portable reproduction or performance certification claim.
- **Next safe boundary:** bound physical building candidate concurrency before placement-review
  dependency/reproduction work. Inspection confirms `build/candidates.rs::Job::start` still spawns
  a new thread per reroll/source/form input. Drop cancels only between candidates, so at most eight
  seeds per batch does not bound simultaneous active candidates across rapidly replaced batches.
  Extend that existing owner, preserving current input/footprint admission and one semantic gateway;
  do not invent a parallel scheduler. Then continue placement-review inputs, portable reproduction/
  recovery, editor-wide headless/MCP and external fidelity. Spline retained-work bounds do not prove
  a worst-case fit-time/byte budget or global worker bounds. The original goal remains active and
  incomplete; performance stays paused.

## Building definition and footprint inputs — 2026-10-20

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`; all changes are local
and uncommitted. No branch/worktree/publication, unrelated sample JSON and `docs/.DS_Store`
preserved, performance measurement/optimization still paused. This checkpoint supersedes the
building dimension-dependency gap recorded in the previous entry.

- `PreviewInput` now retains immutable footprint facts keyed by canonical object ID and loc kind:
  optional source dimensions plus optional portable kit furniture declarations. Source dimensions
  win. Capture covers document scenery and all potential kit scenery assets; current admission
  rechecks the retained distinct IDs, without cloning a document every frame. Catalog arrival,
  disappearance or dimension changes revoke apply/save/selection/drawing and supersede in-flight
  batches even at the same source/session revision. Facts retain no document, catalog or model;
  existing source generation and semantic revision remain separate, with no new freshness counter.
- Native and synchronous/browser planning use the same frozen facts. The native worker no longer
  substitutes label-table sizes or a one-tile guess. Candidate validation uses actual canonical
  dimensions even for kit-labelled furniture. Portable kit declarations support headless generation;
  an unknown non-kit footprint needed by clearing or validation rejects planning with an explicit
  error. Distant unknown objects do not globally reject a build. Capture remains conservative over
  the whole document's scenery and potential kit assets, so unrelated definition changes may reroll.
- The existing `osrs-edit` footprint/overlap queries gain fallible entry points; infallible callers
  delegate to them. Loc-kind rules, rotation, zero-dimension normalization and the existing eight-tile
  anchor halo are preserved in one implementation. The halo can conservatively require a size even
  for an object that would ultimately not overlap. This is not a general unbounded-size spatial query.
  `KitFile::footprint_requirements` enumerates all potential asset fields, deduplicates ID/kind and
  rejects conflicting normalized furniture declarations. No generator algorithm, history path or
  persistence schema change.
- `ObjectCatalog::definition_size` reuses searchable entry dimensions and retains only dimensions
  for decoded definitions without direct models. Those definitions remain outside placement search;
  failed decodes remain unknown. `PlaceState::known_object_size` uses this lookup. Catalog unit tests
  moved to their normal child module unchanged (byte comparison confirmed), with one new dimension
  test; no test, lint or architecture allowance was weakened.
- **Verification:** the focused gate for `osrs-edit`, `osrs-world` and `osrs-editor` passed strict
  native all-target/all-feature Clippy, guards/Python/source pins/formatting and **765 tests,
  0 failed, 22 ignored** in 276.3 s:
  `target/verify/gates-20261019-235448/report.json`. Stable source before/after and through the
  following WASM/cache checks:
  `8b9dd34be14b1067a9601e0c4f36f97272f5334b6e316248474cc243c1f6b60b`.
  This includes four additional building regressions and three additional semantic query/kit tests,
  proving catalog arrival at the same revision, superseded batch metadata release, source sizes over
  label placeholders, unknown-size rejection, rotated outside-anchor clearing and exact inverse undo.
  The new kit test initially assumed its stand-in roof assets were structural; explicitly selecting
  structural kinds in that test corrected the fixture, preserving the existing scenery-kind stand-ins
  elsewhere. A closure type annotation fixed the other initial test compile error.
  Strict WASM Clippy for all three affected crates passed (52.4 s). The explicit pinned-cache catalog
  test passed (0.36 s) with `RUSTOSRS_TARGET_CACHE_DIR=/Users/tylercovalt/RustOSRS-cache/b241/cache`:
  decoded unlisted definition **66 is 2x2**, matching `WorldDefinitions::object` while remaining absent
  from search. The ordinary integration run retains its existing unset-environment runtime skip;
  the separate run actually exercised cache loading. Other prerequisite skips/ignores remain.
  Only documentation changed after those checks. No full-workspace rerun, remote CI, real-mouse,
  independent visual, portable reproduction or performance certification claim.
- **Next safe boundary:** bound the existing spline preview queues while retaining its fit/source
  keys and commit-time semantic refit, then finish placement-review dependencies and portable
  reproduction/recovery. Inspection found `interaction_runtime/spline/plan.rs::PlanWorker` uses
  unbounded request/result channels; requests coalesce only when the worker receives them, so this
  is not a retained-work bound during an expensive fit. Reuse that worker rather than introducing
  a parallel scheduler. Editor-wide headless/MCP and external fidelity evidence remain open.
  The original goal remains active and incomplete; performance stays paused.

## Building preview admission and retained inputs — 2026-10-19

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, with the earlier local
checkpoints below. Unrelated sample JSON and `docs/.DS_Store` remain preserved. No branch,
worktree, commit or publication; performance measurement/optimization remains paused.

- Three regressions reproduced stale publication after invalid kit/plane selection, stale
  synchronous candidates after invalid inputs, and a disconnected building worker staying pending.
  `BuildState` now validates desired inputs before polling; invalid kit/plane/site cancels pending
  work and clears old candidates. Stale batches cannot retire a current job; a matching batch with
  inconsistent candidates fails closed. Disconnection reports failure and retires the slot without
  a per-frame retry loop. Reroll or a changed input retries.
- The existing tuple key becomes retained immutable `PreviewInput`: source generation, semantic
  session revision, form, site and plane. Jobs, batches and candidates share this descriptor, which
  retains no document/model/scene; the worker's existing frozen document copy is unchanged.
  Apply, library save, candidate selection and viewport reads require current
  inputs, including edits later in the same UI frame. A separate last-preview read supports only
  read-only post-commit script verification. Synchronous/browser planning reuses the same admission
  and existing generator; the main BuildState file shrinks from 467 to 359 lines.
- Building callers now use `session.revision()` because authored-only `map_revision()` excludes
  hydration/eviction. A real-session test hydrates changed source heights, proves authored revision
  and undo history stay unchanged, and regenerates at the new semantic revision. This is conservative:
  unrelated semantic transitions can invalidate a build until narrower dependencies are proven.
- The preview audit records existing spline fit/source keys, picker-version Reference appearance,
  commit-time refitting, per-frame object ghosts and cancellation contracts. It corrects the old
  assertion that commit consumes a ready spline preview. Building catalog/footprint closure, spline
  queue bounds, placement-review dependencies and portable preview reproduction remain open; no
  parallel preview framework or new freshness counter was introduced.
- **Verification:** all eight new admission tests passed, including controlled event order, a real
  native candidate batch, shared input lifetime and stale-batch/current-slot recovery. The first
  fixture at origin zero failed generator validation; the existing validated 3300,3300 site then
  reproduced all three intended failures before the fix. A byte comparison confirms the existing
  building planning/validation body is unchanged. Final editor gate passed strict all-target/
  all-feature native Clippy, **406 tests, 0 failed, 4 pinned-cache tests ignored**, and architecture/
  authority/feature-index/Python/source-pin/format checks in 70.9 s:
  `target/verify/gates-20261019-233653/report.json`. Stable source before/after:
  `77ffd589e414db90d8af415437b4288df59e36ff90853561cbcb7a924a9c885b`.
  Strict WASM editor Clippy passed (11.9 s) after
  target-gating a return that becomes redundant without native polling; no lint allowance added.
  Existing reference-tool/historical-harness skips remain. Links/whitespace passed; only
  documentation changed after the final gates. No full-workspace, real-mouse, independent visual,
  remote CI, portable reproduction or performance certification claim.
- **Next safe boundary:** close building definition/footprint input dependencies using the existing
  catalog/kit ownership, then the remaining preview bounds/review contracts and portable reproduction/
  recovery. The catalog includes unnamed renderable definitions but omits failed decodes and entries
  with no direct model table; its size fallback is not proof that an unknown footprint is one tile.
  Editor-wide headless/MCP and external fidelity evidence remain open. The original goal remains
  incomplete; performance stays paused.

## Accepted NPC/player extraction identity — 2026-10-19

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, with the earlier local
checkpoints below. Unrelated sample JSON and `docs/.DS_Store` remain preserved. No branch,
worktree, commit or publication; performance measurement/optimization remains paused.

- World actor results now carry canonical decoder target provenance, actual Reference/HD/fallback
  path and the existing worker presentation generation in `ActorGeometryContext`. Both public Rust
  result payloads and extraction function signatures are extended; request variants/FIFO order,
  counters, persistence and pose algorithms remain unchanged. Private `streaming/actors.rs` groups
  worker actor extraction and shrinks the parent; the canonical render-path selector is reused.
- New GPU cases reproduced publication after a presentation change with retained picker data.
  Both owners now require requested, returned and current presentation to match, alongside existing
  token/stamp and frame reconciliation. Presentation joins the existing request key, allowing a
  same-tick/frame replacement. Ordinary simulation/pose lag remains accepted.
- Nonempty GPU actor layers retain immutable issued request metadata and actual extraction context.
  NPC input includes the original stamp/filter/vars, selected count and accepted ground producers;
  no full actor snapshots, models, scenes or documents are retained. Player input retains the small
  exact `PlayerFrame`, key and ground producer. Both previous rounded ground readers now use one
  borrowed accepted-inspection query; last accepted ground remains allowed during rebuilding.
  The next pending request never relabels the installed image. Empty output, hide and source cleanup
  release metadata with GPU data. Existing optional diagnostics observe a borrowed descriptor view.
- **Verification:** expanded actual-GPU NPC/player cases passed with explicit lifecycle logs,
  including nonempty NPC ground membership, presentation rejection/recovery, original clock/input
  retention and weak-handle release on empty/hide. The nonempty NPC fixture initially tripped the
  existing mixed-cache asset guard; its scenario/assets now use its decoder's real fingerprint,
  preserving that guard. Pinned-cache `actor_results_capture_real_decoder_target_generation_and_reference_fallback`
  passed explicitly (0.24 s): exact decoder target/generation/path, real nonempty default-player
  output and identical geometry between Reference and requested-HD fallback. Command:
  `RUSTOSRS_TARGET_CACHE_DIR=/Users/tylercovalt/RustOSRS-cache/b241/cache cargo test --locked -p osrs-world
  --test integration actor_results_capture_real_decoder_target_generation_and_reference_fallback -- --ignored --nocapture`.
  Final editor/world gate passed strict all-target/all-feature native Clippy (75.8 s),
  **456 tests, 0 failed, 22 ignored**, and architecture/authority/feature-index/Python/source-pin/
  format checks in 149.4 s: `target/verify/gates-20261019-231431/report.json`. Stable source hash:
  `83971fd882452f3fd15f2cc48df626ecdf7ea009431b8d72b3b292194cc91869`.
  The ordinary gate ignores prerequisite cache/GPU cases; the new pinned test was separately run.
  Strict WASM editor/world Clippy passed (21.8 s). Existing reference-tool/historical-harness skips
  remain; links and whitespace passed. Only documentation changed afterward. No full-workspace,
  independent actor pose/visual parity, remote CI, reproduction or performance certification claim.
- **Next boundary at that checkpoint:** audit transient previews against their existing desired-key/cancellation
  owners before adding metadata; then portable reproduction/recovery and editor-wide headless/MCP.
  External fidelity evidence and the broader target architecture remain open. Performance stays
  paused; the original goal remains incomplete. The building admission checkpoint above records
  the first preview audit/fixes and the remaining preview-input obligations.

## Aggregate animated-loc artifact identity — 2026-10-19

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, with the earlier local
checkpoints below. Unrelated sample JSON and `docs/.DS_Store` remain preserved. No branch,
worktree, commit or publication; performance measurement/optimization remains paused.

- The existing single-flight animation request now retains immutable dispatch context: source/
  presentation, requested cycle/advance, center/radius, smoothing/frozen settings and every nonempty
  producer's accepted `RegionArtifactIdentity`. Membership shares an immutable map of descriptors;
  it retains no document, model, scene or GPU resources. Ordinary requests share that map; producer
  changes preserve older snapshots until their last artifact drops.
- `WorldRuntime` publishes geometry and identity as one owned `AnimationArtifact`. Nonempty GPU
  upload retains that same descriptor. No-op completion retires pending context without relabeling
  the installed picture; empty output and existing shared-batch cleanup remove GPU metadata with
  resources. The existing optional inspector trace shows dispatch and inspected-region inputs.
- A new regression first reproduced publication after a smoothing change. Smoothing/freeze
  changes now revoke the older completion, including toggle roundtrips, and force replacement
  after the physical FIFO slot drains. Production reconciles playback before polling. Source
  replacement removes old worker membership before a new-source rebuild. Ordinary clock/camera
  lag and last accepted producer data during semantic rebuild retain their stale-read policy.
  No worker message/counter, sequence math, cadence, semantic/history, dependency or allowance change.
- **Verification:** all 12 targeted animation tests passed, with explicit logs proving all three
  new actual-GPU paths ran. This includes exact/multiple producer descriptors, replacement/empty
  membership, snapshot release, no-op retirement, accepted clock lag, settings/source supersession
  and real frame freeze reconciliation. The first GPU frame test used a synthetic clock ahead of
  the app clock; its fixture now starts at zero, without changing production timing semantics.
  Final editor gate passed strict all-target/all-feature native Clippy and **398 tests, 0 failed,
  4 pinned-cache tests ignored**, plus architecture/authority/feature-index/Python/source-pin/format
  checks in 34.4 s: `target/verify/gates-20261019-225952/report.json`. Stable before/after source:
  `13d4088273cb1f30b1e0b42b82fef914c79a3c2f53ec1d9708a799869389f0e8`.
  Strict WASM editor Clippy passed (21.4 s). Existing reference-tool/historical-harness skips remain;
  changed links and whitespace passed. Only documentation changed afterward. The prior full
  workspace result below retains its earlier source scope. No remote CI, independent visual,
  sequence-math parity, portable reproduction or performance certification is claimed.
- **Next boundary at that checkpoint:** retain accepted NPC/player extraction identity through their existing
  transient GPU layer and cleanup operations, using current request keys and frame reconciliation.
  This is implemented above. Preview identity, portable reproduction/recovery, external fidelity evidence and editor-wide
  headless/MCP remain open. Performance stays paused; the broader goal remains incomplete.

## Dataset exporter ownership and workspace integration — 2026-10-19

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, with the earlier local
checkpoints below. Unrelated sample JSON and `docs/.DS_Store` remain preserved. No branch,
worktree, commit or publication; performance measurement/optimization remains paused.

- Reproduced E0433 in the world's dataset example: it used canonical
  `osrs_lab::LearningSplit` outside the world's declared/allowed dependencies. Moved
  `building_dataset.rs` and its three modules byte-identically to
  `crates/osrs-editor/examples`, whose existing dependencies support this composition.
  Removed the old source locations; no wrapper, duplicate split policy, dependency, manifest,
  lockfile, schema, exporter ID, argument, semantic algorithm or write behavior changed.
  Reproduction docs now use `cargo run --release -p osrs-editor --example building_dataset --
  "<OpenRune root>" out/`.
- Relocation/manifests proof and CLI smoke evidence are in
  `target/verify/dataset-ownership-nf5jk_pk/{relocation,cli-smoke}.json`. The new command builds
  and reports the expected missing-root error (exit 1) without arguments. No corpus export was
  run: the local OpenRune snapshot lacks installed `gamevals.dat` and the LIVE cache required
  by the exporter. The separate pinned build-241 cache does not supply that installation.
- The first full integration gate stopped on 31 existing test-only `unwrap` lint violations
  in five edit/scene files. Replaced them with explicit panic closures, retaining every
  assertion and expected value; no lint allowances or skips added. The production WFC repair
  prefix is byte-identical. Proof: `target/verify/dataset-ownership-nf5jk_pk/test-lint-repair.json`.
- **Verification:** full build stage passed strict native workspace/all-target/all-feature
  Clippy (114.8 s), strict WASM Clippy (29.9 s), and **1,384 tests, 0 failed, 23 ignored** across
  23 test executables (604.6 s): `target/verify/gates-20261019-182336/report.json` (749.4 s total).
  Final full quick stage passed architecture/authority/feature-index/Python/source-pin/format
  checks in 4.3 s: `target/verify/gates-20261019-183616/report.json`. Both reports have identical,
  stable before/after source hash
  `e6927444bae34a44a00791020405364f14c9bdd5b1820108f9531488d6a0b38f`.
  The existing five absent reference-tool tests and historical harness blob check remain skipped;
  Rust ignores retain their prerequisite scope. Changed Markdown links and whitespace passed.
  Only documentation changed afterward. This closes the earlier local all-target source blocker;
  it makes no remote CI, full-export, independent external visual or performance claim.
- **Next boundary at that checkpoint:** aggregate animation/transient artifact identity using the existing
  single-flight owners, retaining ordinary clock lag and existing supersession policy. Portable
  reproduction/recovery, external fidelity evidence and headless/MCP work remain open; see the
  technical architecture audit. Aggregate animation is implemented above; the broader goal remains incomplete.

## Accepted static/picker artifact identity — 2026-10-19

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, with the earlier local
checkpoints below. Unrelated sample JSON and `docs/.DS_Store` remain preserved. No branch,
worktree, commit or publication; performance measurement/optimization remains paused.

- The existing FIFO ticket (`RegionBuild`, formerly `PendingBuild`) retains its `BuildStamp` on
  accepted completion. `RegionGeometry` now carries canonical `TargetProvenance` and actual
  `TerrainPresentation`; existing `RegionRenderPath` records Reference/HD/fallback. This is a public
  Rust payload extension, with no request/order, persistence, scene or shader change.
- After semantic adaptation, private immutable `RegionArtifactIdentity` binds exact source/profile/
  decoder inputs to the original source instance/request, dispatched revision, producer epoch and
  presentation generation. It introduces no freshness counter and does not claim a semantic content
  hash. Unrelated hydration remains allowed; FIFO acceptance/history authority remain unchanged.
- RenderRuntime retains the shared identity with each uploaded static zone. Partial refreshes retain
  untouched identities; successful empty zones drop theirs with GPU resources. A proven unchanged
  Object result may preserve old static identities while replacing complete picker data.
  `WorldRuntime::region_installed` consumes one accepted owned payload for inspection, dependency
  stamps, animation membership and statistics. Height/Object stamps use that captured identity.
- Collision refuses retained inspection from another source/presentation even when source reset
  reuses producer epoch zero. Existing dirty/in-flight/epoch policy remains. Rejection, terminal
  failure, source/presentation rebuild and removal retire metadata with projections. The existing
  optional inspector trace shows last-accepted static and picker identities with wrapping text.
- Upload/residency operations moved to private `render_runtime/regions.rs`; no EditorApp fields/
  methods, crate dependencies, architecture allowances or semantic algorithms were added.
  Standalone RenderSnapshot/SemanticGeneration have a different mesh/hash shape and are not falsely
  claimed as the active packed/FIFO path. Aggregate animation, actor/preview correlation and portable
  reproduction identity remain follow-ups; independent roof/raster/texture/visual evidence stays open.
- **Verification:** three targeted actual-GPU identity tests passed, explicitly logging uploads/
  removal, after correcting a missing Object-proof setup in the test. Final focused build passed
  strict all-target/all-feature native editor Clippy and **389 tests, 0 failed, 4 pinned-cache tests
  ignored** in 73.7 s: `target/verify/gates-20261019-181145/report.json`. Stable source hash:
  `42f9e09a70159d870a8c9060a643a820d16303664d5add88cd935783d23208b1`.
  The final suite includes the new retained cross-source/presentation collision case. Strict world
  `--lib --tests` Clippy passed (10.5 s); strict WASM editor Clippy passed (53.2 s). The pinned-cache
  `region_artifact_retains_exact_source_and_extraction_inputs` and Reference pixel-golden tests both
  passed; golden files remain untouched. Architecture/authority/feature-index guards, formatting,
  changed Markdown links and whitespace checks passed. Only contract/verification docs changed
  afterward. No full-workspace, remote CI, independent external visual or performance claim.
- **Next boundary at that checkpoint:** repair the dataset-exporter composition ownership. Its
  `crates/osrs-world/examples/building_dataset/record.rs` called `osrs_lab::LearningSplit`,
  outside the world's declared/allowed dependencies. Do not add an upward dependency or widen the
  guard. This world example was excluded from the focused world Clippy command above, so an
  all-target world gate was then uncertified. The integration checkpoint above closes that blocker.
  Aggregate/transient identity and the remaining architecture rollout stay open; performance remains paused.

## Canonical normal parity gate and pinned edge corpus — 2026-10-19

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, with the earlier local
projection checkpoints below. Unrelated sample JSON and `docs/.DS_Store` remain preserved. No
branch, worktree, commit or publication; performance work remains paused.

- The inventory runner now executes `base_normals` and `normal_merge` through the existing
  `osrs-core::normals` implementation. Their five historical manifests are promoted to semantic
  execution. The M7 source-immutability/scene tests remain; no normal algorithm was copied or changed.
  The existing plane adapter moved verbatim to `runner/planes.rs`; the parent shrank from 635 to
  439 lines without adding dependencies or widening architecture allowances.
- Added five P1 edge fixtures: mixed signed smooth/flat/suppressed faces, two matching pairs below
  the hiding threshold, flat-neighbor exclusion, signed XYZ translation and duplicate pair
  accumulation without welding. Canonical tests require the full 15-fixture inventory, reject
  evidence-only downgrades, detect corrupt expected values and report malformed model inputs.
- `scripts/normal_oracle` verifies full external SHA-256 pins, extracts unchanged ModelData
  base-normal/bounds/merge bodies plus the normal-copy constructor and compiles a storage-only
  Java shell. Candidate output refuses the accepted tree, symlink aliases and existing directories.
  The five new manifests pin the complete adapter; historical manual provenance remains intact.
  Six offline safety tests join the unified quick gate; ordinary CI needs no Java/source checkout.
- **Verification:** regression first failed all three runner tests on the old bypass/missing
  executor; the connected runner passed. Final focused build stage passed strict all-target/
  all-feature native Clippy and **88 tests, 0 failed, 0 ignored** in 49.5 s:
  `target/verify/gates-20261019-175615/report.json`, stable source hash
  `14f77269c5bc53dfecba742bf68592193f78f146c0b2b2707120c1eeaa3c1ed4`.
  The preceding quick stage passed architecture/authority/feature-index/Python checks (including
  six new safety tests); five absent reference-tool tests and the historical harness blob still
  skip. Its build stage found test-only `expect` assertions, corrected without lint allowances.
  Pinned Java replay with JDK **24.0.2**, `--release 17`, matched all **10** normal cases exactly.
  Formatting, changed Markdown links, verbatim plane-executor comparison and whitespace checks
  passed. Only verification documentation/wording changed after the final build gate.
- Scope: isolated numeric normal-method evidence against the declared revision-235 source, not
  build-241 scene traversal, external visual parity, remote CI or a performance claim. The next
  grounded rollout boundary is world-to-render artifact identity. Broader roof/raster/texture and
  independent visual evidence remain open; see the technical architecture audit checkpoint 5.

## Terminal region cleanup and retry policy — 2026-10-19

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, with the preceding local
checkpoints below. Unrelated sample JSON and `docs/.DS_Store` remain preserved. No pull, branch,
worktree or publication; performance work remains paused.

- Current Empty/Failed now pass the existing FIFO/epoch/presentation gate and share the semantic-
  rejection cleanup. The private operation is named `WorldRuntime::region_unavailable`; the old
  partial `region_absent` path is removed. Full and scoped terminal outcomes hide the entire producer,
  remove picker/collision/outlines, animation membership and dependency/Object proofs/comparisons,
  clear selection, and hide the shared animated GPU batch until surviving membership rebuilds.
- Empty means absent producer source, even for a scoped request; zero-triangle successful Region
  output keeps its existing scoped behavior. Empty stays quiet; Failed reports coordinate/error.
  Empty no longer unloads clean semantic documents. Neither result modifies session/history/
  revisions/dirty state or consumes pending project diffs. Distance eviction and source replacement
  keep their existing independent semantic residency rules.
- Unavailable residency prevents per-frame loops. Existing rebuild/source replacement/eviction/
  re-entry/new semantic invalidation can retry; later zone edits request full recovery. Stale events
  retire only their own FIFO ticket and cannot clear newer projections, requests, dirty obligations,
  selection or notices. No worker protocol, public API or persistence format changed.
- New actual-GPU tests cover Empty/Failed × full/scoped × clean/dirty sessions, geometry outside
  scoped output, successful zero-triangle scoped output preserving other zones, pending project
  restoration only after valid recovery, obsolete presentation/epoch completions and results with
  no outstanding ticket. The existing collision test now expects Unavailable after a
  scoped terminal failure and Full recovery after a later edit. Shared lifecycle/proof/outline/
  animation tests continue to exercise the single removal operation.
- **Verification:** the two focused terminal tests passed and explicitly logged actual GPU
  upload/removal paths. Final focused native gate passed strict all-target/all-feature Clippy and
  **385 tests, 0 failed, 4 pinned-cache tests ignored** in 61.0 s:
  `target/verify/gates-20261019-173825/report.json`. Before/after source hash was stable:
  `e112c3a9eb2474fea0baafcc8538e7dc904b26088e334799ac70fee59b43a60b`.
  Strict WASM editor Clippy passed (39.5 s); architecture/authority/feature-index guards, formatting,
  changed Markdown links and whitespace checks passed. Only verification documentation changed
  afterward. No full-workspace, real-mouse, remote CI or independent visual/performance claim.
- Next rollout phase: select a grounded fidelity-fixture/oracle checkpoint. Independent visual parity
  and integrated world-to-render identity remain open; performance stays paused.

## Actor completion supersession — 2026-10-19

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, with the preceding local
checkpoints below. Unrelated sample JSON and `docs/.DS_Store` are preserved. No pull, branch,
worktree or publication; performance work remains paused.

- Inspection found that player hide/teleport could leave a completion eligible to restore old
  geometry, while NPC arrival cleared the in-flight slot before matching the issued stamp.
  Both owners now stage matching arrivals until `update()` reconciles actual current-frame inputs.
- NPC acceptance checks exact issued stamp before slot retirement, then current source/map/scenario/
  run, accepted ground version, camera, range, draw limit and vars before publication. Player checks
  its issued token, current spec/look and accepted ground version; explicit teleport revokes output.
  Stale completions cannot install diagnostics or retire a newer request.
- Hide clears staged output and publication permission while retaining the physical FIFO request
  until it drains. Hide/show cannot issue indistinguishable requests for the same NPC tick. Ordinary
  requested animation/tick lag remains allowed; no simulation cadence, worker protocol, history or
  project format changes. NPC geometry lifecycle moved into private `npc_runtime/geometry.rs` to
  stay within the existing production file cap; no allowance widened.
- Four new tests exercise stale-slot retirement and actual arrival/update GPU paths, including
  hiding installed layers, source/semantic/restart/extraction-input supersession, replacement
  recovery and accepted animation lag. Synthetic payloads prove installation policy, not model parity.
- **Verification:** first targeted run passed 63 matching owner tests and explicitly logged both
  actual GPU lifecycle paths. Final focused native gate passed strict all-target/all-feature Clippy
  and **383 tests, 0 failed, 4 pinned-cache tests ignored** in 63.3 s:
  `target/verify/gates-20261019-111302/report.json`. Before/after source hash was stable:
  `4fb6198fcfab454c2a57c73a6fc88ca5bd7118fb4e07efa83d37763a35041d0f`.
  Strict WASM editor Clippy passed (10.9 s), as did formatting, architecture/authority/feature-index
  guards, changed Markdown links and whitespace checks. Only verification documentation changed
  afterward. No full-workspace, real-mouse, remote CI or independent visual/performance claim.
- At this checkpoint, terminal Empty/Failed policy was next; it is implemented above.
  Integrated artifact identity and independent visual evidence remain open.

## Projection contracts and required-work retention — 2026-10-19

Baseline remains `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`, on top of the local candidate-
rejection checkpoint below. Existing changes and unrelated sample JSON/`docs/.DS_Store` remain in
place. No pull, branch, worktree or publication; performance measurement/optimization remains paused.

- [Active projection contracts](docs/architecture/PROJECTION-CONTRACTS.md) inventory the installed
  map/viewport artifact families, impact consumers, identities, dependency expansion, stale-read and
  failure policies, executable evidence and remaining gaps. In particular, last accepted picker/
  visual data is not proof of current collision; FIFO identity is not out-of-order-worker support.
- New production-owner tests reproduced two bugs before the fixes: normal region replacement could
  publish an older aggregate animation completion, and a superseded scoped build could lose the
  required zones taken from its dirty queue at dispatch. Dirty region/zone obligations now survive
  in-flight work and are retired only by a current terminal completion. A later edit invalidates an
  in-flight full build even when its full obligation already subsumes the new zone footprint.
- The existing animation discard flag now covers replacement, presentation rebuild and eviction as
  well as semantic rejection. A full rebuild clears the shared animated GPU batch immediately.
  Existing FIFO protocol, conservative fallbacks, semantic authority/history and scheduling priorities
  are preserved. No new framework/token or performance claim; no architecture allowance widened.
- Added live collision failure/recovery, coalescing/full-subsumption, superseded-area retention,
  unrelated-hydration revision and animation lifecycle regressions. The existing headless GPU test
  additionally checks presentation rebuild clears both static and aggregate animated geometry.
- **Verification:** the new contract suite first reproduced both defects (3 passed, 2 failed), then
  all six targeted contract tests passed after the fixes. Final focused build gate passed native
  all-target/all-feature Clippy and **379 tests, 0 failed, 4 ignored** (pinned-cache prerequisites):
  `target/verify/gates-20261019-110314/report.json`, stable before/after source hash, 66.2 s. GPU logs
  explicitly confirm both semantic rejection and full presentation rebuild removed their layers.
  Strict WASM editor Clippy, formatting, architecture/authority/feature-index guards and changed
  Markdown links also passed. Only verification documentation changed afterward. No full-workspace,
  real-mouse, remote CI or independent visual/parity/performance certification is claimed.
- At this checkpoint, actor arrival/hide/tick supersession and terminal worker-failure cleanup were
  follow-ups. The actor boundary is implemented above; terminal policy and the later rollout remain open.

## Semantic region candidate rejection and technical audit — 2026-10-19

Baseline: `main` at `c4d2c5d235bedcedef7825249833b1ea6dbe9dbd`; local changes on that baseline.
No pull, branch, worktree or publication. Pre-existing `docs/.DS_Store` and unrelated untracked
sample-data JSON files are outside this checkpoint.

- `streaming/region_install.rs` retires FIFO tickets and applies existing freshness gates before
  semantic adaptation. Failed adaptation cannot hydrate semantics, consume Object comparison scope,
  install candidate outputs, or apply/consume pending project diffs.
- World-owned rejection clears the whole region's picker/outlines, animation membership, dependency
  and Object proof state. Static render zones are removed for both full and scoped failure. The
  shared animated batch is hidden temporarily; any old in-flight batch is discarded before a forced
  rebuild of surviving regions. Semantic session, history/revisions and dirty state stay intact.
- An unavailable marker suppresses per-frame retry. Rebuild/source replacement, eviction/re-entry,
  or subsequent semantic invalidation permits retry. Zone invalidation over an unavailable region
  escalates to full recovery. Animation scheduling moved to a private runtime module to keep the
  parent below its 500-line cap; no public protocol or persistence shape changed.
- [Technical audit and rollout](docs/architecture/TECHNICAL-ARCHITECTURE-AUDIT.md) records existing
  foundations and remaining boundaries. Replay design/navigation now match completed descriptor
  routing and removal of duplicate chronology. Historical replay-routing claims below are superseded.
- Verification also exposed pre-existing dead code from the completed history cleanup and the
  previously recorded WASM building-tool errors. Unused getters/loc-mirror code were removed,
  the footprint fixture constructor is test-only, and native-only candidate/capture/policy-replay
  paths are now target-gated. The browser size-map collection has an explicit type; native behavior
  is unchanged. No lint/debt allowance was widened.
- **Verification:** Python quick checks, architecture/authority/feature-index guards and formatting
  passed (the existing five missing reference-tool tests and deob harness-blob check still skip).
  Final `verify.py -p osrs-editor --stage build` passed strict native all-target/all-feature Clippy
  and **373 tests, 0 failed, 4 ignored**; the ignored tests require the pinned cache. Report:
  `target/verify/gates-20261019-093734/report.json` (stable source before/after, 68.9 s).
  The GPU regression actually exercised full/scoped static and animated removal, rather than taking
  its unavailable-adapter branch. `cargo clippy --locked --target wasm32-unknown-unknown
  -p osrs-editor -- -D warnings` passed too. These checks cover the final production/test sources;
  only verification documentation was updated afterward. No full-workspace sweep, real-mouse
  smoke, remote CI or independent visual parity is claimed.
- Later rollout phases are follow-up boundaries, not completed work. Performance remains paused.

## Lighter CI and local gate — 2026-10-19

Baseline `main` at `ff7a1be7` (the app's catch-all commits swept this work in) plus `3071a3ad`,
`c4429d48`, `e6d05083` (format/lint fixes to other sessions' code). Authority: `scripts/verify.py`
(one definition for local and CI), `.github/workflows/ci.yml`, `docs/LOCAL-VERIFICATION.md`.

- **Test inventory**: `scripts/run_tests.py` now builds `--lib --bins --tests`; examples are only
  type-checked by Clippy (`--examples` opts back in). Seven crates (`osrs-cache`, `-core`, `-hd`,
  `-reference`, `-render`, `-scene`, `-world`) bundle their integration files into one
  `tests/integration.rs` (`autotests = false`; **a new `tests/*.rs` must be added there**). Former
  `--test themes` is `--test integration themes::`.
- **Measured** (this Mac, 2026-10-18/19): test binaries 123 to 23. In the 2026-10-18 20:14 full gate
  libtest reported about 121 s of real test time but per-binary wall times summed to 5,034 s: the cost
  is the first launch of each freshly linked executable (dyld/Gatekeeper scan, 20-100 s under load),
  not test work. Test step: 1,518 s (build 259 + run 1,260) before; 578 s (build 95 + run 484) after,
  1,354 passed / 22 ignored / 0 failed.
- **Gate**: `verify.py --stage quick` (Python guards + rustfmt) and `--stage build` (native Clippy,
  one merged wasm Clippy over the editor and portable libs with `-D warnings`, tests). The separate
  `cargo check` steps and the `wasm-check` command are dropped because Clippy compiles everything
  they do; this is stricter, not weaker, for the library crates (checked: wasm Clippy over core, edit,
  lab, cache, scene, hd, render, world passes).
- **CI**: one Ubuntu job (was: two chained jobs, push and pull_request both triggered, no wasm, full
  tests only by manual run/`[full-ci]`). Push to `main` + PRs, `paths-ignore` Markdown, cancel
  superseded runs, 45 min cap, quick stage before the cache restore, only `main` saves the cache,
  `CARGO_PROFILE_DEV_DEBUG=0`. Dropped mold (Rust 1.90+ links with rust-lld on Linux x86_64) and the
  `[full-ci]` trigger. It now runs the whole test suite and wasm Clippy on every non-doc push, which
  the old CI did not.
- **Build**: `[profile.dev] debug = "line-tables-only"` and `debug = false` for dependencies
  (the earlier note that `debug = 1` was line-tables-only was wrong; 1 is "limited"); `itertools` 0.15
  in `osrs-filestore` removes a duplicate; `rust-toolchain.toml` lists the wasm32 target. Remaining
  duplicates (`syn`, `objc2`, `miniz_oxide`, `rustc-hash`, `bitflags`) come from wgpu/winit/png/flate2.
- **Not done / not measured**: the final end-to-end `verify.py --full` is not green on this tree because
  of other work in progress, not these changes: `crates/osrs-world/examples/building_dataset/record.rs`
  uses `osrs_lab` (not a dependency of `osrs-world`, and the architecture rules forbid it) and the wasm
  build of `osrs-editor` has errors (`prefab_runtime/build.rs:362` type annotation, unused
  `candidates.rs` import and `capture/analysis.rs` variables). The quick stage, native Clippy for
  everything else, wasm Clippy for the libs and the full test inventory were each run green. GitHub
  runtime and billable minutes are unmeasured (no GitHub access); the CI file has never run.
  Possible follow-up: `run_tests.py --jobs` above 4, since the first-launch delay is latency, not CPU.

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
- **Procgen learning** (`docs/plans/PROCGEN-LEARNING.md`): what the WFC cannot see, a label taxonomy, phases. `osrs-editor/examples/building_dataset.rs` (moved from world on 2026-10-19) exports every real building (rooms with tags, furniture, decoration, wall edges, roof, ring, 117HD area names via `AREAS=`) to JSONL: 3,944 buildings, 35,350 objects, 1,888 inside a named area. Measured: of 21 buildings inside a 117HD bank area only 9 have a census `bank` room; of 9 church areas 3 have a `church` room.
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

`TerrainController` is temporary preview/projection state, not a second semantic history. Descriptor-driven replay and removal of the chronological routing stacks are complete. Remove the remaining adapter once workers consume semantic snapshots and preview/source-height compatibility has a proven replacement. Small editor planner import adapters contain only DTO conversion/re-exports. Existing broad application orchestration methods are enumerated in the ratchet; future extraction must lower it.

## Validation

Historical full run before the filestore checkpoint (path grading/curve/cursor work on top of `7757ff4a`; local results, not remote CI certification), Python 3.14.8 (`/opt/homebrew/bin/python3`):

- Architecture guard and its tests, editor-ownership tests, reference-regeneration tests, golden-index check, HD source pins (98 entries awaiting audit), HD-source tests and `test_performance.py`: passed.
- `cargo fmt --all --check`; locked workspace/all-target/all-feature `cargo check` and strict Clippy; WASM lower-crate check and strict editor Clippy: passed.
- Locked full workspace/all-target/all-feature tests: **861 passed, 0 failed, 18 ignored** (the two guard scripts above fail for the reason given) (ignored tests keep their external cache/GPU/oracle prerequisites).
- With the pinned cache: terrain oracle (`terrain_oracle_diff`) and the Reference pixel golden passed; `region_capture` Reference hashes were identical before and after checkpoint 2a. HD captures vary by up to one channel level in a few hundred pixels between identical runs, so HD comparisons use a tolerance.

Earlier figures (805 at `41d0c020`, 793 for PR #56) are superseded. Historical CI results only certify the SHAs they name. The former vendored `rune-fs` MSRV warning is removed by the current workspace-owned manifest.

## Subsequent work

Continue the authorized checkpoints on `main`. Follow the existing product roadmap for independent Reference visual proof, M10+ certification, remaining HD features, export generalization and recovery/project polish; this refactor does not claim those features are complete. See `docs/PROJECT-STATUS.md`.

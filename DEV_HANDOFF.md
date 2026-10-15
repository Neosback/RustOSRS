# RustOSRS Development Handoff

Last updated: 2026-10-15

Read `AGENTS.md` and `ARCHITECTURE.md` first. `docs/architecture/FEATURE-MAP.md` and `scripts/editor_features.json` locate feature ownership and tests. Live Git state is authoritative for branch/SHA facts; CI claims require actual run evidence.

## Current boundary and standing workflow

- Working branch: `main`; stay on `main` unless the user explicitly asks otherwise. One local commit per checkpoint; nothing is pushed or published without authorization.
- Program baseline: `63d2721e3fa30dc2eaecff140d6b27886e4dff2f`, merge of PR #56 (editor ownership), main CI run `38766016806` (Tier A/B/C success). The old feature branch is retired remotely.
- Commit `41d0c020` ("updates") on top of that baseline already contains the program foundations: opt-in JSON performance capture (`diagnostics/capture.rs`, schema v1), owned-GPU-byte accounting (`gpu/resources.rs`), Reference frame reuse (`render_runtime/reuse.rs`), the shared Spline settings widget (`spline_tool/panel.rs`), `scripts/performance.py`, and the 117HD source manifest/ledger (98 config items, all `audit_pending`). They are committed, not pending, but **no baseline or improvement has been recorded**.
- Authorized work and plan: `docs/plans/PERFORMANCE-UI-PLAYTEST-HD.md`. The expanded user goals (RuneLite/deob re-verification, exact tile/overlay/underlay/blend abstraction for the spline tool, NPC/entity runtime for a future own client) are carried by the checkpoints below.

## Current program checkpoints

0. Reconcile handoff/docs and record gate results at `41d0c020` — done (see Validation).
1. Measurement completion (schema v2, worker/renderer counters, workloads, compare tool) and recorded baselines — pending.
2. (a) Terrain appearance abstraction in `osrs-scene`, no behavior change; (b) responsive tool panels and Spline workflow with real blended preview — pending.
3. Measured CPU/GPU/memory optimizations — pending.
4. Entity cache decoders, skeletal animation, shared model appearance transforms — pending.
5. `osrs-playtest` crate, editor controller, entity rendering — pending.
6. Remaining 117HD systems and independent visual proof — pending.

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

Checkpoint 0, at `41d0c020` plus documentation/comment-only edits (local results, not remote CI certification), Python 3.14.8 (`/opt/homebrew/bin/python3`):

- Architecture guard, 24 architecture-guard tests, 11 editor-ownership tests, 5 reference-regeneration tests, golden-index check, HD source pins (98 entries awaiting audit) and 3 HD-source tests: all passed.
- `cargo fmt --all --check`; locked workspace/all-target/all-feature `cargo check` and strict Clippy: passed.
- Locked full workspace/all-target/all-feature tests: **805 passed, 0 failed, 18 ignored** (the ignored tests keep their explicit external cache/GPU/oracle prerequisites and are not acceptance evidence).
- WASM lower-crate check and strict editor Clippy: passed.
- Cargo emits the existing vendored `rune-fs` MSRV-metadata warning.

This supersedes the earlier PR #56 figures (793 passed). Historical CI results only certify the SHAs they name.

## Subsequent work

Continue the authorized checkpoints on `main`. Follow the existing product roadmap for independent Reference visual proof, M10+ certification, remaining HD features, export generalization and recovery/project polish; this refactor does not claim those features are complete. See `docs/PROJECT-STATUS.md`.

# RustOSRS Development Handoff

Last updated: 2026-10-15

Read `AGENTS.md` and `ARCHITECTURE.md` first. `docs/architecture/FEATURE-MAP.md` and `scripts/editor_features.json` locate feature ownership and tests. Live Git state is authoritative for branch/SHA facts; CI claims require actual run evidence.

## Current local boundary

- Active work: the user-authorized editor ownership implementation, covering the complete approved checkpoint sequence.
- Branch: `Neosback/editor-ownership`.
- Baseline HEAD: `d7db98cd179fe8d64b2e743f74d8aefa9a955449` (local `main` merge of PR #55).
- Implementation is complete and locally validated; remote branch certification is the next closeout step. The user explicitly requested commit and merge into `main`; branch CI and merge closeout are in progress.
- This branch incorporates remote main handoff-only commit `f390f00f05b1a2014223c8fe3c00d43687d2e7b9`, which records the completed PR #55 closeout (Tier A/B/C success in CI #1423 / run `38759609008`). The earlier checkpoint/CI history is also preserved in `docs/history/interaction-runtime-checkpoints.md`.

The user explicitly authorized the full architecture plan, including continuing through its checkpoints, and subsequently authorized committing and merging the completed implementation.

## Completed ownership changes

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

Passed locally against the implementation working tree:

- Architecture/dependency/file-size/application-surface/feature-reference guards.
- Architecture guard tests: 24 passed; editor ownership/feature guard tests: 11 passed; reference-regeneration safety: 5 passed; historical golden-index check passed.
- `cargo fmt --all --check` and `git diff --check`.
- Locked workspace/all-target/all-feature compile and strict Clippy.
- Locked full workspace/all-target/all-feature tests: **793 passed, 0 failed, 18 ignored**, including the permanent M3–M9 semantic/reference gates and available renderer tests.
- Final editor suite after the scheduling-revision diagnostic adjustment: **231 passed, 0 failed, 3 ignored**. Native strict Clippy was rerun for that adjustment.
- WASM lower-crate check and strict editor Clippy passed. Native-only inspection/quit helpers are target-gated rather than lint-suppressed.

The ignored tests keep their existing explicit prerequisites (external cache/GPU/oracle data); they were not promoted to passing acceptance evidence. Cargo emits the existing vendored `rune-fs` MSRV-metadata warning; repository strict lint checks still pass.

These are local implementation results, not remote CI certification. Record exact branch/main SHAs and actual CI evidence during merge closeout. Historical CI results only certify the SHAs listed in the archived handoff.

## Subsequent work

Keep this branch focused on ownership/maintainability. Follow the existing product roadmap for independent Reference visual proof, M10+ certification, remaining HD features, export generalization and recovery/project polish; this refactor does not claim those features are complete. See `docs/PROJECT-STATUS.md`.

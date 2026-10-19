# RustOSRS contributor entry point

Read the nearest `AGENTS.md` for subsystem-specific rules. This root file contains only repository-wide rules.

## Before editing

1. Read `ARCHITECTURE.md`.
2. Read `DEV_HANDOFF.md`, then verify the live branch, working tree, and exact HEAD.
3. Run `python3 scripts/features.py find "<concept>"` before creating an implementation.
4. Inspect the existing owner, callers, feature-registry entry, relevant `docs/specs/`, and accepted ADRs.

`index.md` defines documentation authority. Current code and verified Git state outrank chat history, stale plans, archived handoffs, or assumptions.

## Universal architecture rules

- `MapDocument` / `EditorSession` is semantic authority. GPU buffers, worker mirrors, previews, caches, indexes, and UI state are projections only.
- Commit, undo, and redo flow through the existing edit gateway / `EditRuntime` path.
- Pure semantic planning belongs in `osrs-edit`; canonical coordinates and IDs come from `osrs-core`.
- Exact OSRS scene behavior belongs in `osrs-scene`; reusable 117HD rules/data belong in `osrs-hd`; GPU projection belongs in `osrs-render`; scheduling/streaming/projection orchestration belongs in `osrs-world`.
- `osrs-editor` is the composition/product layer. Keep `EditorApp` thin and put feature behavior on dedicated owners.
- Search before creating a type, planner, history system, worker message, cache, renderer pass, coordinate conversion, or helper module.
- Reuse or extend the canonical implementation. Do not add a parallel implementation to avoid understanding the first one.
- Temporary adapters must name their authoritative source, allowed callers, and removal condition.
- Preserve OSRS/117HD quirks that are part of parity. Do not "simplify" source behavior without evidence and a regression fixture.
- Prefer narrow private state and explicit operations over broad mutable access, service locators, or catch-all `utils` modules.

## Feature changes

The machine-readable navigation source is `scripts/editor_features.json` and its canonical loader is `scripts/feature_registry.py`.

For an authored/mutating feature, keep the registered controller, planner, gateway, planning test, cancellation test, commit test, and undo test accurate. Lower architecture baselines when ownership shrinks; never widen a ratchet only to silence a guard.

## Working style

- Stay on `main` unless the user explicitly authorizes another branch.
- Use focused, reviewable checkpoints.
- Do not create a PR/worktree or publish unless authorized.
- Inspect before refactoring; preserve working semantics while moving ownership.
- Update the handoff when ownership, public contracts, or verification facts materially change.

## Verification ladder

Use the cheapest level that proves the checkpoint:

1. Documentation-only: inspect changed files/links; no build.
2. Python/architecture tooling: direct unit test or command, then architecture guard if relevant.
3. Local Rust behavior: smallest affected test or `python3 scripts/verify.py -p <crate>`.
4. Integration/parity: only the affected integration/parity suites.
5. Full workspace: `python3 scripts/verify.py --full` only for a ready integration/merge checkpoint or when broad changes require it.

Do not repeat unchanged green suites, overlap Cargo runs, or use the full build as the default inner loop. GitHub CI runs the same full verification definition; never claim CI state that was not observed.

Use the pinned toolchain and Python 3.11+. See `docs/LOCAL-VERIFICATION.md` for the exact commands and platform notes.

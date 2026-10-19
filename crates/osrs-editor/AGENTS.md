# osrs-editor guidance

## Owns

- Product composition, UI/input, tool controllers, runtime owners, platform integration, and routing between semantic edits and projections.
- `EditRuntime` transaction execution/replay and application-level lifecycle/effects.

## Does not own

- Duplicate map semantics, GPU truth, cache codecs, OSRS scene rules, or a second planner/history system.
- Broad feature behavior directly on `EditorApp` when a dedicated runtime/controller can own it.

## Invariants

- `EditorApp` stays a thin composition root and frame/lifecycle coordinator.
- Committed map changes go through the existing semantic gateway and `EditRuntime` path.
- UI emits typed actions/intents; previews remain transient and reversible.
- UI/widget state belongs in `UiRuntime`; settings in `SettingsRuntime`; diagnostics observe but do not decide validity.
- Worker/GPU/preview state is projection only.
- One logical gesture produces at most one logical semantic transaction unless the feature contract explicitly defines otherwise.

## Before adding code

1. Run `python3 scripts/features.py find "<concept>"`.
2. Inspect the registered controller, planner, gateway, tests, and callers.
3. Search for an existing runtime/controller before adding an `EditorApp` method or field.
4. If changing ownership, update the relevant registry/docs/ratchet rather than widening a baseline to silence a guard.
5. Temporary adapters must state authoritative source, allowed callers, and removal condition.

## Verification

Prefer the registered feature tests or the narrow affected controller/runtime tests. Run `python3 scripts/check_architecture.py` for ownership changes before considering crate-wide or full workspace verification.

# osrs-edit guidance

## Owns

- Headless editable-map semantics and deterministic edit planning.
- `MapPatch`, map queries, edit validation, prefab/building planning, and other domain transformations.
- Pure logic that can be tested without UI, GPU, filesystem dialogs, browser APIs, or runtime orchestration.

## Does not own

- `EditorApp`, widgets, input handling, GPU resources, worker scheduling, cache transport, or platform services.
- A second history/document/coordinate/object model when a canonical one already exists.

## Invariants

- Plan from typed intent plus read-only semantic state into a deterministic patch/result.
- Do not mutate renderer/world/UI projections from planners.
- Reuse `osrs-core` coordinates and IDs at boundaries.
- One user gesture should remain representable as one logical semantic transaction unless the OSRS/editor contract explicitly defines multiple steps.
- Preserve exact OSRS quirks instead of generalizing them away.

## Before adding code

1. Run `python3 scripts/features.py find "<concept>"` from the repository root.
2. Search this crate for an existing planner/type before creating another.
3. Read the relevant `docs/specs/` contract and callers.
4. Prefer extending an existing semantic owner over introducing a parallel abstraction.

## Verification

Start with the smallest affected unit/integration test. Use `python3 scripts/verify.py -p osrs-edit` only when crate-wide validation is justified. Do not run the full workspace gate for a local planner-only change unless required by the checkpoint.

# osrs-lab guidance

## Owns

- Headless tool environments, deterministic tasks/replay, training/evaluation helpers, reconstruction/scoring, and automation-oriented experimentation over canonical semantic APIs.

## Does not own

- Production UI, GPU rendering, cache transport, or an alternate editable map model.
- Special mutation paths that bypass `osrs-edit` planners/patch semantics merely because a caller is automated.

## Invariants

- Lab actions must be deterministic and replayable from explicit inputs/seeds.
- Reuse canonical edit/query types and existing planners wherever possible.
- Keep curriculum/training artifacts distinct from held-out evaluation evidence.
- A Lab mirror/environment is isolated experiment state, not the main editor's semantic authority.
- Automation-facing operations should be narrow enough to become future MCP/agent tools without embedding UI coordinates or OSRS logic in the protocol layer.

## Before adding code

1. Search existing action/environment/replay/reconstruction APIs.
2. Run `python3 scripts/features.py find "<concept>"` when the task corresponds to an editor feature.
3. Prefer wrapping an existing semantic capability over cloning it for Lab.
4. Preserve source/revision stamps on generated/training artifacts.

## Verification

Run the focused Lab test for the changed action/task first. Use `python3 scripts/verify.py -p osrs-lab` only for crate-wide contract changes.

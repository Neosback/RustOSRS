# osrs-scene guidance

## Owns

- Exact OSRS scene semantics between decoded data and rendering.
- Terrain/object construction, placement semantics, bridge/plane rules, collision/blocker semantics, and scene-level identities.

## Does not own

- UI state, editable document history, GPU resources, cache transport, or platform integration.
- Renderer-specific shortcuts that alter semantic scene results.

## Invariants

- Scene output must remain deterministic for identical semantic inputs.
- Collision and visibility semantics come from OSRS/source rules, not from what happens to render.
- Invisible/non-rendered placements may still matter semantically.
- Preserve source-plane versus presentation-plane distinctions.
- Prefer one canonical implementation of each OSRS rule with focused policy helpers for real variants.

## Before adding code

1. Search `docs/specs/` and reference/parity fixtures for the OSRS rule.
2. Search this crate for the existing placement/collision/terrain implementation.
3. Inspect consumers in `osrs-world` and `osrs-render` before changing public scene structures.
4. Add a fixture/regression when fixing a compatibility quirk.

## Verification

Run the narrow scene/parity test that exercises the changed rule first. Escalate to `python3 scripts/verify.py -p osrs-scene` only when the public scene contract changes.

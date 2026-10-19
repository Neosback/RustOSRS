# osrs-core guidance

## Owns

- Canonical IDs, typed coordinates, planes, and small shared value vocabulary.
- Types whose meaning is stable across editing, cache, scene, world, and rendering layers.

## Does not own

- Cache transport/codecs, map editing policy, scene construction, HD rules, GPU state, UI, or orchestration.
- Convenience helpers that exist only for one upper-layer feature.

## Invariants

- Keep dependencies minimal and directionally downward.
- Prefer typed coordinates/IDs over raw tuples or interchangeable integers at domain boundaries.
- One canonical type per concept. Do not create parallel coordinate, plane, ID, or identity models in other crates.
- Shared types should encode invariants cheaply without pulling in product/runtime dependencies.

## Before adding code

1. Search for the existing canonical type and all call sites.
2. Confirm the concept is genuinely cross-layer rather than local to one crate.
3. Avoid broad utility modules and feature-specific behavior.
4. Check conversions for loss, coordinate-space ambiguity, and OSRS plane/tile semantics.

## Verification

Use the smallest core unit test first. Run `python3 scripts/verify.py -p osrs-core` only for crate-level API changes.

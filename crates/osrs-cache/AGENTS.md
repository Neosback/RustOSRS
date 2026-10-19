# osrs-cache guidance

## Owns

- OSRS cache archive/config/model/map codecs and cache-source abstractions above raw JS5 storage.
- Exact decode/encode behavior and definition lookup needed by scene/world tooling.

## Does not own

- Editor history, UI, GPU resources, world scheduling, or semantic scene shortcuts.
- A second editable map document. Decoded cache values are inputs to semantic hydration, not edit authority.

## Invariants

- Preserve target cache-format behavior exactly, including signedness, sentinel values, ordering, compression/encryption boundaries, and lossy export boundaries.
- Decode/encode logic must stay deterministic and testable without editor UI or renderer state.
- Keep cache I/O/source concerns separate from higher-level OSRS scene semantics.
- Reuse `osrs-core` IDs/coordinates where they are semantic rather than inventing transport-shaped domain types.

## Before adding code

1. Search existing codecs/definitions and pinned source evidence.
2. Confirm the behavior belongs to cache format rather than `osrs-scene` semantics.
3. Reuse existing source abstractions instead of special-casing editor paths.
4. Add a focused round-trip/fixture test for format quirks.

## Verification

Run the exact affected codec/fixture test first. Escalate to `python3 scripts/verify.py -p osrs-cache` only when the crate contract changes.

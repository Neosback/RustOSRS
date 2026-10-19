# osrs-playtest guidance

## Owns

- Headless play/simulation value types and deterministic gameplay-style checks that depend only on canonical core vocabulary.

## Does not own

- Editor UI, authored map history, world streaming, GPU rendering, cache I/O, or scene construction.
- A parallel set of coordinates/IDs or map semantics.

## Invariants

- Keep the crate deterministic, headless, and independent of editor/platform/rendering dependencies.
- Consume explicit semantic inputs rather than reaching into presentation state.
- Reuse `osrs-core` types and preserve exact source-plane/coordinate meaning.
- Simulation state must not become authored map authority.

## Before adding code

1. Search existing simulation/state types.
2. Confirm the behavior belongs in reusable playtest logic rather than `osrs-scene` or the editor.
3. Keep APIs input-driven and reproducible.
4. Avoid adding dependencies that pull runtime/product concerns downward.

## Verification

Use the smallest playtest unit test first. Escalate to `python3 scripts/verify.py -p osrs-playtest` only for crate-wide changes.

# osrs-world guidance

## Owns

- World/region streaming, async scene construction, projection workers, rebuild scheduling, dependency tracking, and CPU-side presentation indexes.
- Coordination between cache/scene/HD/render inputs without becoming semantic edit authority.

## Does not own

- `MapDocument` history, UI gestures, GPU resource ownership, or new OSRS semantic rules that belong in `osrs-scene`.

## Invariants

- Preserve FIFO/freshness assumptions and reject stale producer generations.
- Rebuild/invalidation decisions must be explainable from semantic receipts/dependencies, with conservative fallback when proof is incomplete.
- Worker mirrors and indexes are disposable projections, never authored state.
- Keep semantic revision, source generation, request identity, and presentation generation distinct.
- Do not solve synchronization by exposing broad mutable world state across layers.

## Before adding code

1. Search existing worker messages, tickets, dependency proofs, and invalidation routes.
2. Determine whether the change belongs in scene semantics, world orchestration, or renderer projection before adding a new type.
3. Reuse existing generation/revision identities instead of inventing another freshness token.
4. Preserve deterministic rebuild behavior where tests depend on exact ordering.

## Verification

Use the smallest worker/invalidation/streaming test that covers the changed path. Escalate to `python3 scripts/verify.py -p osrs-world` only for crate-level contract changes.

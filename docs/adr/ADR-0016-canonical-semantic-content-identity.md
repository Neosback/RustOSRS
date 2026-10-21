# ADR-0016: Versioned canonical semantic content identity

Status: **Accepted**  
Date: **2026-10-20**

## Context

Session revisions are local freshness observations. They change on semantic hydration/eviction and
cannot identify portable content. Source/build descriptors likewise identify inputs without hashing
the edited document. General reproduction needs independently verifiable initial/result map and
ordered patch identity. Project v2 stores source-relative diffs, while Lab snapshots use task-local
unsigned prefab coordinates. Neither is a lossless canonical full-world content-hash contract.

## Decision

`osrs-edit` owns versioned content digests on existing `MapDocument` and `MapPatch`. Version 1 uses
SHA-256 over the fixed-width, domain-separated canonical byte grammar in
[semantic-digests.md](../specs/semantic-digests.md), returning the existing `osrs-core::Digest256`
value. The v1 method names and algorithm identifiers are explicit and frozen. No replacement map,
patch, coordinate or history model is introduced. `sha2` reuses the workspace's existing pinned version.

Map identity covers canonical sparse terrain, explicit heights and exact object occurrences. Patch
identity preserves operation order, polarity and before/after values, including no-ops. Source,
profile/build, availability, workspace appearance/scenario, feature intent and deterministic clock/
seed remain separate envelope inputs. Digests never authorize stale asynchronous publication.

Hashing is an explicit read-only operation, not work added to normal commit/undo/frame loops.
This does not choose a project/recovery serializer or make the undo stack a persistence format.
It supplies the first primitive for the target's reproduction boundary while preserving
[ADR-0008](ADR-0008-command-transaction-history.md),
[ADR-0009](ADR-0009-project-save-export-separation.md) and the existing replay projection design.

## Consequences

- Content identity is reproducible across insertion order, host word size and endianness.
- Negative coordinates, full u32 IDs, signed heights and absent values remain exact.
- Independent byte vectors freeze the protocol without relying on candidate-produced expectations.
- Future semantic channels or encoding changes require a new version rather than changing v1.
- Map identity alone cannot certify a complete workspace, planner inputs, source or render output.
- Explicit document hashing enumerates terrain/heights and sorts existing object occurrences.

## Alternatives considered

Rust `Hash`/debug text are unsuitable portable formats. JSON would require an independently specified
canonical representation and validated semantic round trip. Lab snapshots narrow world coordinates.
Project diffs depend on external source baselines. Embedding feature/intent provenance into MapPatch
would violate the existing separation between semantic transactions and their invocation context.
A new general snapshot/history framework would duplicate established owners.

## Constraints / invariants affected

Semantic authority, transaction/history routing, projection freshness and source provenance retain
their existing contracts. This adds a read-only public content protocol and no new mutation route.

## Follow-up work

Add versioned lossless snapshot/operation envelopes and a headless verifier using canonical planners
and existing EditRuntime policy. Retain source/profile/build identity, required definition/config
inputs, expected patch/result digests and explicit unsupported-feature failures. Specify recovery
journals separately; do not claim portable editor replay from these two hash methods alone.

## Supersedes / superseded by

None.

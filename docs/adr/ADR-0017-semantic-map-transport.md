# ADR-0017: Lossless semantic map and patch transport

Status: **Accepted**  
Date: **2026-10-20**

## Context

Portable reproduction needs a complete map snapshot and exact patch evidence that can cross process
boundaries. Source-relative project diffs cannot supply a complete starting map without opening the
pinned source, and Lab snapshots intentionally use bounded unsigned task coordinates. Neither may be
relabeled as a full-world map serialization. Canonical v1 map/patch digests now provide content checks.

## Decision

Existing `MapDocument` and `MapPatch` expose explicit v1 JSON encode/decode methods in `osrs-edit`.
Their private transport records are infrastructure data, not a second semantic document/history.
[The transport contract](../specs/semantic-transport.md) specifies distinct versioned map-snapshot
and ordered-patch envelopes with the corresponding canonical algorithm/digest from
[ADR-0016](ADR-0016-canonical-semantic-content-identity.md).

Decoding validates the complete schema, integer ranges and canonical semantic constructors before
returning a new value. Snapshot duplicate coordinates/exact occurrences are errors, even if equal;
sparse default terrain retains existing normalization. Patch order, no-ops, polarity and before-state
remain exact. `MapPatch::new` recomputes impact; serialized impact is forbidden. Decoding verifies the
canonical content digest. It never applies operations, enters history or replaces a live session.

Records preserve signed i32 world coordinates/heights, unsigned u32 IDs, explicit optional channels
and source-plane distinctions. Unknown fields/operations, duplicate JSON fields, missing channels,
unsupported format/hash versions, malformed values and digest mismatch fail closed. Value errors
identify their snapshot collection or patch operation index and retain their typed cause.

This transport serves trusted infrastructure snapshot/reproduction assembly. Agents and Lab tools do
not gain a raw snapshot/patch action. Live authored work still crosses the normal edit gateway.
Source/profile/build, availability, appearance/scenarios and invocation inputs remain separate
reproduction-envelope obligations. Existing project saves, target export, undo and crash recovery keep
their own formats/contracts under [ADR-0009](ADR-0009-project-save-export-separation.md).

## Consequences

- Complete map state and exact patch evidence round-trip without narrowing coordinates or IDs.
- Semantic constructors and existing document normalization remain the authority.
- Import cannot silently overwrite duplicate records or trust caller-supplied impact.
- JSON record/key order and whitespace do not define content identity; canonical digests do.
- Hash integrity does not establish patch applicability or authorize a live operation.
- Snapshot encoding/decoding enumerates complete input and is explicit infrastructure work; callers
  remain responsible for their I/O/request limits. No per-frame snapshot or general memory-budget claim.

## Alternatives considered

Direct serde derivation over private semantic storage would couple the protocol to implementation
layout and risk bypassing validated constructors. Public parallel snapshot/operation models would
invite competing authority. Reusing project diffs requires external source state; reusing Lab prefab
coordinates narrows full-world coordinates. Treating raw patch replay as intent reproduction would
hide planner or dependency drift. Unversioned JSON cannot provide a durable reproduction boundary.

## Constraints / invariants affected

No mutation route, history, revision, source-acceptance, scene/cache rule or persistent project format
changes. New read-only infrastructure APIs transport the existing canonical values.

## Follow-up work

Capture actual building planner inputs in a versioned reproduction envelope, including immutable
source/profile/build identity, kit/definition facts, initial snapshot, seed/config and expected patch/
result digests. Add a headless verifier invoking the same planner. Extend to other editor operations
with explicit unsupported-feature failures, then compose workspace domains and projection diagnostics.
Design recovery journals separately; these codecs alone are not editor-wide replay or crash recovery.

## Supersedes / superseded by

None.

# ADR-0018: Frozen building reproduction through the product planner

Status: **Accepted**  
Date: **2026-10-20**

## Context

A seed and script line cannot reproduce building behavior after kit, definition, document or planner
changes. Native candidate batches also retain a base request seed separately from the selected
candidate's wrapping offset seed. Existing semantic digests and lossless transport supply map/patch
identity, but do not record actual product generation modes, validation inputs or gateway execution.

## Decision

The existing `PrefabRuntime::BuildState` owns explicit frozen building reproduction. Capture requires
a currently admitted candidate and retains its actual request/candidate forms, kit file, canonical
FootprintInputs facts, complete map snapshot and source/build context. It never substitutes defaults
or samples a new live catalog during replay. The same `build/plan.rs::plan` produces the replayed
candidate. Expected building patch, apply patch and result-map digests are compared; accepted work
executes and replays undo/redo through the existing EditRuntime, in an isolated EditorSession.

[The v1 contract](../specs/building-reproduction.md) uses the existing `buildings` feature ID. Unsupported
formats/features/contracts and malformed/inconsistent data fail explicitly. These are semantic map
reproduction operations, not renderer settling, project recovery or an alternative AI editing surface.
The product exposes Save reproduction on a current native preview, scripts expose `prefab reproduce
PATH`, and `osrs-editor --reproduce-building FILE` verifies without constructing EditorApp/GPU/cache.

Source metadata comes from an accepted region descriptor only when its existing source generation
matches the world owner's current worker source. Project and worker counters are independent and
are never compared to each other. The source profile must also match the project's opened profile; map
and definition validity still come from ordinary semantic preview admission. No FIFO/revision policy
or source counter changes. The bundle retains full TargetProvenance fields plus source build. Offline
verification validates the declared identity but does not authenticate an external cache installation.

A compile-time workspace source-tree SHA-256 and recorded compiler/target/profile/feature/flag context
identify the capture implementation. This is separate from Git/verification working-tree identity and
from an executable signature. Source-tree hashing includes workspace crate manifests/build scripts/
src/data, root Cargo.toml/Cargo.lock/toolchain/.cargo and profiles. It is an intentionally conservative
input identity, not a claim to identify every external linker/toolchain artifact. Cross-build replay
may still match; the report exposes captured and verifier contexts and their recorded equality.

Native export retains one frozen background request per building owner. Later edits do not rewrite
the captured evidence. The existing export staging/file owner gains complete create-only file
publication: synced staged bytes are hard-linked into the destination, then staging retires. Existing
reports are never replaced; unsupported hard-link filesystems report failure. No second persistence
or transaction system is introduced.

## Consequences

- Successful candidates in every existing generation mode/stage retain actual inputs and patch checks.
- Wrapping candidate seeds, signed coordinates, locks, drawn rooms and repair areas remain exact.
- Source dimensions keep precedence over portable furniture declarations through the canonical resolver.
- Hash/schema/input mismatches fail before applying isolated semantic changes; live authored work is untouched.
- I/O stays behind the native boundary, with a 128 MiB bundle cap and bounded export retention.
- Capture requires current accepted source metadata; no missing identity or dimensions are invented.
- This first contract captures successful candidates. Failed/unsatisfiable batch outcomes, workspace
  appearance/scenario, projection/image reproduction, recovery journals and other feature operations
  require subsequent contracts. Performance measurement/optimization remains paused.

## Alternatives considered

Replaying serialized patches would bypass planner drift detection. Reconstructing inputs from a seed,
current kit/catalog or unsigned Lab snapshot would silently change the request. Adding feature data
to MapPatch or creating another history/planner would violate existing ownership. Git HEAD alone
would misidentify local uncommitted source; bare package version likewise cannot identify the build.
A new generalized replay framework is unnecessary for this first concrete operation contract.

## Constraints / invariants affected

MapDocument/EditorSession/EditRuntime remain authority and chronology. Spatial object queries now
share the existing canonical document placement order: unordered region HashSet traversal can no
longer enter clearing/capture patches. Membership, footprint/halo rules, fitting/generation math,
gateway, OSRS/117HD behavior, source/FIFO protocol and project persistence formats are unchanged.
Serialization added to canonical unit enums preserves their complete variants, not display-label parsing.

## Follow-up work

2026-10-20 addendum: [ADR-0019](ADR-0019-rejected-building-reproduction.md) adopts typed rejected
attempt capture and exact no-edit replay using a separate failure format. This ADR's successful
v1 envelope and transaction policy are unchanged.

Extend reproduction to other features and workspace domains. Add declared projection
settling/diagnostic/image outputs, then
compose stable headless operations before MCP. Keep recovery journal design distinct from replay.

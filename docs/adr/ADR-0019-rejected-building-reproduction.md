# ADR-0019: Reproduce rejected building attempts without an edit

Status: **Accepted**  
Date: **2026-10-20**

## Context

ADR-0018 records successful candidates only. A failed batch previously retained display strings and
discarded each failed seed's request identity. A rejection cannot honestly be represented by an empty
successful patch, nor can display strings be parsed into semantic error identity.

## Decision

The existing product planner returns a private typed failure with its actual boundary stage, original
user message and exact opaque lower-owner diagnostic evidence. Stages are request, generation,
clearing, map validation, candidate validation and thumbnail construction. This does not introduce a
second semantic error taxonomy inside osrs-edit. Diagnostic text remains opaque; exact evidence
comparison deliberately detects wording/debug-output drift as well as changed rejection behavior.

The existing candidate worker and synchronous path retain each rejected seed's actual BuildForm,
shared PreviewInput and typed failure. They retain no additional document/kit snapshot. Cancellation,
replacement and normal input/source/revision/definition admission retire stale evidence. A malformed
completion cannot introduce a failure from another request. The failure list is private to BuildState;
UI reads current messages through a narrow operation.

Explicit capture freezes a current rejected attempt through the same building reproduction owner,
source gate, canonical map/kit/fact transport and bounded create-only native export. Default capture
uses the selected successful preview, otherwise the first failed attempt. Each current failed seed can
also be chosen explicitly, including in a mixed batch, without replacing the selected success.

[The failure contract](../specs/building-failure-reproduction.md) has a distinct closed format and
planner contract. The existing headless command dispatches original text to its closed decoder,
checks all inputs and the initial map digest, invokes the same product planner, and compares the
complete recorded failure. Unexpected success, a different stage/message/cause or changed map fails.
MATCH reports a reproduced rejection with zero operations and equal initial/result digests. No patch,
history entry, commit/undo/redo or projection is manufactured for failed planning. ADR-0018 success
schema and semantic gateway behavior remain unchanged.

## Consequences

- Actual constraints, missing input data and validation rejections can be captured without a preview.
- All eight native wrapping seed attempts retain independent evidence; a bundle reproduces one attempt.
- Stale evidence is ineligible immediately; one explicit capture may outlive later edits or cancellation.
- Source/build provenance, signed coordinates, actual forms/kit/facts and the 128 MiB cap remain shared.
- Exact diagnostic evidence is conservative rather than a promise of wording compatibility across builds.
- Pre-planner admission failures, worker panic/disconnection/cancellation and global solver
  unsatisfiability are not relabeled as deterministic planner rejections.
- Workspace/projection reproduction, crash recovery, other operations/MCP, independent fidelity and
  global resource budgets remain open. Performance measurement/optimization remains paused.

## Alternatives considered

Parsing `Seed N: ...` would confuse presentation with evidence and lose lower-owner distinctions.
Treating any failure as MATCH would hide changed behavior. Executing an empty patch would claim a
successful transaction that never occurred. Retaining a full document per failed seed would duplicate
the worker snapshot unnecessarily; ordinary freshness plus an explicit capture clone suffices.

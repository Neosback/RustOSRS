# Edit Gateway Core Migration Plan

Status: Checkpoint 3H implementation complete, pending exact-head certification

Active branch: `refactor/edit-gateway-objects`

Current branch base / `main`: `90ed1f9cc17f20312b20398a2b8cbc250fb139e3`

Implementation head before this documentation refresh: `2f60df89a19313f07ee5ecdcfb957ae856662e0a`

## Authority

This plan is subordinate to:

1. `ARCHITECTURE.md`
2. accepted ADRs, especially ADR-0008 and ADR-0009
3. current code
4. `DEV_HANDOFF.md`

It does not replace the architecture contract.

## Goal

Centralize the application policy for interactive semantic edits without creating a second semantic authority.

`MapDocument`, `MapPatch`, `EditHistory`, and `EditorSession` remain the semantic model. `EditRuntime` owns the one `EditorSession` and owns the checkpoint -> projection -> finalize/restore lifecycle for migrated interactive routes.

The gateway does not own renderer or world-worker algorithms. Those remain downstream projections.

## Implemented gateway contract

`crates/osrs-editor/src/app/edit_runtime.rs` now owns:

```text
EditRuntime {
  EditorSession
}

execute(MapPatch, project)
  -> checkpointed semantic commit
  -> project from authoritative post-commit MapDocument
  -> success: finalize once
  -> failure: exact restore

replay(Undo | Redo, touched_regions, project)
  -> checkpointed semantic Undo/Redo
  -> project from authoritative post-replay MapDocument
  -> success: finalize once
  -> failure: exact restore
```

Successful transactions return an `EditReceipt` containing semantic revision, `ChangeImpact`, and normalized touched regions. Empty patches remain no-ops. Rollback and finalize invariant failures stay explicit.

## Completed checkpoint sequence

### 3A: audit and contract

Completed.

- audited all interactive semantic mutation routes
- established one application transaction-policy boundary
- retained existing semantic authority and crate ownership

### 3B: gateway core

Completed.

- introduced `EditRuntime`
- moved the authoritative `EditorSession` behind it
- implemented checkpoint/finalize/restore policy
- added success, no-op, semantic rejection, projection rollback, rollback-failure, and finalize-failure coverage

### 3C: Tile Paste

Completed.

- Paste commits through `EditRuntime::execute`
- projection reads authoritative post-commit state
- failed projection restores exact pre-transaction semantic state
- worker/history/UI side effects occur only after gateway success

### 3D: Height

Completed.

- Height semantic commit routes through `EditRuntime::execute`
- failed post-commit projection is revision/history neutral
- prior redo survives failed projection and is invalidated only by successful finalization

### 3E: Paint

Completed.

- Paint stroke commit routes through the same gateway
- semantic state remains authoritative
- reconciliation/worker mutation occurs only after gateway success

### 3F: Terrain Path

Completed.

The former Path flow cloned the complete `MapDocument` solely to preflight legacy projection. That scratch semantic document is gone.

Current Path flow:

```text
build semantic patch
  -> snapshot only required legacy TileValues
  -> EditRuntime::execute
       -> checkpoint semantic commit
       -> prepare projection from real post-commit MapDocument
       -> success: finalize
       -> failure: exact restore
  -> apply already-validated worker projection
  -> push lightweight semantic Path marker
```

Repeated fitted Path tiles are deduplicated for projection/history routing. Height quantization and lower-plane safety behavior are preserved.

### 3G: Objects

Completed.

All normal Object actions funnel through the semantic planner and `EditRuntime::execute`, including:

- add / placement
- delete
- move / drag
- rotate
- duplicate
- floor shift
- scripted placement

`build_object_commit_plan` remains the canonical planner. Worker `LocEdit`s are derived from canonical semantic operations, not raw UI intent.

The gateway projection validates the authoritative post-commit object state before finalization. On projection failure:

- semantic state is restored exactly
- no worker `LocEdit` is emitted
- no mixed-history marker is pushed
- selection/reselect state is not mutated

### 3H: semantic Undo/Redo replay

Implemented on head `2f60df89a19313f07ee5ecdcfb957ae856662e0a`; exact-head full CI certification is the remaining checkpoint gate.

Semantic Undo/Redo no longer owns checkpoint policy in `editing/history/routing.rs`. It now routes through `EditRuntime::replay`.

Current replay flow:

```text
pop mixed-history marker
  -> snapshot only transitional projection inputs
  -> EditRuntime::replay(Undo | Redo)
       -> checkpoint semantic history transition
       -> prepare projection from authoritative post-replay MapDocument
       -> success: finalize
       -> failure: exact restore
  -> after success only:
       apply worker/UI projection
       move mixed-history marker to opposite stack
```

Projection preparation is pure with respect to `EditorSession`, worker state, selection, and global history. Tile/Object worker mutations occur only after gateway finalization.

Failure restores both systems:

- exact semantic session state, including history/redo/revisions/ownership metadata
- the application mixed-history marker to the stack it came from

Replay tests are split into `editing/history/routing/tests.rs` so the production router remains under the 500-line architecture cap.

## Gateway coverage after 3H

```text
EditRuntime transaction policy
├─ Tile Paste                  ✅ execute
├─ Height                      ✅ execute
├─ Paint                       ✅ execute
├─ Terrain Path                ✅ execute
├─ Objects                     ✅ execute
└─ semantic Undo/Redo replay   ✅ replay
```

Interactive semantic transaction lifecycle policy is now centralized behind `EditRuntime`.

## Transitional compatibility still present

`EditRuntime` still implements broad `Deref` / `DerefMut` compatibility to `EditorSession` for staged migration compatibility.

That escape is no longer the intended path for interactive semantic mutation. It should be removed or narrowed only in a separate checkpoint after an explicit usage audit. Do not mechanically delete it without classifying remaining callers, because source/project lifecycle, hydration, export/query access, and non-interactive session operations have different semantics from interactive edit execution.

## Next allowed checkpoint: 3I compatibility-surface audit

Scope the next checkpoint narrowly to the `EditRuntime` compatibility surface.

Recommended sequence:

1. enumerate every remaining `EditRuntime` deref/deref-mut caller;
2. classify each as read-only query, source/residency lifecycle, project application, export/snapshot, or accidental interactive mutation bypass;
3. add narrow explicit forwarding methods where they improve ownership clarity;
4. remove `DerefMut` if no legitimate mutable caller remains;
5. consider replacing read-only `Deref` with explicit query/session accessors rather than deleting it mechanically;
6. add an architecture guard only if it can enforce the intended boundary without blocking legitimate lifecycle operations.

Do not combine 3I with projection-invalidation redesign or runtime extraction.

## Explicit non-goals

This gateway sequence does not implement:

- centralized `ChangeImpact` -> projection invalidation service
- 8x8 zone/chunk invalidation
- ProjectRuntime, UiRuntime, or DiagnosticsRuntime extraction
- InteractionRuntime behavioral encapsulation
- cache codec/repack redesign
- renderer or 117HD work
- spline/path fitter redesign
- a replacement semantic history system

Project open/application intentionally remains outside interactive `execute`/`replay`: it uses untracked project-application/retry semantics and stored `RegionDiff`s.

## Acceptance rule

The gateway work is complete only when application components no longer independently decide interactive semantic checkpoint/finalize/restore policy.

A route is considered migrated only when:

- semantic transition occurs through `EditRuntime`;
- fallible projection is prepared from authoritative post-transition semantic state;
- projection failure exactly restores semantic state;
- worker/UI/global-history side effects occur only after successful gateway finalization.

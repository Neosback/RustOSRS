# Edit Gateway Core Migration Plan

Status: Checkpoint 3I implementation complete and certified; gateway migration sequence complete

Active branch: `refactor/edit-gateway-objects`

Current branch base / `main`: `90ed1f9cc17f20312b20398a2b8cbc250fb139e3`

Certified implementation head before this documentation refresh: `9d4ebb1ed1b7ce70343bbbcc5a48ce1e4707c0f8`

Certification: CI run `38589143724` / run #1154 passed Tier A, Tier B, and Tier C M3-M9 on the exact implementation head.

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

`crates/osrs-editor/src/app/edit_runtime.rs` owns:

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

The production `EditRuntime` surface now also exposes only narrow non-transaction operations required by the application:

- read-only semantic document access
- revision / edit-revision queries
- dirty-region iteration
- untracked project application
- source reset
- streamed region ingest / unload

It does not expose production forwarding methods for `commit`, `undo`, `redo`, checkpoint creation, checkpoint finalization, or checkpoint restore.

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

Completed and certified.

Semantic Undo/Redo no longer owns checkpoint policy in `editing/history/routing.rs`. It routes through `EditRuntime::replay`.

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

### 3I: compatibility-surface closure

Completed and certified on exact head `9d4ebb1ed1b7ce70343bbbcc5a48ce1e4707c0f8` by CI run `38589143724` / #1154.

The broad `Deref` and `DerefMut` implementations from `EditRuntime` to `EditorSession` are removed.

The removal was compiler-audited rather than performed mechanically:

1. remove both traits;
2. let the workspace compiler enumerate remaining callers;
3. classify each caller by semantic role;
4. add only narrow explicit runtime operations for legitimate query/source/project behavior;
5. keep interactive transaction lifecycle unavailable outside `execute` / `replay`.

Production callers now use explicit operations for:

- `document`
- `revision`
- `edit_revision`
- `dirty_regions`
- `apply_untracked`
- `reset`
- `ingest_region`
- `unload_region`

Project replacement/application helpers were migrated from `&mut EditorSession` to `&mut EditRuntime`, preserving their intentionally untracked project semantics without exposing interactive commit policy.

Tests that need to construct history use test-only helpers that delegate to `execute` / `replay`; they do not recreate a production bypass. History availability queries used only by tests are also test-only.

The existing `edit_runtime.rs` tests were moved to `edit_runtime/tests.rs` when the production module crossed the repository's 500-line architecture cap. This kept the architecture ratchet intact rather than expanding or allowlisting the module.

Exact-head #1154 certification passed:

- architecture boundaries / guard
- reference regeneration safety
- historical deob golden index
- formatting
- full workspace compile
- strict Clippy
- WebAssembly portability gate
- full workspace tests
- M3 through M9 semantic/reference parity

## Gateway coverage after 3I

```text
EditRuntime transaction policy
├─ Tile Paste                  ✅ execute
├─ Height                      ✅ execute
├─ Paint                       ✅ execute
├─ Terrain Path                ✅ execute
├─ Objects                     ✅ execute
└─ semantic Undo/Redo replay   ✅ replay

Compatibility boundary
├─ broad Deref                 removed ✅
├─ broad DerefMut              removed ✅
├─ production commit bypass    unavailable ✅
└─ production replay bypass    unavailable ✅
```

Interactive semantic transaction lifecycle policy is centralized behind `EditRuntime`.

## Compatibility surface after 3I

There is no broad production compatibility escape from `EditRuntime` to `EditorSession`.

Read/query, source residency, and project application remain explicit because they have semantics distinct from interactive edit execution. In particular, Project open/application intentionally remains outside interactive `execute` / `replay`: it uses untracked project-application/retry semantics and stored `RegionDiff`s.

Test-only conveniences may construct semantic history through the gateway, but production code cannot use them.

## Gateway sequence status

The Edit Gateway Core migration sequence is complete under its original acceptance rule.

Any future work should begin as a separate architecture checkpoint or branch rather than extending this migration opportunistically. Possible follow-on areas from the broader architecture contract include:

- centralized `ChangeImpact` -> projection invalidation
- 8x8 zone/chunk invalidation
- runtime extraction such as ProjectRuntime / UiRuntime / DiagnosticsRuntime
- deeper InteractionRuntime behavioral encapsulation

Those are not part of the Edit Gateway Core migration and should not be mixed into this branch merely because the gateway now provides a clean boundary.

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

## Acceptance rule

The gateway work is complete only when application components no longer independently decide interactive semantic checkpoint/finalize/restore policy.

A route is considered migrated only when:

- semantic transition occurs through `EditRuntime`;
- fallible projection is prepared from authoritative post-transition semantic state;
- projection failure exactly restores semantic state;
- worker/UI/global-history side effects occur only after successful gateway finalization.

Checkpoint 3I closes the remaining compatibility hole: production callers can no longer obtain broad `EditorSession` behavior through implicit dereferencing, so the compiler now enforces the intended application transaction boundary.

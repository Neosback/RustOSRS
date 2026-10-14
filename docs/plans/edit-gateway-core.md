# Edit Gateway Core Migration Plan

Status: Checkpoint 3A architecture audit and branch baseline

Branch: `refactor/edit-gateway-core`

Base / certified main: `fa474c9ae9f660b3cf9ef187f79ac05979b4cbe2`

## Authority

This plan is subordinate to:

1. `ARCHITECTURE.md`
2. accepted ADRs, especially ADR-0008 and ADR-0009
3. current code
4. `DEV_HANDOFF.md`

It does not replace the architecture contract.

## Why this branch exists

The rollback prerequisite is merged. `EditorSession` and `EditHistory` now support exact checkpointed commit/undo/redo transitions, so a failed downstream projection can restore the exact pre-transaction semantic state instead of consuming another user history operation.

The next architecture problem is application-level orchestration. Semantic edits are authoritative in one `EditorSession`, but the application still has several separate orchestration paths that each decide for themselves how to:

- plan or obtain a `MapPatch`
- commit or checkpoint it
- project the authoritative result
- finalize or roll back the semantic checkpoint
- record lightweight application history/routing metadata
- report success/failure to the caller

That duplication is the next source of drift risk.

## Audit findings at 3A

### Existing authority is correct

Do not replace these types or create parallel equivalents:

- `MapDocument` remains authoritative edited map state.
- `MapPatch` remains the semantic transaction primitive.
- `EditHistory` remains the semantic undo/redo authority.
- `EditorSession` remains the persistent semantic session and exact rollback boundary.
- worker tile/location edit stores remain projections only.

### Current application orchestration is distributed

The main routes currently coordinate semantic mutation independently:

- Paint stroke commit in `app/editing/commit.rs`
- Height stroke commit in `app/editing/commit.rs`
- Tile Paste in `app/editing/paste.rs`
- Terrain Path in `app/editing/path.rs`
- Object actions in `app/object_tool.rs`
- semantic Undo/Redo routing in `app/editing/history/routing.rs`

They already share semantic authority, but they do not yet share one application dispatcher.

### A cosmetic forwarding wrapper would be wrong

The gateway must not be a second `EditRuntime` that simply exposes the same `EditorSession` methods while callers continue to own transaction policy.

The application gateway must own the policy boundary for a semantic transaction:

```text
MapPatch
  -> checkpoint semantic mutation when projection can fail
  -> expose authoritative post-commit state to projection
  -> projection success: finalize checkpoint
  -> projection failure: exact restore
  -> return one typed application result
```

The gateway does not own renderer/worker projection algorithms. Those remain downstream adapters supplied by the application until the separate projection-invalidation branch.

## First gateway contract

Checkpoint 3B should introduce the smallest contained application gateway that can prove the policy boundary without migrating every tool.

Recommended shape, names subject to implementation evidence:

```rust
pub struct EditRuntime {
    session: EditorSession,
}

pub struct EditReceipt {
    pub revision: u64,
    pub impact: ChangeImpact,
    pub touched_regions: Vec<RegionCoord>,
}

pub enum EditExecutionError<E> {
    Semantic(PatchApplyError),
    Projection(E),
    Rollback {
        projection: E,
        rollback: EditCheckpointError,
    },
    Finalize(EditCheckpointError),
}
```

The exact generic/error surface may be simplified during implementation. The architectural requirements are:

1. the runtime owns the authoritative `EditorSession`, rather than introducing a second semantic store;
2. callers cannot finalize/restore a checkpoint themselves once routed through the gateway;
3. a successful non-empty execution returns an application receipt derived from the semantic commit;
4. a no-op remains a no-op and does not destroy redo;
5. projection failure restores the exact pre-transaction session state;
6. finalize/rollback invariant failures remain explicit and are never silently converted into normal projection errors;
7. project `apply_untracked` remains outside the interactive edit gateway because project application has separate retry semantics;
8. projection invalidation redesign is not part of this branch.

## First migration target: Tile Paste

Tile Paste is the best proof route because it already has:

- a pure patch planner (`build_terrain_paint_patch`)
- one `MapPatch`
- exact `commit_checkpointed` rollback
- a synchronous fallible projection boundary (`project_document_tiles`)
- lightweight mixed-history metadata recorded only after success
- focused success/failure regression tests

Target flow for Paste in 3B/3C:

```text
Paste input
  -> pure planner -> MapPatch
  -> EditRuntime::execute(...)
       -> EditorSession checkpointed commit
       -> caller-supplied projection adapter reads authoritative state
       -> success: finalize
       -> failure: exact restore
       -> EditReceipt
  -> only after success: push application history marker / selection / notice
```

The first migration must preserve current Paste behavior exactly:

- one gesture -> one semantic history entry
- failed projection leaves document/history/revisions/dirty ownership/redo unchanged
- successful commit invalidates prior redo only after projection succeeds
- application mixed-history marker is not pushed on failure
- no project-open behavior changes

## Deferred routes

Do not migrate these in the same first gateway checkpoint:

- Height
- Paint
- Path
- Object actions
- Undo/Redo routing

They should move in small follow-up checkpoints after the gateway contract is proven by Paste.

## Non-goals

This branch does not yet implement:

- centralized projection invalidation from `ChangeImpact`
- 8x8 zone/chunk invalidation
- ProjectRuntime, UiRuntime, DiagnosticsRuntime extraction
- InteractionRuntime behavior encapsulation
- cache write fixture expansion
- 117HD work
- spline/path fitter redesign
- a second command hierarchy or replacement history system

## Checkpoint sequence

### 3A: audit and contract

- create branch from exact certified main
- re-read architecture contract and accepted ADRs
- audit current semantic mutation routes
- define the gateway policy boundary
- choose Tile Paste as the first route
- no production mutation route changes

### 3B: gateway core only

- introduce contained `EditRuntime` / execution result types
- move `EditorSession` ownership behind that runtime with narrow accessors needed by existing code
- implement checkpoint/finalize/restore execution policy
- add headless unit tests for success, no-op, semantic failure, projection failure, rollback invariant failure where injectable
- do not migrate Paste yet if doing so would enlarge the checkpoint beyond the core boundary

### 3C: migrate Tile Paste only

- route Paste through the gateway
- preserve existing projection adapter and application history marker behavior
- remove Paste-local checkpoint policy duplication
- add app-level regression coverage

Later checkpoints migrate one route family at a time.

## Acceptance rule

A gateway checkpoint is successful only if it reduces the number of application components allowed to decide semantic transaction lifecycle policy. Adding another forwarding layer while leaving checkpoint/finalize/restore policy in every caller does not count as progress.
